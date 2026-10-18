//! Sheet composer: paper, title block and placed viewports.

use crate::paint::{HIGHLIGHT, PaintOptions, Xf, paint_drawing};
use crate::state::AppState;
use crate::t;
use cc_core::{ViewId, ViewKind, Viewport};
use cc_views::Drawing2D;
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, Ui};
use glam::DVec2;
use std::collections::BTreeSet;
use std::rc::Rc;

const TITLE_W: f64 = 170.0;
const TITLE_H: f64 = 42.0;
const MARGIN: f64 = 10.0;

/// A viewport resolved to sheet millimeters.
struct Placed {
    index: usize,
    view: ViewId,
    drawing: Rc<Drawing2D>,
    /// Drawing point that maps to the viewport center.
    anchor: DVec2,
    center: DVec2,
    /// Millimeters per drawing unit.
    k: f64,
    scale: u32,
    half: DVec2,
    title: String,
}

fn place(state: &mut AppState, viewports: &[Viewport]) -> Vec<Placed> {
    viewports
        .iter()
        .enumerate()
        .filter_map(|(index, vp)| {
            let view = state.project().view(vp.view)?.clone();
            let drawing = state.drawing(vp.view)?;
            let (lo, hi) = drawing.bounds().unwrap_or((DVec2::ZERO, DVec2::ONE));
            // 3D views are drawn at a nominal scale; others at their view scale.
            let scale = view.scale.max(1);
            let k = 1000.0 / scale as f64;
            Some(Placed {
                index,
                view: vp.view,
                anchor: (lo + hi) * 0.5,
                center: vp.center,
                k,
                scale,
                half: (hi - lo) * 0.5 * k,
                drawing,
                title: view.name.clone(),
            })
        })
        .collect()
}

pub fn show(ui: &mut Ui, state: &mut AppState, sheet_id: ViewId) {
    let Some(sheet) = state.project().view(sheet_id).cloned() else { return };
    let ViewKind::Sheet { number, size, viewports } = &sheet.kind else { return };
    let paper = size.size_mm();
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    painter.rect_filled(rect, 0.0, crate::theme::PALETTE.sheet_bg);

    let st = state.view2d.entry(sheet_id).or_default();
    if !st.fitted {
        st.fit((DVec2::ZERO, paper), rect);
    }
    st.navigate(ui, &resp, rect);
    let xf = st.xf(rect);
    let to = |p: DVec2| xf.to_screen(p);

    // Paper with shadow, border and title block.
    let paper_rect = Rect::from_two_pos(to(DVec2::ZERO), to(paper));
    painter.rect_filled(paper_rect.translate(egui::vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(60));
    painter.rect_filled(paper_rect, 0.0, Color32::WHITE);
    let border = Rect::from_two_pos(to(DVec2::splat(MARGIN)), to(paper - DVec2::splat(MARGIN)));
    painter.rect_stroke(border, 0.0, Stroke::new(1.5, Color32::BLACK), egui::StrokeKind::Middle);
    title_block(&painter, &xf, state, paper, number, &sheet.name);

    let placed = place(state, viewports);
    for p in &placed {
        let vxf = xf.then_place(p.anchor, p.center, p.k);
        let opts = PaintOptions { highlight: &BTreeSet::new(), px_per_mm: xf.scale, mono: None, only: None };
        let clip = Rect::from_two_pos(
            to(p.center - p.half - DVec2::splat(4.0)),
            to(p.center + p.half + DVec2::new(25.0, 6.0)),
        );
        paint_drawing(&painter.with_clip_rect(clip.intersect(rect)), &p.drawing, &vxf, &opts);
        // Viewport title: name, scale and underline.
        let tpos = p.center + DVec2::new(-p.half.x, -p.half.y - 7.0);
        let px = (3.0 * xf.scale) as f32;
        if px > 3.0 {
            painter.text(to(tpos), Align2::LEFT_BOTTOM, &p.title, FontId::proportional(px), Color32::BLACK);
            let is_3d = matches!(state.project().view(p.view).map(|v| &v.kind), Some(ViewKind::ThreeD { .. }));
            if !is_3d {
                painter.text(
                    to(tpos - DVec2::new(0.0, 4.0)),
                    Align2::LEFT_BOTTOM,
                    format!("1:{}", p.scale),
                    FontId::proportional(px * 0.75),
                    Color32::DARK_GRAY,
                );
            }
        }
        painter.line_segment(
            [to(tpos + DVec2::new(0.0, -0.8)), to(tpos + DVec2::new(p.half.x.min(60.0), -0.8))],
            Stroke::new(1.2, Color32::BLACK),
        );
        if state.sheet_selection == Some((sheet_id, p.index)) {
            painter.rect_stroke(clip, 0.0, Stroke::new(1.5, HIGHLIGHT), egui::StrokeKind::Middle);
        }
    }

    // Select and drag viewports.
    let hit = |pos: egui::Pos2| {
        let w = xf.to_world(pos);
        placed.iter().rev().find(|p| (w - p.center).abs().cmple(p.half + DVec2::splat(4.0)).all()).map(|p| p.index)
    };
    if resp.clicked() {
        state.sheet_selection = resp.interact_pointer_pos().and_then(hit).map(|i| (sheet_id, i));
        state.selection.clear();
    }
    if resp.drag_started_by(egui::PointerButton::Primary) {
        let origin = ui.input(|i| i.pointer.press_origin());
        state.sheet_selection = origin.and_then(hit).map(|i| (sheet_id, i));
    }
    if let Some((sid, index)) = state.sheet_selection {
        if sid == sheet_id && resp.dragged_by(egui::PointerButton::Primary) {
            let d = resp.drag_delta();
            let delta = DVec2::new(d.x as f64, -d.y as f64) / xf.scale;
            let key = sheet_id.0 ^ ((index as u64) << 48);
            state.doc.edit_merged(t!("tx-move-viewport"), key, |e| {
                if let Some(ViewKind::Sheet { viewports, .. }) = e.view_mut(sheet_id).map(|v| &mut v.kind)
                    && let Some(vp) = viewports.get_mut(index)
                {
                    vp.center += delta;
                }
            });
        }
        if resp.drag_stopped() {
            state.doc.break_merge();
        }
    }
    if let Some(p) = resp.hover_pos() {
        state.cursor = Some(xf.to_world(p));
    }
}

fn title_block(painter: &egui::Painter, xf: &Xf, state: &AppState, paper: DVec2, number: &str, name: &str) {
    let to = |p: DVec2| xf.to_screen(p);
    let x1 = paper.x - MARGIN;
    let x0 = x1 - TITLE_W;
    let y0 = MARGIN;
    let y1 = y0 + TITLE_H;
    let stroke = Stroke::new(1.0, Color32::BLACK);
    painter.rect_stroke(
        Rect::from_two_pos(to(DVec2::new(x0, y0)), to(DVec2::new(x1, y1))),
        0.0,
        stroke,
        egui::StrokeKind::Middle,
    );
    for y in [y0 + 14.0, y0 + 28.0] {
        painter.line_segment([to(DVec2::new(x0, y)), to(DVec2::new(x1, y))], stroke);
    }
    painter.line_segment([to(DVec2::new(x1 - 40.0, y0)), to(DVec2::new(x1 - 40.0, y0 + 28.0))], stroke);
    let info = &state.project().info;
    let px = |mm: f64| FontId::proportional((mm * xf.scale) as f32);
    let small = 2.2 * xf.scale;
    let rows = [
        (y1 - 3.0, t!("title-project"), info.name.clone(), 4.0),
        (y0 + 25.0, t!("title-sheet"), name.to_string(), 3.5),
        (y0 + 11.0, t!("title-client"), info.client.clone(), 3.0),
    ];
    for (y, caption, value, size) in rows {
        if small > 1.8 {
            painter.text(to(DVec2::new(x0 + 2.0, y)), Align2::LEFT_TOP, caption, px(2.0), Color32::DARK_GRAY);
            painter.text(to(DVec2::new(x0 + 2.0, y - 3.0)), Align2::LEFT_TOP, value, px(size), Color32::BLACK);
        }
    }
    if small > 1.8 {
        painter.text(
            to(DVec2::new(x1 - 38.0, y0 + 25.0)),
            Align2::LEFT_TOP,
            t!("title-number"),
            px(2.0),
            Color32::DARK_GRAY,
        );
        painter.text(to(DVec2::new(x1 - 38.0, y0 + 21.0)), Align2::LEFT_TOP, number, px(6.0), Color32::BLACK);
        painter.text(
            to(DVec2::new(x1 - 38.0, y0 + 11.0)),
            Align2::LEFT_TOP,
            format!("{} {}", t!("title-project-number"), info.number),
            px(2.2),
            Color32::BLACK,
        );
        painter.text(
            to(DVec2::new(x1 - 38.0, y0 + 6.0)),
            Align2::LEFT_TOP,
            format!("{} {}", t!("title-author"), info.author),
            px(2.2),
            Color32::BLACK,
        );
    }
}
