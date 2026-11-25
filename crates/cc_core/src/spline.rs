//! Splines of sketches: a chain of cubic Bézier segments, kept as their
//! control points `p0 c1 c2 p1 c1 c2 p2 …` (`3n + 1` points for `n`
//! segments). The drawing tool passes through the points clicked
//! ([`interpolate`]); splitting ([`split_at`]), reversing, moving and the
//! enclosed area ([`swept_area`]) stay exact on that form.

use glam::DVec2;

/// Most segments a spline holds (a guard against crafted files).
pub const MAX_SEGMENTS: usize = 64;

/// Points each segment is split into to draw it.
pub const STEPS: usize = 16;

/// Points closer than this (meters) are one.
const TOL: f64 = 1e-4;

/// The number of segments of `points`, when they are a spline's control points.
pub fn segment_count(points: &[DVec2]) -> Option<usize> {
    let n = points.len().checked_sub(1)?;
    (n >= 3 && n % 3 == 0).then_some(n / 3)
}

/// The control points of segment `i`.
pub fn segment(points: &[DVec2], i: usize) -> Option<[DVec2; 4]> {
    let s = points.get(3 * i..3 * i + 4)?;
    Some([s[0], s[1], s[2], s[3]])
}

/// The segments of `points`.
pub fn segments(points: &[DVec2]) -> impl Iterator<Item = [DVec2; 4]> + '_ {
    (0..segment_count(points).unwrap_or(0)).filter_map(|i| segment(points, i))
}

/// A whole number of segments, not too many, finite, and not all in one point.
pub fn is_valid(points: &[DVec2]) -> bool {
    let Some(n) = segment_count(points) else { return false };
    n <= MAX_SEGMENTS && points.iter().all(|p| p.is_finite()) && points.iter().any(|p| p.distance(points[0]) > TOL)
}

/// Whether the spline ends where it starts (with more than one segment).
pub fn is_closed(points: &[DVec2]) -> bool {
    segment_count(points).is_some_and(|n| n >= 2)
        && points.first().zip(points.last()).is_some_and(|(a, b)| a.distance(*b) <= TOL)
}

/// The point of a segment at `t` in `0..=1`.
pub fn point([p0, p1, p2, p3]: [DVec2; 4], t: f64) -> DVec2 {
    let s = 1.0 - t;
    p0 * (s * s * s) + p1 * (3.0 * s * s * t) + p2 * (3.0 * s * t * t) + p3 * (t * t * t)
}

/// The derivative of a segment at `t`.
pub fn derivative([p0, p1, p2, p3]: [DVec2; 4], t: f64) -> DVec2 {
    let s = 1.0 - t;
    (p1 - p0) * (3.0 * s * s) + (p2 - p1) * (6.0 * s * t) + (p3 - p2) * (3.0 * t * t)
}

/// The point at the spline's parameter `s` in `0..=n` (segment `floor(s)`).
pub fn point_at(points: &[DVec2], s: f64) -> Option<DVec2> {
    let (i, t) = locate(points, s)?;
    Some(point(segment(points, i)?, t))
}

/// The unit direction of travel at the parameter `s`.
pub fn direction_at(points: &[DVec2], s: f64) -> Option<DVec2> {
    let (i, t) = locate(points, s)?;
    let seg = segment(points, i)?;
    let d = derivative(seg, t);
    // A handle on its knot leaves no derivative there: look along the chord.
    let d = if d.length() > 1e-12 { d } else { point(seg, (t + 0.01).min(1.0)) - point(seg, (t - 0.01).max(0.0)) };
    d.try_normalize()
}

/// The segment and its local parameter of the spline's parameter `s`.
fn locate(points: &[DVec2], s: f64) -> Option<(usize, f64)> {
    let n = segment_count(points)?;
    if !s.is_finite() {
        return None;
    }
    let s = s.clamp(0.0, n as f64);
    let i = (s.floor() as usize).min(n - 1);
    Some((i, s - i as f64))
}

/// The spline as points from its start to its end, each segment split in
/// [`STEPS`]; the ends are exact.
pub fn flatten(points: &[DVec2]) -> Vec<DVec2> {
    let mut out = Vec::with_capacity(segment_count(points).unwrap_or(0) * STEPS + 1);
    for seg in segments(points) {
        if out.is_empty() {
            out.push(seg[0]);
        }
        out.extend((1..STEPS).map(|k| point(seg, k as f64 / STEPS as f64)));
        out.push(seg[3]);
    }
    out
}

/// The length of the drawn spline.
pub fn length(points: &[DVec2]) -> f64 {
    flatten(points).windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// The signed area the spline sweeps around the origin, the integral of
/// `(x dy - y dx) / 2` along it, exact: the integrand is a polynomial of
/// degree 5, which 3-point Gauss-Legendre integrates exactly.
pub fn swept_area(points: &[DVec2]) -> f64 {
    const NODES: [(f64, f64); 3] =
        [(0.112_701_665_379_258_3, 5.0 / 18.0), (0.5, 8.0 / 18.0), (0.887_298_334_620_741_7, 5.0 / 18.0)];
    segments(points)
        .map(|seg| NODES.iter().map(|&(t, w)| w * point(seg, t).perp_dot(derivative(seg, t))).sum::<f64>() * 0.5)
        .sum()
}

/// A segment split at `t`: the part before and the part after.
pub fn split([p0, p1, p2, p3]: [DVec2; 4], t: f64) -> ([DVec2; 4], [DVec2; 4]) {
    let a = p0.lerp(p1, t);
    let b = p1.lerp(p2, t);
    let c = p2.lerp(p3, t);
    let d = a.lerp(b, t);
    let e = b.lerp(c, t);
    let m = d.lerp(e, t);
    ([p0, a, d, m], [m, e, c, p3])
}

/// The spline split at its parameter `s`: the part before and the part
/// after (either may be empty at an end).
pub fn split_at(points: &[DVec2], s: f64) -> Option<(Vec<DVec2>, Vec<DVec2>)> {
    let (i, t) = locate(points, s)?;
    let seg = segment(points, i)?;
    let (before, after) = split(seg, t);
    let mut first: Vec<DVec2> = points.get(..3 * i)?.to_vec();
    first.extend(before);
    let mut second: Vec<DVec2> = after.to_vec();
    second.extend(points.get(3 * i + 4..)?);
    let keep = |p: Vec<DVec2>| if is_valid(&p) { p } else { Vec::new() };
    Some((keep(first), keep(second)))
}

/// The part of the spline between the parameters `s0` and `s1` (`s0 < s1`).
pub fn between(points: &[DVec2], s0: f64, s1: f64) -> Option<Vec<DVec2>> {
    let n = segment_count(points)? as f64;
    let (s0, s1) = (s0.clamp(0.0, n), s1.clamp(0.0, n));
    if s1 - s0 < 1e-9 {
        return None;
    }
    let (_, after) = split_at(points, s0)?;
    if after.is_empty() {
        return None;
    }
    // The parameter on what is left: whole segments keep theirs, the first
    // one cut shrinks.
    let i0 = s0.floor().min(n - 1.0);
    let t0 = s0 - i0;
    let local = if s1 < i0 + 1.0 { (s1 - s0) / (1.0 - t0).max(1e-12) } else { s1 - i0 };
    let (before, _) = split_at(&after, local)?;
    (!before.is_empty()).then_some(before)
}

/// The nearest point of the spline to `q`: its parameter, the point and the distance.
pub fn nearest(points: &[DVec2], q: DVec2) -> Option<(f64, DVec2, f64)> {
    const SAMPLES: usize = 32;
    let mut best: Option<(f64, f64)> = None;
    for (i, seg) in segments(points).enumerate() {
        for k in 0..=SAMPLES {
            let t = k as f64 / SAMPLES as f64;
            let d = point(seg, t).distance_squared(q);
            if best.is_none_or(|(_, b)| d < b) {
                best = Some((i as f64 + t, d));
            }
        }
    }
    let (mut s, _) = best?;
    // Narrow it down around the best sample.
    let n = segment_count(points)? as f64;
    let mut step = 1.0 / SAMPLES as f64;
    for _ in 0..30 {
        let cands = [s - step, s, s + step].map(|c| c.clamp(0.0, n));
        let at = |c: f64| point_at(points, c).map_or(f64::INFINITY, |p| p.distance_squared(q));
        s = cands.into_iter().min_by(|a, b| at(*a).total_cmp(&at(*b))).unwrap_or(s);
        step *= 0.5;
    }
    let p = point_at(points, s)?;
    Some((s, p, p.distance(q)))
}

/// The same spline run the other way.
pub fn reversed(points: &[DVec2]) -> Vec<DVec2> {
    points.iter().rev().copied().collect()
}

/// A spline through the `fit` points (closed back to the first when
/// `closed`): tangents from the neighbors, spaced by the distances between
/// the points, so it neither overshoots short spans nor flattens long ones.
/// Points repeated in a row count once; fewer than two distinct points make
/// no spline.
pub fn interpolate(fit: &[DVec2], closed: bool) -> Vec<DVec2> {
    let mut p: Vec<DVec2> = Vec::with_capacity(fit.len() + 1);
    for &q in fit {
        if p.last().is_none_or(|l| l.distance(q) > TOL) {
            p.push(q);
        }
    }
    let closed = closed && p.len() >= 3;
    if closed && p.first().zip(p.last()).is_some_and(|(a, b)| a.distance(*b) <= TOL) {
        p.pop();
    }
    if p.len() < 2 {
        return Vec::new();
    }
    let n = p.len();
    let at = |i: isize| p[i.rem_euclid(n as isize) as usize];
    // The tangent (per unit of distance) at each point.
    let tangent = |i: usize| -> DVec2 {
        let i = i as isize;
        let (prev, here, next) = (at(i - 1), at(i), at(i + 1));
        if !closed && i == 0 {
            return (next - here) / here.distance(next).max(TOL);
        }
        if !closed && i as usize == n - 1 {
            return (here - prev) / prev.distance(here).max(TOL);
        }
        let (d0, d1) = (prev.distance(here).max(TOL), here.distance(next).max(TOL));
        // The weighted mean of the two chords' directions.
        ((here - prev) / d0 * d1 + (next - here) / d1 * d0) / (d0 + d1)
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(3 * spans + 1);
    out.push(p[0]);
    for i in 0..spans {
        let (a, b) = (at(i as isize), at(i as isize + 1));
        let d = a.distance(b) / 3.0;
        out.push(a + tangent(i) * d);
        out.push(b - tangent((i + 1) % n) * d);
        out.push(b);
    }
    out
}

/// An ellipse, or the part of it from parameter angle `start_deg` turning
/// `sweep_deg` (degrees, negative clockwise), as the control points of
/// cubic segments of at most a quarter turn each: the circle's Bézier arcs
/// stretched to the semi-axes `radii` and turned `rotation_deg`, which keeps
/// them within 0.03 % of the radius. `None` for a degenerate ellipse.
pub fn ellipse(center: DVec2, radii: DVec2, rotation_deg: f64, start_deg: f64, sweep_deg: f64) -> Option<Vec<DVec2>> {
    let finite = [rotation_deg, start_deg, sweep_deg].iter().all(|v| v.is_finite()) && center.is_finite();
    if !finite || !(radii.x > TOL && radii.y > TOL && radii.is_finite()) || sweep_deg.abs() <= 1e-9 {
        return None;
    }
    let sweep = sweep_deg.clamp(-360.0, 360.0).to_radians();
    let n = ((sweep.abs() / std::f64::consts::FRAC_PI_2) - 1e-9).ceil().max(1.0) as usize;
    let step = sweep / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let turn = DVec2::from_angle(rotation_deg.to_radians());
    // A point of the unit circle stretched and turned onto the ellipse.
    let place = |v: DVec2| center + turn.rotate(v * radii);
    let start = start_deg.to_radians();
    let mut out = Vec::with_capacity(3 * n + 1);
    for i in 0..n {
        let (t0, t1) = (start + step * i as f64, start + step * (i + 1) as f64);
        let (u0, u1) = (DVec2::from_angle(t0), DVec2::from_angle(t1));
        if i == 0 {
            out.push(place(u0));
        }
        out.push(place(u0 + u0.perp() * k));
        out.push(place(u1 - u1.perp() * k));
        out.push(place(u1));
    }
    // A whole ellipse ends exactly where it starts.
    if sweep_deg.abs() >= 360.0 - 1e-9
        && let Some(&first) = out.first()
        && let Some(last) = out.last_mut()
    {
        *last = first;
    }
    Some(out)
}

/// A spline `d` to the left of this one (to the right when negative), as
/// near as offsetting each segment's control points along its end normals
/// allows (exact for straight segments, close for gentle curves).
pub fn offset(points: &[DVec2], d: f64) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = Vec::with_capacity(points.len());
    for seg in segments(points) {
        let n0 = direction_at(&seg, 0.0).map_or(DVec2::ZERO, DVec2::perp);
        let n1 = direction_at(&seg, 1.0).map_or(DVec2::ZERO, DVec2::perp);
        let moved = [seg[0] + n0 * d, seg[1] + n0 * d, seg[2] + n1 * d, seg[3] + n1 * d];
        match out.last_mut() {
            // Neighbors meet halfway between their moved ends.
            Some(last) => *last = (*last + moved[0]) * 0.5,
            None => out.push(moved[0]),
        }
        out.extend_from_slice(&moved[1..]);
    }
    if is_closed(points)
        && let (Some(&first), Some(&last)) = (out.first(), out.last())
    {
        let m = (first + last) * 0.5;
        if let Some(f) = out.first_mut() {
            *f = m;
        }
        if let Some(l) = out.last_mut() {
            *l = m;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn interpolation_passes_through_the_points() {
        let fit = [DVec2::new(0.0, 0.0), DVec2::new(1.0, 1.0), DVec2::new(2.0, 0.0), DVec2::new(3.0, 1.0)];
        let s = interpolate(&fit, false);
        assert_eq!(segment_count(&s), Some(3));
        for (i, p) in fit.iter().enumerate() {
            assert!(point_at(&s, i as f64).unwrap().distance(*p) < 1e-12);
        }
        assert!(is_valid(&s) && !is_closed(&s));
        let closed = interpolate(&fit, true);
        assert_eq!(segment_count(&closed), Some(4));
        assert!(is_closed(&closed));
        // Two points: a straight segment.
        let two = interpolate(&[DVec2::ZERO, DVec2::X], false);
        assert!((length(&two) - 1.0).abs() < 1e-9);
        assert!(interpolate(&[DVec2::ZERO, DVec2::ZERO], false).is_empty());
    }

    #[test]
    fn the_swept_area_is_exact() {
        // A unit square as four straight segments: area 1.
        let c = [DVec2::new(0.0, 0.0), DVec2::new(1.0, 0.0), DVec2::new(1.0, 1.0), DVec2::new(0.0, 1.0)];
        let mut pts = vec![c[0]];
        for i in 0..4 {
            let (a, b) = (c[i], c[(i + 1) % 4]);
            pts.extend([a.lerp(b, 1.0 / 3.0), a.lerp(b, 2.0 / 3.0), b]);
        }
        assert!((swept_area(&pts) - 1.0).abs() < 1e-12);
        // A circle of 4 Bézier quarters (the usual 0.5523 handles): pi to 0.03 %.
        let k = 0.552_284_749_8;
        let q = |a: DVec2, b: DVec2| [a + a.perp() * k, b - b.perp() * k, b];
        let mut circle = vec![DVec2::X];
        for (a, b) in [(DVec2::X, DVec2::Y), (DVec2::Y, -DVec2::X), (-DVec2::X, -DVec2::Y), (-DVec2::Y, DVec2::X)] {
            circle.extend(q(a, b));
        }
        assert!((swept_area(&circle) / PI - 1.0).abs() < 3e-4);
        // The flattened curve agrees.
        let flat = flatten(&circle);
        let poly: f64 = flat.windows(2).map(|w| w[0].perp_dot(w[1])).sum::<f64>() * 0.5;
        assert!((poly - swept_area(&circle)).abs() < 0.01);
    }

    #[test]
    fn splitting_keeps_the_curve() {
        let s = interpolate(&[DVec2::ZERO, DVec2::new(1.0, 1.0), DVec2::new(2.0, 0.0)], false);
        let (a, b) = split_at(&s, 0.5).unwrap();
        assert_eq!((segment_count(&a), segment_count(&b)), (Some(1), Some(2)));
        assert!(a.last().unwrap().distance(point_at(&s, 0.5).unwrap()) < 1e-12);
        assert!((length(&a) + length(&b) - length(&s)).abs() < 1e-3);
        // A part between two parameters.
        let mid = between(&s, 0.5, 1.5).unwrap();
        assert!(mid[0].distance(point_at(&s, 0.5).unwrap()) < 1e-9);
        assert!(mid.last().unwrap().distance(point_at(&s, 1.5).unwrap()) < 1e-9);
        let inside = between(&s, 0.25, 0.75).unwrap();
        assert!(inside.last().unwrap().distance(point_at(&s, 0.75).unwrap()) < 1e-9);
        // The nearest point.
        let (t, p, d) = nearest(&s, DVec2::new(1.0, 2.0)).unwrap();
        assert!((t - 1.0).abs() < 1e-3 && p.distance(DVec2::new(1.0, 1.0)) < 1e-3 && (d - 1.0).abs() < 1e-3);
    }

    #[test]
    fn offset_moves_straight_segments_exactly() {
        let line = vec![DVec2::ZERO, DVec2::new(1.0, 0.0), DVec2::new(2.0, 0.0), DVec2::new(3.0, 0.0)];
        let o = offset(&line, 0.5);
        assert!(o.iter().all(|p| (p.y - 0.5).abs() < 1e-12));
    }

    #[test]
    fn bad_splines_are_invalid() {
        assert!(!is_valid(&[DVec2::ZERO, DVec2::X]));
        assert!(!is_valid(&[DVec2::ZERO; 4]));
        assert!(!is_valid(&[DVec2::ZERO, DVec2::X, DVec2::NAN, DVec2::Y]));
        assert!(!is_valid(&vec![DVec2::X; 3 * (MAX_SEGMENTS + 1) + 1]));
    }

    #[test]
    fn ellipse_segments_stay_on_the_ellipse() {
        let (c, r) = (DVec2::new(1.0, 2.0), DVec2::new(3.0, 1.5));
        let pts = ellipse(c, r, 30.0, 10.0, 360.0).unwrap_or_default();
        assert_eq!(segment_count(&pts), Some(4));
        assert!(is_closed(&pts));
        let turn = DVec2::from_angle(-30f64.to_radians());
        for seg in segments(&pts) {
            for i in 0..=8 {
                let q = turn.rotate(point(seg, i as f64 / 8.0) - c) / r;
                assert!((q.length() - 1.0).abs() < 1e-3, "{q}");
            }
        }
        let part = ellipse(c, r, 0.0, 0.0, -100.0).unwrap_or_default();
        assert_eq!(segment_count(&part), Some(2));
        assert!(part[0].distance(c + DVec2::new(3.0, 0.0)) < 1e-9);
        let end = 100f64.to_radians();
        assert!(part[6].distance(c + DVec2::new(3.0 * end.cos(), -1.5 * end.sin())) < 1e-9);
        assert!(ellipse(c, DVec2::new(0.0, 1.0), 0.0, 0.0, 360.0).is_none());
    }
}
