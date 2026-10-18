//! Schedules: one row per element of a category, one column per field of the
//! schedule definition, with filtering, sorting and totals.

use crate::format;
use cc_core::{Category, Element, ElementKind, Project, ScheduleDef, ScheduleField, TypeData};
use cc_geom::{ModelGeometry, polygon};
use std::cmp::Ordering;

/// A cell: plain text, or a translation key for the application to resolve.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Text(String),
    Key(&'static str),
}

impl Cell {
    /// The text, or the key itself for untranslated cells.
    pub fn raw(&self) -> &str {
        match self {
            Cell::Text(s) => s,
            Cell::Key(k) => k,
        }
    }
}

/// A computed schedule. Headers are translation keys.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    pub headers: Vec<&'static str>,
    pub rows: Vec<Row>,
    /// Totals row aligned with `headers` (the first cell holds the row count);
    /// empty when totals are hidden.
    pub totals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub element: cc_core::ElementId,
    pub cells: Vec<Cell>,
}

/// A field value before formatting, used for sorting and totals.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Text(String),
    Key(&'static str),
    Count(usize),
    Length(f64),
    Area(f64),
    Volume(f64),
    Money(f64),
    Missing,
}

impl Value {
    fn cell(&self) -> Cell {
        match self {
            Value::Text(s) => Cell::Text(s.clone()),
            Value::Key(k) => Cell::Key(k),
            Value::Count(n) => Cell::Text(n.to_string()),
            Value::Length(v) => Cell::Text(format::meters(*v)),
            Value::Area(v) | Value::Money(v) => Cell::Text(format::number(*v, 2)),
            Value::Volume(v) => Cell::Text(format::number(*v, 3)),
            Value::Missing => Cell::Text(String::new()),
        }
    }

    fn number(&self) -> Option<f64> {
        match self {
            Value::Count(n) => Some(*n as f64),
            Value::Length(v) | Value::Area(v) | Value::Volume(v) | Value::Money(v) => Some(*v),
            _ => None,
        }
    }
}

/// Natural order: numbers numerically (so "10" follows "9"), then text.
fn natural(a: &str, b: &str) -> Ordering {
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(x), Ok(y)) => x.total_cmp(&y),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        _ => a.cmp(b),
    }
}

fn compare(a: &Value, b: &Value) -> Ordering {
    match (a.number(), b.number()) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        _ => natural(&text_of(a), &text_of(b)),
    }
}

fn text_of(v: &Value) -> String {
    v.cell().raw().to_string()
}

/// Whether a field's values add up in the totals row.
fn summable(field: ScheduleField) -> bool {
    matches!(field, ScheduleField::Length | ScheduleField::Area | ScheduleField::Volume | ScheduleField::Cost)
}

/// Net side area of a wall: length x height minus its openings.
fn wall_area(project: &Project, e: &Element, w: &cc_core::Wall) -> f64 {
    let openings: f64 = project
        .hosted_by(e.id)
        .filter_map(|h| project.element_type(h.type_id)?.opening_size())
        .map(|(w, h)| w * h)
        .sum();
    (w.length() * project.wall_height(e, w) - openings).max(0.0)
}

/// Width (X), depth (Y) and height of a component's pieces in its own frame.
fn component_extents(project: &Project, e: &Element) -> Option<(f64, f64, f64)> {
    let ElementKind::Component(c) = &e.kind else { return None };
    let Some(TypeData::Component { def, .. }) = project.element_type(e.type_id).map(|t| &t.data) else { return None };
    let (parts, _) = def.resolve(&c.overrides);
    let mut lo = glam::DVec3::splat(f64::INFINITY);
    let mut hi = glam::DVec3::splat(f64::NEG_INFINITY);
    for p in &parts {
        for q in &p.footprint {
            lo = lo.min(q.extend(p.z0));
            hi = hi.max(q.extend(p.z1));
        }
    }
    let size = hi - lo;
    size.is_finite().then_some((size.x, size.y, size.z))
}

fn value(project: &Project, geom: &ModelGeometry, e: &Element, field: ScheduleField) -> Value {
    use ScheduleField as F;
    let ty = project.element_type(e.type_id);
    let info = ty.map(|t| &t.info);
    let text = |s: &str| if s.is_empty() { Value::Missing } else { Value::Text(s.to_string()) };
    let room_boundary = geom.rooms.get(&e.id);
    match field {
        F::Mark => text(&e.mark),
        F::Type => text(project.type_name(e.type_id)),
        F::Level => text(project.level_name(e.level)),
        F::Comments => text(&e.comments),
        F::TypeMark => info.map_or(Value::Missing, |i| text(&i.type_mark)),
        F::Description => info.map_or(Value::Missing, |i| text(&i.description)),
        F::Manufacturer => info.map_or(Value::Missing, |i| text(&i.manufacturer)),
        F::Model => info.map_or(Value::Missing, |i| text(&i.model)),
        F::FireRating => info.map_or(Value::Missing, |i| text(&i.fire_rating)),
        F::Cost => info.and_then(|i| i.cost).map_or(Value::Missing, Value::Money),
        F::Length => match &e.kind {
            ElementKind::Wall(w) => Value::Length(w.length()),
            ElementKind::Beam(b) => Value::Length(b.length()),
            ElementKind::Railing(r) => Value::Length(r.length()),
            _ => Value::Missing,
        },
        F::Height => match (&e.kind, ty.map(|t| &t.data)) {
            (ElementKind::Wall(w), _) => Value::Length(project.wall_height(e, w)),
            (ElementKind::Column(c), _) => {
                let (base, top) = project.column_span(e, c);
                Value::Length(top - base)
            }
            (ElementKind::Room(r), _) => Value::Length(r.height),
            (_, Some(TypeData::Door { height, .. } | TypeData::Window { height, .. })) => Value::Length(*height),
            (_, Some(TypeData::Railing { height, .. })) => Value::Length(*height),
            (ElementKind::Component(_), _) => {
                component_extents(project, e).map_or(Value::Missing, |s| Value::Length(s.2))
            }
            _ => Value::Missing,
        },
        F::Width => match ty.map(|t| &t.data) {
            Some(TypeData::Door { width, .. } | TypeData::Window { width, .. } | TypeData::Stair { width, .. }) => {
                Value::Length(*width)
            }
            _ => component_extents(project, e).map_or(Value::Missing, |s| Value::Length(s.0)),
        },
        F::Depth => component_extents(project, e).map_or(Value::Missing, |s| Value::Length(s.1)),
        F::ComponentName => match ty.map(|t| &t.data) {
            Some(TypeData::Component { component, .. }) => text(component),
            _ => Value::Missing,
        },
        F::ComponentKind => match ty.map(|t| &t.data) {
            Some(TypeData::Component { def, .. }) => Value::Key(def.kind.key()),
            _ => Value::Missing,
        },
        F::Thickness => match ty {
            Some(t) if t.thickness() > 0.0 => Value::Length(t.thickness()),
            _ => Value::Missing,
        },
        F::Area => match &e.kind {
            ElementKind::Wall(w) => Value::Area(wall_area(project, e, w)),
            ElementKind::Room(_) => room_boundary.map_or(Value::Missing, |b| Value::Area(polygon::area(b))),
            _ => e.sketch().map_or(Value::Missing, |s| Value::Area(polygon::area(s))),
        },
        F::Perimeter => match &e.kind {
            ElementKind::Room(_) => room_boundary.map_or(Value::Missing, |b| Value::Length(polygon::perimeter(b))),
            _ => e.sketch().map_or(Value::Missing, |s| Value::Length(polygon::perimeter(s))),
        },
        F::Volume => match &e.kind {
            ElementKind::Room(r) => {
                room_boundary.map_or(Value::Missing, |b| Value::Volume(polygon::area(b) * r.height))
            }
            _ => Value::Volume(geom.volume(e.id)),
        },
        F::Sill => e.as_hosted().map_or(Value::Missing, |h| Value::Length(h.sill_height)),
        F::Host => e
            .as_hosted()
            .and_then(|h| project.element(h.host))
            .map_or(Value::Missing, |w| text(&if w.mark.is_empty() { w.id.to_string() } else { w.mark.clone() })),
        F::Function => ty.and_then(|t| t.wall_function()).map_or(Value::Missing, |f| Value::Key(f.key())),
        F::Structural => match &e.kind {
            ElementKind::Wall(w) => Value::Key(if w.structural { "value-yes" } else { "value-no" }),
            _ => Value::Missing,
        },
        F::Number => match &e.kind {
            ElementKind::Room(r) => text(&r.number),
            _ => Value::Missing,
        },
        F::Name => match &e.kind {
            ElementKind::Room(r) => text(&r.name),
            _ => Value::Missing,
        },
        F::Risers | F::RiserHeight | F::Run => match &e.kind {
            ElementKind::Stair(s) => match cc_geom::stair_geom(project, e, s) {
                Some(g) if field == F::Risers => Value::Count(g.risers),
                Some(g) if field == F::RiserHeight => Value::Length(g.riser),
                Some(g) => Value::Length(g.run()),
                None => Value::Missing,
            },
            _ => Value::Missing,
        },
    }
}

/// Default row order: by mark (rooms by number), naturally, then by id.
fn default_order(a: &Element, b: &Element) -> Ordering {
    let key = |e: &Element| match &e.kind {
        ElementKind::Room(r) => r.number.clone(),
        _ => e.mark.clone(),
    };
    natural(&key(a), &key(b)).then(a.id.cmp(&b.id))
}

pub fn schedule(project: &Project, geom: &ModelGeometry, category: Category, def: &ScheduleDef) -> Table {
    let fields = def.fields_for(category);
    let mut elements: Vec<&Element> = project.elements_of(category).collect();
    elements.sort_by(|a, b| default_order(a, b));

    let mut rows: Vec<(&Element, Vec<Value>)> =
        elements.into_iter().map(|e| (e, fields.iter().map(|&f| value(project, geom, e, f)).collect())).collect();

    if let Some((field, needle)) = &def.filter {
        let needle = needle.trim().to_lowercase();
        if !needle.is_empty() {
            rows.retain(|(e, _)| text_of(&value(project, geom, e, *field)).to_lowercase().contains(&needle));
        }
    }
    if let Some(field) = def.sort_by {
        // Stable: ties keep the default order.
        rows.sort_by(|(a, _), (b, _)| {
            let order = compare(&value(project, geom, a, field), &value(project, geom, b, field));
            if def.descending { order.reverse() } else { order }
        });
    }

    let totals = if def.hide_totals {
        Vec::new()
    } else {
        let mut totals: Vec<String> = fields
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                if !summable(f) {
                    return String::new();
                }
                let sum: f64 = rows.iter().filter_map(|(_, values)| values.get(i)?.number()).sum();
                match f {
                    ScheduleField::Length => format::meters(sum),
                    ScheduleField::Volume => format::number(sum, 3),
                    _ => format::number(sum, 2),
                }
            })
            .collect();
        if let Some(first) = totals.first_mut() {
            *first = rows.len().to_string();
        }
        totals
    };

    Table {
        headers: fields.iter().map(|f| f.key()).collect(),
        rows: rows
            .into_iter()
            .map(|(e, values)| Row { element: e.id, cells: values.iter().map(Value::cell).collect() })
            .collect(),
        totals,
    }
}

/// The table as CSV: `;`-separated when the decimal separator is a comma, so
/// spreadsheets in those locales open it directly. `header` and `cell`
/// translate keys.
pub fn to_csv(table: &Table, header: impl Fn(&str) -> String, cell: impl Fn(&Cell) -> String) -> String {
    let sep = if format::number(0.5, 1).contains(',') { ';' } else { ',' };
    let quote = |s: &str| {
        if s.contains(sep) || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    let mut line = |cells: Vec<String>| {
        out.push_str(&cells.iter().map(|c| quote(c)).collect::<Vec<_>>().join(&sep.to_string()));
        out.push_str("\r\n");
    };
    line(table.headers.iter().map(|h| header(h)).collect());
    for row in &table.rows {
        line(row.cells.iter().map(&cell).collect());
    }
    if !table.totals.is_empty() {
        line(table.totals.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Project, ModelGeometry) {
        let project = cc_core::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap();
        let geom = ModelGeometry::build(&project);
        (project, geom)
    }

    #[test]
    fn sorting_filtering_and_totals() {
        let (project, geom) = sample();
        let def = ScheduleDef {
            fields: vec![ScheduleField::Mark, ScheduleField::Width, ScheduleField::Area],
            sort_by: Some(ScheduleField::Mark),
            descending: true,
            ..Default::default()
        };
        let t = schedule(&project, &geom, Category::Window, &def);
        assert_eq!(t.headers, vec!["col-mark", "col-width", "col-area"]);
        assert_eq!(t.rows.len(), 4);
        // Marks 1..4, descending; windows have no area (empty cell).
        let marks: Vec<&str> = t.rows.iter().map(|r| r.cells[0].raw()).collect();
        assert_eq!(marks, ["4", "3", "2", "1"]);
        assert_eq!(t.rows[0].cells[1], Cell::Text(format::meters(1.2)));
        assert_eq!(t.totals[0], "4");

        let filtered = ScheduleDef { filter: Some((ScheduleField::Mark, "3".into())), ..def };
        let t = schedule(&project, &geom, Category::Window, &filtered);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.totals[0], "1");
    }

    #[test]
    fn csv_quotes_and_separates() {
        let table = Table {
            headers: vec!["a", "b"],
            rows: vec![Row { element: cc_core::ElementId(1), cells: vec![Cell::Text("x;y".into()), Cell::Key("k")] }],
            totals: vec!["1".into(), String::new()],
        };
        let csv = to_csv(&table, |h| h.to_uppercase(), |c| c.raw().to_string());
        assert_eq!(csv, "A;B\r\n\"x;y\";k\r\n1;\r\n");
    }
}
