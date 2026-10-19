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
    let kept: Vec<(DVec3, DVec3, Vec<usize>)> = edges
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
        .collect();
    let mut out = hide_t_junctions(kept, &normals);
    out.sort_by(|x, y| key(x.0).cmp(&key(y.0)).then(key(x.1).cmp(&key(y.1))));
    out
}

/// Line of an edge: canonical direction and the point closest to the origin, quantized.
fn line_key(a: DVec3, b: DVec3) -> Option<([i64; 3], [i64; 3], DVec3)> {
    let mut d = (b - a).try_normalize()?;
    let first = [d.x, d.y, d.z].into_iter().find(|c| c.abs() > 1e-9).unwrap_or(1.0);
    if first < 0.0 {
        d = -d;
    }
    let q = |v: DVec3, s: f64| [(v.x * s).round() as i64, (v.y * s).round() as i64, (v.z * s).round() as i64];
    Some((q(d, 1e4), q(a - d * a.dot(d), 1e4), d))
}

/// Faces cut into fragments (booleans) meet in T-junctions, whose edges are
/// not shared end to end: the parts of a boundary edge that run along an
/// edge of a coplanar neighbor are interior and left out.
fn hide_t_junctions(edges: Vec<(DVec3, DVec3, Vec<usize>)>, normals: &[DVec3]) -> Vec<(DVec3, DVec3)> {
    let mut lines: HashMap<([i64; 3], [i64; 3]), Vec<usize>> = HashMap::new();
    for (i, (a, b, adjacent)) in edges.iter().enumerate() {
        if adjacent.len() == 1
            && let Some((dk, pk, _)) = line_key(*a, *b)
        {
            lines.entry((dk, pk)).or_default().push(i);
        }
    }
    let mut out = Vec::with_capacity(edges.len());
    for (i, (a, b, adjacent)) in edges.iter().enumerate() {
        let (Some(&face), Some((dk, pk, d))) = (adjacent.first(), line_key(*a, *b)) else {
            out.push((*a, *b));
            continue;
        };
        let others = lines.get(&(dk, pk)).map(Vec::as_slice).unwrap_or_default();
        let (ta, tb) = (a.dot(d), b.dot(d));
        let (lo, hi) = (ta.min(tb), ta.max(tb));
        let mut covered: Vec<(f64, f64)> = others
            .iter()
            .filter(|j| **j != i && adjacent.len() == 1)
            .filter_map(|j| {
                let (c, e, adj) = &edges[*j];
                let other = *adj.first()?;
                (other != face && normals[other].dot(normals[face]) > 0.9999)
                    .then(|| (c.dot(d).min(e.dot(d)).max(lo), c.dot(d).max(e.dot(d)).min(hi)))
            })
            .filter(|(x, y)| y - x > 1e-9)
            .collect();
        if covered.is_empty() {
            out.push((*a, *b));
            continue;
        }
        covered.sort_by(|x, y| x.0.total_cmp(&y.0));
        let at = |t: f64| *a + d * (t - ta);
        let mut t = lo;
        for (x, y) in covered {
            if x > t + 1e-9 {
                out.push((at(t), at(x)));
            }
            t = t.max(y);
        }
        if t < hi - 1e-9 {
            out.push((at(t), at(hi)));
        }
    }
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

    #[test]
    fn t_junctions_of_fragments_are_hidden() {
        // A unit square top split into a left half and two right quarters:
        // the middle line meets the quarters' edges in a T.
        let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
        let faces = vec![
            vec![p(0.0, 0.0), p(0.5, 0.0), p(0.5, 1.0), p(0.0, 1.0)],
            vec![p(0.5, 0.0), p(1.0, 0.0), p(1.0, 0.5), p(0.5, 0.5)],
            vec![p(0.5, 0.5), p(1.0, 0.5), p(1.0, 1.0), p(0.5, 1.0)],
        ];
        let total: f64 = plan_edges(&faces).iter().map(|(a, b)| a.distance(*b)).sum();
        assert!((total - 4.0).abs() < 1e-9, "{total}");
    }
}
