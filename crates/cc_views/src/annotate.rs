//! Annotations drawn over views: room, door and window tags, grids and
//! dimensions. Sizes are given in paper millimeters and converted with the
//! view scale, so annotations read the same at any scale.

use crate::drawing::Anchor;
use crate::{Drawing2D, LineStyle, format};
use cc_core::{ElementKind, LevelId, Project, TypeData, ViewId};
use cc_geom::{ModelGeometry, polygon};
use glam::DVec2;

/// Model meters per paper millimeter at `scale`.
fn mm(scale: u32) -> f64 {
    scale as f64 / 1000.0
}

/// Fill color that is invisible but highlights when the element is selected.
pub const HIGHLIGHT_ONLY: [u8; 4] = [255, 255, 255, 0];
/// Nearly invisible fill that makes text tags clickable.
const PICK_AREA: [u8; 4] = [255, 255, 255, 1];

/// Tags of rooms and openings, and grids, for a floor or ceiling plan of `level`.
pub fn plan_annotations(
    d: &mut Drawing2D,
    project: &Project,
    geom: &ModelGeometry,
    level: LevelId,
    ceiling: bool,
    scale: u32,
) {
    let mm = mm(scale);
    for e in project.elements.values() {
        match &e.kind {
            ElementKind::Room(room) if e.level == level => {
                if let Some(boundary) = geom.rooms.get(&e.id) {
                    d.fill(boundary.clone(), HIGHLIGHT_ONLY, Some(e.id));
                    let area = format::area(polygon::area(boundary));
                    room_tag(d, room.position, &room.name, &room.number, &area, mm, e.id);
                } else {
                    // Not enclosed: the tag is crossed out.
                    room_tag(d, room.position, &room.name, &room.number, "—", mm, e.id);
                    let (p, s) = (room.position, 2.5 * mm);
                    d.line(p + DVec2::new(-s, -s), p + DVec2::new(s, s), LineStyle::Annotation, Some(e.id));
                    d.line(p + DVec2::new(-s, s), p + DVec2::new(s, -s), LineStyle::Annotation, Some(e.id));
                }
            }
            ElementKind::Grid(grid) => {
                let bubble = match project.element_type(e.type_id).map(|t| &t.data) {
                    Some(TypeData::Grid { bubble }) => *bubble,
                    _ => 8.0,
                };
                grid_line(d, grid.start, grid.end, &grid.name, bubble, mm, e.id);
            }
            _ => {}
        }
    }
    if !ceiling {
        opening_tags(d, project, geom, level, mm);
    }
}

fn room_tag(d: &mut Drawing2D, p: DVec2, name: &str, number: &str, area: &str, mm: f64, id: cc_core::ElementId) {
    let line = 3.6 * mm;
    d.text_at(p + DVec2::new(0.0, 0.4 * mm), name, 2.5, Anchor::CenterBottom, 0.0);
    d.text_at(p - DVec2::new(0.0, line - 0.4 * mm), number, 2.2, Anchor::CenterBottom, 0.0);
    d.text_at(p - DVec2::new(0.0, 2.0 * line - 0.4 * mm), area, 2.0, Anchor::CenterBottom, 0.0);
    let width = (name.chars().count().max(number.chars().count()).max(area.chars().count()) as f64 * 1.6 + 2.0) * mm;
    let (x0, x1) = (p.x - width / 2.0, p.x + width / 2.0);
    let (y0, y1) = (p.y - 2.0 * line, p.y + 3.4 * mm);
    let rect = vec![DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)];
    d.fill(rect, PICK_AREA, Some(id));
}

/// Grid line with a bubble of `bubble_mm` paper millimeters at its end.
fn grid_line(d: &mut Drawing2D, start: DVec2, end: DVec2, name: &str, bubble_mm: f64, mm: f64, id: cc_core::ElementId) {
    let dir = (end - start).normalize_or_zero();
    let r = bubble_mm * mm / 2.0;
    let center = end + dir * r;
    d.line(start, end, LineStyle::Center, Some(id));
    let circle: Vec<DVec2> =
        (0..32).map(|i| center + DVec2::from_angle(i as f64 / 32.0 * std::f64::consts::TAU) * r).collect();
    d.polyline(&circle, true, LineStyle::Annotation, Some(id));
    d.fill(circle, PICK_AREA, Some(id));
    d.text_at(center, name, bubble_mm * 0.45, Anchor::Center, 0.0);
}

fn opening_tags(d: &mut Drawing2D, project: &Project, geom: &ModelGeometry, level: LevelId, mm: f64) {
    for wall in geom.walls.values() {
        if project.element(wall.element).is_none_or(|e| e.level != level) {
            continue;
        }
        for o in &wall.openings {
            let Some(e) = project.element(o.element) else { continue };
            if e.mark.is_empty() {
                continue;
            }
            let Some(host) = e.as_hosted() else { continue };
            let door = matches!(e.kind, ElementKind::Door(_));
            // Tags sit outside the wall, on the side opposite the door swing.
            let side = if host.flip_facing { 1.0 } else { -1.0 };
            let u = (o.u0 + o.u1) / 2.0;
            let r = 2.6 * mm;
            let c = wall.world(u, side * (wall.half + r + 2.0 * mm));
            if door {
                let circle: Vec<DVec2> =
                    (0..24).map(|i| c + DVec2::from_angle(i as f64 / 24.0 * std::f64::consts::TAU) * r).collect();
                d.polyline(&circle, true, LineStyle::Annotation, Some(e.id));
            } else {
                // Windows: a hexagon.
                let hex: Vec<DVec2> =
                    (0..6).map(|i| c + DVec2::from_angle(i as f64 / 6.0 * std::f64::consts::TAU) * r).collect();
                d.polyline(&hex, true, LineStyle::Annotation, Some(e.id));
            }
            d.text_at(c, &e.mark, 2.0, Anchor::Center, 0.0);
        }
    }
}

/// Dimensions placed in `view`, in its drawing coordinates.
pub fn dimensions(d: &mut Drawing2D, project: &Project, view: ViewId, scale: u32) {
    let mm = mm(scale);
    for e in project.elements.values() {
        let ElementKind::Dimension(dim) = &e.kind else { continue };
        if dim.view != view || dim.length() < 1e-9 {
            continue;
        }
        let text_size = match project.element_type(e.type_id).map(|t| &t.data) {
            Some(TypeData::Dimension { text_size }) => *text_size,
            _ => 2.5,
        };
        let id = Some(e.id);
        let u = (dim.b - dim.a) / dim.length();
        let n = u.perp();
        let side = if dim.offset >= 0.0 { 1.0 } else { -1.0 };
        let (a, b) = (dim.a + n * dim.offset, dim.b + n * dim.offset);
        // Witness lines leave a small gap at the measured points and pass the dimension line.
        for p in [dim.a, dim.b] {
            let from = p + n * side * (1.0 * mm).min(dim.offset.abs());
            d.line(from, p + n * (dim.offset + side * 1.5 * mm), LineStyle::Annotation, id);
        }
        d.line(a - u * 1.5 * mm, b + u * 1.5 * mm, LineStyle::Annotation, id);
        // Architectural ticks.
        let tick = (u + n) * std::f64::consts::FRAC_1_SQRT_2 * 1.3 * mm;
        for p in [a, b] {
            d.line(p - tick, p + tick, LineStyle::Cut, id);
        }
        // Text above the line, kept upright.
        let mut angle = u.y.atan2(u.x);
        let mut up = n;
        if angle > std::f64::consts::FRAC_PI_2 + 1e-9 || angle <= -std::f64::consts::FRAC_PI_2 + 1e-9 {
            angle += std::f64::consts::PI;
            up = -n;
        }
        let mid = (a + b) / 2.0 + up * 0.8 * mm;
        d.text_at(mid, format::meters(dim.length()), text_size, Anchor::CenterBottom, angle);
    }
}
