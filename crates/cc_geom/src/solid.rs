use crate::polygon;
use cc_core::{Category, ElementId};
use glam::{DVec2, DVec3};

/// A planar polygon face with an outward normal.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub verts: Vec<DVec3>,
    pub normal: DVec3,
}

impl Face {
    pub fn new(verts: Vec<DVec3>, normal: DVec3) -> Self {
        Self { verts, normal: normal.normalize_or_zero() }
    }

    /// Orthonormal in-plane basis `(u, v)`.
    fn basis(&self) -> (DVec3, DVec3) {
        let n = self.normal;
        let helper = if n.z.abs() < 0.9 { DVec3::Z } else { DVec3::X };
        let u = n.cross(helper).normalize();
        (u, n.cross(u))
    }

    /// Triangles as index triples into `verts`.
    pub fn triangulate(&self) -> Vec<[usize; 3]> {
        if self.verts.len() == 4 {
            return vec![[0, 1, 2], [0, 2, 3]];
        }
        let (u, v) = self.basis();
        let flat: Vec<DVec2> = self.verts.iter().map(|p| DVec2::new(p.dot(u), p.dot(v))).collect();
        polygon::triangulate(&flat)
    }

    pub fn centroid(&self) -> DVec3 {
        self.verts.iter().copied().sum::<DVec3>() / self.verts.len().max(1) as f64
    }

    /// Area of the planar polygon.
    pub fn area(&self) -> f64 {
        let n = self.verts.len();
        let sum: DVec3 = (0..n).map(|i| self.verts[i].cross(self.verts[(i + 1) % n])).sum();
        sum.dot(self.normal).abs() * 0.5
    }
}

/// Boundary representation of one element (or a part of one).
#[derive(Debug, Clone)]
pub struct Solid {
    pub element: ElementId,
    pub category: Category,
    pub color: [u8; 3],
    /// 0 (opaque) to 1 (invisible), from the material.
    pub transparency: f64,
    pub faces: Vec<Face>,
}

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: DVec3,
    pub dir: DVec3,
}

impl Solid {
    pub fn new(element: ElementId, category: Category, color: [u8; 3]) -> Self {
        Self { element, category, color, transparency: 0.0, faces: Vec::new() }
    }

    /// An empty solid colored and made transparent like material `name`.
    pub fn of_material(element: ElementId, category: Category, project: &cc_core::Project, name: &str) -> Self {
        Self {
            transparency: project.material_transparency(name),
            ..Self::new(element, category, project.material_color(name))
        }
    }

    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        let mut it = self.faces.iter().flat_map(|f| f.verts.iter().copied());
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

    /// Volume via the divergence theorem (faces must be closed and outward).
    pub fn volume(&self) -> f64 {
        self.faces.iter().map(|f| f.centroid().dot(f.normal) * f.area()).sum::<f64>() / 3.0
    }

    /// Visible outline edges grouped by face: the parts of each face boundary
    /// not shared with a coplanar neighbor face (so split faces read as one).
    pub fn feature_edges(&self) -> Vec<Vec<(DVec3, DVec3)>> {
        let edges = |f: &Face| {
            let n = f.verts.len();
            (0..n).map(move |i| (f.verts[i], f.verts[(i + 1) % n])).collect::<Vec<_>>()
        };
        let all: Vec<Vec<(DVec3, DVec3)>> = self.faces.iter().map(edges).collect();
        let mut out = Vec::with_capacity(all.len());
        for (fi, face) in self.faces.iter().enumerate() {
            let mut face_out = Vec::new();
            for &(a, b) in &all[fi] {
                let ab = b - a;
                let len2 = ab.length_squared();
                if len2 < 1e-12 {
                    continue;
                }
                let mut covered: Vec<(f64, f64)> = Vec::new();
                for (fj, other) in self.faces.iter().enumerate() {
                    if fj == fi || other.normal.dot(face.normal) < 0.9999 {
                        continue;
                    }
                    for &(c, d) in &all[fj] {
                        let off = |p: DVec3| (p - a).cross(ab).length_squared() / len2;
                        if off(c) > 1e-10 || off(d) > 1e-10 {
                            continue;
                        }
                        let (tc, td) = ((c - a).dot(ab) / len2, (d - a).dot(ab) / len2);
                        let (lo, hi) = (tc.min(td).max(0.0), tc.max(td).min(1.0));
                        if hi - lo > 1e-9 {
                            covered.push((lo, hi));
                        }
                    }
                }
                covered.sort_by(|x, y| x.0.total_cmp(&y.0));
                let mut t = 0.0;
                for (lo, hi) in covered {
                    if lo > t + 1e-9 {
                        face_out.push((a + ab * t, a + ab * lo));
                    }
                    t = t.max(hi);
                }
                if t < 1.0 - 1e-9 {
                    face_out.push((a + ab * t, b));
                }
            }
            out.push(face_out);
        }
        out
    }

    /// Nearest hit distance along `ray`, if any.
    pub fn raycast(&self, ray: Ray) -> Option<f64> {
        let mut best: Option<f64> = None;
        for face in &self.faces {
            for [a, b, c] in face.triangulate() {
                if let Some(t) = ray_triangle(ray, face.verts[a], face.verts[b], face.verts[c]) {
                    best = Some(best.map_or(t, |bt| bt.min(t)));
                }
            }
        }
        best
    }

    /// Append planar polygons, each with the normal of its winding (Newell's
    /// method); degenerate polygons are dropped.
    pub fn add_polygons(&mut self, polygons: impl IntoIterator<Item = Vec<DVec3>>) {
        for verts in polygons {
            let n = verts.len();
            let normal: DVec3 = (0..n)
                .map(|i| {
                    let (a, b) = (verts[i], verts[(i + 1) % n]);
                    DVec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y))
                })
                .sum();
            if n >= 3 && normal.length_squared() > 1e-20 && normal.is_finite() {
                self.faces.push(Face::new(verts, normal));
            }
        }
    }

    /// Append an axis-aligned (in the local frame) box.
    ///
    /// `origin` is the frame origin, `u`/`v` horizontal unit axes, Z is up.
    #[allow(clippy::too_many_arguments)]
    pub fn add_box(&mut self, origin: DVec2, u: DVec2, v: DVec2, u0: f64, u1: f64, v0: f64, v1: f64, z0: f64, z1: f64) {
        if u1 - u0 < 1e-6 || v1 - v0 < 1e-6 || z1 - z0 < 1e-6 {
            return;
        }
        let p = |a: f64, b: f64, z: f64| {
            let q = origin + u * a + v * b;
            DVec3::new(q.x, q.y, z)
        };
        let u3 = DVec3::new(u.x, u.y, 0.0);
        let v3 = DVec3::new(v.x, v.y, 0.0);
        let f = &mut self.faces;
        f.push(Face::new(vec![p(u0, v0, z1), p(u1, v0, z1), p(u1, v1, z1), p(u0, v1, z1)], DVec3::Z));
        f.push(Face::new(vec![p(u0, v0, z0), p(u0, v1, z0), p(u1, v1, z0), p(u1, v0, z0)], -DVec3::Z));
        f.push(Face::new(vec![p(u0, v0, z0), p(u1, v0, z0), p(u1, v0, z1), p(u0, v0, z1)], -v3));
        f.push(Face::new(vec![p(u1, v1, z0), p(u0, v1, z0), p(u0, v1, z1), p(u1, v1, z1)], v3));
        f.push(Face::new(vec![p(u0, v1, z0), p(u0, v0, z0), p(u0, v0, z1), p(u0, v1, z1)], -u3));
        f.push(Face::new(vec![p(u1, v0, z0), p(u1, v1, z0), p(u1, v1, z1), p(u1, v0, z1)], u3));
    }

    /// Append a prism between two copies of a polygon: `bottom(p)`/`top(p)` give
    /// the z of each footprint vertex (planar surfaces only).
    pub fn add_prism(&mut self, footprint: &[DVec2], bottom: impl Fn(DVec2) -> f64, top: impl Fn(DVec2) -> f64) {
        let fp = polygon::ccw(footprint);
        let n = fp.len();
        if n < 3 {
            return;
        }
        let lift = |p: DVec2, z: f64| DVec3::new(p.x, p.y, z);
        let plane_normal = |pts: &[DVec3]| -> DVec3 {
            let sum: DVec3 = (0..pts.len()).map(|i| pts[i].cross(pts[(i + 1) % pts.len()])).sum();
            sum.normalize_or_zero()
        };
        let top_pts: Vec<DVec3> = fp.iter().map(|&p| lift(p, top(p))).collect();
        let mut bottom_pts: Vec<DVec3> = fp.iter().map(|&p| lift(p, bottom(p))).collect();
        let top_n = plane_normal(&top_pts);
        bottom_pts.reverse();
        let bottom_n = plane_normal(&bottom_pts);
        self.faces.push(Face::new(top_pts, top_n));
        self.faces.push(Face::new(bottom_pts, bottom_n));
        for i in 0..n {
            let (a, b) = (fp[i], fp[(i + 1) % n]);
            let out = -(b - a).normalize_or_zero().perp();
            self.faces.push(Face::new(
                vec![lift(a, bottom(a)), lift(b, bottom(b)), lift(b, top(b)), lift(a, top(a))],
                DVec3::new(out.x, out.y, 0.0),
            ));
        }
    }
}

/// Möller–Trumbore, two-sided.
fn ray_triangle(ray: Ray, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let p = ray.dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = ray.origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = ray.dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-9).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_volume_and_raycast() {
        let mut s = Solid::new(ElementId(1), Category::Wall, [0; 3]);
        s.add_box(DVec2::ZERO, DVec2::X, DVec2::Y, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0);
        assert!((s.volume() - 24.0).abs() < 1e-9);
        let hit = s.raycast(Ray { origin: DVec3::new(1.0, -5.0, 1.0), dir: DVec3::Y }).unwrap();
        assert!((hit - 5.0).abs() < 1e-9);
    }

    #[test]
    fn split_faces_hide_shared_edges() {
        let mut s = Solid::new(ElementId(1), Category::Wall, [0; 3]);
        // Two boxes side by side form one 2x1x1 block: its outline has 12 edges.
        s.add_box(DVec2::ZERO, DVec2::X, DVec2::Y, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0);
        s.add_box(DVec2::ZERO, DVec2::X, DVec2::Y, 1.0, 2.0, 0.0, 1.0, 0.0, 1.0);
        // Drop the two internal faces at x = 1 (+u of the first box, -u of the second).
        s.faces.remove(10);
        s.faces.remove(5);
        let len: f64 = s.feature_edges().iter().flatten().map(|(a, b)| a.distance(*b)).sum();
        // Each visible edge is reported by both adjacent faces: 2 * (4*2 + 8*1).
        assert!((len - 2.0 * (8.0 + 8.0)).abs() < 1e-9, "{len}");
    }

    #[test]
    fn prism_volume() {
        let mut s = Solid::new(ElementId(1), Category::Floor, [0; 3]);
        let sq = [DVec2::new(0.0, 0.0), DVec2::new(0.0, 2.0), DVec2::new(2.0, 2.0), DVec2::new(2.0, 0.0)];
        s.add_prism(&sq, |_| -0.5, |_| 0.0);
        assert!((s.volume() - 2.0).abs() < 1e-9);
    }
}
