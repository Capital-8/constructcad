//! Fills of cut regions (plans and sections) from the material's cut pattern.

use crate::{Drawing2D, LineStyle};
use cc_core::{ElementId, Pattern, Project};
use glam::DVec2;

/// Neutral fill of cut regions whose material has no cut pattern.
pub(crate) const POCHE: [u8; 4] = [196, 196, 192, 255];

/// Base spacing of hatch patterns on paper, millimeters.
pub(crate) const PATTERN_MM: f64 = 1.8;

/// Hatch spacing in drawing units for a view at `1:scale`.
pub(crate) fn spacing(scale: u32) -> f64 {
    PATTERN_MM * scale.max(1) as f64 / 1000.0
}

/// Fill `poly` with the cut pattern of `material` (or the neutral poche).
/// A white or colored background keeps the region selectable.
pub(crate) fn cut_fill(
    d: &mut Drawing2D,
    project: &Project,
    material: &str,
    poly: Vec<DVec2>,
    spacing: f64,
    id: Option<ElementId>,
) {
    let Some(fill) = project.material(material).map(|m| m.graphics.cut).filter(|f| !f.is_empty()) else {
        d.cut_fill(poly, POCHE, id);
        return;
    };
    let rgba = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    let base = match (fill.foreground, fill.background) {
        (Pattern::Solid, _) => rgba(fill.foreground_color),
        (_, Pattern::Solid) => rgba(fill.background_color),
        _ => [255, 255, 255, 255],
    };
    d.cut_fill(poly.clone(), base, id);
    for pattern in [fill.background, fill.foreground] {
        if matches!(pattern, Pattern::Solid | Pattern::None) {
            continue;
        }
        for (a, b) in crate::hatch::hatch_in(&project.fill_patterns, pattern, &poly, spacing) {
            // Tagged with the element, so joined geometry can trim them.
            d.line(a, b, LineStyle::Hatch, id);
        }
    }
}

/// [`cut_fill`] of a region with `holes`: one keyhole ring (triangles when
/// no ring can be made), the pattern lines clipped out of the holes.
pub(crate) fn cut_fill_with_holes(
    d: &mut Drawing2D,
    project: &Project,
    material: &str,
    outer: &[DVec2],
    holes: &[Vec<DVec2>],
    spacing: f64,
    id: Option<ElementId>,
) {
    if holes.is_empty() {
        cut_fill(d, project, material, outer.to_vec(), spacing, id);
        return;
    }
    let fill = project.material(material).map(|m| m.graphics.cut).filter(|f| !f.is_empty());
    let rgba = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    let base = match fill.map(|f| (f.foreground, f.background)) {
        None => POCHE,
        Some((Pattern::Solid, _)) => fill.map_or(POCHE, |f| rgba(f.foreground_color)),
        Some((_, Pattern::Solid)) => fill.map_or(POCHE, |f| rgba(f.background_color)),
        _ => [255, 255, 255, 255],
    };
    match cc_geom::polygon::keyhole(outer, holes) {
        Some(ring) => d.cut_fill(ring, base, id),
        None => {
            for t in cc_geom::polygon::triangulate_with_holes(outer, holes) {
                d.cut_fill(t.to_vec(), base, id);
            }
        }
    }
    let Some(fill) = fill else { return };
    for pattern in [fill.background, fill.foreground] {
        if matches!(pattern, Pattern::Solid | Pattern::None) {
            continue;
        }
        for (a, b) in crate::hatch::hatch_with_holes_in(&project.fill_patterns, pattern, outer, holes, spacing) {
            d.line(a, b, LineStyle::Hatch, id);
        }
    }
}
