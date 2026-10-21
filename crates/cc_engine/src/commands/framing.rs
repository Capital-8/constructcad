//! Bracing and trusses: sloped members between points in space, braces
//! between columns and trusses laid out from a span, a height and a pattern.

use super::{add_element, edit, ids_json};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, Ref, finite, point, positive};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{
    Beam, BeamVertical, Category, Column, ColumnSection, ElementId, ElementKind, ElementTypeId, LevelId, Project,
    TypeData, WallTop,
};
use glam::{DVec2, DVec3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A point in space: `[x, y, z]`, `z` above the level.
pub type Point3 = [f64; 3];

fn point3(p: Point3) -> Result<DVec3> {
    let v = DVec3::from_array(p);
    v.is_finite().then_some(v).ok_or_else(|| EngineError::invalid("points must be finite numbers"))
}

/// A member from `a` to `b` (elevations above the level), its axis through
/// both: a beam centered on its location line, sloped when the ends differ.
fn member(a: DVec3, b: DVec3) -> Result<Beam> {
    if a.truncate().distance(b.truncate()) < 1e-3 {
        return Err(EngineError::invalid(t!("msg-member-vertical")));
    }
    let mut beam = Beam::new(a.truncate(), b.truncate(), a.z);
    beam.vertical = BeamVertical::Center;
    if (b.z - a.z).abs() > 1e-9 {
        beam.end_offset = Some(b.z);
    }
    Ok(beam)
}

/// The column type for truss posts: `r`, else the first profile column type, else the default.
fn post_type(p: &Project, r: Option<&Ref>) -> Result<ElementTypeId> {
    if r.is_some() {
        return refs::type_of(p, Category::Column, r);
    }
    let profile = p.types_of(Category::Column).find(|t| {
        matches!(t.data, TypeData::Column { section: ColumnSection::Profile { .. } | ColumnSection::Pipe { .. }, .. })
    });
    match profile {
        Some(t) => Ok(t.id),
        None => refs::type_of(p, Category::Column, None),
    }
}

/// Create a brace: a member between two points in space (`[x, y, z]`, `z`
/// above the level), its axis through both. Its ends must not be one above the other.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BraceCreate {
    /// Start point `[x, y, z]`.
    pub start: Point3,
    /// End point `[x, y, z]`.
    pub end: Point3,
    /// Level id or name (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// Beam type id or name (default: the category's default type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
}

impl Command for BraceCreate {
    const NAME: &'static str = "brace_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let type_id = refs::type_of(p, Category::Beam, self.type_ref.as_ref())?;
        let beam = member(point3(self.start)?, point3(self.end)?)?;
        let id = edit(s, t!("tx-create-brace"), |e| Ok(add_element(e, level, type_id, ElementKind::Beam(beam))))?;
        Ok(super::created(s, id))
    }
}

/// Brace the bay between two columns: `x` (default) crosses two members
/// from the base of each column to the top of the other, `diagonal` places
/// one from the base of the first to the top of the second.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct BracesBetweenColumns {
    /// The two column ids.
    pub columns: Vec<u64>,
    /// `x` or `diagonal`.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Beam type id or name (default: the category's default type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
}

impl Command for BracesBetweenColumns {
    const NAME: &'static str = "braces_between_columns";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let [a, b] = self.columns[..] else { return Err(EngineError::invalid(t!("msg-columns-two-exactly"))) };
        let axis = |id: u64| -> Result<(LevelId, DVec3, DVec3)> {
            let e = p.element(refs::element(p, id)?).ok_or_else(|| EngineError::not_found("column"))?;
            let ElementKind::Column(c) = &e.kind else {
                return Err(EngineError::invalid(format!("element {id} is not a column")));
            };
            let level_z = p.level_elevation(e.level);
            let (base, top) = p.column_span(e, c);
            Ok((e.level, c.position.extend(base - level_z), c.position.extend(top - level_z)))
        };
        let (level, a0, a1) = axis(a)?;
        let (other, b0, b1) = axis(b)?;
        // Elevations relative to the first column's level.
        let shift = DVec3::Z * (p.level_elevation(other) - p.level_elevation(level));
        let (b0, b1) = (b0 + shift, b1 + shift);
        let type_id = refs::type_of(p, Category::Beam, self.type_ref.as_ref())?;
        let mut members = vec![member(a0, b1)?];
        match self.pattern.as_deref().unwrap_or("x") {
            "x" => members.push(member(b0, a1)?),
            "diagonal" => {}
            other => return Err(EngineError::invalid(format!("pattern must be 'x' or 'diagonal', not '{other}'"))),
        }
        let created = edit(s, t!("tx-create-brace"), |e| {
            Ok(members.into_iter().map(|m| add_element(e, level, type_id, ElementKind::Beam(m))).collect::<Vec<_>>())
        })?;
        Ok(json!({"created": ids_json(&created)}))
    }
}

/// Lay out a plane truss from `start` to `end`: a bottom and a top chord
/// `height` apart, `panels` panels and the web of `pattern` (`pratt`, the
/// default, `howe` or `warren`). Pratt and Howe trusses get a post at every
/// panel point, Warren trusses only at the ends; posts are columns. The
/// members form a group, so the truss moves as one.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct TrussCreate {
    /// Start of the truss in plan.
    pub start: Point,
    /// End of the truss in plan.
    pub end: Point,
    /// Distance between the chord axes, meters.
    pub height: f64,
    /// Number of panels (default: about one per 1.5 m).
    #[serde(default)]
    pub panels: Option<u32>,
    /// `pratt`, `howe` or `warren`.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Elevation of the bottom chord's axis above the level (default: the level height minus `height`).
    #[serde(default)]
    pub bottom_offset: Option<f64>,
    /// Level id or name (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// Beam type of the chords (default: the category's default type).
    #[serde(default)]
    pub chord_type: Option<Ref>,
    /// Beam type of the diagonals (default: the chord type).
    #[serde(default)]
    pub web_type: Option<Ref>,
    /// Column type of the posts (default: the first profile column type).
    #[serde(default)]
    pub post_type: Option<Ref>,
}

/// A panel point of a truss: its index along the span and whether it is on the top chord.
type PanelPoint = (usize, bool);

/// The web of a truss: diagonals as pairs of panel points.
fn diagonals(pattern: &str, n: usize) -> Result<Vec<(PanelPoint, PanelPoint)>> {
    let half = n as f64 / 2.0;
    Ok((0..n)
        .map(|i| {
            // Pratt diagonals fall towards the middle; Howe diagonals rise towards it.
            let left = (i as f64) + 0.5 < half;
            let falls = match pattern {
                "pratt" => left,
                "howe" => !left,
                // Warren: alternating.
                _ => i % 2 == 1,
            };
            if falls { ((i, true), (i + 1, false)) } else { ((i, false), (i + 1, true)) }
        })
        .collect::<Vec<_>>())
    .and_then(|d| match pattern {
        "pratt" | "howe" | "warren" => Ok(d),
        other => Err(EngineError::invalid(format!("pattern must be pratt, howe or warren, not '{other}'"))),
    })
}

impl Command for TrussCreate {
    const NAME: &'static str = "truss_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let (start, end) = (point(self.start)?, point(self.end)?);
        let length = start.distance(end);
        if length < 0.1 {
            return Err(EngineError::invalid("the truss has no length"));
        }
        let height = positive(self.height, "height")?;
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let bottom =
            finite(self.bottom_offset.unwrap_or_else(|| p.level_height(level, 3.0) - height), "bottom_offset")?;
        let n = self.panels.map_or_else(|| (length / 1.5).round().max(1.0) as usize, |k| k as usize).clamp(1, 100);
        let pattern = self.pattern.as_deref().unwrap_or("pratt");
        let web = diagonals(pattern, n)?;
        let chord_type = refs::type_of(p, Category::Beam, self.chord_type.as_ref())?;
        let web_type = match &self.web_type {
            Some(r) => refs::type_of(p, Category::Beam, Some(r))?,
            None => chord_type,
        };
        let post_type = post_type(p, self.post_type.as_ref())?;
        let at = |i: usize, top: bool| -> DVec3 {
            let xy: DVec2 = start + (end - start) * (i as f64 / n as f64);
            xy.extend(if top { bottom + height } else { bottom })
        };
        let mut beams: Vec<(ElementTypeId, Beam)> =
            vec![(chord_type, member(at(0, false), at(n, false))?), (chord_type, member(at(0, true), at(n, true))?)];
        for (a, b) in web {
            beams.push((web_type, member(at(a.0, a.1), at(b.0, b.1))?));
        }
        let post_points: Vec<usize> = if pattern == "warren" { vec![0, n] } else { (0..=n).collect() };
        let posts: Vec<Column> = post_points
            .into_iter()
            .map(|i| Column {
                position: at(i, false).truncate(),
                rotation_deg: (end - start).to_angle().to_degrees(),
                base_offset: bottom,
                height,
                top: WallTop::Unconnected,
                structural: true,
            })
            .collect();
        let name = (1..)
            .map(|k| t!("truss-default-name", "n" => k))
            .find(|n| !p.groups.values().any(|g| &g.name == n))
            .unwrap_or_default();
        let (ids, group) = edit(s, t!("tx-create-truss"), |e| {
            let mut ids: Vec<ElementId> =
                beams.into_iter().map(|(ty, b)| add_element(e, level, ty, ElementKind::Beam(b))).collect();
            ids.extend(posts.into_iter().map(|c| add_element(e, level, post_type, ElementKind::Column(c))));
            let group = e.add_group(name.clone(), ids.iter().copied().collect());
            Ok((ids, group))
        })?;
        Ok(json!({"created": ids_json(&ids), "group": group.0, "name": name}))
    }
}
