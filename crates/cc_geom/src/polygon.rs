//! 2D polygon helpers.

use glam::DVec2;

/// Signed area (positive for counter-clockwise).
pub fn signed_area(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].perp_dot(poly[(i + 1) % n])).sum::<f64>() * 0.5
}

pub fn area(poly: &[DVec2]) -> f64 {
    signed_area(poly).abs()
}

pub fn perimeter(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].distance(poly[(i + 1) % n])).sum()
}

/// Copy of `poly` wound counter-clockwise.
pub fn ccw(poly: &[DVec2]) -> Vec<DVec2> {
    let mut p = poly.to_vec();
    if signed_area(&p) < 0.0 {
        p.reverse();
    }
    p
}

/// Offset a simple polygon outward by `d` using mitered corners.
pub fn offset(poly: &[DVec2], d: f64) -> Vec<DVec2> {
    let p = ccw(poly);
    let n = p.len();
    if n < 3 || d == 0.0 {
        return p;
    }
    (0..n)
        .map(|i| {
            let prev = p[(i + n - 1) % n];
            let cur = p[i];
            let next = p[(i + 1) % n];
            // Outward normals of a CCW polygon point to the right of each edge.
            let n1 = -(cur - prev).normalize_or_zero().perp();
            let n2 = -(next - cur).normalize_or_zero().perp();
            let bisector = (n1 + n2).normalize_or_zero();
            let cos = bisector.dot(n1).max(0.2);
            cur + bisector * (d / cos)
        })
        .collect()
}

pub fn contains(poly: &[DVec2], pt: DVec2) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > pt.y) != (b.y > pt.y) && pt.x < (b.x - a.x) * (pt.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Triangulate a simple polygon; returns index triples.
pub fn triangulate(poly: &[DVec2]) -> Vec<[usize; 3]> {
    match poly.len() {
        0..=2 => vec![],
        3 => vec![[0, 1, 2]],
        _ => {
            let flat: Vec<f64> = poly.iter().flat_map(|p| [p.x, p.y]).collect();
            let indices = earcutr::earcut(&flat, &[], 2).unwrap_or_default();
            indices.as_chunks::<3>().0.iter().map(|t| [t[0], t[1], t[2]]).collect()
        }
    }
}

/// Parts of the infinite line `origin + t*dir` inside `poly`, as `(t0, t1)` pairs.
pub fn clip_line(poly: &[DVec2], origin: DVec2, dir: DVec2) -> Vec<(f64, f64)> {
    let n = poly.len();
    let mut ts = Vec::new();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let e = b - a;
        let denom = dir.perp_dot(e);
        if denom.abs() < 1e-12 {
            continue;
        }
        let w = a - origin;
        let s = -dir.perp_dot(w) / denom; // parameter on the edge
        // Half-open interval avoids counting shared vertices twice.
        if (0.0..1.0).contains(&s) {
            ts.push(w.perp_dot(e) / denom);
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect()
}

/// Closest point on segment `a`-`b` to `p` and the segment parameter.
pub fn closest_on_segment(a: DVec2, b: DVec2, p: DVec2) -> (DVec2, f64) {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t, t)
}

/// Intersection of lines `p1 + t*d1` and `p2 + s*d2`, returning `t`.
pub fn line_intersect(p1: DVec2, d1: DVec2, p2: DVec2, d2: DVec2) -> Option<f64> {
    let denom = d1.perp_dot(d2);
    if denom.abs() < 1e-9 {
        return None;
    }
    Some((p2 - p1).perp_dot(d2) / denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<DVec2> {
        vec![DVec2::new(0.0, 0.0), DVec2::new(4.0, 0.0), DVec2::new(4.0, 4.0), DVec2::new(0.0, 4.0)]
    }

    #[test]
    fn area_and_offset() {
        assert_eq!(area(&square()), 16.0);
        let o = offset(&square(), 1.0);
        assert!((area(&o) - 36.0).abs() < 1e-9);
    }

    #[test]
    fn clip_line_through_square() {
        let parts = clip_line(&square(), DVec2::new(-1.0, 2.0), DVec2::X);
        assert_eq!(parts.len(), 1);
        assert!((parts[0].0 - 1.0).abs() < 1e-9 && (parts[0].1 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn triangulates_l_shape() {
        let l = [
            DVec2::new(0.0, 0.0),
            DVec2::new(2.0, 0.0),
            DVec2::new(2.0, 1.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(1.0, 2.0),
            DVec2::new(0.0, 2.0),
        ];
        assert_eq!(triangulate(&l).len(), 4);
    }
}
