//! Plan drawings of polyhedral meshes (component pieces imported from IFC):
//! the edges seen from above and the section at the cut plane.

use crate::cut::face_plane_segments;
use crate::solid::Face;
use glam::{DVec2, DVec3};
use std::collections::HashMap;

/// Creases between up-facing faces sharper than this are drawn (cos 20°).
const CREASE_COS: f64 = 0.94;
/// Faces with a normal Z component above this face upwards.
const UP: f64 = 1e-3;

/// What a mesh shows in a plan cut at `cut_z`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshPlan {
    /// Section of the mesh by the cut plane.
    pub cut: Vec<(DVec2, DVec2)>,
    /// Visible edges at or below the cut plane.
    pub below: Vec<(DVec2, DVec2)>,
    /// Edges above the cut plane.
    pub above: Vec<(DVec2, DVec2)>,
}

fn newell(verts: &[DVec3]) -> DVec3 {
    let n = verts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (verts[i], verts[(i + 1) % n]);
            DVec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y))
        })
        .sum::<DVec3>()
        .normalize_or_zero()
}

type EdgeKey = ([i64; 3], [i64; 3]);

/// Vertex key for matching edges of faces that do not share indices.
fn key(p: DVec3) -> [i64; 3] {
    let q = |v: f64| (v * 1e5).round() as i64;
    [q(p.x), q(p.y), q(p.z)]
}

/// Edges of `faces` that read in a view from above: outlines of upward
/// faces where they meet faces turned away, open boundaries and sharp
/// creases (shared edges of smooth or flat regions are left out).
pub fn plan_edges(faces: &[Vec<DVec3>]) -> Vec<(DVec3, DVec3)> {
    let normals: Vec<DVec3> = faces.iter().map(|f| newell(f)).collect();
    let mut edges: HashMap<EdgeKey, (DVec3, DVec3, Vec<usize>)> = HashMap::new();
    for (fi, f) in faces.iter().enumerate() {
        let n = f.len();
        for i in 0..n {
            let (a, b) = (f[i], f[(i + 1) % n]);
            let (ka, kb) = (key(a), key(b));
            if ka == kb {
                continue;
            }
            let k = if ka < kb { (ka, kb) } else { (kb, ka) };
            edges.entry(k).or_insert_with(|| (a, b, Vec::new())).2.push(fi);
        }
    }
    let mut out: Vec<(DVec3, DVec3)> = edges
        .into_values()
        .filter(|(a, b, adjacent)| {
            if (a.truncate() - b.truncate()).length_squared() < 1e-12 {
                return false;
            }
            let up: Vec<DVec3> = adjacent.iter().map(|i| normals[*i]).filter(|n| n.z > UP).collect();
            match (adjacent.len(), up.len()) {
                (_, 0) => false,
                (1, _) => true,
                (_, k) if k < adjacent.len() => true,
                _ => up.iter().zip(up.iter().skip(1)).any(|(m, n)| m.dot(*n) < CREASE_COS),
            }
        })
        .map(|(a, b, _)| (a, b))
        .collect();
    out.sort_by(|x, y| key(x.0).cmp(&key(y.0)).then(key(x.1).cmp(&key(y.1))));
    out
}

/// Plan of a mesh cut at `cut_z`.
pub fn mesh_plan(faces: &[Vec<DVec3>], cut_z: f64) -> MeshPlan {
    let mut plan = MeshPlan::default();
    let flat = |(a, b): (DVec3, DVec3)| (a.truncate(), b.truncate());
    for (a, b) in plan_edges(faces) {
        let (lo, hi) = if a.z <= b.z { (a, b) } else { (b, a) };
        if hi.z <= cut_z {
            plan.below.push(flat((a, b)));
        } else if lo.z >= cut_z {
            plan.above.push(flat((a, b)));
        } else {
            let mid = lo.lerp(hi, (cut_z - lo.z) / (hi.z - lo.z));
            plan.below.push(flat((lo, mid)));
            plan.above.push(flat((mid, hi)));
        }
    }
    let point = DVec3::new(0.0, 0.0, cut_z);
    for f in faces.iter().filter(|f| f.len() >= 3) {
        let face = Face::new(f.clone(), newell(f));
        plan.cut.extend(face_plane_segments(&face, point, DVec3::Z).into_iter().map(flat));
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Solid;
    use cc_core::{Category, ElementId};

    fn cube_faces() -> Vec<Vec<DVec3>> {
        let mut s = Solid::new(ElementId(1), Category::Component, [0; 3]);
        s.add_box(DVec2::ZERO, DVec2::X, DVec2::Y, 0.0, 2.0, 0.0, 1.0, 0.0, 3.0);
        s.faces.into_iter().map(|f| f.verts).collect()
    }

    #[test]
    fn box_shows_its_top_outline_and_cut() {
        let faces = cube_faces();
        let plan = mesh_plan(&faces, 1.0);
        assert_eq!(plan.cut.len(), 4);
        // Top outline (4 edges) above the cut; vertical edges project to points.
        assert_eq!(plan.above.len(), 4);
        assert!(plan.below.is_empty());
        let below = mesh_plan(&faces, 5.0);
        assert_eq!(below.below.len(), 4);
        assert!(below.cut.is_empty());
    }

    #[test]
    fn flat_split_faces_hide_their_shared_edge() {
        let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
        let faces = vec![vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)], vec![p(0.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]];
        assert_eq!(plan_edges(&faces).len(), 4);
    }
}
