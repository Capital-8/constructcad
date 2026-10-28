//! Ramp geometry: the sloped flights and flat landings of a ramp element
//! laid out from its shape, its slab solids and the paths its railings
//! follow.
//!
//! Like stairs, shapes are built in a local frame `(a, b)`: `a` along the
//! first flight from the middle of its foot, `b` across it towards the side
//! the ramp turns to; a ramp turning right is the mirror image.

use crate::stair::{Pieces, add_wedge};
use cc_core::{Project, RampCheck, RampLayout, RampRules, RampShape, StairSide, TypeData};
use glam::{DVec2, DVec3};
use std::f64::consts::FRAC_PI_2;

/// One surface of a ramp: a sloped flight or a flat landing.
#[derive(Debug, Clone, PartialEq)]
pub struct RampPiece {
    /// Plan outline (world).
    pub outline: Vec<DVec2>,
    /// Elevation of the walking surface at each outline vertex.
    pub z: Vec<f64>,
    pub landing: bool,
}

/// Layout of a ramp element.
#[derive(Debug, Clone, PartialEq)]
pub struct RampGeom {
    pub start: DVec2,
    pub u: DVec2,
    pub v: DVec2,
    pub width: f64,
    pub base: f64,
    pub top: f64,
    pub layout: RampLayout,
    pub check: RampCheck,
    /// Landing length along the way.
    pub landing: f64,
    pub pieces: Vec<RampPiece>,
    /// Walking line from the foot to the top, with elevations.
    pub walkline: Vec<DVec3>,
    /// Center of a spiral ramp.
    pub center: Option<DVec2>,
}

/// Segments per radian on arcs.
const ARC_STEPS: f64 = 12.0;

impl RampGeom {
    fn sign(&self) -> f64 {
        self.layout.turn.sign()
    }

    fn world(&self, p: DVec2) -> DVec2 {
        self.start + self.u * p.x + self.v * (p.y * self.sign())
    }

    /// Plan length of the walking line.
    pub fn run(&self) -> f64 {
        self.walkline.windows(2).map(|w| w[0].truncate().distance(w[1].truncate())).sum()
    }

    /// The slab of each piece, `thickness` thick square to its slope.
    /// The slab of each piece, `thickness` thick square to its slope; a
    /// flight whose underside meets the floor is split there, so the slab
    /// keeps its thickness above it.
    pub fn pieces(&self, thickness: f64) -> Pieces {
        let mut out = Pieces::new();
        let t = thickness.max(0.01) * (1.0 + self.check.slope * self.check.slope).sqrt();
        let mut slab = |outline: &[DVec2], z: &[f64], t: f64| {
            let bottom: Vec<f64> = z.iter().map(|z| (z - t).max(self.base)).collect();
            add_wedge(&mut out, outline, &bottom, z);
        };
        for p in &self.pieces {
            if p.landing {
                slab(&p.outline, &p.z, thickness.max(0.01));
                continue;
            }
            // Flights rise along their first and third edges.
            if let ([p0, p1, p2, p3], [z0, z1, ..]) = (p.outline.as_slice(), p.z.as_slice()) {
                let floor = self.base + t;
                let s = (floor - z0) / (z1 - z0);
                if (z1 - z0).abs() > 1e-9 && s > 1e-6 && s < 1.0 - 1e-6 {
                    let (q0, q3) = (p0.lerp(*p1, s), p3.lerp(*p2, s));
                    slab(&[*p0, q0, q3, *p3], &[*z0, floor, floor, *z0], t);
                    slab(&[q0, *p1, *p2, q3], &[floor, *z1, *z1, floor], t);
                    continue;
                }
            }
            slab(&p.outline, &p.z, t);
        }
        out
    }

    pub fn add_to(&self, thickness: f64, solid: &mut crate::Solid) {
        for piece in self.pieces(thickness) {
            solid.add_polygons(piece);
        }
    }

    /// Path of a railing along `side`, `inset` in from the edge, at the
    /// height of the walking surface.
    pub fn railing_path(&self, side: StairSide, inset: f64) -> Vec<DVec3> {
        let e = inset.clamp(-0.5, self.width / 2.0 - 0.01);
        let turn_side = (side == StairSide::Left) == (self.sign() > 0.0);
        let local = self.local_rail(turn_side, e);
        let mut out: Vec<DVec3> = Vec::with_capacity(local.len());
        for (p, z) in local {
            let w = self.world(p);
            let q = DVec3::new(w.x, w.y, z);
            if out.last().is_none_or(|l: &DVec3| l.distance(q) > 1e-6) {
                out.push(q);
            }
        }
        out
    }

    fn local_rail(&self, turn_side: bool, e: f64) -> Vec<(DVec2, f64)> {
        let (w, hw) = (self.width, self.width / 2.0);
        let o = hw - e;
        let side = if turn_side { 1.0 } else { -1.0 };
        let (n, rise, run) = (self.check.flights, self.check.flight_rise, self.check.flight_run);
        let z = |k: usize| self.base + rise * k as f64;
        let mut pts: Vec<(DVec2, f64)> = Vec::new();
        match self.layout.shape {
            RampShape::Straight => {
                for k in 0..n {
                    let a0 = k as f64 * (run + self.landing);
                    pts.push((DVec2::new(a0, side * o), z(k)));
                    pts.push((DVec2::new(a0 + run, side * o), z(k + 1)));
                }
            }
            RampShape::LShaped => {
                pts.push((DVec2::new(0.0, side * o), z(0)));
                pts.push((DVec2::new(run, side * o), z(1)));
                if !turn_side {
                    pts.push((DVec2::new(run + w - e, -o), z(1)));
                }
                let a = if turn_side { run + e } else { run + w - e };
                pts.push((DVec2::new(a, hw), z(1)));
                pts.push((DVec2::new(a, hw + run), z(2)));
            }
            RampShape::UShaped => {
                let step = w + self.layout.gap;
                for k in 0..n {
                    let b = k as f64 * step;
                    let forward = k % 2 == 0;
                    // Walking +a the left is +b; walking -a it is -b.
                    let off = if forward { side * o } else { -side * o };
                    let (a0, a1) = if forward { (0.0, run) } else { (run, 0.0) };
                    pts.push((DVec2::new(a0, b + off), z(k)));
                    pts.push((DVec2::new(a1, b + off), z(k + 1)));
                    if k + 1 < n {
                        // Round the landing at this end to the next flight (one
                        // more step towards +b): the side facing it goes round
                        // the end of the gap, the other round the landing. The
                        // turns alternate, so a side is inner at every other one.
                        let dir = if forward { 1.0 } else { -1.0 };
                        let inner = off > 0.0;
                        let reach = if inner { e } else { self.landing - e };
                        let (from, to) = (b + off, b + step - off);
                        pts.push((DVec2::new(a1 + dir * reach, from), z(k + 1)));
                        pts.push((DVec2::new(a1 + dir * reach, to), z(k + 1)));
                    }
                }
            }
            RampShape::Spiral => {
                let inner = self.layout.inner_radius;
                let r = if turn_side { inner + e } else { inner + w - e };
                if r < 0.02 {
                    return pts;
                }
                let rw = inner + hw;
                let sweep = run / rw.max(1e-6);
                let m = ((sweep * ARC_STEPS).ceil() as usize).clamp(1, 4000);
                let center = DVec2::new(0.0, rw);
                for k in 0..=m {
                    let t = k as f64 / m as f64;
                    pts.push((center + DVec2::from_angle(-FRAC_PI_2 + sweep * t) * r, self.base + rise * t));
                }
            }
        }
        pts
    }
}

/// Layout of a ramp element from its type, constraints and shape.
pub fn ramp_geom(project: &Project, element: &cc_core::Element, ramp: &cc_core::Ramp) -> Option<RampGeom> {
    let TypeData::Ramp { width, rules, .. } = project.element_type(element.type_id)?.data else {
        return None;
    };
    let (base, top) = project.ramp_span(element, ramp);
    let u = DVec2::from_angle(ramp.direction_deg.to_radians());
    Some(layout_ramp(ramp.start, u, width, base, top, &rules, ramp.layout))
}

/// Lay out a ramp from its foot `start`, first-flight direction `u`, width
/// and elevations.
pub fn layout_ramp(
    start: DVec2,
    u: DVec2,
    width: f64,
    base: f64,
    top: f64,
    rules: &RampRules,
    layout: RampLayout,
) -> RampGeom {
    let width = if width.is_finite() { width.clamp(0.3, 50.0) } else { 1.2 };
    let check = cc_core::ramp_flights(top - base, rules, &layout);
    let landing = if layout.landing > 1e-3 { layout.landing } else { rules.min_landing.max(0.1) };
    let mut g = RampGeom {
        start,
        u,
        v: u.perp(),
        width,
        base,
        top,
        layout,
        check,
        landing,
        pieces: Vec::new(),
        walkline: Vec::new(),
        center: None,
    };
    let (n, rise, run) = (g.check.flights, g.check.flight_rise, g.check.flight_run);
    let hw = width / 2.0;
    let z = |k: usize| base + rise * k as f64;
    let rect = |a0: f64, a1: f64, b0: f64, b1: f64| {
        vec![DVec2::new(a0, b0), DVec2::new(a1, b0), DVec2::new(a1, b1), DVec2::new(a0, b1)]
    };
    // Local pieces: outline, elevations, landing.
    let mut pieces: Vec<(Vec<DVec2>, Vec<f64>, bool)> = Vec::new();
    let mut walk: Vec<(DVec2, f64)> = Vec::new();
    match layout.shape {
        RampShape::Straight => {
            for k in 0..n {
                let a0 = k as f64 * (run + landing);
                pieces.push((rect(a0, a0 + run, -hw, hw), vec![z(k), z(k + 1), z(k + 1), z(k)], false));
                walk.push((DVec2::new(a0, 0.0), z(k)));
                walk.push((DVec2::new(a0 + run, 0.0), z(k + 1)));
                if k + 1 < n {
                    pieces.push((rect(a0 + run, a0 + run + landing, -hw, hw), vec![z(k + 1); 4], true));
                }
            }
        }
        RampShape::LShaped => {
            pieces.push((rect(0.0, run, -hw, hw), vec![z(0), z(1), z(1), z(0)], false));
            pieces.push((rect(run, run + width, -hw, hw), vec![z(1); 4], true));
            // The second flight goes along +b from the corner landing.
            let corner = |a: f64, b: f64| DVec2::new(a, b);
            let outline =
                vec![corner(run + width, hw), corner(run + width, hw + run), corner(run, hw + run), corner(run, hw)];
            pieces.push((outline, vec![z(1), z(2), z(2), z(1)], false));
            walk.push((DVec2::ZERO, z(0)));
            walk.push((DVec2::new(run + hw, 0.0), z(1)));
            walk.push((DVec2::new(run + hw, hw + run), z(2)));
        }
        RampShape::UShaped => {
            let step = width + layout.gap;
            for k in 0..n {
                let b = k as f64 * step;
                let forward = k % 2 == 0;
                let zs =
                    if forward { vec![z(k), z(k + 1), z(k + 1), z(k)] } else { vec![z(k + 1), z(k), z(k), z(k + 1)] };
                pieces.push((rect(0.0, run, b - hw, b + hw), zs, false));
                let (a0, a1) = if forward { (0.0, run) } else { (run, 0.0) };
                walk.push((DVec2::new(a0, b), z(k)));
                walk.push((DVec2::new(a1, b), z(k + 1)));
                if k + 1 < n {
                    let (l0, l1) = if forward { (run, run + landing) } else { (-landing, 0.0) };
                    pieces.push((rect(l0, l1, b - hw, b + step + hw), vec![z(k + 1); 4], true));
                }
            }
        }
        RampShape::Spiral => {
            let inner = layout.inner_radius;
            let rw = inner + hw;
            let center = DVec2::new(0.0, rw);
            let sweep = run / rw.max(1e-6);
            let m = ((sweep * ARC_STEPS).ceil() as usize).clamp(1, 4000);
            let at = |r: f64, t: f64| center + DVec2::from_angle(-FRAC_PI_2 + sweep * t) * r;
            for k in 0..m {
                let (t0, t1) = (k as f64 / m as f64, (k + 1) as f64 / m as f64);
                let (z0, z1) = (base + rise * t0, base + rise * t1);
                let mut outline = vec![at(inner + width, t0), at(inner + width, t1)];
                let mut zs = vec![z0, z1];
                if inner < 1e-3 {
                    outline.push(center);
                    zs.push((z0 + z1) / 2.0);
                } else {
                    outline.extend([at(inner, t1), at(inner, t0)]);
                    zs.extend([z1, z0]);
                }
                pieces.push((outline, zs, false));
            }
            walk = (0..=m).map(|k| (at(rw, k as f64 / m as f64), base + rise * k as f64 / m as f64)).collect();
            g.center = Some(center);
        }
    }
    let (sign, v) = (g.sign(), g.v);
    let world = move |p: DVec2| start + u * p.x + v * (p.y * sign);
    g.pieces = pieces
        .into_iter()
        .map(|(outline, z, landing)| RampPiece { outline: outline.into_iter().map(world).collect(), z, landing })
        .collect();
    g.walkline = walk
        .into_iter()
        .map(|(p, z)| {
            let w = world(p);
            DVec3::new(w.x, w.y, z)
        })
        .collect();
    g.center = g.center.map(world);
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Solid;
    use cc_core::Turn;

    fn geom(layout: RampLayout, height: f64) -> RampGeom {
        layout_ramp(DVec2::ZERO, DVec2::X, 1.2, 0.0, height, &RampRules::default(), layout)
    }

    fn volume(g: &RampGeom) -> f64 {
        let mut s = Solid::new(cc_core::ElementId(1), cc_core::Category::Ramp, [0, 0, 0]);
        g.add_to(0.15, &mut s);
        s.volume()
    }

    #[test]
    fn straight_ramp_gets_landings_as_the_rules_need() {
        // 1.50 m at 8.33 %: two flights of 9.00 m and a 1.20 m landing.
        let g = geom(RampLayout::default(), 1.5);
        let run = 0.75 / 0.0833;
        assert_eq!(g.pieces.len(), 3);
        assert!(g.pieces[1].landing);
        assert!((g.run() - (2.0 * run + 1.2)).abs() < 1e-9);
        let end = g.walkline.last().copied().unwrap_or_default();
        assert!((end.z - 1.5).abs() < 1e-12);
        // A slab 0.15 m square to the slope: flights 1.2 × run × 0.15 / cos, the landing 1.2 × 1.2 × 0.15.
        let slope = (1.0_f64 + 0.0833 * 0.0833).sqrt();
        let flights = 2.0 * 1.2 * run * 0.15 * slope;
        let v = volume(&g);
        // The foot of the first flight is clipped at the base.
        assert!(v < flights + 1.2 * 1.2 * 0.15 + 1e-9 && v > 0.95 * flights, "{v}");
        let rail = g.railing_path(StairSide::Left, 0.05);
        assert_eq!(rail.len(), 4);
        assert!((rail[3] - DVec3::new(2.0 * run + 1.2, 0.55, 1.5)).length() < 1e-9);
    }

    #[test]
    fn turning_ramps_come_back_and_mirror() {
        let u = RampLayout { shape: RampShape::UShaped, gap: 0.2, ..Default::default() };
        let g = geom(u, 2.0);
        // 3 flights of 0.667 m: up, back, up again, side by side.
        assert_eq!(g.check.flights, 3);
        let end = g.walkline.last().copied().unwrap_or_default();
        assert!((end.x - g.check.flight_run).abs() < 1e-9 && (end.y - 2.8).abs() < 1e-9, "{end}");
        let right = geom(RampLayout { turn: Turn::Right, ..u }, 2.0);
        let end_r = right.walkline.last().copied().unwrap_or_default();
        assert!((end_r.y + 2.8).abs() < 1e-9);
        for side in [StairSide::Left, StairSide::Right] {
            let rail = g.railing_path(side, 0.05);
            assert!(rail.last().is_some_and(|p| (p.z - 2.0).abs() < 1e-9));
        }
        let l = geom(RampLayout { shape: RampShape::LShaped, ..Default::default() }, 1.0);
        assert_eq!(l.pieces.len(), 3);
        assert!(volume(&l) > 0.0);
    }

    #[test]
    fn spiral_ramp_turns_around_its_center() {
        let g = geom(RampLayout { shape: RampShape::Spiral, inner_radius: 2.0, ..Default::default() }, 0.8);
        let c = g.center.unwrap_or_default();
        for p in g.pieces.iter().flat_map(|p| &p.outline) {
            let r = p.distance(c);
            assert!(r > 2.0 - 1e-9 && r < 3.2 + 1e-9);
        }
        assert!((g.run() - 0.8 / 0.0833).abs() < 0.01, "{}", g.run());
        assert!(volume(&g) > 0.0);
        assert!(!g.railing_path(StairSide::Right, 0.05).is_empty());
    }

    #[test]
    fn flat_and_bad_ramps_do_not_break() {
        for shape in RampShape::ALL {
            for height in [0.0, 1e-4, 0.3] {
                let g = geom(RampLayout { shape, inner_radius: 0.0, ..Default::default() }, height);
                let _ = g.railing_path(StairSide::Left, 0.05);
                assert!(g.pieces.iter().all(|p| p.z.iter().all(|z| z.is_finite())));
            }
        }
    }
}
