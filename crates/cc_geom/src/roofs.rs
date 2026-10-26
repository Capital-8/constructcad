//! Pitched roofs (gable and hip) over any footprint, courtyards included:
//! every sloped eave raises a plane and every gable edge a vertical plane;
//! the weighted straight skeleton ([`crate::skeleton`]) gives the part of
//! each plane over the footprint, with its hips, valleys and ridges.
//!
//! The older construction, kept as a fallback when the skeleton fails,
//! takes the lowest of the planes rising from each sloped eave: right for
//! convex footprints only.

use crate::polygon;
use crate::{Face, Solid};
use glam::{DVec2, DVec3};

/// One plane of a pitched roof: rises with `slope` from the eave through
/// `a` (counter-clockwise footprint, so the inside is on the left).
#[derive(Debug, Clone, Copy)]
pub struct Eave {
    pub a: DVec2,
    pub normal: DVec2,
    pub slope: f64,
}

impl Eave {
    /// Height above the eave of the plane at `p`.
    pub fn height(&self, p: DVec2) -> f64 {
        self.slope * (p - self.a).dot(self.normal)
    }
}

/// Keep the part of `poly` where `p · w <= c`.
fn clip_half_plane(poly: &[DVec2], w: DVec2, c: f64) -> Vec<DVec2> {
    let mut out = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (dp, dq) = (p.dot(w) - c, q.dot(w) - c);
        if dp <= 0.0 {
            out.push(p);
        }
        if (dp <= 0.0) != (dq <= 0.0) {
            out.push(p + (q - p) * (dp / (dp - dq)));
        }
    }
    out
}

/// The slope of each edge of a counter-clockwise loop: its own slope when
/// `own` sets one (0 = a gable end), else `slope` on a hip roof (no
/// `ridge`) or on edges within 45° of the ridge direction, 0 on the others.
pub fn edge_slopes(pts: &[DVec2], slope: f64, ridge: Option<DVec2>, own: &[Option<f64>]) -> Vec<f64> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let s = own.get(i).copied().flatten().unwrap_or_else(|| {
                let dir = (pts[(i + 1) % n] - pts[i]).normalize_or_zero();
                let sloped = ridge.is_none_or(|r| dir.dot(r).abs() >= std::f64::consts::FRAC_1_SQRT_2);
                if sloped { slope } else { 0.0 }
            });
            if s.is_finite() { s.max(0.0) } else { 0.0 }
        })
        .collect()
}

/// A roof's own edge slopes in the order of its counter-clockwise outline
/// (`polygon::ccw` reverses a clockwise footprint, and with it the edges).
pub fn ccw_edge_slopes(r: &cc_core::Roof) -> Vec<Option<f64>> {
    let n = r.footprint.len();
    if n == 0 || polygon::signed_area(&r.footprint) >= 0.0 {
        return (0..n).map(|i| r.edge_slope(i)).collect();
    }
    // Reversed, edge `j` runs from old point `n-1-j` to `n-2-j`: old edge `n-2-j`.
    (0..n).map(|j| r.edge_slope((2 * n - 2 - j) % n)).collect()
}

/// The sloped eaves of a counter-clockwise footprint, given the slope of
/// each edge (see [`edge_slopes`]); edges of slope 0 are gables.
pub fn sloped_eaves(footprint: &[DVec2], slopes: &[f64]) -> Vec<Eave> {
    let n = footprint.len();
    (0..n)
        .filter_map(|i| {
            let (a, b) = (footprint[i], footprint[(i + 1) % n]);
            let dir = (b - a).try_normalize()?;
            let slope = slopes.get(i).copied().unwrap_or(0.0);
            (slope > 0.0).then(|| Eave { a, normal: dir.perp(), slope })
        })
        .collect()
}

/// Part of the footprint owned by each eave's plane (empty parts are skipped).
pub fn roof_regions(footprint: &[DVec2], eaves: &[Eave]) -> Vec<(usize, Vec<DVec2>)> {
    eaves
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let mut region = footprint.to_vec();
            for (j, other) in eaves.iter().enumerate() {
                if i != j {
                    // height_i <= height_j  <=>  p·(s_i n_i - s_j n_j) <= s_i a_i·n_i - s_j a_j·n_j
                    let (ni, nj) = (e.normal * e.slope, other.normal * other.slope);
                    region = clip_half_plane(&region, ni - nj, e.a.dot(ni) - other.a.dot(nj));
                }
            }
            (polygon::area(&region) > 1e-9).then_some((i, region))
        })
        .collect()
}

/// Ridge and hip lines in plan: region edges that are not on the footprint boundary.
pub fn ridge_lines(footprint: &[DVec2], regions: &[(usize, Vec<DVec2>)]) -> Vec<(DVec2, DVec2)> {
    let on_boundary = |p: DVec2, q: DVec2| on_footprint_edge(footprint, (p + q) * 0.5);
    let mut lines: Vec<(DVec2, DVec2)> = Vec::new();
    for (_, region) in regions {
        let n = region.len();
        for i in 0..n {
            let (p, q) = (region[i], region[(i + 1) % n]);
            if p.distance(q) < 1e-6 || on_boundary(p, q) {
                continue;
            }
            // Shared edges appear in both neighbouring regions; keep one copy.
            let duplicate = lines.iter().any(|&(a, b)| {
                (a.distance(p) < 1e-6 && b.distance(q) < 1e-6) || (a.distance(q) < 1e-6 && b.distance(p) < 1e-6)
            });
            if !duplicate {
                lines.push((p, q));
            }
        }
    }
    lines
}

fn on_footprint_edge(footprint: &[DVec2], m: DVec2) -> bool {
    let n = footprint.len();
    (0..n).any(|i| polygon::closest_on_segment(footprint[i], footprint[(i + 1) % n], m).0.distance(m) < 1e-7)
}

fn plane_normal(pts: &[DVec3]) -> DVec3 {
    let n = pts.len();
    let sum: DVec3 = (0..n).map(|i| pts[i].cross(pts[(i + 1) % n])).sum();
    sum.normalize_or_zero()
}

/// Add the closed solid of a pitched roof of vertical thickness `thickness`
/// whose underside is at `base` along the eaves.
pub fn add_pitched_roof(solid: &mut Solid, footprint: &[DVec2], eaves: &[Eave], base: f64, thickness: f64) {
    let regions = roof_regions(footprint, eaves);
    for (i, region) in &regions {
        let Some(eave) = eaves.get(*i) else { continue };
        let bottom = |p: DVec2| DVec3::new(p.x, p.y, base + eave.height(p));
        let top: Vec<DVec3> = region.iter().map(|&p| bottom(p) + DVec3::Z * thickness).collect();
        let mut under: Vec<DVec3> = region.iter().map(|&p| bottom(p)).collect();
        under.reverse();
        let (tn, un) = (plane_normal(&top), plane_normal(&under));
        solid.faces.push(Face::new(top, tn));
        solid.faces.push(Face::new(under, un));
        // Vertical faces where the region meets the footprint outline (eaves and gable ends).
        let n = region.len();
        for k in 0..n {
            let (p, q) = (region[k], region[(k + 1) % n]);
            if p.distance(q) < 1e-9 || !on_footprint_edge(footprint, (p + q) * 0.5) {
                continue;
            }
            let out = -(q - p).normalize_or_zero().perp();
            let quad = vec![bottom(p), bottom(q), bottom(q) + DVec3::Z * thickness, bottom(p) + DVec3::Z * thickness];
            solid.faces.push(Face::new(quad, DVec3::new(out.x, out.y, 0.0)));
        }
    }
}

/// Plan area covered by a roof: its outline grown by the overhang, less
/// its openings shrunk by it.
pub fn plan_area(r: &cc_core::Roof) -> f64 {
    let holes: f64 = r
        .holes
        .iter()
        .map(|h| polygon::offset(h, -r.overhang))
        .filter(|h| h.len() >= 3 && polygon::signed_area(h) > 0.0)
        .map(|h| polygon::area(&h))
        .sum();
    (polygon::area(&polygon::offset(&r.footprint, r.overhang)) - holes).max(0.0)
}

/// Edge weights of a footprint's loops for the skeleton: sloped eaves move
/// in by `1 / slope` per unit of height, gables (see [`edge_slopes`]) stay.
/// `own` holds the outline's own edge slopes; openings follow the shape.
pub fn eave_weights(loops: &[Vec<DVec2>], slope: f64, ridge: Option<DVec2>, own: &[Option<f64>]) -> Vec<Vec<f64>> {
    loops
        .iter()
        .enumerate()
        .map(|(l, pts)| {
            let own = if l == 0 { own } else { &[] };
            edge_slopes(pts, slope, ridge, own).into_iter().map(|s| if s > 0.0 { 1.0 / s } else { 0.0 }).collect()
        })
        .collect()
}

/// Add the closed solid of a roof from its skeleton: each plane's part
/// lifted from `base` (its underside) with `thickness` measured vertically,
/// flat tops where the planes never close, and vertical faces along the
/// eaves and the gables.
pub fn add_skeleton_roof(solid: &mut Solid, sk: &crate::skeleton::Skeleton, base: f64, thickness: f64) {
    let up = DVec3::Z * thickness;
    let mut surface = |pts: &[DVec2], height: &dyn Fn(DVec2) -> f64| {
        let under: Vec<DVec3> = pts.iter().map(|&p| DVec3::new(p.x, p.y, base + height(p))).collect();
        let top: Vec<DVec3> = under.iter().map(|&p| p + up).collect();
        let mut under = under;
        under.reverse();
        let (tn, un) = (plane_normal(&top), plane_normal(&under));
        if tn != DVec3::ZERO && un != DVec3::ZERO {
            solid.faces.push(Face::new(top, tn));
            solid.faces.push(Face::new(under, un));
        }
    };
    for (plane, pts) in &sk.faces {
        surface(pts, &|p| sk.face_height(*plane, p));
    }
    for (height, pts) in &sk.flats {
        surface(pts, &|_| *height);
    }
    let mut strip = |a: DVec3, b: DVec3, out: DVec2| {
        let mut quad = vec![a, b, b + up, a + up];
        let out = DVec3::new(out.x, out.y, 0.0);
        if plane_normal(&quad).dot(out) < 0.0 {
            quad.reverse();
        }
        solid.faces.push(Face::new(quad, out));
    };
    for plane in sk.planes.iter().filter(|p| p.weight > 0.0) {
        let a = plane.origin;
        let b = a + plane.dir * plane.length;
        strip(DVec3::new(a.x, a.y, base), DVec3::new(b.x, b.y, base), -plane.dir.perp());
    }
    for &(p, t0, q, t1, out) in &sk.gables {
        strip(DVec3::new(p.x, p.y, base + t0), DVec3::new(q.x, q.y, base + t1), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::{Category, ElementId};

    fn rect(w: f64, d: f64) -> Vec<DVec2> {
        vec![DVec2::ZERO, DVec2::new(w, 0.0), DVec2::new(w, d), DVec2::new(0.0, d)]
    }

    #[test]
    fn hip_roof_on_a_rectangle_has_a_ridge_and_four_hips() {
        let fp = rect(10.0, 6.0);
        let eaves = sloped_eaves(&fp, &edge_slopes(&fp, 1.0, None, &[]));
        let regions = roof_regions(&fp, &eaves);
        assert_eq!(regions.len(), 4);
        let total: f64 = regions.iter().map(|(_, r)| polygon::area(r)).sum();
        assert!((total - 60.0).abs() < 1e-9);
        // Ridge from (3,3) to (7,3) plus four hips.
        let lines = ridge_lines(&fp, &regions);
        assert_eq!(lines.len(), 5, "{lines:?}");
        assert!(
            lines.iter().any(|&(a, b)| (a.y - 3.0).abs() < 1e-9 && (b.y - 3.0).abs() < 1e-9 && (a.x - b.x).abs() > 3.9)
        );
    }

    #[test]
    fn gable_roof_volume_and_ridge_height() {
        let fp = rect(10.0, 6.0);
        let (slope, thickness) = (0.5, 0.2);
        let eaves = sloped_eaves(&fp, &edge_slopes(&fp, slope, Some(DVec2::X), &[]));
        assert_eq!(eaves.len(), 2);
        let mut s = Solid::new(ElementId(1), Category::Roof, [0; 3]);
        add_pitched_roof(&mut s, &fp, &eaves, 3.0, thickness);
        // Volume of a constant vertical thickness: plan area x thickness.
        assert!((s.volume() - 60.0 * thickness).abs() < 1e-9, "{}", s.volume());
        let (_, hi) = s.bounds().unwrap();
        assert!((hi.z - (3.0 + slope * 3.0 + thickness)).abs() < 1e-9);
    }

    fn skeleton_solid(loops: &[Vec<DVec2>], slope: f64, ridge: Option<DVec2>, thickness: f64) -> Solid {
        let weights = eave_weights(loops, slope, ridge, &[]);
        let sk = crate::skeleton::skeleton(loops, &weights).unwrap();
        let mut s = Solid::new(ElementId(1), Category::Roof, [0; 3]);
        add_skeleton_roof(&mut s, &sk, 3.0, thickness);
        s
    }

    #[test]
    fn skeleton_roofs_are_closed_slabs() {
        let l = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 4.0),
            DVec2::new(4.0, 4.0),
            DVec2::new(4.0, 10.0),
            DVec2::new(0.0, 10.0),
        ];
        let courtyard = [
            rect(12.0, 12.0),
            vec![DVec2::new(4.0, 4.0), DVec2::new(4.0, 8.0), DVec2::new(8.0, 8.0), DVec2::new(8.0, 4.0)],
        ];
        for (loops, area) in [(vec![rect(10.0, 6.0)], 60.0), (vec![l], 64.0), (courtyard.to_vec(), 128.0)] {
            for ridge in [None, Some(DVec2::X)] {
                let s = skeleton_solid(&loops, 0.5, ridge, 0.2);
                // A constant vertical thickness: plan area x thickness.
                assert!((s.volume() - area * 0.2).abs() < 1e-6, "{ridge:?}: {} vs {}", s.volume(), area * 0.2);
            }
        }
        // A square hip roof peaks at half its width times the slope.
        let s = skeleton_solid(&[rect(8.0, 8.0)], 0.6, None, 0.1);
        let (_, hi) = s.bounds().unwrap();
        assert!((hi.z - (3.0 + 4.0 * 0.6 + 0.1)).abs() < 1e-9);
        // Equal slopes: the sloped faces' true areas sum to the footprint over cos(slope).
        let sk =
            crate::skeleton::skeleton(&[rect(10.0, 6.0)], &eave_weights(&[rect(10.0, 6.0)], 0.75, None, &[])).unwrap();
        let true_area: f64 = sk.faces.iter().map(|(_, f)| polygon::area(f) * (1.0f64 + 0.75 * 0.75).sqrt()).sum();
        assert!((true_area - 60.0 * 1.25).abs() < 1e-9);
    }

    #[test]
    fn hip_roof_solid_is_closed() {
        let fp = rect(8.0, 8.0);
        let eaves = sloped_eaves(&fp, &edge_slopes(&fp, 1.0, None, &[]));
        let mut s = Solid::new(ElementId(1), Category::Roof, [0; 3]);
        add_pitched_roof(&mut s, &fp, &eaves, 0.0, 0.1);
        assert!((s.volume() - 6.4).abs() < 1e-9);
    }

    #[test]
    fn edge_slopes_make_gables_and_steeper_eaves() {
        let fp = rect(10.0, 6.0);
        // A hip roof with its east edge (edge 1) turned into a gable end.
        let own = [None, Some(0.0)];
        let weights = eave_weights(std::slice::from_ref(&fp), 0.5, None, &own);
        assert_eq!(weights[0], vec![2.0, 0.0, 2.0, 2.0]);
        let sk = crate::skeleton::skeleton(std::slice::from_ref(&fp), &weights).unwrap();
        assert!(sk.faces.iter().all(|(p, _)| *p != 1), "a gable edge has no sloped face");
        let mut s = Solid::new(ElementId(1), Category::Roof, [0; 3]);
        add_skeleton_roof(&mut s, &sk, 0.0, 0.2);
        assert!((s.volume() - 60.0 * 0.2).abs() < 1e-6, "{}", s.volume());
        // A gable roof whose south eave is steeper: the planes meet where 1.0 y = 0.5 (6 - y).
        let slopes = edge_slopes(&fp, 0.5, Some(DVec2::X), &[Some(1.0)]);
        assert_eq!(slopes, vec![1.0, 0.0, 0.5, 0.0]);
        let mut s = Solid::new(ElementId(1), Category::Roof, [0; 3]);
        add_pitched_roof(&mut s, &fp, &sloped_eaves(&fp, &slopes), 0.0, 0.1);
        let (_, hi) = s.bounds().unwrap();
        assert!((hi.z - 2.1).abs() < 1e-9, "{}", hi.z);
        assert!((s.volume() - 6.0).abs() < 1e-9, "{}", s.volume());
    }

    #[test]
    fn edge_slopes_follow_a_clockwise_footprint() {
        let mut fp = rect(10.0, 6.0);
        fp.reverse();
        let mut r = cc_core::Roof::new(fp, 0.0, 0.0, cc_core::RoofShape::Hip { slope: 0.5 });
        r.edge_slopes = vec![None, Some(0.9)];
        let ccw = polygon::ccw(&r.footprint);
        let own = ccw_edge_slopes(&r);
        assert_eq!(own.iter().filter(|s| s.is_some()).count(), 1);
        let i = own.iter().position(Option::is_some).unwrap();
        // The same segment as the footprint's edge 1, walked the other way.
        let (a, b) = (ccw[i], ccw[(i + 1) % 4]);
        assert!(a.distance(r.footprint[2]) < 1e-9 && b.distance(r.footprint[1]) < 1e-9, "{a} {b}");
    }
}
