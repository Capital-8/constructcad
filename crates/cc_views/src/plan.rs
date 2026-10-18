use crate::{Drawing2D, LineStyle};
use cc_core::{ElementKind, LevelId, Project, ViewKind};
use cc_geom::{Column, ModelGeometry, WallGeom, polygon};
use glam::DVec2;
use std::f64::consts::{PI, TAU};

pub(crate) const POCHE: [u8; 4] = [196, 196, 192, 255];
const CEILING_GRID: f64 = 0.625;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnCut {
    Cut,
    Below,
    None,
}

fn column_state(c: &Column, cut_z: f64) -> ColumnCut {
    if c.is_solid_at(cut_z) {
        ColumnCut::Cut
    } else if c.solid.first().is_some_and(|s| s.0 < cut_z) {
        ColumnCut::Below
    } else {
        ColumnCut::None
    }
}

fn style_of(state: ColumnCut) -> Option<LineStyle> {
    match state {
        ColumnCut::Cut => Some(LineStyle::Cut),
        ColumnCut::Below => Some(LineStyle::Projection),
        ColumnCut::None => None,
    }
}

/// Floor plan (`ceiling == false`) or reflected ceiling plan of `level`.
pub fn plan(project: &Project, geom: &ModelGeometry, level: LevelId, cut_height: f64, ceiling: bool) -> Drawing2D {
    let mut d = Drawing2D::default();
    let level_z = project.level_elevation(level);
    let cut_z = level_z + cut_height;

    for e in project.elements.values().filter(|e| e.level == level) {
        let id = Some(e.id);
        match &e.kind {
            ElementKind::Floor(f) if !ceiling => d.polyline(&f.boundary, true, LineStyle::Projection, id),
            ElementKind::Ceiling(c) if ceiling => {
                d.polyline(&c.boundary, true, LineStyle::Projection, id);
                ceiling_grid(&mut d, &c.boundary, e.id);
            }
            ElementKind::Roof(r) => {
                let outline = polygon::offset(&r.footprint, r.overhang);
                d.polyline(&outline, true, LineStyle::Projection, id);
                if r.overhang > 0.0 {
                    d.polyline(&r.footprint, true, LineStyle::Hidden, id);
                }
            }
            _ => {}
        }
    }

    for (e, _) in project.walls().filter(|(e, _)| e.level == level) {
        if let Some(g) = geom.walls.get(&e.id) {
            draw_wall(&mut d, g, cut_z);
            draw_openings(&mut d, project, g, ceiling);
        }
    }

    if !ceiling {
        for v in project.views.values() {
            if let ViewKind::Section { start, end, depth } = v.kind {
                section_mark(&mut d, start, end, depth, &v.name);
            }
        }
        if let Some((lo, hi)) = geom.bounds() {
            let (lo, hi) = (DVec2::new(lo.x, lo.y), DVec2::new(hi.x, hi.y));
            for v in project.views.values() {
                if let ViewKind::Elevation { direction } = v.kind {
                    elevation_mark(&mut d, lo, hi, direction.look(), &v.name);
                }
            }
        }
    }
    d
}

/// Distance from the model extents to elevation markers, in meters.
const ELEVATION_MARK_OFFSET: f64 = 1.2;
const ELEVATION_MARK_RADIUS: f64 = 0.25;

/// Elevation marker outside the model: a circle with a filled arrow pointing
/// along `look` (towards the model), labeled with the view name.
fn elevation_mark(d: &mut Drawing2D, lo: DVec2, hi: DVec2, look: DVec2, name: &str) {
    let center = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    let reach = (half.x * look.x.abs() + half.y * look.y.abs()) + ELEVATION_MARK_OFFSET;
    let c = center - look * reach;
    let r = ELEVATION_MARK_RADIUS;
    let circle: Vec<DVec2> = (0..24)
        .map(|i| {
            let a = i as f64 / 24.0 * std::f64::consts::TAU;
            c + DVec2::new(a.cos(), a.sin()) * r
        })
        .collect();
    d.polyline(&circle, true, LineStyle::Annotation, None);
    let side = look.perp();
    let arrow = vec![c + look * (r * 1.6), c + side * (r * 0.8), c - side * (r * 0.8)];
    d.fill(arrow, [30, 90, 170, 255], None);
    // Label below and to the right of the circle, clear of the arrow.
    d.text(c + DVec2::new(r + 0.08, -r - 0.05), name, 2.5);
}

fn draw_wall(d: &mut Drawing2D, g: &WallGeom, cut_z: f64) {
    let id = Some(g.element);
    let h = g.half;
    let states: Vec<ColumnCut> = g.columns.iter().map(|c| column_state(c, cut_z)).collect();
    for (i, c) in g.columns.iter().enumerate() {
        let poly = g.column_polygon(c);
        let state = states[i];
        if state == ColumnCut::Cut {
            d.fill(poly.to_vec(), POCHE, id);
            // Interior layer boundaries; the outer faces are drawn below.
            for &(_, v, _) in g.layers.iter().take(g.layers.len().saturating_sub(1)) {
                let s = (v + h) / (2.0 * h);
                let u0 = c.right.0 + (c.left.0 - c.right.0) * s;
                let u1 = c.right.1 + (c.left.1 - c.right.1) * s;
                d.line(g.world(u0, v), g.world(u1, v), LineStyle::Thin, id);
            }
        }
        if let Some(style) = style_of(state) {
            d.line(poly[0], poly[1], style, id);
            d.line(poly[2], poly[3], style, id);
        }
        if let Some(&next) = states.get(i + 1)
            && next != state
        {
            let style =
                if state == ColumnCut::Cut || next == ColumnCut::Cut { LineStyle::Cut } else { LineStyle::Projection };
            d.line(g.world(c.left.1, -h), g.world(c.left.1, h), style, id);
        }
    }
    let caps = [
        (g.start_joined, g.columns.first(), states.first(), g.start_ext),
        (g.end_joined, g.columns.last(), states.last(), g.end_ext),
    ];
    for (joined, col, state, (ul, ur)) in caps {
        if let (false, Some(_), Some(&state)) = (joined, col, state)
            && let Some(style) = style_of(state)
        {
            d.line(g.world(ur, -h), g.world(ul, h), style, id);
        }
    }
}

fn draw_openings(d: &mut Drawing2D, project: &Project, g: &WallGeom, ceiling: bool) {
    for o in &g.openings {
        let Some(e) = project.element(o.element) else { continue };
        let Some(host) = e.as_hosted() else { continue };
        let id = Some(e.id);
        match e.kind {
            ElementKind::Door(_) if !ceiling => {
                let width = o.u1 - o.u0;
                let side = if host.flip_facing { -1.0 } else { 1.0 };
                let (hinge_u, toward) = if host.flip_hand { (o.u1, -1.0) } else { (o.u0, 1.0) };
                let hinge = g.world(hinge_u, side * g.half);
                let leaf_dir = g.normal * side;
                d.line(hinge, hinge + leaf_dir * width, LineStyle::Projection, id);
                let a0 = leaf_dir.to_angle();
                let mut a1 = (g.dir * toward).to_angle();
                if a1 - a0 > PI {
                    a1 -= TAU;
                } else if a0 - a1 > PI {
                    a1 += TAU;
                }
                d.arc(hinge, width, a0, a1, LineStyle::Thin, id);
            }
            ElementKind::Window(_) => {
                for v in [-0.01, 0.01] {
                    d.line(g.world(o.u0, v), g.world(o.u1, v), LineStyle::Thin, id);
                }
            }
            _ => {}
        }
    }
}

fn ceiling_grid(d: &mut Drawing2D, boundary: &[DVec2], id: cc_core::ElementId) {
    let Some((lo, hi)) = boundary
        .iter()
        .fold(None, |acc: Option<(DVec2, DVec2)>, p| Some(acc.map_or((*p, *p), |(lo, hi)| (lo.min(*p), hi.max(*p)))))
    else {
        return;
    };
    for (axis, dir, start, end) in [(0, DVec2::Y, lo.x, hi.x), (1, DVec2::X, lo.y, hi.y)] {
        let mut t = start + CEILING_GRID;
        while t < end - 1e-6 {
            let origin = if axis == 0 { DVec2::new(t, lo.y - 1.0) } else { DVec2::new(lo.x - 1.0, t) };
            for (a, b) in polygon::clip_line(boundary, origin, dir) {
                d.line(origin + dir * a, origin + dir * b, LineStyle::Thin, Some(id));
            }
            t += CEILING_GRID;
        }
    }
}

fn section_mark(d: &mut Drawing2D, start: DVec2, end: DVec2, depth: f64, name: &str) {
    let dir = (end - start).normalize_or_zero();
    let look = -dir.perp();
    d.line(start, end, LineStyle::Annotation, None);
    for p in [start, end] {
        let tip = p + look * 0.6;
        d.line(p, tip, LineStyle::Annotation, None);
        d.line(tip, tip - look * 0.2 + dir * 0.12, LineStyle::Annotation, None);
        d.line(tip, tip - look * 0.2 - dir * 0.12, LineStyle::Annotation, None);
    }
    // Far clip extent, dashed.
    d.line(start + look * depth, end + look * depth, LineStyle::Hidden, None);
    d.text(start - dir * 0.4 + DVec2::new(0.0, 0.1), name, 2.5);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Prim;
    use cc_core::{Category, Document, Element, ElementId, Hosted, Wall};

    #[test]
    fn plan_cuts_wall_and_gaps_at_door() {
        let mut doc = Document::new(Project::from_template());
        let level = doc.project.levels[0].id;
        let (wall_id, door_id) = doc.edit("t", |e| {
            let wall_t = e.project().default_type(Category::Wall).unwrap();
            let door_t = e.project().default_type(Category::Door).unwrap();
            let w = ElementId(e.alloc_id());
            e.add_element(Element {
                id: w,
                level,
                type_id: wall_t,
                mark: String::new(),
                comments: String::new(),
                kind: ElementKind::Wall(Wall {
                    start: DVec2::ZERO,
                    end: DVec2::new(5.0, 0.0),
                    base_offset: 0.0,
                    height: 3.0,
                }),
            });
            let dr = ElementId(e.alloc_id());
            e.add_element(Element {
                id: dr,
                level,
                type_id: door_t,
                mark: "1".into(),
                comments: String::new(),
                kind: ElementKind::Door(Hosted {
                    host: w,
                    offset: 2.5,
                    sill_height: 0.0,
                    flip_facing: false,
                    flip_hand: false,
                }),
            });
            (w, dr)
        });
        let geom = ModelGeometry::build(&doc.project);
        let d = plan(&doc.project, &geom, level, 1.2, false);
        let fills =
            d.prims.iter().filter(|p| matches!(p, Prim::Fill { element: Some(id), .. } if *id == wall_id)).count();
        assert_eq!(fills, 2, "wall poche split by the door");
        assert_eq!(d.hit_test(DVec2::new(1.0, 0.0), 0.01), Some(wall_id));
        // On the open leaf, drawn from the hinge at u = 2.1 on the left face.
        assert_eq!(d.hit_test(DVec2::new(2.12, 0.5), 0.05), Some(door_id));
    }
}
