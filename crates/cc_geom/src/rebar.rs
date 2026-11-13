//! Reinforcing bars: the center lines of each set of bars in its beam or
//! column, from the host's body, the cover and the bar diameters.
//!
//! Longitudinal bars sit inside the stirrups (cover plus the host's largest
//! stirrup diameter); stirrups are closed loops `cover` inside the faces,
//! spaced evenly and centered along the host. Along an arc beam the bars
//! follow the arc, each at its own radius, and the stirrups are radial. Top
//! and bottom bars that do not fit side by side go in further layers, side
//! bars run between them, and bars longer than the stock length are spliced.

use crate::BeamGeom;
use cc_core::{BarFace, ElementId, ElementKind, Project, RebarLayout, Splice, TypeData};
use glam::{DVec2, DVec3};
use std::collections::BTreeMap;

/// Density of steel, kg/m³.
pub const STEEL_DENSITY: f64 = 7850.0;

/// Segments of round ties.
const ROUND: usize = 16;

/// At most this many stirrups per set (a crafted spacing cannot explode).
const MAX_STIRRUPS: usize = 2000;

/// At most this many layers of top or bottom bars; the rest crowd the last.
const MAX_LAYERS: usize = 4;

/// At most this many bars in one set, splice pieces included.
const MAX_BARS: usize = 20_000;

/// Least clear distance between parallel bars: the bar diameter, and 20 mm.
fn clear_distance(d: f64) -> f64 {
    d.max(0.02)
}

/// One bar: its center line, closed for stirrups.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub points: Vec<DVec3>,
    pub closed: bool,
    /// The bar's true length when its center line is drawn with chords (a
    /// bar following an arc): the arc's length at the bar's radius.
    pub true_length: Option<f64>,
}

impl Bar {
    /// A bar along straight segments.
    pub fn new(points: Vec<DVec3>, closed: bool) -> Self {
        Self { points, closed, true_length: None }
    }

    pub fn length(&self) -> f64 {
        if let Some(length) = self.true_length {
            return length;
        }
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
    /// The beam or column the set is laid out in (none for shaped bars).
    pub host: Option<ElementId>,
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
                beam_bars(g, *height, r.layout, r.cover, d, ds, r.splice)
            }
            ElementKind::Column(c) => {
                let Some(data @ TypeData::Column { section, .. }) = project.element_type(host.type_id).map(|t| &t.data)
                else {
                    continue;
                };
                let (base, top) = project.column_span(host, c);
                let shape = (section.size(), project.section_is_round(data));
                column_bars(shape, c.position, c.rotation_deg, (base, top), r.layout, r.cover, d, ds)
            }
            _ => continue,
        };
        out.insert(e.id, RebarGeom { element: e.id, host: Some(r.host), diameter: d, bars });
    }
    // Shaped bars stand where they are drawn.
    for e in project.elements.values() {
        let ElementKind::ShapedRebar(r) = &e.kind else { continue };
        let Some(d) = diameter(project, e) else { continue };
        let z = project.level_elevation(e.level);
        let bars = r.bars.iter().filter_map(|b| shaped_bar(b, z)).collect();
        out.insert(e.id, RebarGeom { element: e.id, host: None, diameter: d, bars });
    }
    out
}

/// A shaped bar in the model, `z` up (its level's elevation): closed when its
/// ends meet (the last point dropped); `None` with less than two points apart.
fn shaped_bar(points: &[DVec3], z: f64) -> Option<Bar> {
    let mut points: Vec<DVec3> = points.iter().filter(|p| p.is_finite()).map(|p| *p + DVec3::Z * z).collect();
    points.dedup_by(|a, b| a.distance(*b) < 1e-9);
    let closed = points.len() > 3 && points.first().zip(points.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
    if closed {
        points.pop();
    }
    (points.len() >= 2).then_some(Bar::new(points, closed))
}

/// How many bars go in each layer: `count` bars `d` thick across a width of
/// `across` between the outermost bar centers. Bars that do not fit side by
/// side at the least clear distance go in further layers, the last of
/// [`MAX_LAYERS`] taking what is left.
fn layers(count: usize, across: f64, d: f64) -> Vec<usize> {
    let pitch = d + clear_distance(d);
    let fit = if across.is_finite() && pitch > 0.0 { (across.max(0.0) / pitch).floor() as usize + 1 } else { 1 };
    // Two corner bars always share a layer.
    let fit = fit.max(2);
    let mut out = Vec::new();
    let mut left = count;
    while left > 0 {
        let n = if out.len() + 1 == MAX_LAYERS { left } else { left.min(fit) };
        out.push(n);
        left -= n;
    }
    out
}

/// Heights of side bars between `lo` and `hi` (the lowest and highest bar
/// lines of the section): `count` evenly spread, or with a count of 0 as
/// many as keep them at most `spacing` apart.
fn side_heights(count: u32, spacing: f64, lo: f64, hi: f64) -> Vec<f64> {
    if hi <= lo {
        return Vec::new();
    }
    let n = if count > 0 {
        count as usize
    } else if spacing.is_finite() && spacing > 0.0 {
        (((hi - lo) / spacing) - 1e-9).ceil().max(1.0) as usize - 1
    } else {
        0
    };
    let n = n.min(200);
    (1..=n).map(|i| lo + (hi - lo) * i as f64 / (n + 1) as f64).collect()
}

/// Bars along a beam: longitudinal and side bars from end to end inside the
/// cover (following the arc of an arc beam), stirrups in vertical planes
/// across it (radial on an arc).
fn beam_bars(
    g: &BeamGeom,
    height: f64,
    layout: RebarLayout,
    cover: f64,
    d: f64,
    ds: f64,
    splice: Option<Splice>,
) -> Vec<Bar> {
    let k = g.stretch();
    let u0 = g.start_ext.0.max(g.start_ext.1) + cover;
    let u1 = g.end_ext.0.min(g.end_ext.1) - cover;
    if u1 <= u0 {
        return Vec::new();
    }
    let at = |u: f64, v: f64, z: f64| g.world(u, v).extend(g.z_at(u).0 + z * k);
    // The bars along the lines `(v, z)` of the section, spliced where longer than the stock.
    let runs = |lines: Vec<(f64, f64)>| {
        let mut bars = Vec::new();
        for (index, (v, z)) in lines.into_iter().enumerate() {
            // Length of the bar per meter of the axis: its own radius on an arc, the slope.
            let scale = g.arc.as_ref().map_or(k, |a| (a.radius_at(v) / a.radius.max(1e-9)).max(1e-9));
            let length = (u1 - u0) * scale;
            let pieces = splice.map_or_else(|| vec![(0.0, length)], |s| s.pieces(length, index));
            for (from, to) in pieces {
                if bars.len() >= MAX_BARS {
                    return bars;
                }
                let (a, b) = (u0 + from / scale, u0 + to / scale);
                bars.push(match &g.arc {
                    Some(arc) => {
                        let n = cc_core::arc::segments((b - a) / arc.radius.max(1e-9), cc_core::arc::ARC_STEP);
                        let points = (0..=n).map(|i| at(a + (b - a) * i as f64 / n as f64, v, z)).collect();
                        Bar { points, closed: false, true_length: Some(to - from) }
                    }
                    None => Bar::new(vec![at(a, v, z), at(b, v, z)], false),
                });
            }
        }
        bars
    };
    let inset = cover + ds + d / 2.0;
    match layout {
        RebarLayout::Longitudinal { face, count } => {
            let half = (g.half - inset).max(0.0);
            let step = d + clear_distance(d);
            let mut lines = Vec::new();
            for (layer, n) in layers(count as usize, 2.0 * half, d).into_iter().enumerate() {
                let z = match face {
                    BarFace::Top => (height - inset - step * layer as f64).max(inset),
                    BarFace::Bottom | BarFace::Perimeter => {
                        (inset + step * layer as f64).min((height - inset).max(inset))
                    }
                };
                lines.extend(spread(n, half).into_iter().map(|v| (v, z)));
            }
            runs(lines)
        }
        RebarLayout::Sides { count, spacing } => {
            let v = (g.half - inset).max(0.0);
            let heights = side_heights(count, spacing, inset, height - inset);
            runs(heights.into_iter().flat_map(|z| [(-v, z), (v, z)]).collect())
        }
        RebarLayout::Stirrups { spacing } => {
            let inset = cover + d / 2.0;
            let (v, z0, z1) = ((g.half - inset).max(0.0), inset, (height - inset).max(inset));
            stations(u0, u1, spacing)
                .into_iter()
                .map(|u| Bar::new(vec![at(u, -v, z0), at(u, v, z0), at(u, v, z1), at(u, -v, z1)], true))
                .collect()
        }
    }
}

/// Bars of a column: vertical bars around the section over its height,
/// horizontal ties up the column.
#[allow(clippy::too_many_arguments)]
fn column_bars(
    ((w, depth), round): ((f64, f64), bool),
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
    match layout {
        RebarLayout::Longitudinal { count, .. } => {
            let inset = cover + ds + d / 2.0;
            let plan = if round {
                circle(count as usize, (w / 2.0 - inset).max(0.0))
            } else {
                perimeter(count as usize, (w / 2.0 - inset).max(0.0), (depth / 2.0 - inset).max(0.0))
            };
            plan.into_iter().map(|p| Bar::new(vec![place(p, base), place(p, top)], false)).collect()
        }
        // Side bars belong to beams.
        RebarLayout::Sides { .. } => Vec::new(),
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
                .map(|z| Bar::new(outline.iter().map(|p| place(*p, z)).collect(), true))
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
    fn bars_that_do_not_fit_take_a_second_layer() {
        // 0.14 m between the outer bar centers, 16 mm bars 36 mm apart: four fit.
        assert_eq!(layers(4, 0.14, 0.016), vec![4]);
        assert_eq!(layers(6, 0.14, 0.016), vec![4, 2]);
        // Two corner bars share a layer however narrow the beam.
        assert_eq!(layers(3, 0.01, 0.016), vec![2, 1]);
        // The last layer takes what is left.
        assert_eq!(layers(11, 0.01, 0.016), vec![2, 2, 2, 5]);
        assert!(layers(0, 0.14, 0.016).is_empty());
    }

    #[test]
    fn side_bars_come_from_a_count_or_a_spacing() {
        let h = side_heights(2, 0.0, 0.05, 0.65);
        assert!((h[0] - 0.25).abs() < 1e-12 && (h[1] - 0.45).abs() < 1e-12, "{h:?}");
        // 0.60 m at most 0.20 apart: two bars, 0.20 from each other and the ends.
        assert_eq!(side_heights(0, 0.2, 0.05, 0.65).len(), 2);
        assert_eq!(side_heights(0, 0.25, 0.05, 0.65).len(), 2);
        assert!(side_heights(0, 0.6, 0.05, 0.65).is_empty());
        assert!(side_heights(3, 0.0, 0.5, 0.5).is_empty());
    }

    #[test]
    fn long_bars_are_cut_in_the_fewest_pieces_and_staggered() {
        let s = Splice { max_length: 12.0, lap: 0.5 };
        // Within the stock: one piece, whatever the bar.
        assert_eq!(s.pieces(12.0, 0), vec![(0.0, 12.0)]);
        assert_eq!(s.pieces(8.0, 1), vec![(0.0, 8.0)]);
        // 23.5 m is exactly two pieces of 12 m lapping 0.5 m.
        let two = s.pieces(23.5, 0);
        assert_eq!(two.len(), 2);
        assert!((two[0].1 - 12.0).abs() < 1e-9 && (two[1].0 - 11.5).abs() < 1e-9 && (two[1].1 - 23.5).abs() < 1e-9);
        // A little more takes three, each as long as the others.
        let three = s.pieces(23.6, 0);
        assert_eq!(three.len(), 3);
        let lengths: Vec<f64> = three.iter().map(|(a, b)| b - a).collect();
        assert!(lengths.iter().all(|l| (l - lengths[0]).abs() < 1e-9 && *l <= 12.0), "{lengths:?}");
        assert!((lengths.iter().sum::<f64>() - (23.6 + 2.0 * 0.5)).abs() < 1e-9);
        // The next bar is cut half a piece off: one more piece, short ones at both ends.
        let odd = s.pieces(23.6, 1);
        assert_eq!(odd.len(), 4);
        assert!((odd[0].0).abs() < 1e-12 && (odd[3].1 - 23.6).abs() < 1e-9);
        assert!(odd.windows(2).all(|w| (w[0].1 - w[1].0 - 0.5).abs() < 1e-9), "{odd:?}");
        assert!(((odd[0].1 - odd[0].0) - (odd[3].1 - odd[3].0)).abs() < 1e-9);
        // Values a file may hold: no splices rather than a loop.
        assert_eq!(Splice { max_length: 0.0, lap: 0.5 }.pieces(30.0, 0), vec![(0.0, 30.0)]);
        assert_eq!(Splice { max_length: 12.0, lap: 12.0 }.pieces(30.0, 0), vec![(0.0, 30.0)]);
        assert_eq!(s.pieces(f64::NAN, 0).len(), 1);
        assert!(s.pieces(1e9, 0).len() <= 10_001);
    }

    #[test]
    fn a_stirrup_is_a_closed_loop() {
        let bar = Bar::new(
            vec![DVec3::ZERO, DVec3::new(0.1, 0.0, 0.0), DVec3::new(0.1, 0.0, 0.3), DVec3::new(0.0, 0.0, 0.3)],
            true,
        );
        assert!((bar.length() - 0.8).abs() < 1e-12);
    }
}
