//! Interface icons: SVG drawings in `assets/icons/ui` (24 x 24 grid, white
//! strokes and fills), rasterized with resvg at the exact pixel size they
//! are shown at and tinted with the theme color. Textures are cached per
//! icon and size, so each one is rasterized once.

use egui::{Color32, ColorImage, Context, Painter, Rect, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    New,
    Open,
    Save,
    SaveAs,
    Export,
    About,
    Wall,
    Door,
    Window,
    Floor,
    Ceiling,
    Roof,
    Select,
    Copy,
    Delete,
    Undo,
    Redo,
    FloorPlan,
    CeilingPlan,
    View3D,
    Section,
    Elevation,
    Legend,
    Schedule,
    Sheet,
    ZoomFit,
    ProjectInfo,
    Levels,
    Materials,
    ChevronDown,
    ChevronRight,
    Plus,
    Home,
    ArrowLeft,
    CloseProject,
    Close,
    Room,
    Grid,
    Dimension,
    Column,
    Beam,
    Stair,
    Railing,
    Component,
    Move,
    Rotate,
    MirrorPick,
    MirrorDraw,
    Array,
    TrimCorner,
    TrimExtend,
    Split,
    Offset,
    Align,
    Pin,
    Unpin,
    Join,
    Cut,
    Paste,
    MatchType,
    DimLinear,
    DimAngular,
    DimRadial,
    DimDiameter,
}

impl Icon {
    #[cfg(test)]
    pub const ALL: [Icon; 64] = [
        Icon::New,
        Icon::Open,
        Icon::Save,
        Icon::SaveAs,
        Icon::Export,
        Icon::About,
        Icon::Wall,
        Icon::Door,
        Icon::Window,
        Icon::Floor,
        Icon::Ceiling,
        Icon::Roof,
        Icon::Select,
        Icon::Copy,
        Icon::Delete,
        Icon::Undo,
        Icon::Redo,
        Icon::FloorPlan,
        Icon::CeilingPlan,
        Icon::View3D,
        Icon::Section,
        Icon::Elevation,
        Icon::Legend,
        Icon::Schedule,
        Icon::Sheet,
        Icon::ZoomFit,
        Icon::ProjectInfo,
        Icon::Levels,
        Icon::Materials,
        Icon::ChevronDown,
        Icon::ChevronRight,
        Icon::Plus,
        Icon::Home,
        Icon::ArrowLeft,
        Icon::CloseProject,
        Icon::Close,
        Icon::Room,
        Icon::Grid,
        Icon::Dimension,
        Icon::Column,
        Icon::Beam,
        Icon::Stair,
        Icon::Railing,
        Icon::Component,
        Icon::Move,
        Icon::Rotate,
        Icon::MirrorPick,
        Icon::MirrorDraw,
        Icon::Array,
        Icon::TrimCorner,
        Icon::TrimExtend,
        Icon::Split,
        Icon::Offset,
        Icon::Align,
        Icon::Pin,
        Icon::Unpin,
        Icon::Join,
        Icon::Cut,
        Icon::Paste,
        Icon::MatchType,
        Icon::DimLinear,
        Icon::DimAngular,
        Icon::DimRadial,
        Icon::DimDiameter,
    ];

    /// The icon's SVG source.
    fn source(self) -> &'static str {
        match self {
            Icon::New => include_str!("../../../assets/icons/ui/new.svg"),
            Icon::Open => include_str!("../../../assets/icons/ui/open.svg"),
            Icon::Save => include_str!("../../../assets/icons/ui/save.svg"),
            Icon::SaveAs => include_str!("../../../assets/icons/ui/save-as.svg"),
            Icon::Export => include_str!("../../../assets/icons/ui/export.svg"),
            Icon::About => include_str!("../../../assets/icons/ui/about.svg"),
            Icon::Wall => include_str!("../../../assets/icons/ui/wall.svg"),
            Icon::Door => include_str!("../../../assets/icons/ui/door.svg"),
            Icon::Window => include_str!("../../../assets/icons/ui/window.svg"),
            Icon::Floor => include_str!("../../../assets/icons/ui/floor.svg"),
            Icon::Ceiling => include_str!("../../../assets/icons/ui/ceiling.svg"),
            Icon::Roof => include_str!("../../../assets/icons/ui/roof.svg"),
            Icon::Select => include_str!("../../../assets/icons/ui/select.svg"),
            Icon::Copy => include_str!("../../../assets/icons/ui/copy.svg"),
            Icon::Delete => include_str!("../../../assets/icons/ui/delete.svg"),
            Icon::Undo => include_str!("../../../assets/icons/ui/undo.svg"),
            Icon::Redo => include_str!("../../../assets/icons/ui/redo.svg"),
            Icon::FloorPlan => include_str!("../../../assets/icons/ui/floor-plan.svg"),
            Icon::CeilingPlan => include_str!("../../../assets/icons/ui/ceiling-plan.svg"),
            Icon::View3D => include_str!("../../../assets/icons/ui/view-3d.svg"),
            Icon::Section => include_str!("../../../assets/icons/ui/section.svg"),
            Icon::Elevation => include_str!("../../../assets/icons/ui/elevation.svg"),
            Icon::Legend => include_str!("../../../assets/icons/ui/legend.svg"),
            Icon::Schedule => include_str!("../../../assets/icons/ui/schedule.svg"),
            Icon::Sheet => include_str!("../../../assets/icons/ui/sheet.svg"),
            Icon::ZoomFit => include_str!("../../../assets/icons/ui/zoom-fit.svg"),
            Icon::ProjectInfo => include_str!("../../../assets/icons/ui/project-info.svg"),
            Icon::Levels => include_str!("../../../assets/icons/ui/levels.svg"),
            Icon::Materials => include_str!("../../../assets/icons/ui/materials.svg"),
            Icon::ChevronDown => include_str!("../../../assets/icons/ui/chevron-down.svg"),
            Icon::ChevronRight => include_str!("../../../assets/icons/ui/chevron-right.svg"),
            Icon::Plus => include_str!("../../../assets/icons/ui/plus.svg"),
            Icon::Home => include_str!("../../../assets/icons/ui/home.svg"),
            Icon::ArrowLeft => include_str!("../../../assets/icons/ui/arrow-left.svg"),
            Icon::CloseProject => include_str!("../../../assets/icons/ui/close-project.svg"),
            Icon::Close => include_str!("../../../assets/icons/ui/close.svg"),
            Icon::Room => include_str!("../../../assets/icons/ui/room.svg"),
            Icon::Grid => include_str!("../../../assets/icons/ui/grid.svg"),
            Icon::Dimension => include_str!("../../../assets/icons/ui/dimension.svg"),
            Icon::Column => include_str!("../../../assets/icons/ui/column.svg"),
            Icon::Beam => include_str!("../../../assets/icons/ui/beam.svg"),
            Icon::Stair => include_str!("../../../assets/icons/ui/stair.svg"),
            Icon::Railing => include_str!("../../../assets/icons/ui/railing.svg"),
            Icon::Component => include_str!("../../../assets/icons/ui/component.svg"),
            Icon::Move => include_str!("../../../assets/icons/ui/move.svg"),
            Icon::Rotate => include_str!("../../../assets/icons/ui/rotate.svg"),
            Icon::MirrorPick => include_str!("../../../assets/icons/ui/mirror-pick.svg"),
            Icon::MirrorDraw => include_str!("../../../assets/icons/ui/mirror-draw.svg"),
            Icon::Array => include_str!("../../../assets/icons/ui/array.svg"),
            Icon::TrimCorner => include_str!("../../../assets/icons/ui/trim-corner.svg"),
            Icon::TrimExtend => include_str!("../../../assets/icons/ui/trim-extend.svg"),
            Icon::Split => include_str!("../../../assets/icons/ui/split.svg"),
            Icon::Offset => include_str!("../../../assets/icons/ui/offset.svg"),
            Icon::Align => include_str!("../../../assets/icons/ui/align.svg"),
            Icon::Pin => include_str!("../../../assets/icons/ui/pin.svg"),
            Icon::Unpin => include_str!("../../../assets/icons/ui/unpin.svg"),
            Icon::Join => include_str!("../../../assets/icons/ui/join.svg"),
            Icon::Cut => include_str!("../../../assets/icons/ui/cut.svg"),
            Icon::Paste => include_str!("../../../assets/icons/ui/paste.svg"),
            Icon::MatchType => include_str!("../../../assets/icons/ui/match-type.svg"),
            Icon::DimLinear => include_str!("../../../assets/icons/ui/dim-linear.svg"),
            Icon::DimAngular => include_str!("../../../assets/icons/ui/dim-angular.svg"),
            Icon::DimRadial => include_str!("../../../assets/icons/ui/dim-radial.svg"),
            Icon::DimDiameter => include_str!("../../../assets/icons/ui/dim-diameter.svg"),
        }
    }
}

/// Rasterized icons by (icon, side in physical pixels); `None` when an
/// icon failed to rasterize, so it is not retried every frame.
#[derive(Clone, Default)]
struct Cache(Arc<std::sync::Mutex<Textures>>);

type Textures = HashMap<(Icon, u32), Option<TextureHandle>>;

/// Paint `icon` centered in `rect` (the largest square that fits), tinted with `color`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let ppp = painter.pixels_per_point();
    let side = rect.width().min(rect.height());
    // Whole physical pixels, placed on the pixel grid, keep the strokes crisp.
    let px = (side * ppp).round().clamp(1.0, 512.0);
    let size = px / ppp;
    let min = ((rect.center() - egui::vec2(size, size) / 2.0) * ppp).round() / ppp;
    let target = Rect::from_min_size(min, egui::vec2(size, size));
    let Some(texture) = texture(painter.ctx(), icon, px as u32) else { return };
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(texture.id(), target, uv, color);
}

fn texture(ctx: &Context, icon: Icon, px: u32) -> Option<TextureHandle> {
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(egui::Id::new("icon-cache")).clone());
    let mut map = cache.0.lock().ok()?;
    map.entry((icon, px))
        .or_insert_with(|| {
            let image = rasterize(icon, px);
            if image.is_none() {
                log::warn!("icon {icon:?} failed to rasterize");
            }
            image.map(|image| ctx.load_texture(format!("icon-{icon:?}-{px}"), image, TextureOptions::LINEAR))
        })
        .clone()
}

/// Render an icon's SVG into a `px` x `px` image (premultiplied alpha).
fn rasterize(icon: Icon, px: u32) -> Option<ColorImage> {
    let tree = resvg::usvg::Tree::from_str(icon.source(), &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let side = px as usize;
    Some(ColorImage::from_rgba_premultiplied([side, side], pixmap.data()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_rasterizes() {
        for icon in Icon::ALL {
            let image = rasterize(icon, 20).unwrap_or_else(|| panic!("{icon:?} does not rasterize"));
            let inked = image.pixels.iter().filter(|p| p.a() > 0).count();
            assert!(inked > 20, "{icon:?} is (nearly) empty");
            // Strokes stay inside the icon, with a margin.
            let edge = image.pixels.iter().enumerate().filter(|(i, p)| {
                let (x, y) = (i % 20, i / 20);
                p.a() > 128 && (x == 0 || y == 0 || x == 19 || y == 19)
            });
            assert_eq!(edge.count(), 0, "{icon:?} touches the border");
        }
    }

    /// `ICON_SHEET=path.png cargo test -p cc_app icon_sheet` writes every
    /// icon at a few sizes, dark on light, for review.
    #[test]
    fn icon_sheet() {
        let Ok(path) = std::env::var("ICON_SHEET") else { return };
        let sizes = [16u32, 20, 24, 32, 48];
        let cell = 56usize;
        let cols = Icon::ALL.len();
        let (w, h) = (cols * cell, sizes.len() * cell);
        let mut rgba = vec![248u8; w * h * 4];
        for (row, &px) in sizes.iter().enumerate() {
            for (col, icon) in Icon::ALL.iter().enumerate() {
                let image = rasterize(*icon, px).unwrap();
                for (i, p) in image.pixels.iter().enumerate() {
                    let (x, y) = (col * cell + 4 + i % px as usize, row * cell + 4 + i / px as usize);
                    let a = p.a() as f32 / 255.0;
                    let o = (y * w + x) * 4;
                    for c in 0..3 {
                        rgba[o + c] = (248.0 * (1.0 - a) + 40.0 * a) as u8;
                    }
                }
            }
        }
        let file = std::fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    }
}
