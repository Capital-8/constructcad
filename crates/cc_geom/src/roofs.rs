//! Pitched roofs (gable and hip): the roof surface is the lowest of the planes
//! rising from each sloped eave, so each plane owns the part of the footprint
//! where it is below all others. For a convex footprint with every eave
//! sloped this is the hip roof (its straight skeleton); with only the eaves
//! parallel to the ridge sloped, the other edges become gable ends.

use crate::polygon;
use crate::{Face, Solid};
use glam::{DVec2, DVec3};

/// One plane of a pitched roof: rises with `slope` from the eave `a`-`b`
/// (counter-clockwise footprint, so the inside is on the left).
#[derive(Debug, Clone, Copy)]
pub struct Eave {
    pub a: DVec2,
    pub normal: DVec2,
}

impl Eave {
    /// Height above the eave of the plane at `p`.
    pub fn height(&self, p: DVec2, slope: f64) -> f64 {
        slope * (p - self.a).dot(self.normal)
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

/// The sloped eaves of a counter-clockwise footprint: all edges for a hip
/// roof, or for a gable roof the edges within 45° of the ridge direction.
pub fn sloped_eaves(footprint: &[DVec2], ridge: Option<DVec2>) -> Vec<Eave> {
    let n = footprint.len();
    (0..n)
        .filter_map(|i| {
            let (a, b) = (footprint[i], footprint[(i + 1) % n]);
            let dir = (b - a).try_normalize()?;
            let sloped = ridge.is_none_or(|r| dir.dot(r).abs() >= std::f64::consts::FRAC_1_SQRT_2);
            sloped.then(|| Eave { a, normal: dir.perp() })
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
                    // height_i <= height_j  <=>  p·(n_i - n_j) <= a_i·n_i - a_j·n_j
                    let w = e.normal - other.normal;
                    region = clip_half_plane(&region, w, e.a.dot(e.normal) - other.a.dot(other.normal));
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
pub fn add_pitched_roof(solid: &mut Solid, footprint: &[DVec2], eaves: &[Eave], slope: f64, base: f64, thickness: f64) {
    let regions = roof_regions(footprint, eaves);
    for (i, region) in &regions {
        let Some(eave) = eaves.get(*i) else { continue };
        let bottom = |p: DVec2| DVec3::new(p.x, p.y, base + eave.height(p, slope));
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
        let eaves = sloped_eaves(&fp, None);
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
        let eaves = sloped_eaves(&fp, Some(DVec2::X));
        assert_eq!(eaves.len(), 2);
        let (slope, thickness) = (0.5, 0.2);
        let mut s = Solid::new(ElementId(1), Category::Roof, [0; 3]);
        add_pitched_roof(&mut s, &fp, &eaves, slope, 3.0, thickness);
        // Volume of a constant vertical thickness: plan area x thickness.
        assert!((s.volume() - 60.0 * thickness).abs() < 1e-9, "{}", s.volume());
        let (_, hi) = s.bounds().unwrap();
        assert!((hi.z - (3.0 + slope * 3.0 + thickness)).abs() < 1e-9);
    }

    #[test]
    fn hip_roof_solid_is_closed() {
        let fp = rect(8.0, 8.0);
        let eaves = sloped_eaves(&fp, None);
        let mut s = Solid::new(ElementId(1), Category::Roof, [0; 3]);
        add_pitched_roof(&mut s, &fp, &eaves, 1.0, 0.0, 0.1);
        assert!((s.volume() - 6.4).abs() < 1e-9);
    }
}
