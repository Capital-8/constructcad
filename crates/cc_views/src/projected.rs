//! Orthographic projected views: elevations, sections and axonometrics,
//! with painter's-algorithm hidden surface removal.

use crate::{Drawing2D, LineStyle, format, plan::POCHE};
use cc_core::{CameraState, Direction, ElementId, Project};
use cc_geom::{ModelGeometry, clip_polygon, face_plane_segments};
use glam::{DVec2, DVec3};

/// Orthographic view frame with optional clip planes.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub origin: DVec3,
    pub right: DVec3,
    pub up: DVec3,
    /// View direction (into the scene).
    pub forward: DVec3,
    /// Keep depth in `near..far` (relative to `origin`).
    pub near: Option<f64>,
    pub far: Option<f64>,
    /// Keep screen x in this range.
    pub crop_x: Option<(f64, f64)>,
}

impl Frame {
    pub fn looking(origin: DVec3, forward: DVec3) -> Self {
        let forward = forward.normalize();
        let right = forward.cross(DVec3::Z).try_normalize().unwrap_or(DVec3::X);
        let up = right.cross(forward);
        Self { origin, right, up, forward, near: None, far: None, crop_x: None }
    }

    pub fn project(&self, p: DVec3) -> DVec2 {
        let r = p - self.origin;
        DVec2::new(r.dot(self.right), r.dot(self.up))
    }

    pub fn depth(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.forward)
    }

    /// Clip planes as (point, normal) keeping the positive side.
    fn planes(&self) -> Vec<(DVec3, DVec3)> {
        let mut planes = Vec::new();
        if let Some(n) = self.near {
            planes.push((self.origin + self.forward * n, self.forward));
        }
        if let Some(f) = self.far {
            planes.push((self.origin + self.forward * f, -self.forward));
        }
        if let Some((x0, x1)) = self.crop_x {
            planes.push((self.origin + self.right * x0, self.right));
            planes.push((self.origin + self.right * x1, -self.right));
        }
        planes
    }
}

fn clip_segment(a: DVec3, b: DVec3, point: DVec3, normal: DVec3) -> Option<(DVec3, DVec3)> {
    let (da, db) = ((a - point).dot(normal), (b - point).dot(normal));
    match (da >= 0.0, db >= 0.0) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (true, false) => Some((a, a.lerp(b, da / (da - db)))),
        (false, true) => Some((a.lerp(b, da / (da - db)), b)),
    }
}

fn shade(color: [u8; 3], normal: DVec3, frame: &Frame) -> [u8; 4] {
    let light = (frame.forward * 0.6 + frame.right * 0.3 - frame.up * 0.5).normalize();
    let k = 0.62 + 0.38 * (-normal.dot(light)).max(0.0);
    // Wash colors towards white so linework stays legible.
    let c = |v: u8| ((v as f64 * 0.55 + 255.0 * 0.45) * k).min(255.0) as u8;
    [c(color[0]), c(color[1]), c(color[2]), 255]
}

struct ProjectedFace {
    depth: f64,
    pts: Vec<DVec2>,
    color: [u8; 4],
    element: ElementId,
    edges: Vec<(DVec2, DVec2)>,
}

/// Render all solids through `frame`.
pub fn render(geom: &ModelGeometry, frame: &Frame) -> Drawing2D {
    let planes = frame.planes();
    let mut faces = Vec::new();
    for solid in &geom.solids {
        let edges = solid.feature_edges();
        for (face, face_edges) in solid.faces.iter().zip(edges) {
            if face.normal.dot(frame.forward) > -1e-6 {
                continue;
            }
            let mut verts = face.verts.clone();
            for &(p, n) in &planes {
                verts = clip_polygon(&verts, p, n);
            }
            if verts.len() < 3 {
                continue;
            }
            let edges = face_edges
                .into_iter()
                .filter_map(|(a, b)| planes.iter().try_fold((a, b), |(a, b), &(p, n)| clip_segment(a, b, p, n)))
                .map(|(a, b)| (frame.project(a), frame.project(b)))
                .collect();
            let depth = verts.iter().map(|v| frame.depth(*v)).sum::<f64>() / verts.len() as f64;
            faces.push(ProjectedFace {
                depth,
                pts: verts.iter().map(|v| frame.project(*v)).collect(),
                color: shade(solid.color, face.normal, frame),
                element: solid.element,
                edges,
            });
        }
    }
    faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));

    let mut d = Drawing2D::default();
    for f in faces {
        d.fill(f.pts, f.color, Some(f.element));
        for (a, b) in f.edges {
            d.line(a, b, LineStyle::Projection, Some(f.element));
        }
    }

    // Section cut: poche where loops close, heavy outline always.
    if let Some(near) = frame.near {
        let point = frame.origin + frame.forward * near;
        for solid in &geom.solids {
            let segs: Vec<(DVec2, DVec2)> = solid
                .faces
                .iter()
                .flat_map(|f| face_plane_segments(f, point, frame.forward))
                .filter_map(|(a, b)| planes.iter().skip(1).try_fold((a, b), |(a, b), &(p, n)| clip_segment(a, b, p, n)))
                .map(|(a, b)| (frame.project(a), frame.project(b)))
                .collect();
            for pts in chain_loops(&segs) {
                d.fill(pts, POCHE, Some(solid.element));
            }
            for (a, b) in segs {
                d.line(a, b, LineStyle::Cut, Some(solid.element));
            }
        }
    }
    d
}

/// Join segments end-to-end into closed loops.
fn chain_loops(segs: &[(DVec2, DVec2)]) -> Vec<Vec<DVec2>> {
    const TOL: f64 = 1e-6;
    let mut used = vec![false; segs.len()];
    let mut loops = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut pts = vec![segs[start].0, segs[start].1];
        loop {
            let tail = *pts.last().unwrap();
            if pts.len() > 2 && tail.distance(pts[0]) < TOL {
                pts.pop();
                loops.push(pts);
                break;
            }
            let next = (0..segs.len())
                .find(|&i| !used[i] && (segs[i].0.distance(tail) < TOL || segs[i].1.distance(tail) < TOL));
            let Some(i) = next else { break };
            used[i] = true;
            let (a, b) = segs[i];
            pts.push(if a.distance(tail) < TOL { b } else { a });
        }
    }
    loops
}

fn level_lines(d: &mut Drawing2D, project: &Project, frame: &Frame) {
    let Some((lo, hi)) = d.bounds() else { return };
    for level in &project.levels {
        let y = frame.project(DVec3::new(0.0, 0.0, level.elevation)).y;
        let (x0, x1) = (lo.x - 1.5, hi.x + 1.5);
        d.line(DVec2::new(x0, y), DVec2::new(x1, y), LineStyle::Annotation, None);
        // Elevation target symbol: a small triangle on the line.
        d.line(DVec2::new(x1, y), DVec2::new(x1 + 0.15, y + 0.2), LineStyle::Annotation, None);
        d.line(DVec2::new(x1 + 0.15, y + 0.2), DVec2::new(x1 - 0.15, y + 0.2), LineStyle::Annotation, None);
        d.line(DVec2::new(x1 - 0.15, y + 0.2), DVec2::new(x1, y), LineStyle::Annotation, None);
        d.text(DVec2::new(x1 + 0.3, y + 0.12), &level.name, 2.5);
        d.text(DVec2::new(x1 + 0.3, y - 0.18), format::elevation(level.elevation), 2.0);
    }
}

pub fn elevation(project: &Project, geom: &ModelGeometry, direction: Direction) -> Drawing2D {
    let look = direction.look();
    let frame = Frame::looking(DVec3::ZERO, DVec3::new(look.x, look.y, 0.0));
    let mut d = render(geom, &frame);
    level_lines(&mut d, project, &frame);
    d
}

pub fn section(project: &Project, geom: &ModelGeometry, start: DVec2, end: DVec2, depth: f64) -> Drawing2D {
    let dir = (end - start).normalize_or_zero();
    let look = -dir.perp();
    let mut frame = Frame::looking(DVec3::new(start.x, start.y, 0.0), DVec3::new(look.x, look.y, 0.0));
    let xe = frame.project(DVec3::new(end.x, end.y, 0.0)).x;
    frame.near = Some(0.0);
    frame.far = Some(depth.max(0.01));
    frame.crop_x = Some((xe.min(0.0), xe.max(0.0)));
    let mut d = render(geom, &frame);
    level_lines(&mut d, project, &frame);
    d
}

pub fn axonometric(_project: &Project, geom: &ModelGeometry, camera: &CameraState) -> Drawing2D {
    render(geom, &Frame::looking(DVec3::ZERO, camera.forward()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Prim;
    use cc_core::{Category, Document, Element, ElementKind, Wall};

    fn one_wall() -> (Document, ElementId) {
        let mut doc = Document::new(Project::from_template());
        let id = doc.edit("t", |e| {
            let level = e.project().levels[0].id;
            let type_id = e.project().default_type(Category::Wall).unwrap();
            let id = ElementId(e.alloc_id());
            e.add_element(Element {
                id,
                level,
                type_id,
                mark: String::new(),
                comments: String::new(),
                kind: ElementKind::Wall(Wall {
                    start: DVec2::ZERO,
                    end: DVec2::new(4.0, 0.0),
                    base_offset: 0.0,
                    height: 3.0,
                }),
            })
        });
        (doc, id)
    }

    #[test]
    fn south_elevation_shows_wall_face() {
        let (doc, id) = one_wall();
        let geom = ModelGeometry::build(&doc.project);
        let d = elevation(&doc.project, &geom, Direction::South);
        assert_eq!(d.hit_test(DVec2::new(2.0, 1.5), 0.0), Some(id));
        assert_eq!(d.hit_test(DVec2::new(2.0, 3.5), 0.0), None);
    }

    #[test]
    fn section_cuts_wall_with_poche() {
        let (doc, id) = one_wall();
        let geom = ModelGeometry::build(&doc.project);
        // Section line along X at x = 2, looking -Y... crossing the wall north to south.
        let d = section(&doc.project, &geom, DVec2::new(2.0, 2.0), DVec2::new(2.0, -2.0), 5.0);
        let cut = d.prims.iter().filter(|p| matches!(p, Prim::Line { style: LineStyle::Cut, .. })).count();
        assert_eq!(cut, 4);
        let poche = d.prims.iter().filter(|p| matches!(p, Prim::Fill { color, .. } if *color == POCHE)).count();
        assert_eq!(poche, 1);
        assert!(d.elements_in(DVec2::splat(-100.0), DVec2::splat(100.0), false).contains(&id));
    }
}
