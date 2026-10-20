//! Reinforcing bars: the center lines of each set of bars in its beam or
//! column, from the host's body, the cover and the bar diameters.
//!
//! Longitudinal bars sit inside the stirrups (cover plus the host's largest
//! stirrup diameter); stirrups are closed loops `cover` inside the faces,
//! spaced evenly and centered along the host.

use crate::BeamGeom;
use cc_core::{BarFace, ColumnSection, ElementId, ElementKind, Project, RebarLayout, TypeData};
use glam::{DVec2, DVec3};
use std::collections::BTreeMap;

/// Density of steel, kg/m³.
pub const STEEL_DENSITY: f64 = 7850.0;

/// Segments of round ties.
const ROUND: usize = 16;

/// At most this many stirrups per set (a crafted spacing cannot explode).
const MAX_STIRRUPS: usize = 2000;

/// One bar: its center line, closed for stirrups.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub points: Vec<DVec3>,
    pub closed: bool,
}

impl Bar {
    pub fn length(&self) -> f64 {
        let open: f64 = self.points.windows(2).map(|w| w[0].distance(w[1])).sum();
        let closing = match (self.closed, self.points.first(), self.points.last()) {
            (true, Some(a), Some(b)) => a.distance(*b),
            _ => 0.0,
        };
        open + closing
    }
}

/// The bars of one reinforcement set.
#[derive(Debug, Clone, PartialEq)]
pub struct RebarGeom {
    pub element: ElementId,
    pub host: ElementId,
    pub diameter: f64,
    pub bars: Vec<Bar>,
}

impl RebarGeom {
    pub fn count(&self) -> usize {
        self.bars.len()
    }

    pub fn total_length(&self) -> f64 {
        self.bars.iter().map(Bar::length).sum()
    }

    /// Mean length of one bar.
    pub fn bar_length(&self) -> f64 {
        if self.bars.is_empty() { 0.0 } else { self.total_length() / self.bars.len() as f64 }
    }

    /// Steel weight in kilograms.
    pub fn weight(&self) -> f64 {
        std::f64::consts::PI * self.diameter * self.diameter / 4.0 * self.total_length() * STEEL_DENSITY
    }
}

/// `n` positions spread evenly over `[-half, half]` (the middle for one).
fn spread(n: usize, half: f64) -> Vec<f64> {
    match n {
        0 => Vec::new(),
        1 => vec![0.0],
        _ => (0..n).map(|i| -half + 2.0 * half * i as f64 / (n - 1) as f64).collect(),
    }
}

/// Positions `spacing` apart filling `[a, b]`, centered.
fn stations(a: f64, b: f64, spacing: f64) -> Vec<f64> {
    if b < a || spacing <= 0.0 {
        return Vec::new();
    }
    let count = (((b - a) / spacing).floor() as usize + 1).min(MAX_STIRRUPS);
    let start = a + ((b - a) - spacing * (count - 1) as f64) / 2.0;
    (0..count).map(|i| start + spacing * i as f64).collect()
}

/// `n` points around a `2a` x `2b` rectangle: the corners first, the rest
/// along the sides, each to the side with the widest gap.
fn perimeter(n: usize, a: f64, b: f64) -> Vec<DVec2> {
    let corners = [DVec2::new(-a, -b), DVec2::new(a, -b), DVec2::new(a, b), DVec2::new(-a, b)];
    if n <= 4 {
        // One or two bars sit on the bottom side's corners; three take three corners.
        return corners.into_iter().take(n).collect();
    }
    let lengths = [2.0 * a, 2.0 * b, 2.0 * a, 2.0 * b];
    let mut extra = [0usize; 4];
    for _ in 4..n {
        let side = (0..4)
            .max_by(|&i, &j| (lengths[i] / (extra[i] + 1) as f64).total_cmp(&(lengths[j] / (extra[j] + 1) as f64)))
            .unwrap_or(0);
        extra[side] += 1;
    }
    let mut points = corners.to_vec();
    for side in 0..4 {
        let (p, q) = (corners[side], corners[(side + 1) % 4]);
        for k in 1..=extra[side] {
            points.push(p + (q - p) * (k as f64 / (extra[side] + 1) as f64));
        }
    }
    points
}

fn circle(n: usize, r: f64) -> Vec<DVec2> {
    (0..n).map(|i| DVec2::from_angle(i as f64 / n as f64 * std::f64::consts::TAU) * r).collect()
}

/// The diameter of a rebar element's type.
fn diameter(project: &Project, e: &cc_core::Element) -> Option<f64> {
    match project.element_type(e.type_id).map(|t| &t.data) {
        Some(TypeData::Rebar { diameter, .. }) if diameter.is_finite() && *diameter > 0.0 => Some(*diameter),
        _ => None,
    }
}

/// Bars of every reinforcement set whose host has a body.
pub fn build_rebar(project: &Project, beams: &BTreeMap<ElementId, BeamGeom>) -> BTreeMap<ElementId, RebarGeom> {
    // The largest stirrup of each host: longitudinal bars sit inside it.
    let mut stirrup: BTreeMap<ElementId, f64> = BTreeMap::new();
    for e in project.elements.values() {
        if let ElementKind::Rebar(r) = &e.kind
            && matches!(r.layout, RebarLayout::Stirrups { .. })
            && let Some(d) = diameter(project, e)
        {
            let s = stirrup.entry(r.host).or_insert(0.0);
            *s = s.max(d);
        }
    }
    let mut out = BTreeMap::new();
    for e in project.elements.values() {
        let ElementKind::Rebar(r) = &e.kind else { continue };
        let Some(d) = diameter(project, e) else { continue };
        let Some(host) = project.element(r.host) else { continue };
        let ds = stirrup.get(&r.host).copied().unwrap_or(0.0);
        let bars = match &host.kind {
            ElementKind::Beam(_) => {
                let Some(g) = beams.get(&r.host) else { continue };
                let Some(TypeData::Beam { height, .. }) = project.element_type(host.type_id).map(|t| &t.data) else {
                    continue;
                };
                beam_bars(g, *height, r.layout, r.cover, d, ds)
            }
            ElementKind::Column(c) => {
                let Some(TypeData::Column { section, .. }) = project.element_type(host.type_id).map(|t| &t.data) else {
                    continue;
                };
                let (base, top) = project.column_span(host, c);
                column_bars(*section, c.position, c.rotation_deg, (base, top), r.layout, r.cover, d, ds)
            }
            _ => continue,
        };
        out.insert(e.id, RebarGeom { element: e.id, host: r.host, diameter: d, bars });
    }
    out
}

/// Bars along a beam: longitudinal bars from end to end inside the cover,
/// stirrups in vertical planes across it.
fn beam_bars(g: &BeamGeom, height: f64, layout: RebarLayout, cover: f64, d: f64, ds: f64) -> Vec<Bar> {
    let k = g.stretch();
    let u0 = g.start_ext.0.max(g.start_ext.1) + cover;
    let u1 = g.end_ext.0.min(g.end_ext.1) - cover;
    if u1 <= u0 {
        return Vec::new();
    }
    let at = |u: f64, v: f64, z: f64| g.world(u, v).extend(g.z_at(u).0 + z * k);
    match layout {
        RebarLayout::Longitudinal { face, count } => {
            let inset = cover + ds + d / 2.0;
            let z = match face {
                BarFace::Top => height - inset,
                BarFace::Bottom | BarFace::Perimeter => inset,
            };
            spread(count as usize, (g.half - inset).max(0.0))
                .into_iter()
                .map(|v| Bar { points: vec![at(u0, v, z), at(u1, v, z)], closed: false })
                .collect()
        }
        RebarLayout::Stirrups { spacing } => {
            let inset = cover + d / 2.0;
            let (v, z0, z1) = ((g.half - inset).max(0.0), inset, (height - inset).max(inset));
            stations(u0, u1, spacing)
                .into_iter()
                .map(|u| Bar { points: vec![at(u, -v, z0), at(u, v, z0), at(u, v, z1), at(u, -v, z1)], closed: true })
                .collect()
        }
    }
}

/// Bars of a column: vertical bars around the section over its height,
/// horizontal ties up the column.
#[allow(clippy::too_many_arguments)]
fn column_bars(
    section: ColumnSection,
    position: DVec2,
    rotation_deg: f64,
    (base, top): (f64, f64),
    layout: RebarLayout,
    cover: f64,
    d: f64,
    ds: f64,
) -> Vec<Bar> {
    let rot = DVec2::from_angle(rotation_deg.to_radians());
    let place = |p: DVec2, z: f64| (position + rot.rotate(p)).extend(z);
    let round = matches!(section, ColumnSection::Round { .. } | ColumnSection::Pipe { .. });
    let (w, depth) = section.size();
    match layout {
        RebarLayout::Longitudinal { count, .. } => {
            let inset = cover + ds + d / 2.0;
            let plan = if round {
                circle(count as usize, (w / 2.0 - inset).max(0.0))
            } else {
                perimeter(count as usize, (w / 2.0 - inset).max(0.0), (depth / 2.0 - inset).max(0.0))
            };
            plan.into_iter().map(|p| Bar { points: vec![place(p, base), place(p, top)], closed: false }).collect()
        }
        RebarLayout::Stirrups { spacing } => {
            let inset = cover + d / 2.0;
            let outline = if round {
                circle(ROUND, (w / 2.0 - inset).max(0.0))
            } else {
                let (a, b) = ((w / 2.0 - inset).max(0.0), (depth / 2.0 - inset).max(0.0));
                vec![DVec2::new(-a, -b), DVec2::new(a, -b), DVec2::new(a, b), DVec2::new(-a, b)]
            };
            stations(base + cover, top - cover, spacing)
                .into_iter()
                .map(|z| Bar { points: outline.iter().map(|p| place(*p, z)).collect(), closed: true })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stirrups_fill_the_length_centered() {
        // 1.00 m at 0.15: 7 stirrups spanning 0.90, starting 0.05 in.
        let s = stations(0.0, 1.0, 0.15);
        assert_eq!(s.len(), 7);
        assert!((s[0] - 0.05).abs() < 1e-12 && (s[6] - 0.95).abs() < 1e-12);
        assert!(stations(1.0, 0.0, 0.15).is_empty());
    }

    #[test]
    fn perimeter_bars_take_the_corners_then_the_longer_sides() {
        let p = perimeter(6, 0.2, 0.1);
        assert_eq!(p.len(), 6);
        // The extra two go to the long (0.4 m) sides, at their middles.
        assert!(p.contains(&DVec2::new(0.0, -0.1)) && p.contains(&DVec2::new(0.0, 0.1)));
        assert_eq!(perimeter(4, 0.2, 0.1).len(), 4);
    }

    #[test]
    fn a_stirrup_is_a_closed_loop() {
        let bar = Bar {
            points: vec![DVec3::ZERO, DVec3::new(0.1, 0.0, 0.0), DVec3::new(0.1, 0.0, 0.3), DVec3::new(0.0, 0.0, 0.3)],
            closed: true,
        };
        assert!((bar.length() - 0.8).abs() < 1e-12);
    }
}
