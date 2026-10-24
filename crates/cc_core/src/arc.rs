//! Circular arcs in plan, described like polyline arcs by their two ends and
//! a *bulge*: `tan(sweep / 4)`, positive when the arc turns counter-clockwise
//! from the start to the end, 0 for a straight line, ±1 for a half circle.
//! A bulge is unchanged by moving, rotating or scaling the ends, and by a
//! mirror that also swaps them.

use glam::DVec2;
use std::f64::consts::{PI, TAU};

/// Largest bulge accepted: a sweep of about 337°, short of a full circle.
pub const MAX_BULGE: f64 = 10.0;

/// Bulges closer to 0 than this are straight lines.
const MIN_BULGE: f64 = 1e-6;

/// A circular arc from `start_angle` turning by `sweep` radians
/// (counter-clockwise when positive) around `center`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub center: DVec2,
    pub radius: f64,
    pub start_angle: f64,
    pub sweep: f64,
}

impl Arc {
    /// The arc from `a` to `b` with `bulge`; `None` when it is a straight
    /// line (no bulge), or the ends coincide or are not finite.
    pub fn from_bulge(a: DVec2, b: DVec2, bulge: f64) -> Option<Arc> {
        if !bulge.is_finite() || bulge.abs() < MIN_BULGE || !a.is_finite() || !b.is_finite() {
            return None;
        }
        let bulge = bulge.clamp(-MAX_BULGE, MAX_BULGE);
        let chord = a.distance(b);
        if chord < 1e-9 {
            return None;
        }
        let sweep = 4.0 * bulge.atan();
        let n = (b - a) / chord;
        // The center lies on the chord's bisector, chord / (2 tan(sweep / 2)) to its left.
        let center = (a + b) * 0.5 + n.perp() * (chord / (2.0 * (sweep * 0.5).tan()));
        let radius = center.distance(a);
        let d = a - center;
        let arc = Arc { center, radius, start_angle: d.y.atan2(d.x), sweep };
        (radius.is_finite() && center.is_finite()).then_some(arc)
    }

    /// Bulge of the arc from `a` through `m` to `b`; `None` when the points
    /// are (nearly) collinear, coincide, or `m` is not between the ends.
    pub fn bulge_through(a: DVec2, m: DVec2, b: DVec2) -> Option<f64> {
        if !(a.is_finite() && m.is_finite() && b.is_finite()) {
            return None;
        }
        let chord = a.distance(b);
        if chord < 1e-9 || a.distance(m) < 1e-9 || b.distance(m) < 1e-9 {
            return None;
        }
        // The inscribed angle at `m` is π - sweep / 2 (signed by the turn).
        let (u, v) = (a - m, b - m);
        let cross = u.perp_dot(v);
        if cross.abs() < 1e-9 * u.length() * v.length() {
            return None;
        }
        let inscribed = cross.atan2(u.dot(v)); // in (-π, π)
        // A counter-clockwise arc a → m → b has `m` on the right of a→b, where the
        // angle a-m-b, measured from u to v, is negative.
        let sweep = -(2.0 * (PI - inscribed.abs())) * inscribed.signum();
        let bulge = (sweep / 4.0).tan();
        (bulge.is_finite() && bulge.abs() >= MIN_BULGE && bulge.abs() <= MAX_BULGE).then_some(bulge)
    }

    /// The arc tangent to `tangent` at `a` ending at `b` (a tangent arc),
    /// as a bulge; `None` when `b` lies on the tangent line.
    pub fn bulge_tangent(a: DVec2, tangent: DVec2, b: DVec2) -> Option<f64> {
        let t = tangent.try_normalize()?;
        let chord = b - a;
        let len = chord.length();
        if len < 1e-9 {
            return None;
        }
        // The tangent-chord angle is half the sweep.
        let half = t.angle_to(chord / len);
        let bulge = (half / 2.0).tan();
        (bulge.is_finite() && bulge.abs() >= MIN_BULGE && bulge.abs() <= MAX_BULGE).then_some(bulge)
    }

    /// Turning direction: 1 counter-clockwise, -1 clockwise.
    pub fn sign(&self) -> f64 {
        if self.sweep >= 0.0 { 1.0 } else { -1.0 }
    }

    pub fn length(&self) -> f64 {
        self.radius * self.sweep.abs()
    }

    /// Angle of the point `u` along the arc from its start.
    pub fn angle_at(&self, u: f64) -> f64 {
        let r = self.radius.max(1e-12);
        self.start_angle + self.sign() * u / r
    }

    /// Point `u` along the arc from its start (beyond the ends it continues on the circle).
    pub fn point(&self, u: f64) -> DVec2 {
        self.center + DVec2::from_angle(self.angle_at(u)) * self.radius
    }

    /// Unit direction of travel at `u`.
    pub fn tangent(&self, u: f64) -> DVec2 {
        DVec2::from_angle(self.angle_at(u)).perp() * self.sign()
    }

    /// Point `u` along the arc and `v` to the left of it (on a concentric circle).
    pub fn at(&self, u: f64, v: f64) -> DVec2 {
        let radial = DVec2::from_angle(self.angle_at(u));
        // The left of the travel direction is towards the center on a counter-clockwise arc.
        self.center + radial * (self.radius - self.sign() * v)
    }

    /// Radius of the concentric circle `v` to the left of the arc.
    pub fn radius_at(&self, v: f64) -> f64 {
        self.radius - self.sign() * v
    }

    /// Position of `p` in the arc's frame: distance along the arc from its
    /// start (negative before it, past `length` after it; measured to the
    /// nearer side of the gap of the circle) and distance to the left.
    pub fn locate(&self, p: DVec2) -> (f64, f64) {
        let d = p - self.center;
        let angle = d.y.atan2(d.x);
        // Angle from the start in the direction of travel, centered on the arc's middle.
        let mid = self.sweep.abs() * 0.5;
        let mut t = (angle - self.start_angle) * self.sign();
        t = (t - mid + PI).rem_euclid(TAU) - PI + mid;
        (t * self.radius, (self.radius - d.length()) * self.sign())
    }

    /// The arc between `u0` and `u1` along it (either order), as points at
    /// most `max_step` radians apart (at least two points).
    pub fn points(&self, u0: f64, u1: f64, v: f64, max_step: f64) -> Vec<DVec2> {
        let r = self.radius.max(1e-12);
        let span = ((u1 - u0) / r).abs();
        let n = segments(span, max_step);
        (0..=n).map(|i| self.at(u0 + (u1 - u0) * i as f64 / n as f64, v)).collect()
    }

    /// The bulge of the part of this arc between `u0` and `u1`.
    pub fn bulge_between(&self, u0: f64, u1: f64) -> f64 {
        let sweep = self.sign() * (u1 - u0) / self.radius.max(1e-12);
        (sweep / 4.0).tan()
    }
}

/// Number of segments for a span of `angle` radians, each at most `max_step`.
pub fn segments(angle: f64, max_step: f64) -> usize {
    let step = if max_step.is_finite() && max_step > 1e-3 { max_step } else { 0.05 };
    let n = (angle.abs() / step).ceil();
    if n.is_finite() { (n as usize).clamp(1, 720) } else { 1 }
}

/// Default angular step for drawing and meshing arcs: 3°.
pub const ARC_STEP: f64 = 3.0 * PI / 180.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn half_circle_from_bulge() {
        // Counter-clockwise around the origin from angle 0 to π, through (0, 1).
        let a = Arc::from_bulge(DVec2::new(1.0, 0.0), DVec2::new(-1.0, 0.0), 1.0).unwrap();
        assert!(close(a.center, DVec2::ZERO), "{a:?}");
        assert!((a.radius - 1.0).abs() < 1e-12);
        assert!(close(a.point(a.length() / 2.0), DVec2::new(0.0, 1.0)));
        assert!(close(a.point(a.length()), DVec2::new(-1.0, 0.0)));
        assert!((a.length() - PI).abs() < 1e-12);
        // Its tangent at the start points up, and the left is towards the center.
        assert!(close(a.tangent(0.0), DVec2::Y));
        assert!(close(a.at(0.0, 0.25), DVec2::new(0.75, 0.0)));
    }

    #[test]
    fn three_points_and_bulge_agree() {
        let (p, q) = (DVec2::new(0.0, 0.0), DVec2::new(4.0, 0.0));
        for m in [DVec2::new(2.0, -1.0), DVec2::new(2.0, 1.0), DVec2::new(0.5, 3.0), DVec2::new(3.0, -2.5)] {
            let b = Arc::bulge_through(p, m, q).unwrap();
            let arc = Arc::from_bulge(p, q, b).unwrap();
            assert!((arc.center.distance(m) - arc.radius).abs() < 1e-9, "{m}: {arc:?}");
            assert!(close(arc.point(arc.length()), q));
            let (u, v) = arc.locate(m);
            assert!(u > 0.0 && u < arc.length() && v.abs() < 1e-9, "{m}: {u} {v}");
        }
        assert!(Arc::bulge_through(p, DVec2::new(2.0, 0.0), q).is_none());
        // A point below the chord makes the arc turn left (counter-clockwise).
        assert!(Arc::bulge_through(p, DVec2::new(2.0, -1.0), q).unwrap() > 0.0);
    }

    #[test]
    fn tangent_arc_continues_the_direction() {
        let b = Arc::bulge_tangent(DVec2::ZERO, DVec2::X, DVec2::new(2.0, 2.0)).unwrap();
        let arc = Arc::from_bulge(DVec2::ZERO, DVec2::new(2.0, 2.0), b).unwrap();
        assert!(close(arc.tangent(0.0), DVec2::X));
        assert!((arc.radius - 2.0).abs() < 1e-9);
        assert!(Arc::bulge_tangent(DVec2::ZERO, DVec2::X, DVec2::new(3.0, 0.0)).is_none());
    }

    #[test]
    fn degenerate_arcs_are_straight() {
        assert!(Arc::from_bulge(DVec2::ZERO, DVec2::X, 0.0).is_none());
        assert!(Arc::from_bulge(DVec2::ZERO, DVec2::ZERO, 1.0).is_none());
        assert!(Arc::from_bulge(DVec2::ZERO, DVec2::X, f64::NAN).is_none());
        assert!(Arc::from_bulge(DVec2::new(f64::INFINITY, 0.0), DVec2::X, 0.5).is_none());
        // Huge bulges are clamped short of a full circle.
        let a = Arc::from_bulge(DVec2::ZERO, DVec2::X, 1e9).unwrap();
        assert!(a.sweep < TAU && a.sweep > PI);
    }

    #[test]
    fn locate_measures_around_the_gap() {
        let a = Arc::from_bulge(DVec2::new(1.0, 0.0), DVec2::new(0.0, 1.0), (PI / 8.0).tan()).unwrap();
        // A point just before the start is at a small negative distance.
        let (u, _) = a.locate(DVec2::from_angle(-0.1) * 2.0);
        assert!((u - -0.1).abs() < 1e-9, "{u}");
        let (u, v) = a.locate(DVec2::from_angle(PI / 2.0 + 0.2) * 0.5);
        assert!((u - (PI / 2.0 + 0.2)).abs() < 1e-9 && (v - 0.5).abs() < 1e-9, "{u} {v}");
        assert!((a.bulge_between(0.0, a.length()) - (PI / 8.0).tan()).abs() < 1e-12);
    }
}
