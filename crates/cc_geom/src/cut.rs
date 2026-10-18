//! Plane cuts used by sections and plan views.

use crate::Face;
use glam::DVec3;

/// Segments where `face` crosses the plane through `point` with `normal`.
pub fn face_plane_segments(face: &Face, point: DVec3, normal: DVec3) -> Vec<(DVec3, DVec3)> {
    let line_dir = face.normal.cross(normal);
    if line_dir.length_squared() < 1e-12 {
        return Vec::new(); // face parallel to the plane
    }
    let n = face.verts.len();
    let dist: Vec<f64> = face.verts.iter().map(|v| (*v - point).dot(normal)).collect();
    let mut hits = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let (da, db) = (dist[i], dist[j]);
        // Half-open test so a vertex lying on the plane is counted once.
        if (da < 0.0) != (db < 0.0) {
            let t = da / (da - db);
            hits.push(face.verts[i].lerp(face.verts[j], t));
        }
    }
    hits.sort_by(|a, b| a.dot(line_dir).total_cmp(&b.dot(line_dir)));
    hits.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).filter(|(a, b)| a.distance(*b) > 1e-9).collect()
}

/// Sutherland–Hodgman clip keeping the part with `(p - point)·normal >= 0`.
pub fn clip_polygon(verts: &[DVec3], point: DVec3, normal: DVec3) -> Vec<DVec3> {
    let n = verts.len();
    let mut out = Vec::with_capacity(n + 2);
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let da = (a - point).dot(normal);
        let db = (b - point).dot(normal);
        if da >= 0.0 {
            out.push(a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            out.push(a.lerp(b, da / (da - db)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Solid;
    use cc_core::{Category, ElementId};
    use glam::DVec2;

    #[test]
    fn cutting_a_box_gives_its_outline() {
        let mut s = Solid::new(ElementId(1), Category::Wall, [0; 3]);
        s.add_box(DVec2::ZERO, DVec2::X, DVec2::Y, 0.0, 2.0, 0.0, 1.0, 0.0, 3.0);
        let segs: Vec<_> =
            s.faces.iter().flat_map(|f| face_plane_segments(f, DVec3::new(0.0, 0.0, 1.5), DVec3::Z)).collect();
        assert_eq!(segs.len(), 4);
        let total: f64 = segs.iter().map(|(a, b)| a.distance(*b)).sum();
        assert!((total - 6.0).abs() < 1e-9);
    }

    #[test]
    fn clip_square_in_half() {
        let sq = [DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y];
        let half = clip_polygon(&sq, DVec3::new(0.5, 0.0, 0.0), DVec3::X);
        assert_eq!(half.len(), 4);
        assert!(half.iter().all(|p| p.x >= 0.5 - 1e-12));
    }
}
