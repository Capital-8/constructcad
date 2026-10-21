//! Profiles of component pieces: closed outlines of straight segments and
//! arcs drawn on one of the component's planes (plan, front or side), with
//! every coordinate an [`crate::expr`] expression, and the solids made from
//! them: an extrusion across the plane or a revolution about an axis in it.
//!
//! A profile's plane is placed at `offset` along the axis normal to it (Z
//! for the plan, Y for the front, X for the side), and an extrusion runs
//! `depth` from there along that axis. The solids come out as closed meshes
//! with outward faces, in the component's frame (X to the right, Y to the
//! back, Z up).

use crate::expr::{self, ExprError};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// Arc segments per full turn.
const ARC_SEGMENTS: f64 = 32.0;
/// Segments of a full revolution.
const REVOLUTION_SEGMENTS: f64 = 48.0;
/// Distance under which a point lies on a revolution axis or two points coincide.
const EPS: f64 = 1e-9;

/// The plane a profile is drawn on, as seen in the matching view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SketchPlane {
    /// Seen from above: U along X, V along Y; placed and extruded along Z.
    #[default]
    Plan,
    /// Seen from the front (-Y): U along X, V along Z; placed and extruded along Y.
    Front,
    /// Seen from the right (+X): U along Y, V along Z; placed and extruded along X.
    Side,
}

impl SketchPlane {
    pub const ALL: [SketchPlane; 3] = [SketchPlane::Plan, SketchPlane::Front, SketchPlane::Side];

    pub fn key(self) -> &'static str {
        match self {
            SketchPlane::Plan => "comp-plane-plan",
            SketchPlane::Front => "comp-plane-front",
            SketchPlane::Side => "comp-plane-side",
        }
    }

    /// The point at `u`, `v` on the plane placed at `w` on its normal axis.
    pub fn point(self, u: f64, v: f64, w: f64) -> DVec3 {
        match self {
            SketchPlane::Plan => DVec3::new(u, v, w),
            SketchPlane::Front => DVec3::new(u, w, v),
            SketchPlane::Side => DVec3::new(w, u, v),
        }
    }

    /// World directions of U, V and the extrusion (the plane's normal axis).
    fn axes(self) -> (DVec3, DVec3, DVec3) {
        match self {
            SketchPlane::Plan => (DVec3::X, DVec3::Y, DVec3::Z),
            SketchPlane::Front => (DVec3::X, DVec3::Z, DVec3::Y),
            SketchPlane::Side => (DVec3::Y, DVec3::Z, DVec3::X),
        }
    }
}

/// A vertex of a profile. The segment to the next vertex is straight, or an
/// arc of `radius` when it is not empty: a positive radius bulges to the
/// right of the direction of travel (outwards in a counter-clockwise
/// profile), a negative one to the left.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProfilePoint {
    pub u: String,
    pub v: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub radius: String,
}

impl ProfilePoint {
    pub fn new(u: impl Into<String>, v: impl Into<String>) -> Self {
        Self { u: u.into(), v: v.into(), radius: String::new() }
    }

    pub fn arc(u: impl Into<String>, v: impl Into<String>, radius: impl Into<String>) -> Self {
        Self { u: u.into(), v: v.into(), radius: radius.into() }
    }
}

/// Why a profile could not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileError {
    Expr {
        field: &'static str,
        error: ExprError,
    },
    /// Fewer than 3 distinct points, no area, an arc shorter than its chord,
    /// a zero size or a profile crossing its revolution axis.
    Degenerate,
}

/// The outline of `points` in plane coordinates, arcs divided into segments,
/// counter-clockwise.
pub fn outline(points: &[ProfilePoint], lookup: &dyn Fn(&str) -> Option<f64>) -> Result<Vec<DVec2>, ProfileError> {
    let ev =
        |field: &'static str, text: &str| expr::eval(text, lookup).map_err(|error| ProfileError::Expr { field, error });
    let mut corners = Vec::with_capacity(points.len());
    for p in points {
        let radius = if p.radius.trim().is_empty() { None } else { Some(ev("radius", &p.radius)?) };
        corners.push((DVec2::new(ev("u", &p.u)?, ev("v", &p.v)?), radius));
    }
    let n = corners.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (a, radius) = corners[i];
        let b = corners[(i + 1) % n].0;
        out.push(a);
        if let Some(r) = radius {
            arc_points(a, b, r, &mut out)?;
        }
    }
    out.dedup_by(|a, b| a.distance(*b) < EPS);
    while out.len() > 1 && out.first().zip(out.last()).is_some_and(|(f, l)| f.distance(*l) < EPS) {
        out.pop();
    }
    let area = signed_area(&out);
    if out.len() < 3 || area.abs() < 1e-12 || !area.is_finite() {
        return Err(ProfileError::Degenerate);
    }
    if area < 0.0 {
        out.reverse();
    }
    Ok(out)
}

/// The inner points of the arc of `radius` from `a` to `b` (see [`ProfilePoint`]).
fn arc_points(a: DVec2, b: DVec2, radius: f64, out: &mut Vec<DVec2>) -> Result<(), ProfileError> {
    let chord = a.distance(b);
    let r = radius.abs();
    if chord < EPS {
        return Ok(());
    }
    if !(r.is_finite() && r >= chord / 2.0 - 1e-9) {
        return Err(ProfileError::Degenerate);
    }
    let half = ((chord / 2.0) / r).clamp(-1.0, 1.0).asin();
    let dir = (b - a) / chord;
    let left = dir.perp();
    // Bulging right, the center is on the left and the arc turns counter-clockwise.
    let sign = radius.signum();
    let center = (a + b) * 0.5 + left * (sign * r * half.cos());
    let start = (a - center).to_angle();
    let sweep = 2.0 * half * sign;
    let steps = ((2.0 * half / TAU) * ARC_SEGMENTS).ceil().max(2.0) as usize;
    for k in 1..steps {
        let t = start + sweep * k as f64 / steps as f64;
        out.push(center + DVec2::from_angle(t) * r);
    }
    Ok(())
}

fn signed_area(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].perp_dot(poly[(i + 1) % n])).sum::<f64>() / 2.0
}

/// The closed solid of a counter-clockwise `outline` on `plane`, from `offset`
/// along the plane's normal to `offset + depth`.
pub fn extrude(plane: SketchPlane, outline: &[DVec2], offset: f64, depth: f64) -> Vec<Vec<DVec3>> {
    let (u, v, d) = plane.axes();
    let at = |p: DVec2, w: f64| plane.point(p.x, p.y, w);
    let near: Vec<DVec3> = outline.iter().map(|p| at(*p, offset)).collect();
    let far: Vec<DVec3> = outline.iter().map(|p| at(*p, offset + depth)).collect();
    // A counter-clockwise outline faces U x V; the near cap faces against the extrusion.
    let along = u.cross(v).dot(d) > 0.0;
    let mut faces = Vec::with_capacity(outline.len() + 2);
    let reversed = |f: &[DVec3]| f.iter().rev().copied().collect::<Vec<_>>();
    faces.push(if along { reversed(&near) } else { near.clone() });
    faces.push(if along { far.clone() } else { reversed(&far) });
    let n = outline.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let quad = vec![near[i], near[j], far[j], far[i]];
        faces.push(if along { quad } else { reversed(&quad) });
    }
    faces
}

/// The solid swept by a counter-clockwise `outline` on `plane` (placed
/// `offset` along its normal) turning `angle` degrees about the line
/// `u = axis` of the plane (parallel to V). The outline must lie on one side
/// of the axis; it may touch it.
pub fn revolve(
    plane: SketchPlane,
    outline: &[DVec2],
    offset: f64,
    axis: f64,
    angle: f64,
) -> Result<Vec<Vec<DVec3>>, ProfileError> {
    let (min_u, max_u) =
        outline.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
    // Radial distances from the axis, on the side the outline lies.
    let side = if min_u >= axis - 1e-9 {
        1.0
    } else if max_u <= axis + 1e-9 {
        -1.0
    } else {
        return Err(ProfileError::Degenerate);
    };
    let sweep = angle.to_radians();
    if !(sweep.abs() > 1e-6 && sweep.is_finite()) {
        return Err(ProfileError::Degenerate);
    }
    let sweep = sweep.clamp(-TAU, TAU);
    let full = (sweep.abs() - TAU).abs() < 1e-9;
    let (u, v, _) = plane.axes();
    let radial = u * side;
    // Turning the radial direction towards `tangent` keeps the faces of a
    // counter-clockwise (radius, height) outline outwards.
    let tangent = radial.cross(v) * sweep.signum();
    let base = plane.point(axis, 0.0, offset);
    // A mirrored outline (on the negative side) runs clockwise in (radius, height).
    let mut profile: Vec<DVec2> = outline.iter().map(|p| DVec2::new((p.x - axis) * side, p.y)).collect();
    if signed_area(&profile) < 0.0 {
        profile.reverse();
    }
    if sweep < 0.0 {
        profile.reverse();
    }
    let steps = ((sweep.abs() / TAU) * REVOLUTION_SEGMENTS).ceil().max(3.0) as usize;
    let at = |p: DVec2, k: usize| {
        let t = sweep.abs() * k as f64 / steps as f64;
        base + (radial * t.cos() + tangent * t.sin()) * p.x.max(0.0) + v * p.y
    };
    let n = profile.len();
    let mut faces = Vec::with_capacity(n * steps + 2);
    for i in 0..n {
        let (a, b) = (profile[i], profile[(i + 1) % n]);
        if a.x <= 1e-9 && b.x <= 1e-9 {
            continue;
        }
        for k in 0..steps {
            let next = if full && k + 1 == steps { 0 } else { k + 1 };
            let mut face = vec![at(a, k), at(b, k), at(b, next), at(a, next)];
            face.dedup_by(|p, q| p.distance(*q) < EPS);
            if face.len() >= 3 {
                faces.push(face);
            }
        }
    }
    if !full {
        // The outline itself closes both ends: against the turn at the start, along it at the end.
        faces.push((0..n).rev().map(|i| at(profile[i], 0)).collect());
        faces.push((0..n).map(|i| at(profile[i], steps)).collect());
    }
    Ok(faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(faces: &[Vec<DVec3>]) -> f64 {
        faces.iter().map(|f| (1..f.len() - 1).map(|i| f[0].dot(f[i].cross(f[i + 1]))).sum::<f64>()).sum::<f64>() / 6.0
    }

    fn none(_: &str) -> Option<f64> {
        None
    }

    fn square(size: &str) -> Vec<ProfilePoint> {
        vec![
            ProfilePoint::new("0", "0"),
            ProfilePoint::new(size, "0"),
            ProfilePoint::new(size, size),
            ProfilePoint::new("0", size),
        ]
    }

    #[test]
    fn extrusions_on_every_plane_are_closed_and_outward() {
        let outline = outline(&square("2"), &none).unwrap();
        for plane in SketchPlane::ALL {
            let faces = extrude(plane, &outline, 0.5, 3.0);
            assert!((volume(&faces) - 12.0).abs() < 1e-9, "{plane:?}: {}", volume(&faces));
        }
        // Front: U is X, V is Z, extruded to the back from y = 0.5.
        let faces = extrude(SketchPlane::Front, &outline, 0.5, 3.0);
        let all: Vec<DVec3> = faces.iter().flatten().copied().collect();
        let max = all.iter().fold(DVec3::splat(f64::NEG_INFINITY), |m, p| m.max(*p));
        assert_eq!(max, DVec3::new(2.0, 3.5, 2.0));
        // Side: U is Y, placed and extruded along X.
        let faces = extrude(SketchPlane::Side, &outline, -1.5, 3.0);
        let min = faces.iter().flatten().fold(DVec3::splat(f64::INFINITY), |m, p| m.min(*p));
        let max = faces.iter().flatten().fold(DVec3::splat(f64::NEG_INFINITY), |m, p| m.max(*p));
        assert_eq!((min, max), (DVec3::new(-1.5, 0.0, 0.0), DVec3::new(1.5, 2.0, 2.0)));
        // A profile written clockwise comes out the same.
        let mut cw = square("2");
        cw.reverse();
        assert_eq!(super::outline(&cw, &none).unwrap().len(), 4);
    }

    #[test]
    fn arcs_bulge_by_the_sign_of_their_radius() {
        // A 2 x 1 rectangle whose top edge (right to left) is a half circle bulging up.
        let mut pts = vec![
            ProfilePoint::new("0", "0"),
            ProfilePoint::new("2", "0"),
            ProfilePoint::arc("2", "1", "1"),
            ProfilePoint::new("0", "1"),
        ];
        let o = outline(&pts, &none).unwrap();
        let area = signed_area(&o);
        let half_disc = std::f64::consts::PI / 2.0;
        // The polygon of the arc falls a little short of the circle.
        assert!(area < 2.0 + half_disc && area > 2.0 + half_disc * 0.98, "{area}");
        assert!(o.iter().any(|p| (p.y - 2.0).abs() < 1e-9));
        // A negative radius cuts the half circle out instead.
        pts[2].radius = "-1".into();
        let area = signed_area(&outline(&pts, &none).unwrap());
        assert!(area > 2.0 - half_disc && area < 2.0 - half_disc * 0.98, "{area}");
        // A radius shorter than half the chord cannot join the points.
        pts[2].radius = "0.5".into();
        assert_eq!(outline(&pts, &none), Err(ProfileError::Degenerate));
        pts[2].radius = "R".into();
        assert!(matches!(outline(&pts, &none), Err(ProfileError::Expr { field: "radius", .. })));
    }

    #[test]
    fn revolutions_make_closed_solids_of_the_right_volume() {
        // A 1 x 2 rectangle touching the axis: a cylinder of radius 1, height 2.
        let rect = vec![
            ProfilePoint::new("0", "0"),
            ProfilePoint::new("1", "0"),
            ProfilePoint::new("1", "2"),
            ProfilePoint::new("0", "2"),
        ];
        let o = outline(&rect, &none).unwrap();
        let polygon_area = |n: f64| 0.5 * n * (TAU / n).sin();
        for plane in SketchPlane::ALL {
            let faces = revolve(plane, &o, 0.0, 0.0, 360.0).unwrap();
            let expected = polygon_area(REVOLUTION_SEGMENTS) * 2.0;
            assert!((volume(&faces) - expected).abs() < 1e-9, "{plane:?}: {}", volume(&faces));
        }
        // A quarter turn the other way, and an outline on the negative side of the axis.
        let quarter = revolve(SketchPlane::Front, &o, 0.0, 0.0, -90.0).unwrap();
        let expected = 12.0 * 0.5 * (TAU / 48.0).sin() * 2.0;
        assert!((volume(&quarter) - expected).abs() < 1e-9, "{}", volume(&quarter));
        let left: Vec<DVec2> = o.iter().map(|p| DVec2::new(3.0 - p.x, p.y)).rev().collect();
        let ring = revolve(SketchPlane::Front, &left, 0.0, 3.0, 360.0).unwrap();
        assert!(volume(&ring) > 0.0);
        // Around the vertical axis of the front plane: the cylinder stands on the floor.
        let faces = revolve(SketchPlane::Front, &o, 0.0, 0.0, 360.0).unwrap();
        let top = faces.iter().flatten().fold(f64::NEG_INFINITY, |m, p| m.max(p.z));
        assert!((top - 2.0).abs() < 1e-12);
        // Crossing the axis is refused.
        assert_eq!(revolve(SketchPlane::Plan, &o, 0.0, 0.5, 360.0), Err(ProfileError::Degenerate));
    }
}
