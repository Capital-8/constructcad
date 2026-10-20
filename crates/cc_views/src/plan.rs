use crate::{Drawing2D, LineStyle};
use cc_core::{ElementKind, LevelId, Project, TypeData, ViewKind};
use cc_geom::{Column, ModelGeometry, WallGeom, polygon};
use glam::DVec2;
use std::f64::consts::{PI, TAU};

pub(crate) use crate::cut::POCHE;
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
pub fn plan(
    project: &Project,
    geom: &ModelGeometry,
    level: LevelId,
    cut_height: f64,
    ceiling: bool,
    scale: u32,
) -> Drawing2D {
    let spacing = crate::cut::spacing(scale);
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
                for &(a, b) in geom.roof_ridges.get(&e.id).into_iter().flatten() {
                    d.line(a, b, LineStyle::Projection, id);
                }
            }
            _ => {}
        }
    }
    if !ceiling {
        draw_structure(&mut d, project, level, cut_z, spacing);
    }
    draw_components(&mut d, project, level, cut_z, ceiling);

    for (e, _) in project.walls().filter(|(e, _)| e.level == level) {
        if let Some(g) = geom.walls.get(&e.id) {
            draw_wall(&mut d, project, g, cut_z, spacing);
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
            // Markers stay outside the grid lines too, which usually extend past the model.
            let grid_ends = project.elements.values().filter_map(|e| match &e.kind {
                ElementKind::Grid(g) => Some([g.start, g.end]),
                _ => None,
            });
            let (lo, hi) = grid_ends
                .flatten()
                .fold((DVec2::new(lo.x, lo.y), DVec2::new(hi.x, hi.y)), |(lo, hi), p| (lo.min(p), hi.max(p)));
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

/// Columns, beams, stairs and railings of `level` as seen from the cut plane.
/// Outlines of component pieces marked for plans: in floor plans the pieces
/// starting below the cut (pieces above it dashed), in ceiling plans only
/// the pieces above the cut (light fixtures and the like).
fn draw_components(d: &mut Drawing2D, project: &Project, level: LevelId, cut_z: f64, ceiling: bool) {
    for e in project.elements.values().filter(|e| e.level == level) {
        let ElementKind::Component(c) = &e.kind else { continue };
        for part in cc_geom::component_parts(project, e, c).into_iter().filter(|p| p.in_plan) {
            if !part.faces.is_empty() {
                draw_mesh(d, &part.faces, cut_z, ceiling, e.id);
                continue;
            }
            let above = part.z0 >= cut_z;
            let style = match (ceiling, above) {
                (false, false) | (true, true) => LineStyle::Projection,
                (false, true) => LineStyle::Hidden,
                (true, false) => continue,
            };
            d.polyline(&part.footprint, true, style, Some(e.id));
        }
    }
}

/// A mesh piece: its section at the cut plane, the edges seen below it and,
/// dashed, those above (in ceiling plans only the edges above).
fn draw_mesh(d: &mut Drawing2D, faces: &[Vec<glam::DVec3>], cut_z: f64, ceiling: bool, id: cc_core::ElementId) {
    let plan = cc_geom::mesh_plan(faces, cut_z);
    let id = Some(id);
    if ceiling {
        plan.above.iter().for_each(|&(a, b)| d.line(a, b, LineStyle::Projection, id));
        return;
    }
    plan.cut.iter().for_each(|&(a, b)| d.line(a, b, LineStyle::Cut, id));
    plan.below.iter().for_each(|&(a, b)| d.line(a, b, LineStyle::Projection, id));
    plan.above.iter().for_each(|&(a, b)| d.line(a, b, LineStyle::Hidden, id));
}

fn draw_structure(d: &mut Drawing2D, project: &Project, level: LevelId, cut_z: f64, spacing: f64) {
    for e in project.elements.values().filter(|e| e.level == level) {
        let id = Some(e.id);
        let data = project.element_type(e.type_id).map(|t| &t.data);
        match (&e.kind, data) {
            (ElementKind::Column(c), Some(TypeData::Column { section, .. })) => {
                let outline = cc_geom::column_outline(*section, c.position, c.rotation_deg);
                let (base, top) = project.column_span(e, c);
                if base <= cut_z && cut_z <= top {
                    let material = project.element_type(e.type_id).map_or("", |t| t.data.cut_material());
                    crate::cut::cut_fill(d, project, material, outline.clone(), spacing, id);
                    d.polyline(&outline, true, LineStyle::Cut, id);
                } else {
                    let style = if base > cut_z { LineStyle::Hidden } else { LineStyle::Projection };
                    d.polyline(&outline, true, style, id);
                }
            }
            (ElementKind::Beam(b), Some(TypeData::Beam { width, height, .. })) => {
                let Some(u) = (b.end - b.start).try_normalize() else { continue };
                let n = u.perp() * (width / 2.0);
                let top = project.level_elevation(e.level) + b.top_offset;
                // Beams above the cut plane are dashed; cut beams are drawn heavy.
                let style = if top - height > cut_z {
                    LineStyle::Hidden
                } else if top >= cut_z {
                    LineStyle::Cut
                } else {
                    LineStyle::Projection
                };
                d.polyline(&[b.start - n, b.end - n, b.end + n, b.start + n], true, style, id);
            }
            (ElementKind::Stair(st), Some(TypeData::Stair { .. })) => {
                if let Some(g) = cc_geom::stair_geom(project, e, st) {
                    draw_stair(d, &g, cut_z, e.id);
                }
            }
            (ElementKind::Railing(r), Some(TypeData::Railing { .. })) => {
                d.polyline(&r.path, false, LineStyle::Projection, id);
            }
            _ => {}
        }
    }
}

/// Stair symbol: outline, treads (dashed beyond the cut), a break line where
/// the run passes the cut plane and an arrow pointing up the run.
fn draw_stair(d: &mut Drawing2D, g: &cc_geom::StairGeom, cut_z: f64, element: cc_core::ElementId) {
    let id = Some(element);
    let w = g.width / 2.0;
    let p = |a: f64, b: f64| g.start + g.u * a + g.v * b;
    // Distance along the run where the treads reach the cut plane.
    let cut_u = if g.riser > 0.0 { ((cut_z - g.base) / g.riser).floor() * g.tread } else { g.run() };
    let cut_u = cut_u.clamp(0.0, g.run());
    let style_at = |u: f64| if u <= cut_u + 1e-9 { LineStyle::Projection } else { LineStyle::Hidden };
    for side in [-w, w] {
        d.line(p(0.0, side), p(cut_u, side), LineStyle::Projection, id);
        d.line(p(cut_u, side), p(g.run(), side), LineStyle::Hidden, id);
    }
    for k in 0..g.risers {
        let u = k as f64 * g.tread;
        d.line(p(u, -w), p(u, w), style_at(u), id);
    }
    if cut_u < g.run() {
        // Break line: a diagonal with a small zigzag across the run.
        let (a, b) = (p(cut_u - 0.15, -w), p(cut_u + 0.15, w));
        let m = (a + b) * 0.5;
        let z = g.u * 0.08;
        d.polyline(&[a, m - z, m + z, b], false, LineStyle::Projection, id);
    }
    // Arrow from the first riser up the run, to the cut (or the top).
    let end = p(cut_u.max(g.tread).min(g.run()), 0.0);
    d.line(p(0.0, 0.0), end, LineStyle::Annotation, id);
    let head = 0.12;
    d.line(end, end - g.u * head + g.v * head * 0.6, LineStyle::Annotation, id);
    d.line(end, end - g.u * head - g.v * head * 0.6, LineStyle::Annotation, id);
    let foot = g.v * 0.06;
    d.line(p(0.0, 0.0) - foot, p(0.0, 0.0) + foot, LineStyle::Annotation, id);
}

fn draw_wall(d: &mut Drawing2D, project: &Project, g: &WallGeom, cut_z: f64, spacing: f64) {
    let id = Some(g.element);
    let h = g.half;
    let states: Vec<ColumnCut> = g.columns.iter().map(|c| column_state(c, cut_z)).collect();
    for (i, c) in g.columns.iter().enumerate() {
        let poly = g.column_polygon(c);
        let state = states[i];
        if state == ColumnCut::Cut {
            // Each layer with its material's cut pattern.
            if g.layers.is_empty() {
                d.fill(poly.to_vec(), POCHE, id);
            }
            for (v0, v1, material) in &g.layers {
                let at = |v: f64| {
                    let s = (v + h) / (2.0 * h);
                    (c.right.0 + (c.left.0 - c.right.0) * s, c.right.1 + (c.left.1 - c.right.1) * s)
                };
                let ((a0, a1), (b0, b1)) = (at(*v0), at(*v1));
                let strip = vec![g.world(a0, *v0), g.world(a1, *v0), g.world(b1, *v1), g.world(b0, *v1)];
                crate::cut::cut_fill(d, project, material, strip, spacing, id);
            }
            // Interior layer boundaries; the outer faces are drawn below.
            for &(_, v, _) in g.layers.iter().take(g.layers.len().saturating_sub(1)) {
                let s = (v + h) / (2.0 * h);
                let u0 = c.right.0 + (c.left.0 - c.right.0) * s;
                let u1 = c.right.1 + (c.left.1 - c.right.1) * s;
                d.line(g.world(u0, v), g.world(u1, v), LineStyle::Thin, id);
            }
        }
        if let Some(style) = style_of(state) {
            // Faces, opened where other walls meet them in a T.
            for (side, (u0, u1)) in [(-1.0, c.right), (1.0, c.left)] {
                for (a, b) in g.face_segments(side, u0, u1) {
                    d.line(g.world(a, side * h), g.world(b, side * h), style, id);
                }
            }
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
                uid: Default::default(),
                kind: ElementKind::Wall(Wall {
                    start: DVec2::ZERO,
                    end: DVec2::new(5.0, 0.0),
                    base_offset: 0.0,
                    height: 3.0,
                    ..Default::default()
                }),
            });
            let dr = ElementId(e.alloc_id());
            e.add_element(Element {
                id: dr,
                level,
                type_id: door_t,
                mark: "1".into(),
                comments: String::new(),
                uid: Default::default(),
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
        let d = plan(&doc.project, &geom, level, 1.2, false, 50);
        let fills =
            d.prims.iter().filter(|p| matches!(p, Prim::Fill { element: Some(id), .. } if *id == wall_id)).count();
        // One fill per layer on each side of the door.
        let layers = geom.walls[&wall_id].layers.len();
        assert_eq!(fills, 2 * layers, "wall cut split by the door");
        // The template's masonry core is hatched (the hatch belongs to the wall).
        let hatched =
            |p: &Prim| matches!(p, Prim::Line { style: LineStyle::Thin, element: Some(e), .. } if *e == wall_id);
        assert!(d.prims.iter().any(hatched));
        assert_eq!(d.hit_test(DVec2::new(1.0, 0.0), 0.01), Some(wall_id));
        // On the open leaf, drawn from the hinge at u = 2.1 on the left face.
        assert_eq!(d.hit_test(DVec2::new(2.12, 0.5), 0.05), Some(door_id));
    }

    /// A mesh piece (as imported from IFC) is cut at the plan's cut plane.
    #[test]
    fn plan_cuts_mesh_pieces() {
        use cc_core::{Component, ComponentDef, ComponentItem, ComponentShape, ElementType, ElementTypeId, TypeData};
        let mut doc = Document::new(Project::from_template());
        let level = doc.project.levels[0].id;
        // A 1 m cube standing on the level, rotated 90° and moved to (5, 0).
        let v = |i: u32| [f64::from(i & 1), f64::from((i >> 1) & 1), f64::from((i >> 2) & 1)];
        let faces = vec![
            vec![0, 2, 3, 1],
            vec![4, 5, 7, 6],
            vec![0, 1, 5, 4],
            vec![1, 3, 7, 5],
            vec![3, 2, 6, 7],
            vec![2, 0, 4, 6],
        ];
        let shape = ComponentShape::Mesh { vertices: (0..8).map(v).collect(), faces };
        let item = ComponentItem { name: "m".into(), shape, material: String::new(), in_plan: true };
        let id = doc.edit("t", |e| {
            let type_id = ElementTypeId(e.alloc_id());
            let def = ComponentDef { items: vec![item], ..Default::default() };
            let data = TypeData::Component { component: "Mesh".into(), def };
            e.add_type(ElementType {
                id: type_id,
                name: "Mesh".into(),
                data,
                info: Default::default(),
                uid: Default::default(),
            });
            let id = ElementId(e.alloc_id());
            let kind = ElementKind::Component(Component {
                position: DVec2::new(5.0, 0.0),
                rotation_deg: 90.0,
                base_offset: 0.0,
                overrides: Default::default(),
                properties: Vec::new(),
            });
            e.add_element(Element {
                id,
                level,
                type_id,
                mark: String::new(),
                comments: String::new(),
                uid: Default::default(),
                kind,
            });
            id
        });
        let geom = ModelGeometry::build(&doc.project);
        assert!((geom.volume(id) - 1.0).abs() < 1e-9);
        let d = plan(&doc.project, &geom, level, 0.5, false, 50);
        let cut: Vec<_> = d
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Line { a, b, style: LineStyle::Cut, element: Some(e) } if *e == id => Some((*a, *b)),
                _ => None,
            })
            .collect();
        assert_eq!(cut.len(), 4);
        // Rotated about the insertion point: the square spans x 4..5, y 0..1.
        assert!(cut.iter().all(|(a, b)| {
            [a, b].iter().all(|p| (4.0 - 1e-9..=5.0 + 1e-9).contains(&p.x) && (-1e-9..=1.0 + 1e-9).contains(&p.y))
        }));
    }
}
