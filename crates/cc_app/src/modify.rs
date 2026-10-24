//! Transform tools on the selection: move, copy, rotate, mirror (picking a
//! line or drawing the axis) and linear array. Each works in plan views,
//! previews the result as a ghost of the selection's linework and applies
//! in one undo step.

use crate::paint::{PREVIEW, PaintOptions, Xf, paint_drawing};
use crate::state::AppState;
use crate::t;
use crate::tools::{Tool, draw_snap, label, snap_at, temp_dimension};
use cc_core::{ElementId, ElementKind, LevelId, PlanTransform};
use cc_engine::commands::modify as cmd;
use cc_views::{Drawing2D, Prim, format};
use egui::{Key, Painter, Response, Stroke, Ui};
use glam::DVec2;
use std::collections::BTreeSet;

/// Options of the transform tools, kept between uses.
#[derive(Debug, Clone)]
pub struct ModifyOptions {
    /// Copy keeps placing copies from the same base point.
    pub multiple: bool,
    /// Rotate and mirror make a copy instead of moving.
    pub rotate_copy: bool,
    pub mirror_copy: bool,
    /// Number of items in an array, including the original.
    pub count: u32,
    /// Angle typed in the options bar, degrees.
    pub angle: f64,
    /// Offset distance, meters.
    pub offset: f64,
}

impl Default for ModifyOptions {
    fn default() -> Self {
        Self { multiple: true, rotate_copy: false, mirror_copy: true, count: 3, angle: 90.0, offset: 1.0 }
    }
}

/// Start a transform tool on the current selection; without one, explain.
pub fn start(state: &mut AppState, tool: Tool) {
    let targets: Vec<ElementId> = state.selection.iter().copied().collect();
    if targets.is_empty() {
        state.message = t!("msg-select-first");
        return;
    }
    state.set_tool(tool.clone());
    state.interaction.targets = targets;
    state.message = crate::i18n::tr(hint_key(&tool));
}

/// Start a pick tool (trim, split, offset, align): it acts on clicked elements.
pub fn start_pick(state: &mut AppState, tool: Tool) {
    state.set_tool(tool.clone());
    state.message = crate::i18n::tr(hint_key(&tool));
}

fn hint_key(tool: &Tool) -> &'static str {
    match tool {
        Tool::TrimCorner => "hint-trim-corner",
        Tool::TrimExtend => "hint-trim-extend",
        Tool::Split => "hint-split",
        Tool::Offset => "hint-offset",
        Tool::Align => "hint-align",
        Tool::ToggleJoin => "hint-toggle-join",
        Tool::Move | Tool::Copy | Tool::Array => "hint-base-point",
        Tool::Rotate => "hint-rotate",
        Tool::Mirror { draw: false } => "hint-mirror-pick",
        _ => "hint-mirror-draw",
    }
}

/// The selection's linework, transformed (for previews).
fn ghost(drawing: &Drawing2D, ids: &BTreeSet<ElementId>, t: &PlanTransform) -> Drawing2D {
    let mut d = Drawing2D::default();
    for p in &drawing.prims {
        match p {
            Prim::Line { a, b, style, element: Some(e) } if ids.contains(e) => {
                d.line(t.apply(*a), t.apply(*b), *style, Some(*e));
            }
            Prim::Fill { pts, color, element: Some(e) } if ids.contains(e) && color[3] > 0 => {
                d.fill(pts.iter().map(|q| t.apply(*q)).collect(), *color, Some(*e));
            }
            _ => {}
        }
    }
    d
}

pub(crate) fn paint_ghost(
    painter: &Painter,
    xf: &Xf,
    drawing: &Drawing2D,
    ids: &BTreeSet<ElementId>,
    t: &PlanTransform,
) {
    let g = ghost(drawing, ids, t);
    let opts = PaintOptions { highlight: ids, px_per_mm: 0.0, mono: Some(PREVIEW), only: None };
    paint_drawing(painter, &g, xf, &opts);
}

/// Center of the selection's linework.
fn center_of(drawing: &Drawing2D, ids: &BTreeSet<ElementId>) -> Option<DVec2> {
    let pts = drawing.prims.iter().flat_map(|p| match p {
        Prim::Line { a, b, element: Some(e), .. } if ids.contains(e) => vec![*a, *b],
        _ => Vec::new(),
    });
    let (lo, hi) =
        pts.fold(None, |acc: Option<(DVec2, DVec2)>, p| Some(acc.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p)))))?;
    Some((lo + hi) * 0.5)
}

fn finish(state: &mut AppState, selection: Vec<ElementId>) {
    state.set_tool(Tool::Select);
    state.selection = selection.into_iter().collect();
}

#[allow(clippy::too_many_arguments)]
pub fn handle(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    xf: &Xf,
    drawing: &Drawing2D,
    tool: &Tool,
) {
    if matches!(tool, Tool::TrimCorner | Tool::TrimExtend | Tool::Split | Tool::Offset | Tool::Align | Tool::ToggleJoin)
    {
        pick_tool(resp, painter, state, xf, drawing, tool);
        return;
    }
    let _ = level;
    let targets: BTreeSet<ElementId> = state.interaction.targets.iter().copied().collect();
    if targets.is_empty() {
        state.set_tool(Tool::Select);
        return;
    }
    match tool {
        Tool::Move | Tool::Copy | Tool::Array => {
            base_target(ui, resp, painter, state, level, xf, drawing, tool, &targets)
        }
        Tool::Rotate => rotate(ui, resp, painter, state, level, xf, drawing, &targets),
        Tool::Mirror { draw } => mirror(ui, resp, painter, state, level, xf, drawing, *draw, &targets),
        _ => {}
    }
}

/// Move, copy and array: a base point, then a target point (or a typed distance).
#[allow(clippy::too_many_arguments)]
fn base_target(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    xf: &Xf,
    drawing: &Drawing2D,
    tool: &Tool,
    targets: &BTreeSet<ElementId>,
) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let shift = ui.input(|i| i.modifiers.shift);
    let anchor = state.interaction.anchor;
    let s = snap_at(state, Some(level), raw, anchor, &[], xf.scale, shift);
    draw_snap(painter, xf, s);
    let Some(a) = anchor else {
        if resp.clicked() {
            state.interaction.anchor = Some(s.point);
            state.message = t!("hint-target-point");
        }
        return;
    };
    let mut p = s.point;
    if !state.typed.is_empty() {
        p = state.typed.apply(a, p, None);
    }
    let step = PlanTransform::translate(p - a);
    let count = state.modify.count.max(2);
    let steps: Vec<PlanTransform> = match tool {
        Tool::Array => (1..count).map(|k| PlanTransform::translate((p - a) * k as f64)).collect(),
        _ => vec![step],
    };
    for t in &steps {
        paint_ghost(painter, xf, drawing, targets, t);
    }
    temp_dimension(painter, xf, a, p, None, Some(&state.typed), 0.0);
    let enter = ui.input(|i| i.key_pressed(Key::Enter)) && !state.typed.is_empty();
    if !(resp.clicked() || enter) || a.distance(p) < 1e-6 {
        return;
    }
    let ids: Vec<ElementId> = targets.iter().copied().collect();
    state.typed.clear();
    match tool {
        Tool::Move => {
            crate::tools::move_elements(state, &ids, p - a);
            finish(state, ids);
        }
        Tool::Copy => {
            crate::tools::copy_elements(state, &ids, p - a, 1);
            if state.modify.multiple {
                // Keep copying the same selection from the same base point.
                state.interaction.targets = ids;
                state.message = t!("hint-copy-more");
            } else {
                let copies: Vec<ElementId> = state.selection.iter().copied().collect();
                finish(state, copies);
            }
        }
        _ => {
            crate::tools::copy_elements(state, &ids, p - a, count - 1);
            let mut all = ids;
            all.extend(state.selection.iter().copied());
            finish(state, all);
        }
    }
}

/// Rotate: a reference direction from the center, then the target direction.
#[allow(clippy::too_many_arguments)]
fn rotate(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    xf: &Xf,
    drawing: &Drawing2D,
    targets: &BTreeSet<ElementId>,
) {
    let Some(center) = state.interaction.center.or_else(|| center_of(drawing, targets)) else { return };
    state.interaction.center = Some(center);
    let c = xf.to_screen(center);
    let stroke = Stroke::new(1.5, PREVIEW);
    painter.circle_stroke(c, 5.0, stroke);
    painter.line_segment([c - egui::vec2(9.0, 0.0), c + egui::vec2(9.0, 0.0)], stroke);
    painter.line_segment([c - egui::vec2(0.0, 9.0), c + egui::vec2(0.0, 9.0)], stroke);
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let shift = ui.input(|i| i.modifiers.shift);
    let s = snap_at(state, Some(level), raw, Some(center), &[], xf.scale, shift);
    draw_snap(painter, xf, s);
    if state.interaction.placing_center {
        if resp.clicked() {
            state.interaction.center = Some(s.point);
            state.interaction.placing_center = false;
            state.interaction.points.clear();
        }
        return;
    }
    let Some(reference) = state.interaction.points.first().copied() else {
        painter.line_segment([c, xf.to_screen(s.point)], Stroke::new(1.0, PREVIEW));
        if resp.clicked() && s.point.distance(center) > 1e-6 {
            state.interaction.points.push(s.point);
            state.message = t!("hint-rotate-target");
        }
        return;
    };
    let r = (reference - center).normalize_or(DVec2::X);
    let v = (s.point - center).normalize_or(r);
    let angle = r.angle_to(v);
    let t = PlanTransform::rotate_about(center, angle);
    paint_ghost(painter, xf, drawing, targets, &t);
    let radius = reference.distance(center).max(1e-6);
    painter.line_segment([c, xf.to_screen(reference)], Stroke::new(1.0, PREVIEW));
    painter.line_segment([c, xf.to_screen(center + v * radius)], Stroke::new(1.0, PREVIEW));
    label(painter, xf.to_screen(s.point), format::angle(angle.to_degrees()));
    if resp.clicked() {
        apply_rotation(state, targets, center, angle);
    }
}

/// Rotate (or copy rotated) by `angle` radians about `center`, and finish.
pub fn apply_rotation(state: &mut AppState, targets: &BTreeSet<ElementId>, center: DVec2, angle: f64) {
    let ids: Vec<ElementId> = targets.iter().copied().collect();
    let copy = state.modify.rotate_copy;
    let result = crate::tools::rotate_elements(state, &ids, center, angle, copy);
    finish(state, if copy { result } else { ids });
}

/// The line of a model element usable as a mirror axis (walls, grids, beams).
fn axis_of(state: &AppState, id: ElementId) -> Option<(DVec2, DVec2)> {
    let e = state.project().element(id)?;
    match &e.kind {
        // An arc wall has no straight line to mirror across.
        ElementKind::Wall(w) if w.arc().is_some() => None,
        ElementKind::Wall(w) => Some(state.project().wall_axis(e, w)),
        ElementKind::Grid(g) => Some((g.start, g.end)),
        ElementKind::Beam(b) => Some((b.start, b.end)),
        ElementKind::Railing(r) => Some((*r.path.first()?, *r.path.last()?)),
        _ => None,
    }
}

/// Mirror across a picked line, or across an axis drawn with two clicks.
#[allow(clippy::too_many_arguments)]
fn mirror(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    xf: &Xf,
    drawing: &Drawing2D,
    draw: bool,
    targets: &BTreeSet<ElementId>,
) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let axis = if draw {
        let shift = ui.input(|i| i.modifiers.shift);
        let anchor = state.interaction.anchor;
        let s = snap_at(state, Some(level), raw, anchor, &[], xf.scale, shift);
        draw_snap(painter, xf, s);
        match anchor {
            None => {
                if resp.clicked() {
                    state.interaction.anchor = Some(s.point);
                }
                None
            }
            Some(a) if a.distance(s.point) > 1e-6 => Some((a, s.point)),
            Some(_) => None,
        }
    } else {
        drawing.hit_test(raw, 6.0 / xf.scale).and_then(|id| axis_of(state, id))
    };
    let Some((a, b)) = axis else { return };
    let (sa, sb) = (xf.to_screen(a), xf.to_screen(b));
    let dir = (sb - sa).normalized();
    painter.extend(egui::Shape::dashed_line(
        &[sa - dir * 2000.0, sb + dir * 2000.0],
        Stroke::new(1.2, PREVIEW),
        10.0,
        5.0,
    ));
    let t = PlanTransform::mirror_across(a, b - a);
    paint_ghost(painter, xf, drawing, targets, &t);
    if resp.clicked() {
        let ids: Vec<ElementId> = targets.iter().copied().collect();
        let copy = state.modify.mirror_copy;
        let result = crate::tools::mirror_elements(state, &ids, a, b, copy);
        finish(state, if copy { result } else { ids });
    }
}

/// The linear element (wall, beam, grid) under the pointer, and its line.
fn linear_at(state: &AppState, drawing: &Drawing2D, raw: DVec2, tol: f64) -> Option<(ElementId, (DVec2, DVec2))> {
    let id = drawing.hit_test(raw, tol)?;
    let line = state.project().element(id).and_then(cc_core::wall_ops::line_of)?;
    Some((id, line))
}

/// Run a wall edit; a refusal goes to the status bar.
fn report<C: cc_engine::Command + serde::Serialize>(state: &mut AppState, command: C) {
    crate::actions::run_command(state, command);
}

/// Join or unjoin geometry: the first pick stays, and each further pick is
/// joined with it (or unjoined from it) until Esc.
pub fn join_tool(resp: &Response, painter: &Painter, state: &mut AppState, xf: &Xf, drawing: &Drawing2D, unjoin: bool) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let joinable =
        |id: ElementId| state.project().element(id).is_some_and(|e| cc_core::can_join_geometry(e.category()));
    let hovered = drawing.hit_test(raw, 6.0 / xf.scale).filter(|id| joinable(*id));
    let first = state.interaction.targets.first().copied();
    let mut lit: BTreeSet<ElementId> = first.into_iter().collect();
    lit.extend(hovered);
    paint_ghost(painter, xf, drawing, &lit, &PlanTransform::IDENTITY);
    if !resp.clicked() {
        return;
    }
    let Some(id) = hovered else {
        state.message = t!("hint-join-pick");
        return;
    };
    match first {
        None => {
            state.interaction.targets = vec![id];
            state.message = if unjoin { t!("hint-unjoin-second") } else { t!("hint-join-second") };
        }
        Some(first) if first != id => {
            let done = if unjoin {
                crate::actions::run_command(state, cmd::GeometryUnjoin { first: first.0, second: id.0 })
            } else {
                crate::actions::run_command(state, cmd::GeometryJoin { first: first.0, second: id.0 })
            };
            if done.is_some() {
                state.message = if unjoin { t!("msg-unjoined") } else { t!("msg-joined") };
            }
        }
        Some(_) => {}
    }
}

/// Trim/extend to corner or to a reference, split, offset and align.
fn pick_tool(resp: &Response, painter: &Painter, state: &mut AppState, xf: &Xf, drawing: &Drawing2D, tool: &Tool) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let tol = 6.0 / xf.scale;
    let hovered = linear_at(state, drawing, raw, tol);
    let first = state.interaction.targets.first().copied();
    let first_pick = state.interaction.points.first().copied();
    // Highlight the first pick and the element under the pointer.
    let mut lit: BTreeSet<ElementId> = first.into_iter().collect();
    lit.extend(hovered.map(|h| h.0));
    paint_ghost(painter, xf, drawing, &lit, &PlanTransform::IDENTITY);
    let clicked = resp.clicked();
    // Pinned elements are not edited (offset only copies, so it may pick them).
    let pinned = |state: &AppState, id: ElementId| state.project().is_pinned(id);
    if clicked
        && *tool != Tool::Offset
        && let Some((id, _)) = hovered
        && pinned(state, id)
        && !(matches!(tool, Tool::TrimExtend | Tool::Align) && first.is_none())
    {
        state.message = t!("msg-pinned-skipped", "count" => 1);
        return;
    }
    match tool {
        Tool::TrimCorner | Tool::Align | Tool::TrimExtend => {
            let Some((id, _)) = hovered else { return };
            if !clicked {
                return;
            }
            match (first, first_pick) {
                (None, _) | (_, None) => {
                    state.interaction.targets = vec![id];
                    state.interaction.points = vec![raw];
                    state.message = crate::i18n::tr(match tool {
                        Tool::TrimCorner => "hint-trim-corner-second",
                        Tool::Align => "hint-align-second",
                        _ => "hint-trim-extend-second",
                    });
                }
                (Some(a), Some(pick_a)) if a != id => {
                    match tool {
                        Tool::TrimCorner => report(
                            state,
                            cmd::TrimCorner { a: a.0, pick_a: pick_a.to_array(), b: id.0, pick_b: raw.to_array() },
                        ),
                        Tool::Align => report(state, cmd::ElementAlign { id: id.0, reference: a.0 }),
                        _ => report(state, cmd::TrimExtend { id: id.0, pick: raw.to_array(), reference: a.0 }),
                    }
                    // Trim/extend keeps its reference for more elements.
                    if *tool != Tool::TrimExtend {
                        state.interaction.targets.clear();
                        state.interaction.points.clear();
                        state.message = crate::i18n::tr(hint_key(tool));
                    }
                }
                _ => {}
            }
        }
        Tool::Split => {
            let Some((id, (a, b))) = hovered else { return };
            let dir = (b - a).normalize_or_zero();
            let p = a + dir * (raw - a).dot(dir);
            let n = dir.perp() * (8.0 / xf.scale);
            painter.line_segment([xf.to_screen(p - n), xf.to_screen(p + n)], Stroke::new(2.0, PREVIEW));
            label(painter, xf.to_screen(p), format::length(a.distance(p)));
            if clicked {
                report(state, cmd::ElementSplit { id: id.0, point: p.to_array() });
            }
        }
        Tool::ToggleJoin => {
            let Some((id, (a, b))) = hovered else { return };
            let at_start = raw.distance(a) <= raw.distance(b);
            let end = if at_start { a } else { b };
            painter.circle_stroke(xf.to_screen(end), 7.0, Stroke::new(2.0, PREVIEW));
            let joined = match state.project().element(id).map(|e| &e.kind) {
                Some(ElementKind::Wall(w)) => Some(if at_start { w.join_start } else { w.join_end }),
                Some(ElementKind::Beam(b)) => Some(if at_start { b.join_start } else { b.join_end }),
                _ => None,
            };
            if clicked && let Some(joined) = joined {
                let end = if at_start { "start" } else { "end" };
                report(state, cmd::WallJoin { id: id.0, end: end.into(), joined: !joined });
            }
        }
        Tool::Offset => {
            let Some((id, (a, b))) = hovered else { return };
            let n = (b - a).normalize_or_zero().perp();
            let side = if (raw - a).dot(n) >= 0.0 { 1.0 } else { -1.0 };
            let d = state.modify.offset.abs() * side;
            let (oa, ob) = (a + n * d, b + n * d);
            painter.extend(egui::Shape::dashed_line(
                &[xf.to_screen(oa), xf.to_screen(ob)],
                Stroke::new(1.5, PREVIEW),
                8.0,
                4.0,
            ));
            if clicked {
                report(state, cmd::ElementOffset { id: id.0, distance: d });
            }
        }
        _ => {}
    }
}

/// Pin or unpin the selection (one undo step).
pub fn set_pinned(state: &mut AppState, pinned: bool) {
    let ids: Vec<ElementId> = state.selection.iter().copied().collect();
    if ids.is_empty() {
        state.message = t!("msg-select-first");
        return;
    }
    let command = cmd::ElementsPin { ids: ids.iter().map(|i| i.0).collect(), pinned };
    let Some(out) = crate::actions::run_command(state, command) else { return };
    let n = out["changed"].as_u64().unwrap_or(0);
    state.message = if pinned { t!("msg-pinned", "count" => n) } else { t!("msg-unpinned", "count" => n) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AppState {
        AppState::new(cc_core::Document::new(crate::home::sample_project("casa_simples").unwrap()))
    }

    #[test]
    fn tools_need_a_selection() {
        let mut state = sample();
        start(&mut state, Tool::Move);
        assert_eq!(state.tool, Tool::Select);
        assert_eq!(state.message, t!("msg-select-first"));
        let wall = state.project().walls().next().map(|(e, _)| e.id).unwrap();
        state.selection = [wall].into_iter().collect();
        start(&mut state, Tool::Rotate);
        assert_eq!(state.tool, Tool::Rotate);
        assert_eq!(state.interaction.targets, vec![wall]);
    }

    #[test]
    fn rotation_moves_or_copies_in_one_step() {
        let mut state = sample();
        let (id, start_pt) = state.project().walls().next().map(|(e, w)| (e.id, w.start)).unwrap();
        let targets: BTreeSet<ElementId> = [id].into_iter().collect();
        let center = DVec2::ZERO;
        let depth = state.session.doc.undo_depth();
        apply_rotation(&mut state, &targets, center, std::f64::consts::FRAC_PI_2);
        assert_eq!(state.session.doc.undo_depth(), depth + 1);
        let moved = state.project().element(id).and_then(|e| e.as_wall()).map(|w| w.start).unwrap();
        assert!(moved.distance(DVec2::new(-start_pt.y, start_pt.x)) < 1e-9);
        assert_eq!(state.tool, Tool::Select);
        // With "copy" the original stays and the copy is selected.
        let before = state.project().elements.len();
        state.modify.rotate_copy = true;
        apply_rotation(&mut state, &targets, center, std::f64::consts::FRAC_PI_2);
        assert!(state.project().elements.len() > before);
        assert!(!state.selection.contains(&id));
        assert_eq!(state.project().element(id).and_then(|e| e.as_wall()).map(|w| w.start), Some(moved));
    }

    #[test]
    fn pinned_elements_resist_moves_and_deletes() {
        let mut state = sample();
        let wall = state.project().walls().next().map(|(e, _)| e.id).unwrap();
        state.selection = [wall].into_iter().collect();
        set_pinned(&mut state, true);
        assert!(state.project().is_pinned(wall));
        let before = state.project().element(wall).cloned();
        crate::tools::move_elements(&mut state, &[wall], DVec2::new(1.0, 0.0));
        assert_eq!(state.project().element(wall).cloned(), before);
        state.delete_selection();
        assert!(state.project().element(wall).is_some());
        // Unpin (undoable) and it moves again.
        set_pinned(&mut state, false);
        crate::tools::move_elements(&mut state, &[wall], DVec2::new(1.0, 0.0));
        assert_ne!(state.project().element(wall).cloned(), before);
        state.session.doc.undo();
        state.session.doc.undo();
        assert!(state.project().is_pinned(wall));
    }
}
