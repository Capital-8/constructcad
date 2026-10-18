//! 2D view canvas: plans, ceilings, elevations, sections and legends.

use crate::icons::{self, Icon};
use crate::paint::{PaintOptions, Xf, paint_drawing};
use crate::state::AppState;
use crate::theme::{self, PALETTE};
use cc_core::ViewId;
use egui::{Rect, Response, Sense, Ui};
use glam::DVec2;

/// Pan/zoom state of a 2D view.
#[derive(Debug, Clone, Copy)]
pub struct View2DState {
    /// Drawing coordinates shown at the canvas center.
    pub center: DVec2,
    /// Screen points per drawing unit.
    pub ppm: f64,
    /// Whether the view has been zoomed to fit at least once (or after "zoom extents").
    pub fitted: bool,
}

impl Default for View2DState {
    fn default() -> Self {
        Self { center: DVec2::ZERO, ppm: 40.0, fitted: false }
    }
}

impl View2DState {
    pub fn xf(&self, rect: Rect) -> Xf {
        Xf { screen: rect.center(), origin: self.center, scale: self.ppm }
    }

    pub fn fit(&mut self, (lo, hi): (DVec2, DVec2), rect: Rect) {
        let size = (hi - lo).max(DVec2::splat(1e-3));
        let margin = 0.85;
        self.ppm = (rect.width() as f64 / size.x).min(rect.height() as f64 / size.y) * margin;
        self.ppm = self.ppm.clamp(1e-3, 1e6);
        self.center = (lo + hi) * 0.5;
        self.fitted = true;
    }

    /// Scroll-wheel zoom around the cursor and middle-button pan.
    pub fn navigate(&mut self, ui: &Ui, resp: &Response, rect: Rect) {
        if resp.hovered() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll as f64 * 0.0025).exp() * pinch as f64;
            if (factor - 1.0).abs() > 1e-6
                && let Some(cursor) = resp.hover_pos()
            {
                let before = self.xf(rect).to_world(cursor);
                self.ppm = (self.ppm * factor).clamp(1e-3, 1e6);
                let after = self.xf(rect).to_world(cursor);
                self.center += before - after;
            }
        }
        if resp.dragged_by(egui::PointerButton::Middle) {
            let d = resp.drag_delta();
            self.center += DVec2::new(-d.x as f64, d.y as f64) / self.ppm;
        }
    }
}

/// Show a graphical 2D view and run the active tool on it.
pub fn show(ui: &mut Ui, state: &mut AppState, view_id: ViewId) {
    let Some(view) = state.project().view(view_id).cloned() else { return };
    let drawing = state.drawing(view_id).unwrap_or_default();
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    painter.rect_filled(rect, 0.0, PALETTE.canvas);

    let st = state.view2d.entry(view_id).or_default();
    if !st.fitted {
        match drawing.bounds() {
            Some(b) if b.1.distance(b.0) > 0.5 => st.fit(b, rect),
            _ => {
                st.center = DVec2::new(5.0, 4.0);
                st.fitted = true;
            }
        }
    }
    st.navigate(ui, &resp, rect);
    let xf = st.xf(rect);
    let px_per_mm = st.ppm * view.scale as f64 / 1000.0;

    let opts = PaintOptions { highlight: &state.selection, px_per_mm, mono: None, only: None };
    paint_drawing(&painter, &drawing, &xf, &opts);

    if let Some(p) = resp.hover_pos() {
        state.cursor = Some(xf.to_world(p));
    }
    crate::tools::handle(ui, &resp, &painter, state, &view, &xf, &drawing);
    if view_toolbar(ui, rect, view_id, view.scale)
        && let Some(st) = state.view2d.get_mut(&view_id)
    {
        st.fitted = false;
    }
}

/// Floating chip in the view corner with the scale and a zoom-to-fit button.
/// Returns true when zoom to fit was clicked.
pub fn view_toolbar(ui: &mut Ui, rect: Rect, view_id: ViewId, scale: u32) -> bool {
    let painter = ui.painter_at(rect);
    let size = egui::vec2(96.0, 28.0);
    let bar = Rect::from_min_size(rect.right_bottom() - size - egui::vec2(12.0, 12.0), size);
    painter.rect_filled(bar.translate(egui::vec2(0.0, 1.0)), 7.0, egui::Color32::from_black_alpha(18));
    painter.rect_filled(bar, 7.0, PALETTE.panel);
    painter.rect_stroke(bar, 7.0, egui::Stroke::new(1.0, PALETTE.border), egui::StrokeKind::Inside);
    let label = egui::pos2(bar.left() + 12.0, bar.center().y);
    let font = egui::FontId::new(12.0, theme::semibold());
    painter.text(label, egui::Align2::LEFT_CENTER, format!("1:{scale}"), font, PALETTE.text_muted);
    let button = Rect::from_center_size(egui::pos2(bar.right() - 16.0, bar.center().y), egui::vec2(24.0, 22.0));
    let r = ui.interact(button, egui::Id::new(("zoom-fit", view_id.0)), Sense::click());
    if r.hovered() {
        painter.rect_filled(button, theme::RADIUS as f32, PALETTE.hover);
    }
    icons::paint(&painter, button.shrink(5.0), Icon::ZoomFit, PALETTE.text_muted);
    r.on_hover_text(format!("{} (ZE)", crate::t!("action-zoom-fit"))).clicked()
}
