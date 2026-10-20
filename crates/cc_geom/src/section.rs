//! Cross-sections of columns and beams: the solid parts of a profile (as
//! rectangles or convex quads, which add up to its area exactly) and the
//! outlines to draw.

use cc_core::{ColumnSection, SectionShape};
use glam::DVec2;

/// Segments of round sections.
const ROUND: usize = 24;

/// The solid parts of a profile in its `width` x `depth` box centered at the
/// origin, as `(min, max)` rectangles that do not overlap.
pub fn shape_rects(shape: SectionShape, width: f64, depth: f64) -> Vec<(DVec2, DVec2)> {
    let (w, d) = (width * 0.5, depth * 0.5);
    let r = |x0: f64, y0: f64, x1: f64, y1: f64| (DVec2::new(x0, y0), DVec2::new(x1, y1));
    match shape.fitted(width, depth) {
        SectionShape::Solid => vec![r(-w, -d, w, d)],
        SectionShape::I { web, flange } => {
            vec![r(-w, -d, w, -d + flange), r(-web * 0.5, -d + flange, web * 0.5, d - flange), r(-w, d - flange, w, d)]
        }
        SectionShape::Channel { web, flange } => {
            vec![r(-w, -d, w, -d + flange), r(-w, -d + flange, -w + web, d - flange), r(-w, d - flange, w, d)]
        }
        SectionShape::Angle { thickness: t } => vec![r(-w, -d, -w + t, d), r(-w + t, -d, w, -d + t)],
        SectionShape::Tube { wall: t } => {
            vec![r(-w, -d, w, -d + t), r(-w, d - t, w, d), r(-w, -d + t, -w + t, d - t), r(w - t, -d + t, w, d - t)]
        }
    }
    .into_iter()
    .filter(|(a, b)| b.x - a.x > 1e-9 && b.y - a.y > 1e-9)
    .collect()
}

/// The outlines of a profile (outer boundary, then holes), counter-clockwise.
pub fn shape_loops(shape: SectionShape, width: f64, depth: f64) -> Vec<Vec<DVec2>> {
    let (w, d) = (width * 0.5, depth * 0.5);
    let p = |pts: &[(f64, f64)]| pts.iter().map(|&(x, y)| DVec2::new(x, y)).collect::<Vec<_>>();
    match shape.fitted(width, depth) {
        SectionShape::Solid => vec![p(&[(-w, -d), (w, -d), (w, d), (-w, d)])],
        SectionShape::I { web, flange: f } => {
            let t = web * 0.5;
            vec![p(&[
                (-w, -d),
                (w, -d),
                (w, -d + f),
                (t, -d + f),
                (t, d - f),
                (w, d - f),
                (w, d),
                (-w, d),
                (-w, d - f),
                (-t, d - f),
                (-t, -d + f),
                (-w, -d + f),
            ])]
        }
        SectionShape::Channel { web, flange: f } => vec![p(&[
            (-w, -d),
            (w, -d),
            (w, -d + f),
            (-w + web, -d + f),
            (-w + web, d - f),
            (w, d - f),
            (w, d),
            (-w, d),
        ])],
        SectionShape::Angle { thickness: t } => {
            vec![p(&[(-w, -d), (w, -d), (w, -d + t), (-w + t, -d + t), (-w + t, d), (-w, d)])]
        }
        SectionShape::Tube { wall: t } => vec![
            p(&[(-w, -d), (w, -d), (w, d), (-w, d)]),
            p(&[(-w + t, -d + t), (-w + t, d - t), (w - t, d - t), (w - t, -d + t)]),
        ],
    }
}

fn circle(r: f64) -> Vec<DVec2> {
    (0..ROUND).map(|i| DVec2::from_angle(i as f64 / ROUND as f64 * std::f64::consts::TAU) * r).collect()
}

/// Solid parts of a column section in plan (convex polygons), placed at
/// `position` and turned by `rotation_deg`.
pub fn column_parts(section: ColumnSection, position: DVec2, rotation_deg: f64) -> Vec<Vec<DVec2>> {
    let local: Vec<Vec<DVec2>> = match section {
        ColumnSection::Rectangular { width, depth } => rects(shape_rects(SectionShape::Solid, width, depth)),
        ColumnSection::Profile { width, depth, shape } => rects(shape_rects(shape, width, depth)),
        ColumnSection::Round { diameter } => vec![circle(diameter * 0.5)],
        ColumnSection::Pipe { diameter, wall } => {
            let outer = circle(diameter * 0.5);
            let inner = circle((diameter * 0.5 - wall.clamp(0.0, diameter * 0.5)).max(0.0));
            (0..ROUND).map(|i| vec![outer[i], outer[(i + 1) % ROUND], inner[(i + 1) % ROUND], inner[i]]).collect()
        }
    };
    place(local, position, rotation_deg)
}

/// Outlines of a column section in plan (outer boundary, then holes).
pub fn column_loops(section: ColumnSection, position: DVec2, rotation_deg: f64) -> Vec<Vec<DVec2>> {
    let local = match section {
        ColumnSection::Rectangular { width, depth } => shape_loops(SectionShape::Solid, width, depth),
        ColumnSection::Profile { width, depth, shape } => shape_loops(shape, width, depth),
        ColumnSection::Round { diameter } => vec![circle(diameter * 0.5)],
        ColumnSection::Pipe { diameter, wall } => {
            let inner = (diameter * 0.5 - wall.clamp(0.0, diameter * 0.5)).max(0.0);
            vec![circle(diameter * 0.5), circle(inner).into_iter().rev().collect()]
        }
    };
    place(local, position, rotation_deg)
}

fn rects(rects: Vec<(DVec2, DVec2)>) -> Vec<Vec<DVec2>> {
    rects.into_iter().map(|(a, b)| vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)]).collect()
}

fn place(polys: Vec<Vec<DVec2>>, position: DVec2, rotation_deg: f64) -> Vec<Vec<DVec2>> {
    let rot = DVec2::from_angle(rotation_deg.to_radians());
    polys.into_iter().map(|poly| poly.into_iter().map(|p| position + rot.rotate(p)).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygon::{area, signed_area};

    fn rect_area(rects: &[(DVec2, DVec2)]) -> f64 {
        rects.iter().map(|(a, b)| (b.x - a.x) * (b.y - a.y)).sum()
    }

    fn loops_area(loops: &[Vec<DVec2>]) -> f64 {
        loops.iter().map(|l| signed_area(l)).sum()
    }

    #[test]
    fn parts_and_outlines_have_the_profile_area() {
        let (w, d) = (0.2, 0.3);
        let cases = [
            (SectionShape::Solid, w * d),
            (SectionShape::I { web: 0.01, flange: 0.02 }, 2.0 * w * 0.02 + 0.01 * (d - 0.04)),
            (SectionShape::Channel { web: 0.01, flange: 0.02 }, 2.0 * w * 0.02 + 0.01 * (d - 0.04)),
            (SectionShape::Angle { thickness: 0.02 }, 0.02 * d + (w - 0.02) * 0.02),
            (SectionShape::Tube { wall: 0.01 }, w * d - (w - 0.02) * (d - 0.02)),
        ];
        for (shape, expected) in cases {
            assert!((rect_area(&shape_rects(shape, w, d)) - expected).abs() < 1e-12, "{shape:?} parts");
            assert!((loops_area(&shape_loops(shape, w, d)) - expected).abs() < 1e-12, "{shape:?} outline");
        }
    }

    #[test]
    fn thicknesses_that_do_not_fit_are_clamped() {
        // A web wider than the box fills it; flanges thicker than half the depth meet.
        let rects = shape_rects(SectionShape::I { web: 1.0, flange: 1.0 }, 0.2, 0.3);
        assert!((rect_area(&rects) - 0.2 * 0.3).abs() < 1e-12);
    }

    #[test]
    fn pipes_are_rings() {
        let parts = column_parts(ColumnSection::Pipe { diameter: 0.2, wall: 0.01 }, DVec2::ZERO, 0.0);
        let total: f64 = parts.iter().map(|p| area(p)).sum();
        let ring = |r: f64| 0.5 * ROUND as f64 * r * r * (std::f64::consts::TAU / ROUND as f64).sin();
        assert!((total - (ring(0.1) - ring(0.09))).abs() < 1e-12);
        let loops = column_loops(ColumnSection::Pipe { diameter: 0.2, wall: 0.01 }, DVec2::ZERO, 0.0);
        assert!((loops_area(&loops) - total).abs() < 1e-12);
    }
}
