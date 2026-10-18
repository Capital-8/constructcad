//! Room boundaries: the region enclosed by room-bounding walls around a point.
//!
//! The wall footprints form a planar arrangement once their edges are split
//! where they meet. A ray from the room point finds the nearest edge, and the
//! face containing the point is traced by turning as far left as possible at
//! every vertex. Openings do not break the boundary (footprints are full walls).

use crate::polygon;
use glam::DVec2;
use std::collections::{BTreeMap, BTreeSet};

/// Vertices closer than this are merged (meters).
const MERGE: f64 = 1e-6;
const EPS: f64 = 1e-9;

type Key = (i64, i64);

fn key(p: DVec2) -> Key {
    ((p.x / MERGE).round() as i64, (p.y / MERGE).round() as i64)
}

/// Parameters along `a`-`b` where `c`-`d` touches or crosses it.
fn split_params(a: DVec2, b: DVec2, c: DVec2, d: DVec2, out: &mut Vec<f64>) {
    let r = b - a;
    let s = d - c;
    let len2 = r.length_squared();
    if len2 < EPS {
        return;
    }
    let denom = r.perp_dot(s);
    if denom.abs() < EPS * len2.sqrt() * s.length().max(EPS) {
        // Parallel: if collinear, the other segment's endpoints split this one.
        if (c - a).perp_dot(r).abs() < 1e-7 * len2.sqrt() {
            for p in [c, d] {
                let t = (p - a).dot(r) / len2;
                if t > 0.0 && t < 1.0 {
                    out.push(t);
                }
            }
        }
        return;
    }
    let t = (c - a).perp_dot(s) / denom;
    let u = (c - a).perp_dot(r) / denom;
    let tol = 1e-9;
    if (-tol..=1.0 + tol).contains(&t) && (-tol..=1.0 + tol).contains(&u) && t > tol && t < 1.0 - tol {
        out.push(t);
    }
}

/// Undirected edges of the arrangement of `polygons`' edges, keyed by vertex.
fn arrangement(polygons: &[Vec<DVec2>]) -> (BTreeMap<Key, DVec2>, BTreeSet<(Key, Key)>) {
    let segs: Vec<(DVec2, DVec2)> = polygons
        .iter()
        .flat_map(|p| (0..p.len()).map(move |i| (p[i], p[(i + 1) % p.len()])))
        .filter(|(a, b)| a.distance(*b) > MERGE)
        .collect();
    let mut points = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for (i, &(a, b)) in segs.iter().enumerate() {
        let mut ts = vec![0.0, 1.0];
        for (j, &(c, d)) in segs.iter().enumerate() {
            if i != j {
                split_params(a, b, c, d, &mut ts);
            }
        }
        ts.sort_by(f64::total_cmp);
        for w in ts.windows(2) {
            let (p, q) = (a.lerp(b, w[0]), a.lerp(b, w[1]));
            let (kp, kq) = (key(p), key(q));
            if kp == kq {
                continue;
            }
            points.entry(kp).or_insert(p);
            points.entry(kq).or_insert(q);
            edges.insert(if kp < kq { (kp, kq) } else { (kq, kp) });
        }
    }
    (points, edges)
}

/// Trace the face to the left of the directed edge `start`.
fn trace_face(
    points: &BTreeMap<Key, DVec2>,
    neighbors: &BTreeMap<Key, Vec<Key>>,
    start: (Key, Key),
) -> Option<Vec<DVec2>> {
    let mut face = Vec::new();
    let (mut a, mut b) = start;
    for _ in 0..100_000 {
        face.push(points[&a]);
        let (pa, pb) = (points[&a], points[&b]);
        let back = (pa - pb).y.atan2((pa - pb).x);
        // The neighbor first reached turning clockwise from the way back is the
        // sharpest left turn, which keeps the face on the left.
        let next = neighbors[&b]
            .iter()
            .map(|n| {
                let d = points[n] - pb;
                let mut turn = (back - d.y.atan2(d.x)).rem_euclid(std::f64::consts::TAU);
                if turn < 1e-12 {
                    turn = std::f64::consts::TAU;
                }
                (turn, *n)
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))?
            .1;
        (a, b) = (b, next);
        if (a, b) == start {
            return Some(face);
        }
    }
    None
}

/// Remove vertices where the boundary continues straight on.
fn simplify(poly: Vec<DVec2>) -> Vec<DVec2> {
    let n = poly.len();
    let keep: Vec<DVec2> = (0..n)
        .filter(|&i| {
            let (prev, p, next) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
            (p - prev).perp_dot(next - p).abs() > 1e-9
        })
        .map(|i| poly[i])
        .collect();
    if keep.len() >= 3 { keep } else { poly }
}

/// Boundary (counter-clockwise) of the region enclosed by `walls` (wall
/// footprints) that contains `point`, or `None` if the point is not enclosed.
pub fn room_boundary(walls: &[Vec<DVec2>], point: DVec2) -> Option<Vec<DVec2>> {
    if walls.iter().any(|w| polygon::contains(w, point)) {
        return None;
    }
    let (points, edges) = arrangement(walls);
    let mut neighbors: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
    for &(a, b) in &edges {
        neighbors.entry(a).or_default().push(b);
        neighbors.entry(b).or_default().push(a);
    }
    // Edges hit by a ray from the point towards +X (nudged off vertex heights), nearest first.
    let y = point.y + 1.234_567e-7;
    let mut hits: Vec<(f64, (Key, Key))> = edges
        .iter()
        .filter_map(|&(ka, kb)| {
            let (a, b) = (points[&ka], points[&kb]);
            if (a.y > y) == (b.y > y) {
                return None;
            }
            let x = a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x);
            (x > point.x).then_some((x - point.x, (ka, kb)))
        })
        .collect();
    hits.sort_by(|x, y| x.0.total_cmp(&y.0));
    for (_, (ka, kb)) in hits {
        let (a, b) = (points[&ka], points[&kb]);
        // Orient the edge so the point lies on its left.
        let start = if (b - a).perp_dot(point - a) > 0.0 { (ka, kb) } else { (kb, ka) };
        let Some(face) = trace_face(&points, &neighbors, start) else { continue };
        if polygon::signed_area(&face) > 0.0 && polygon::contains(&face, point) {
            return Some(simplify(face));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
        vec![DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)]
    }

    /// Four walls 0.2 thick around a 4 x 3 interior, overlapping at the corners,
    /// plus a partition from the bottom wall to the top wall at x = 2.
    fn walls() -> Vec<Vec<DVec2>> {
        vec![
            rect(-0.2, -0.2, 4.2, 0.0),
            rect(-0.2, 3.0, 4.2, 3.2),
            rect(-0.2, -0.2, 0.0, 3.2),
            rect(4.0, -0.2, 4.2, 3.2),
            rect(1.95, 0.0, 2.05, 3.0),
        ]
    }

    #[test]
    fn finds_the_enclosed_region() {
        let b = room_boundary(&walls(), DVec2::new(1.0, 1.5)).unwrap();
        assert!((polygon::area(&b) - 1.95 * 3.0).abs() < 1e-9, "{b:?}");
        assert_eq!(b.len(), 4);
        let right = room_boundary(&walls(), DVec2::new(3.0, 1.0)).unwrap();
        assert!((polygon::area(&right) - 1.95 * 3.0).abs() < 1e-9);
    }

    #[test]
    fn open_or_inside_wall_is_not_a_room() {
        let mut open = walls();
        open.remove(3);
        open.remove(3);
        assert!(room_boundary(&open, DVec2::new(1.0, 1.5)).is_none());
        assert!(room_boundary(&walls(), DVec2::new(2.0, 1.5)).is_none());
        assert!(room_boundary(&walls(), DVec2::new(9.0, 1.5)).is_none());
    }

    #[test]
    fn free_standing_wall_inside_does_not_block() {
        let mut w = walls();
        w.remove(4);
        w.push(rect(2.5, 1.0, 2.7, 2.0));
        let b = room_boundary(&w, DVec2::new(1.0, 1.5)).unwrap();
        assert!((polygon::area(&b) - 4.0 * 3.0).abs() < 1e-9);
    }
}
