//! Annotations drawn over views: room, door and window tags, grids and
//! dimensions. Sizes are given in paper millimeters and converted with the
//! view scale, so annotations read the same at any scale.

use crate::drawing::Anchor;
use crate::{Drawing2D, LineStyle, format};
use cc_core::{Dimension, DimensionKind, ElementKind, LevelId, Project, TypeData, ViewId};
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
    structural: bool,
    scale: u32,
) {
    let mm = mm(scale);
    for e in project.elements.values() {
        match &e.kind {
            ElementKind::Room(room) if e.level == level && !structural => {
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
    if !ceiling && !structural {
        opening_tags(d, project, geom, level, mm);
    }
}

/// A section size: centimeters, without decimals when whole, or inches
/// with a fraction in imperial units.
fn cm(meters: f64) -> String {
    if format::imperial() {
        return format::inches(meters);
    }
    let v = meters * 100.0;
    if (v - v.round()).abs() < 1e-6 { format!("{v:.0}") } else { format!("{v:.1}") }
}

/// A section's size in centimeters (inches in imperial units): `15×40`, `Ø30`.
pub fn section_label(data: &TypeData) -> Option<String> {
    use cc_core::ColumnSection as C;
    Some(match data {
        TypeData::Beam { width, height, .. } => format!("{}×{}", cm(*width), cm(*height)),
        TypeData::Column { section: C::Round { diameter } | C::Pipe { diameter, .. }, .. } => {
            format!("Ø{}", cm(*diameter))
        }
        TypeData::Column { section, .. } => {
            let (w, d) = section.size();
            format!("{}×{}", cm(w), cm(d))
        }
        TypeData::Foundation { kind: cc_core::FoundationKind::Pad, width, length, .. } => {
            format!("{}×{}", cm(*width), cm(*length))
        }
        _ => return None,
    })
}

/// Tags of the beams, columns and pad footings of `level`: their mark and
/// section (`V1 15×40`). Beam tags run along the beam, beside it; column
/// and footing tags sit above and to the right.
pub fn structure_tags(d: &mut Drawing2D, project: &Project, geom: &ModelGeometry, level: LevelId, scale: u32) {
    let mm = mm(scale);
    for e in project.elements.values().filter(|e| e.level == level) {
        let Some(section) = project.element_type(e.type_id).and_then(|t| section_label(&t.data)) else { continue };
        let text = if e.mark.is_empty() { section } else { format!("{} {section}", e.mark) };
        match &e.kind {
            ElementKind::Beam(_) => {
                let Some(g) = geom.beams.get(&e.id) else { continue };
                let u = (g.start_ext.0.max(g.start_ext.1) + g.end_ext.0.min(g.end_ext.1)) / 2.0;
                // Readable: text runs left to right or bottom to top.
                let mut dir = g.dir;
                if dir.x < -1e-9 || (dir.x.abs() <= 1e-9 && dir.y < 0.0) {
                    dir = -dir;
                }
                let up = dir.perp();
                let p = g.world(u, 0.0) + up * (g.half + 1.0 * mm);
                d.tag_text(p, &text, 2.5, Anchor::CenterBottom, dir.y.atan2(dir.x), Some(e.id));
            }
            ElementKind::Column(c) if c.structural => {
                let Some(TypeData::Column { section, .. }) = project.element_type(e.type_id).map(|t| &t.data) else {
                    continue;
                };
                let outline = cc_geom::column_outline(*section, c.position, c.rotation_deg);
                let hi = outline.iter().fold(DVec2::splat(f64::NEG_INFINITY), |a, p| a.max(*p));
                d.tag_text(hi + DVec2::splat(1.0 * mm), &text, 2.5, Anchor::LeftBottom, 0.0, Some(e.id));
            }
            ElementKind::Foundation(_) => {
                let Some(g) = geom.foundations.get(&e.id) else { continue };
                let lo = g.footprint.iter().fold(DVec2::splat(f64::INFINITY), |a, p| a.min(*p));
                let hi = g.footprint.iter().fold(DVec2::splat(f64::NEG_INFINITY), |a, p| a.max(*p));
                d.tag_text(DVec2::new(hi.x + mm, lo.y), &text, 2.5, Anchor::LeftBottom, 0.0, Some(e.id));
            }
            _ => {}
        }
    }
}

fn room_tag(d: &mut Drawing2D, p: DVec2, name: &str, number: &str, area: &str, mm: f64, id: cc_core::ElementId) {
    let line = 3.6 * mm;
    d.tag_text(p + DVec2::new(0.0, 0.4 * mm), name, 2.5, Anchor::CenterBottom, 0.0, Some(id));
    d.tag_text(p - DVec2::new(0.0, line - 0.4 * mm), number, 2.2, Anchor::CenterBottom, 0.0, Some(id));
    d.tag_text(p - DVec2::new(0.0, 2.0 * line - 0.4 * mm), area, 2.0, Anchor::CenterBottom, 0.0, Some(id));
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
    d.tag_text(center, name, bubble_mm * 0.45, Anchor::Center, 0.0, Some(id));
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
            d.tag_text(c, &e.mark, 2.0, Anchor::Center, 0.0, Some(e.id));
        }
    }
}

/// Dimensions placed in `view`, in its drawing coordinates.
pub fn dimensions(d: &mut Drawing2D, project: &Project, view: ViewId, scale: u32) {
    for e in project.elements.values() {
        let ElementKind::Dimension(dim) = &e.kind else { continue };
        if dim.view != view {
            continue;
        }
        let text_size = match project.element_type(e.type_id).map(|t| &t.data) {
            Some(TypeData::Dimension { text_size }) => *text_size,
            _ => 2.5,
        };
        let Some(layout) = dimension_layout(dim, scale) else { continue };
        let id = Some(e.id);
        for (a, b, style) in layout.lines {
            d.line(a, b, style, id);
        }
        d.tag_text(layout.text_pos, layout.text, text_size, Anchor::CenterBottom, layout.text_angle, id);
    }
}

/// Linework and text of a dimension, in its view's drawing coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionLayout {
    pub lines: Vec<(DVec2, DVec2, LineStyle)>,
    pub text: String,
    /// Bottom center of the text, which is rotated by `text_angle` (radians).
    pub text_pos: DVec2,
    pub text_angle: f64,
}

/// Lay out a dimension at a view `scale`; `None` when it measures nothing.
pub fn dimension_layout(dim: &Dimension, scale: u32) -> Option<DimensionLayout> {
    let mm = mm(scale);
    let mut lines = Vec::new();
    let (text, text_pos, text_angle) = match dim.kind {
        DimensionKind::Aligned | DimensionKind::Linear { .. } => {
            let dir = dim.direction()?;
            let n = dir.perp();
            let measured = (dim.b - dim.a).dot(dir);
            if measured.abs() < 1e-9 {
                return None;
            }
            let pa = dim.a + n * dim.offset;
            let pb = pa + dir * measured;
            // Witness lines leave a small gap at the measured points and pass the dimension line.
            let side = if dim.offset >= 0.0 { 1.0 } else { -1.0 };
            for (p, q) in [(dim.a, pa), (dim.b, pb)] {
                let v = q - p;
                let out = v.try_normalize().unwrap_or(n * side);
                let from = p + out * (1.0 * mm).min(v.length());
                lines.push((from, q + out * 1.5 * mm, LineStyle::Annotation));
            }
            let u = dir * measured.signum();
            lines.push((pa - u * 1.5 * mm, pb + u * 1.5 * mm, LineStyle::Annotation));
            // Architectural ticks.
            let tick = (u + u.perp()) * std::f64::consts::FRAC_1_SQRT_2 * 1.3 * mm;
            for p in [pa, pb] {
                lines.push((p - tick, p + tick, LineStyle::Cut));
            }
            let (angle, up) = upright(u);
            (format::meters(dim.value()), (pa + pb) / 2.0 + up * 0.8 * mm, angle)
        }
        DimensionKind::Angular { vertex, .. } => {
            let (r1, r2) = ((dim.a - vertex).try_normalize()?, (dim.b - vertex).try_normalize()?);
            let sweep = r1.angle_to(r2);
            if sweep.abs() < 1e-9 {
                return None;
            }
            let radius = dim.offset.abs().max(2.0 * mm);
            let at = |t: f64| vertex + DVec2::from_angle(sweep * t).rotate(r1) * radius;
            let steps = 32;
            for i in 0..steps {
                lines.push((at(i as f64 / steps as f64), at((i + 1) as f64 / steps as f64), LineStyle::Annotation));
            }
            // Witness lines along the rays when the arc lies beyond the measured points.
            for (r, p) in [(r1, dim.a), (r2, dim.b)] {
                let reach = (p - vertex).length();
                if radius > reach + 1.0 * mm {
                    lines.push((
                        vertex + r * (reach + 1.0 * mm),
                        vertex + r * (radius + 1.5 * mm),
                        LineStyle::Annotation,
                    ));
                }
            }
            for (r, t) in [(r1, 0.0), (r2, 1.0)] {
                let p = at(t);
                let tick = (r + r.perp()) * std::f64::consts::FRAC_1_SQRT_2 * 1.3 * mm;
                lines.push((p - tick, p + tick, LineStyle::Cut));
            }
            let mid = DVec2::from_angle(sweep / 2.0).rotate(r1);
            let (angle, up) = upright(mid.perp());
            // The text sits outside the arc.
            let out = if up.dot(mid) >= 0.0 { 0.8 * mm } else { 0.8 * mm + 2.5 * mm };
            (format::angle(dim.value().to_degrees()), vertex + mid * (radius + out), angle)
        }
        DimensionKind::Radial | DimensionKind::Diameter => {
            let center = dim.a;
            let r = (dim.b - center).length();
            let u = (dim.b - center).try_normalize()?;
            let diameter = matches!(dim.kind, DimensionKind::Diameter);
            let start = if diameter { center - u * r } else { center };
            lines.push((start, dim.b, LineStyle::Annotation));
            let mut tips = vec![(dim.b, u)];
            if diameter {
                tips.push((start, -u));
            }
            for (tip, dir) in tips {
                for side in [-1.0, 1.0] {
                    let back = DVec2::from_angle(side * 0.33).rotate(-dir) * 2.5 * mm;
                    lines.push((tip, tip + back, LineStyle::Annotation));
                }
            }
            // Center mark.
            let c = 1.0 * mm;
            lines.push((center - DVec2::X * c, center + DVec2::X * c, LineStyle::Annotation));
            lines.push((center - DVec2::Y * c, center + DVec2::Y * c, LineStyle::Annotation));
            let (angle, up) = upright(u);
            let prefix = if diameter { "\u{d8}" } else { "R" };
            let mid = if diameter { center + u * r * 0.5 } else { (center + dim.b) / 2.0 };
            (format!("{prefix} {}", format::meters(dim.value())), mid + up * 0.8 * mm, angle)
        }
    };
    Some(DimensionLayout { lines, text, text_pos, text_angle })
}

/// Text angle that keeps text along `u` upright, and the side "above" it.
fn upright(u: DVec2) -> (f64, DVec2) {
    let mut angle = u.y.atan2(u.x);
    let mut up = u.perp();
    if angle > std::f64::consts::FRAC_PI_2 + 1e-9 || angle <= -std::f64::consts::FRAC_PI_2 + 1e-9 {
        angle += std::f64::consts::PI;
        up = -up;
    }
    (angle, up)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::{Dimension, ViewId};

    fn dim(a: (f64, f64), b: (f64, f64), offset: f64, kind: DimensionKind) -> Dimension {
        Dimension {
            view: ViewId(1),
            a: DVec2::new(a.0, a.1),
            b: DVec2::new(b.0, b.1),
            offset,
            kind,
            refs: Default::default(),
        }
    }

    #[test]
    fn linear_dimensions_measure_along_their_direction() {
        let d = dim((0.0, 0.0), (3.0, 4.0), 1.0, DimensionKind::Linear { dir: DVec2::X });
        assert!((d.value() - 3.0).abs() < 1e-12);
        let d = dim((0.0, 0.0), (3.0, 4.0), 1.0, DimensionKind::Linear { dir: DVec2::Y });
        assert!((d.value() - 4.0).abs() < 1e-12);
        let layout = dimension_layout(&d, 100).unwrap();
        // The dimension line is vertical, one meter to the left of the points (x = -1).
        assert!(layout.lines.iter().any(|(a, b, _)| (a.x + 1.0).abs() < 1e-9 && (b.x + 1.0).abs() < 1e-9));
        assert!(
            dimension_layout(&dim((0.0, 0.0), (0.0, 4.0), 1.0, DimensionKind::Linear { dir: DVec2::X }), 100).is_none()
        );
    }

    #[test]
    fn angular_radial_and_diameter_values() {
        let d =
            dim((2.0, 0.0), (0.0, 2.0), 1.0, DimensionKind::Angular { vertex: DVec2::ZERO, rays: Default::default() });
        assert!((d.value().to_degrees() - 90.0).abs() < 1e-9);
        assert!(dimension_layout(&d, 50).is_some_and(|l| l.lines.len() > 30));
        let r = dim((1.0, 1.0), (1.0, 3.0), 0.0, DimensionKind::Radial);
        assert!((r.value() - 2.0).abs() < 1e-12);
        assert!(dimension_layout(&r, 50).unwrap().text.starts_with('R'));
        let d = Dimension { kind: DimensionKind::Diameter, ..r };
        assert!((d.value() - 4.0).abs() < 1e-12);
        assert!(dimension_layout(&d, 50).unwrap().text.starts_with('\u{d8}'));
    }
}
