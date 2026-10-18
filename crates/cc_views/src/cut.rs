//! Fills of cut regions (plans and sections) from the material's cut pattern.

use crate::{Drawing2D, LineStyle};
use cc_core::{ElementId, Pattern, Project};
use glam::DVec2;

/// Neutral fill of cut regions whose material has no cut pattern.
pub(crate) const POCHE: [u8; 4] = [196, 196, 192, 255];

/// Base spacing of hatch patterns on paper, millimeters.
const PATTERN_MM: f64 = 1.8;

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
        d.fill(poly, POCHE, id);
        return;
    };
    let rgba = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    let base = match (fill.foreground, fill.background) {
        (Pattern::Solid, _) => rgba(fill.foreground_color),
        (_, Pattern::Solid) => rgba(fill.background_color),
        _ => [255, 255, 255, 255],
    };
    d.fill(poly.clone(), base, id);
    for pattern in [fill.background, fill.foreground] {
        if matches!(pattern, Pattern::Solid | Pattern::None) {
            continue;
        }
        for (a, b) in crate::hatch::hatch(pattern, &poly, spacing) {
            d.line(a, b, LineStyle::Thin, None);
        }
    }
}
