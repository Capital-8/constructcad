//! 2D view canvas: plans, ceilings, elevations, sections and legends.

use crate::paint::{PaintOptions, Xf, paint_drawing};
use crate::state::AppState;
use cc_core::ViewId;
use egui::{Color32, Rect, Response, Sense, Ui};
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
    painter.rect_filled(rect, 0.0, Color32::WHITE);

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
}
