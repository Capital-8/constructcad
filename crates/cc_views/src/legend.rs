use crate::{Drawing2D, LineStyle, plan::POCHE};
use cc_core::{ElementTypeId, Project, TypeData};
use glam::DVec2;

/// Legend: one row per type with a 2D symbol and its name.
///
/// Rows are spaced in paper units so the legend reads the same at any scale.
pub fn legend(project: &Project, types: &[ElementTypeId], scale: u32) -> Drawing2D {
    let mut d = Drawing2D::default();
    let mm = scale as f64 / 1000.0; // one paper millimeter in model meters
    let row = 16.0 * mm;
    let sample = 1.2_f64.max(22.0 * mm);
    let mut y = 0.0;
    for t in types.iter().filter_map(|id| project.element_type(*id)).filter(|t| t.category().is_model()) {
        // Door swings extend above the row; leave room for them.
        let above = match t.data {
            TypeData::Door { width, .. } => width + 0.075,
            _ => 0.0,
        };
        y -= above;
        let o = DVec2::new(0.0, y);
        match &t.data {
            TypeData::Wall { layers, .. } | TypeData::Floor { layers } | TypeData::Roof { layers } => {
                let total: f64 = layers.iter().map(|l| l.thickness).sum();
                let mut v = total * 0.5;
                for layer in layers {
                    let c = project.material_color(&layer.material);
                    let rect = vec![
                        o + DVec2::new(0.0, v - layer.thickness),
                        o + DVec2::new(sample, v - layer.thickness),
                        o + DVec2::new(sample, v),
                        o + DVec2::new(0.0, v),
                    ];
                    d.fill(rect.clone(), [c[0], c[1], c[2], 255], None);
                    d.polyline(&rect, true, LineStyle::Thin, None);
                    v -= layer.thickness;
                }
                let h = total * 0.5;
                d.line(o + DVec2::new(0.0, h), o + DVec2::new(sample, h), LineStyle::Cut, None);
                d.line(o + DVec2::new(0.0, -h), o + DVec2::new(sample, -h), LineStyle::Cut, None);
            }
            TypeData::Door { width, .. } => {
                let h = 0.075;
                let jamb = |d: &mut Drawing2D, x0: f64, x1: f64| {
                    let r =
                        [o + DVec2::new(x0, -h), o + DVec2::new(x1, -h), o + DVec2::new(x1, h), o + DVec2::new(x0, h)];
                    d.fill(r.to_vec(), POCHE, None);
                    d.polyline(&r, true, LineStyle::Cut, None);
                };
                jamb(&mut d, 0.0, 0.2);
                jamb(&mut d, 0.2 + width, 0.4 + width);
                let hinge = o + DVec2::new(0.2, h);
                d.line(hinge, hinge + DVec2::Y * *width, LineStyle::Projection, None);
                d.arc(hinge, *width, std::f64::consts::FRAC_PI_2, 0.0, LineStyle::Thin, None);
            }
            TypeData::Window { width, .. } => {
                let h = 0.075;
                for x in [0.0, 0.2 + width] {
                    let r = [
                        o + DVec2::new(x, -h),
                        o + DVec2::new(x + 0.2, -h),
                        o + DVec2::new(x + 0.2, h),
                        o + DVec2::new(x, h),
                    ];
                    d.fill(r.to_vec(), POCHE, None);
                    d.polyline(&r, true, LineStyle::Cut, None);
                }
                for v in [-h, h] {
                    d.line(o + DVec2::new(0.2, v), o + DVec2::new(0.2 + width, v), LineStyle::Projection, None);
                }
                for v in [-0.01, 0.01] {
                    d.line(o + DVec2::new(0.2, v), o + DVec2::new(0.2 + width, v), LineStyle::Thin, None);
                }
            }
            TypeData::Ceiling { thickness, .. } => {
                let r = [
                    o,
                    o + DVec2::new(sample, 0.0),
                    o + DVec2::new(sample, *thickness),
                    o + DVec2::new(0.0, *thickness),
                ];
                d.polyline(&r, true, LineStyle::Projection, None);
            }
            // Filtered out above: annotation types have no legend symbol.
            TypeData::Room {} | TypeData::Grid { .. } | TypeData::Dimension { .. } => {}
        }
        let text_x = sample.max(t.opening_size().map_or(0.0, |(w, _)| w + 0.4)) + 6.0 * mm;
        d.text(DVec2::new(text_x, y - 1.2 * mm), &t.name, 2.5);
        y -= row;
    }
    d
}
