//! Editing measured values in place. Clicking the text of a selected
//! dimension (or double-clicking a dimension) opens a small field over the
//! view; Enter applies the value, Esc or a click elsewhere cancels.
//!
//! Dimensions store points, not references, so a new value moves what the
//! dimension touches: the elements whose linework passes through its moving
//! end (`b`, or `a` when only that end touches something). Elements crossing
//! the move direction translate; walls, beams and grids along it stretch
//! their nearest end. This works in plans, where drawing coordinates are
//! model coordinates; elsewhere only free dimensions change.

use crate::paint::Xf;
use crate::state::AppState;
use crate::t;
use cc_core::{Category, Dimension, DimensionKind, ElementId, ElementKind, PlanTransform, ViewId, ViewKind};
use cc_views::{Drawing2D, Prim, format};
use egui::{Painter, Pos2, Response, Ui};
use glam::DVec2;
use std::collections::BTreeSet;

/// What an edited value drives.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// The value of a dimension element.
    Dimension(ElementId),
}

/// The open value field.
#[derive(Debug, Clone)]
pub struct ValueEdit {
    target: Target,
    view: ViewId,
    /// Screen point the field is centered on.
    at: Pos2,
    text: String,
    focus: bool,
}

/// Open the value field for `target` at the screen point `at`.
pub fn open(state: &mut AppState, view: ViewId, target: Target, at: Pos2) {
    let text = match &target {
        Target::Dimension(id) => match dimension(state, *id) {
            Some(d) if matches!(d.kind, DimensionKind::Angular { .. }) => format::number(d.value().to_degrees(), 2),
            Some(d) => format::meters(d.value()),
            None => return,
        },
    };
    state.value_edit = Some(ValueEdit { target, view, at, text, focus: true });
}

fn dimension(state: &AppState, id: ElementId) -> Option<Dimension> {
    match state.project().element(id).map(|e| &e.kind) {
        Some(ElementKind::Dimension(d)) => Some(d.clone()),
        _ => None,
    }
}

/// Clicks on dimension text with the Select tool open the value field.
/// Returns true when the pointer was used (the tools skip the frame).
#[allow(clippy::too_many_arguments)]
pub fn canvas(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    view: ViewId,
    xf: &Xf,
    px_per_mm: f64,
    drawing: &Drawing2D,
) -> bool {
    if state.tool != crate::tools::Tool::Select {
        return false;
    }
    let Some(pos) = resp.hover_pos() else { return false };
    // Double-click on any dimension.
    if resp.double_clicked()
        && let Some(hit) = drawing.hit_test(xf.to_world(pos), 5.0 / xf.scale)
        && dimension(state, hit).is_some()
    {
        state.selection = BTreeSet::from([hit]);
        open_at_text(state, view, hit, xf, px_per_mm, painter);
        return true;
    }
    // A click on the text of the selected dimension.
    let [id] = state.selection.iter().copied().collect::<Vec<_>>()[..] else { return false };
    let Some((center, size, angle)) = text_box(state, id, view, xf, px_per_mm, painter) else { return false };
    let local = egui::emath::Rot2::from_angle(angle).inverse() * (pos - center);
    if local.x.abs() > size.x / 2.0 + 4.0 || local.y.abs() > size.y / 2.0 + 4.0 {
        return false;
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| {
        center + egui::emath::Rot2::from_angle(angle) * egui::vec2(x * (size.x / 2.0 + 3.0), y * (size.y / 2.0 + 2.0))
    });
    let accent = crate::theme::PALETTE.accent;
    painter.add(egui::Shape::convex_polygon(
        corners.to_vec(),
        accent.gamma_multiply(0.12),
        egui::Stroke::new(1.0, accent),
    ));
    if resp.clicked() {
        open(state, view, Target::Dimension(id), center);
        return true;
    }
    false
}

fn open_at_text(state: &mut AppState, view: ViewId, id: ElementId, xf: &Xf, px_per_mm: f64, painter: &Painter) {
    let at = text_box(state, id, view, xf, px_per_mm, painter).map(|t| t.0);
    if let Some(at) = at {
        open(state, view, Target::Dimension(id), at);
    }
}

/// Screen center, size and screen rotation of a dimension's text in `view`.
fn text_box(
    state: &AppState,
    id: ElementId,
    view: ViewId,
    xf: &Xf,
    px_per_mm: f64,
    painter: &Painter,
) -> Option<(Pos2, egui::Vec2, f32)> {
    let el = state.project().element(id)?;
    let ElementKind::Dimension(d) = &el.kind else { return None };
    if d.view != view {
        return None;
    }
    let scale = state.project().view(view)?.scale.max(1);
    let layout = cc_views::dimension_layout(d, scale)?;
    let text_size = match state.project().element_type(el.type_id).map(|t| &t.data) {
        Some(cc_core::TypeData::Dimension { text_size }) => *text_size,
        _ => 2.5,
    };
    let px = (text_size * px_per_mm).max(9.0) as f32;
    let size = painter.layout_no_wrap(layout.text, egui::FontId::proportional(px), egui::Color32::BLACK).size();
    // The text sits on its anchor (bottom center); screen Y points down.
    let angle = -layout.text_angle as f32;
    let center = xf.to_screen(layout.text_pos) + egui::emath::Rot2::from_angle(angle) * egui::vec2(0.0, -size.y / 2.0);
    Some((center, size, angle))
}

/// The value field over `view`, when open there.
pub fn show(ui: &Ui, state: &mut AppState, view: ViewId) {
    let Some(edit) = state.value_edit.as_mut().filter(|e| e.view == view) else { return };
    let (mut apply, mut cancel) = (false, false);
    egui::Area::new(egui::Id::new("value-edit"))
        .fixed_pos(edit.at - egui::vec2(48.0, 13.0))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(2)).show(ui, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut edit.text).desired_width(90.0));
                if edit.focus {
                    // Focused with the whole value selected, so typing replaces it.
                    r.request_focus();
                    if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), r.id) {
                        let all = egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(edit.text.chars().count()),
                        );
                        st.cursor.set_char_range(Some(all));
                        st.store(ui.ctx(), r.id);
                        edit.focus = false;
                    }
                }
                if r.lost_focus() {
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply = true;
                    } else {
                        cancel = true;
                    }
                }
            });
        });
    if !(apply || cancel) {
        return;
    }
    let Some(edit) = state.value_edit.take() else { return };
    if apply && let Err(message) = apply_text(state, &edit.target, &edit.text) {
        state.message = message;
    }
}

/// Parse `text` for `target` and apply it.
pub fn apply_text(state: &mut AppState, target: &Target, text: &str) -> Result<(), String> {
    match target {
        Target::Dimension(id) => {
            let angular = dimension(state, *id).is_some_and(|d| matches!(d.kind, DimensionKind::Angular { .. }));
            let value =
                if angular { format::parse_number(text).map(f64::to_radians) } else { format::parse_length(text) };
            set_dimension_value(state, *id, value.ok_or_else(|| t!("msg-invalid-value"))?)
        }
    }
}

/// Elements (not dimensions, not `me`) whose linework in `drawing` passes
/// through `p` but not through `other`.
fn attached(drawing: &Drawing2D, state: &AppState, me: ElementId, p: DVec2, other: DVec2) -> Vec<ElementId> {
    let tol = 1e-3;
    let through = |q: DVec2| -> BTreeSet<ElementId> {
        drawing
            .prims
            .iter()
            .filter_map(|prim| match prim {
                Prim::Line { a, b, element: Some(e), .. } if crate::tools::point_on_segment(*a, *b, q, tol) => Some(*e),
                _ => None,
            })
            .filter(|e| *e != me)
            .filter(|e| state.project().element(*e).is_some_and(|x| x.category() != Category::Dimension))
            .collect()
    };
    let far = through(other);
    through(p).into_iter().filter(|e| !far.contains(e)).collect()
}

/// Give dimension `id` the measured `value` (meters, or radians for angles),
/// moving what its moving end touches (see the module docs).
pub fn set_dimension_value(state: &mut AppState, id: ElementId, value: f64) -> Result<(), String> {
    let dim = dimension(state, id).ok_or_else(|| t!("msg-invalid-value"))?;
    if !value.is_finite() || value <= 1e-6 {
        return Err(t!("msg-invalid-value"));
    }
    let plan = state
        .project()
        .view(dim.view)
        .is_some_and(|v| matches!(v.kind, ViewKind::FloorPlan { .. } | ViewKind::CeilingPlan { .. }));
    let drawing = state.drawing(dim.view).unwrap_or_default();
    // The end that moves, the other one, and the elements to take along.
    let b_side = attached(&drawing, state, id, dim.b, dim.a);
    let a_side = attached(&drawing, state, id, dim.a, dim.b);
    let (at_b, ids) = if !b_side.is_empty() || a_side.is_empty() { (true, b_side) } else { (false, a_side) };
    if !ids.is_empty() && !plan {
        return Err(t!("msg-dim-edit-plan"));
    }
    let (free, pinned): (Vec<_>, Vec<_>) = ids.into_iter().partition(|e| !state.project().is_pinned(*e));
    if free.is_empty() && !pinned.is_empty() {
        return Err(t!("msg-pinned-skipped", "count" => pinned.len()));
    }
    let moving = if at_b { dim.b } else { dim.a };
    match dim.kind {
        DimensionKind::Aligned | DimensionKind::Linear { .. } => {
            let dir = dim.direction().ok_or_else(|| t!("msg-invalid-value"))?;
            let measured = (dim.b - dim.a).dot(dir);
            let target = value * if measured < 0.0 { -1.0 } else { 1.0 };
            let shift = dir * (target - measured) * if at_b { 1.0 } else { -1.0 };
            let result = state.doc.edit(t!("tx-dim-value"), |e| {
                let mut translate = Vec::new();
                let mut result = Ok(());
                for el in &free {
                    let line = e.project().element(*el).and_then(cc_core::wall_ops::line_of);
                    let along =
                        line.and_then(|(s, t)| (t - s).try_normalize()).is_some_and(|u| u.dot(dir).abs() > 0.99);
                    match line {
                        // Along the move: stretch the end at the dimension point.
                        Some((s, t)) if along => {
                            let at_start = s.distance(moving) <= t.distance(moving);
                            let end = if at_start { s } else { t };
                            if end.distance(moving) <= 1.0 {
                                result = result.and(e.move_endpoint(*el, at_start, end + shift).map_err(|_| ()));
                            }
                        }
                        _ => translate.push(*el),
                    }
                }
                e.transform_elements(&translate, &PlanTransform::translate(shift));
                if let Some(ElementKind::Dimension(d)) = e.element_mut(id).map(|x| &mut x.kind) {
                    if at_b {
                        d.b += shift;
                    } else {
                        d.a += shift;
                    }
                }
                result
            });
            result.map_err(|_| t!("msg-degenerate"))
        }
        DimensionKind::Angular { vertex } => {
            if value >= std::f64::consts::PI {
                return Err(t!("msg-invalid-value"));
            }
            let sweep = (dim.a - vertex).angle_to(dim.b - vertex);
            let target = value * if sweep < 0.0 { -1.0 } else { 1.0 };
            let turn = (target - sweep) * if at_b { 1.0 } else { -1.0 };
            let t = PlanTransform::rotate_about(vertex, turn);
            state.doc.edit(t!("tx-dim-value"), |e| {
                e.transform_elements(&free, &t);
                if let Some(ElementKind::Dimension(d)) = e.element_mut(id).map(|x| &mut x.kind) {
                    if at_b {
                        d.b = t.apply(d.b);
                    } else {
                        d.a = t.apply(d.a);
                    }
                }
            });
            Ok(())
        }
        DimensionKind::Radial | DimensionKind::Diameter => Err(t!("msg-dim-measured")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::Document;

    fn sample() -> AppState {
        AppState::new(Document::new(crate::home::sample_project("casa_simples").unwrap()))
    }

    /// A plan view of the sample and a dimension across the first wall that
    /// runs along X, from its start face line to a point 2 m beyond.
    fn plan(state: &AppState) -> ViewId {
        state.project().views.values().find(|v| matches!(v.kind, ViewKind::FloorPlan { .. })).map(|v| v.id).unwrap()
    }

    #[test]
    fn a_free_dimension_just_takes_its_new_value() {
        let mut state = sample();
        let view = plan(&state);
        let level = state.project().levels[0].id;
        let ty = state.project().default_type(Category::Dimension).unwrap();
        // Far from any linework: nothing is attached.
        let dim = Dimension {
            view,
            a: DVec2::new(100.0, 100.0),
            b: DVec2::new(103.0, 100.0),
            offset: 1.0,
            kind: Default::default(),
        };
        let id = state.create_element("dim".into(), level, ty, ElementKind::Dimension(dim));
        state.sync();
        set_dimension_value(&mut state, id, 5.0).unwrap();
        assert!((dimension(&state, id).unwrap().value() - 5.0).abs() < 1e-9);
        assert!(set_dimension_value(&mut state, id, -1.0).is_err());
        assert!(apply_text(&mut state, &Target::Dimension(id), "abc").is_err());
    }

    #[test]
    fn editing_a_dimension_moves_the_column_it_touches() {
        let mut state = sample();
        let view = plan(&state);
        let level = state.project().levels[0].id;
        let col_ty = state.project().default_type(Category::Column).unwrap();
        let col = cc_core::Column {
            position: DVec2::new(50.0, 50.0),
            rotation_deg: 0.0,
            base_offset: 0.0,
            height: 3.0,
            top: cc_core::WallTop::Unconnected,
        };
        let cid = state.create_element("col".into(), level, col_ty, ElementKind::Column(col));
        state.sync();
        // The column's plan outline: find a vertical edge of it to measure to.
        let drawing = state.drawing(view).unwrap();
        let edge = drawing
            .prims
            .iter()
            .find_map(|p| match p {
                Prim::Line { a, b, element: Some(e), .. } if *e == cid && (a.x - b.x).abs() < 1e-9 => {
                    Some((*a + *b) / 2.0)
                }
                _ => None,
            })
            .unwrap();
        let ty = state.project().default_type(Category::Dimension).unwrap();
        let a = DVec2::new(edge.x - 4.0, edge.y);
        let dim = Dimension { view, a, b: edge, offset: 1.0, kind: DimensionKind::Linear { dir: DVec2::X } };
        let id = state.create_element("dim".into(), level, ty, ElementKind::Dimension(dim));
        state.sync();
        set_dimension_value(&mut state, id, 6.0).unwrap();
        let moved = match &state.project().element(cid).unwrap().kind {
            ElementKind::Column(c) => c.position,
            other => panic!("{other:?}"),
        };
        assert!((moved - DVec2::new(52.0, 50.0)).length() < 1e-9, "{moved}");
        assert!((dimension(&state, id).unwrap().value() - 6.0).abs() < 1e-9);
        // One undo step.
        state.doc.undo();
        assert!(
            matches!(&state.project().element(cid).unwrap().kind, ElementKind::Column(c) if c.position == DVec2::new(50.0, 50.0))
        );
    }
}
