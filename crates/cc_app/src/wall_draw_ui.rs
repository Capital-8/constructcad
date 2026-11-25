//! The wall tool's shapes (issue #497): rectangles, inscribed and
//! circumscribed polygons, circles, arcs by their center or tangent to a
//! wall's free end, and ellipses are drawn with the sketch mode's curve
//! tools (see [`crate::sketch_mode::draw`]) and become walls through
//! `walls_create` as soon as each shape is done, with the options bar's
//! offset, corner radius and join setting; *Fillet* rounds the corner of
//! two straight walls (`wall_fillet`) and *Pick Lines* puts walls along
//! detail lines and grids (with *Chain*, along the detail lines joined to
//! the one picked). The ribbon shows these modes in the *Modify | Place
//! Wall* tab while the tool is active.

use crate::paint::{HIGHLIGHT, PREVIEW, Xf};
use crate::sketch_mode::{self, Draw, Session, Target};
use crate::state::AppState;
use crate::t;
use crate::tools::{TypedInput, WallMode, draw_snap, snap_at};
use cc_core::sketch::SketchCurve;
use cc_core::{ElementId, ElementKind, ElementTypeId, LevelId, sketch_edit};
use cc_engine::Ref;
use cc_engine::commands::sketch::CurveArg;
use cc_engine::commands::wall_draw::{WallFillet, WallsCreate};
use cc_views::Drawing2D;
use egui::{Painter, Response, Stroke, Ui};
use glam::DVec2;

/// Pick distance on screen, points.
const PICK_PX: f64 = 6.0;
/// Walls looked at for a tangent arc's free end.
const MAX_WALLS: usize = 5000;
/// Detail lines looked at for a chain.
const MAX_LINES: usize = 5000;

/// The options of the wall tool's shapes (kept between uses, in the options bar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallDrawOptions {
    /// How far the walls lie from the shape drawn: to the left of an open
    /// chain, outside a closed one (negative the other way), meters.
    pub offset: f64,
    /// The radius that rounds the corners of rectangles and polygons (0:
    /// sharp), and with it a circle takes one click, meters.
    pub radius: f64,
    /// The radius of *Fillet*, meters.
    pub fillet_radius: f64,
    /// Sides of the next polygon.
    pub sides: usize,
    /// The walls' ends may join other walls.
    pub join: bool,
    /// *Pick Lines* takes the chain of detail lines joined to the one picked.
    pub chain: bool,
}

impl Default for WallDrawOptions {
    fn default() -> Self {
        Self { offset: 0.0, radius: 0.0, fillet_radius: 1.0, sides: 6, join: true, chain: false }
    }
}

/// The shape being drawn on a level.
#[derive(Debug, Clone)]
pub struct ShapeDraw {
    level: LevelId,
    session: Session,
}

impl ShapeDraw {
    fn new(level: LevelId) -> Self {
        Self { level, session: Session::new(Target::Walls, Vec::new()) }
    }

    /// The values typed for the shape being drawn.
    pub fn typed_mut(&mut self) -> &mut TypedInput {
        &mut self.session.typed
    }
}

/// Whether the wall tool's `mode` is drawn here (else by [`crate::tools`]).
pub fn handles(mode: WallMode) -> bool {
    mode.shape().is_some() || matches!(mode, WallMode::Fillet | WallMode::PickLines)
}

/// The wall tool in a plan, in a mode [`handles`] takes.
#[allow(clippy::too_many_arguments)]
pub fn tool(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    type_id: ElementTypeId,
    xf: &Xf,
    drawing: &Drawing2D,
) {
    // Space swaps the sides of the walls drawn next, as in the other modes.
    if resp.hovered() && ui.input(|i| i.key_pressed(egui::Key::Space)) {
        state.interaction.wall_flip = !state.interaction.wall_flip;
        state.message = t!("msg-wall-side-flipped");
    }
    match state.wall_mode {
        WallMode::Fillet => fillet(resp, painter, state, xf, drawing),
        WallMode::PickLines => pick_lines(resp, painter, state, level, type_id, xf, drawing),
        mode => {
            if let Some(draw) = mode.shape() {
                shape(ui, resp, painter, state, level, type_id, xf, draw);
            }
        }
    }
}

/// A shape mode: the sketch tool previews the shape; each finished shape
/// becomes walls.
#[allow(clippy::too_many_arguments)]
fn shape(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    type_id: ElementTypeId,
    xf: &Xf,
    draw: Draw,
) {
    let mut d = match state.wall_shape.take() {
        Some(d) if d.level == level => d,
        _ => ShapeDraw::new(level),
    };
    let opts = state.wall_draw;
    let s = &mut d.session;
    if s.draw != draw {
        s.picks.clear();
        s.draw = draw;
    }
    s.sides = opts.sides.clamp(3, 64);
    s.chain = false;
    let mut made = Vec::new();
    if let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) {
        // A tangent arc starts at a free end of the walls of the level.
        if s.picks.is_empty() {
            s.curves = if draw == Draw::ArcTangent { wall_curves(state, level) } else { Vec::new() };
        }
        let shift = ui.input(|i| i.modifiers.shift);
        let extra = s.snap_points();
        let anchor = s.picks.last().copied();
        let sn = snap_at(state, Some(level), raw, anchor, &extra, xf.scale, shift);
        draw_snap(painter, xf, sn);
        let s = &mut d.session;
        if !s.typing() {
            s.typed.clear();
        }
        let enter = !s.typed.is_empty() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let click = resp.clicked() || enter;
        let had_typed = !s.typed.is_empty();
        let p = s.typed_point(sn.point);
        if draw == Draw::Circle && opts.radius > 0.0 && s.picks.is_empty() {
            // With a radius, one click places the circle.
            let circle = SketchCurve::Circle { center: p, radius: opts.radius };
            let pts: Vec<egui::Pos2> = circle.points().iter().map(|q| xf.to_screen(*q)).collect();
            painter.add(egui::Shape::line(pts, Stroke::new(1.2, PREVIEW)));
            if click {
                made.push(circle);
            }
        } else {
            let before = s.curves.len();
            sketch_mode::draw(click, painter, s, p, xf, PICK_PX / xf.scale);
            made.extend(s.curves.get(before..).map(<[SketchCurve]>::to_vec).unwrap_or_default());
        }
        if click && had_typed {
            s.typed.clear();
        }
    }
    state.wall_shape = Some(d);
    if !made.is_empty() {
        let radius = matches!(draw, Draw::Rect | Draw::Polygon | Draw::PolygonOut).then_some(opts.radius);
        create(state, level, type_id, &made, radius);
    }
}

/// The location lines of the straight and arc walls of `level`.
fn wall_curves(state: &AppState, level: LevelId) -> Vec<SketchCurve> {
    state
        .project()
        .walls()
        .filter(|(e, w)| e.level == level && w.spline.is_none())
        .take(MAX_WALLS)
        .map(|(_, w)| match w.bulge {
            b if b != 0.0 => SketchCurve::Arc { a: w.start, b: w.end, bulge: b },
            _ => SketchCurve::Line { a: w.start, b: w.end },
        })
        .collect()
}

/// Make walls along `curves` with the tool's options (one undo step); they
/// are selected. A flipped tool (Space) walks each curve the other way.
fn create(state: &mut AppState, level: LevelId, type_id: ElementTypeId, curves: &[SketchCurve], radius: Option<f64>) {
    let flip = state.interaction.wall_flip;
    let curves: Vec<CurveArg> =
        curves.iter().map(|c| CurveArg::of(&if flip { c.reversed() } else { c.clone() })).collect();
    let opts = state.wall_draw;
    let location_line =
        serde_json::to_value(state.wall_location_line).ok().and_then(|v| v.as_str().map(str::to_string));
    let command = WallsCreate {
        curves,
        offset: (opts.offset != 0.0).then_some(opts.offset),
        radius: radius.filter(|r| *r > 0.0),
        level: Some(Ref::Id(level.0)),
        type_ref: Some(Ref::Id(type_id.0)),
        height: Some(state.wall_height),
        top_level: state.wall_top_level.map(|l| Ref::Id(l.0)),
        location_line,
        join: Some(opts.join),
        ..Default::default()
    };
    if let Some(out) = crate::actions::run_command(state, command) {
        let ids = out["ids"].as_array().map(|a| a.iter().filter_map(|v| v.as_u64()).map(ElementId).collect());
        state.selection = ids.unwrap_or_default();
    }
}

/// The straight wall under the pointer, if any.
fn straight_wall(state: &AppState, drawing: &Drawing2D, raw: DVec2, tol: f64) -> Option<ElementId> {
    drawing.hit_test(raw, tol).filter(|id| {
        state.project().element(*id).and_then(|e| e.as_wall()).is_some_and(|w| !w.is_curved() && w.profile.is_none())
    })
}

/// *Fillet*: the first click picks a straight wall, the second another,
/// and an arc of the options' radius rounds their corner.
fn fillet(resp: &Response, painter: &Painter, state: &mut AppState, xf: &Xf, drawing: &Drawing2D) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let hovered = straight_wall(state, drawing, raw, PICK_PX / xf.scale);
    let first = state.interaction.targets.first().copied();
    let mut lit: std::collections::BTreeSet<ElementId> = first.into_iter().collect();
    lit.extend(hovered);
    crate::modify::paint_ghost(painter, xf, drawing, &lit, &cc_core::PlanTransform::IDENTITY);
    if !resp.clicked() {
        return;
    }
    let Some(id) = hovered else { return };
    match (first, state.interaction.points.first().copied()) {
        (Some(a), Some(pick_a)) if a != id => {
            let command = WallFillet {
                a: a.0,
                pick_a: pick_a.to_array(),
                b: id.0,
                pick_b: raw.to_array(),
                radius: state.wall_draw.fillet_radius,
            };
            if let Some(out) = crate::actions::run_command(state, command) {
                state.selection = out["created"].as_u64().map(ElementId).into_iter().collect();
            }
            state.interaction.targets.clear();
            state.interaction.points.clear();
        }
        _ => {
            state.interaction.targets = vec![id];
            state.interaction.points = vec![raw];
        }
    }
}

/// The curve walls follow along element `id`: a detail line of the active
/// view, or a grid.
fn pickable(state: &AppState, id: ElementId) -> Option<SketchCurve> {
    let e = state.project().element(id)?;
    match &e.kind {
        ElementKind::DetailLine(l) if Some(l.view) == state.active_view => Some(l.curve.clone()),
        ElementKind::Grid(_) => cc_core::wall_ops::curve_of(e),
        _ => None,
    }
}

/// The curves *Pick Lines* takes for the line under the pointer: its own,
/// or with *Chain* the detail lines of the view joined end to end with it.
fn picked_curves(state: &AppState, id: ElementId) -> Vec<SketchCurve> {
    let Some(curve) = pickable(state, id) else { return Vec::new() };
    let is_detail = matches!(state.project().element(id).map(|e| &e.kind), Some(ElementKind::DetailLine(_)));
    if !state.wall_draw.chain || !is_detail {
        return vec![curve];
    }
    let lines: Vec<(ElementId, SketchCurve)> = state
        .project()
        .elements
        .values()
        .filter_map(|e| match &e.kind {
            ElementKind::DetailLine(l) if Some(l.view) == state.active_view => Some((e.id, l.curve.clone())),
            _ => None,
        })
        .take(MAX_LINES)
        .collect();
    let curves: Vec<SketchCurve> = lines.iter().map(|(_, c)| c.clone()).collect();
    let Some(i) = lines.iter().position(|(l, _)| *l == id) else { return vec![curve] };
    let (links, _) = sketch_edit::chain(&curves, i);
    links.iter().filter_map(|&(k, _)| curves.get(k).cloned()).collect()
}

/// *Pick Lines*: the line under the pointer (and its chain) is
/// highlighted; a click puts walls along it.
fn pick_lines(
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    type_id: ElementTypeId,
    xf: &Xf,
    drawing: &Drawing2D,
) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let hovered = drawing.hit_test(raw, PICK_PX / xf.scale).filter(|id| pickable(state, *id).is_some());
    let Some(id) = hovered else { return };
    let curves = picked_curves(state, id);
    for c in &curves {
        let pts: Vec<egui::Pos2> = c.points().iter().map(|q| xf.to_screen(*q)).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(2.5, HIGHLIGHT)));
    }
    if resp.clicked() && !curves.is_empty() {
        create(state, level, type_id, &curves, None);
    }
}

/// The shape being drawn takes typed values now (a length, a radius, a
/// rectangle's sides).
pub fn typing(state: &AppState) -> bool {
    matches!(state.tool, crate::tools::Tool::Wall { .. })
        && handles(state.wall_mode)
        && state.wall_shape.as_ref().is_some_and(|d| d.session.typing())
}

/// Esc while a shape is drawn or a fillet's first wall is picked: drop the
/// typed value, else the points, else the first wall. Returns whether it did.
pub fn escape(state: &mut AppState) -> bool {
    if !matches!(state.tool, crate::tools::Tool::Wall { .. }) || !handles(state.wall_mode) {
        return false;
    }
    if state.wall_mode == WallMode::Fillet && !state.interaction.targets.is_empty() {
        state.interaction.targets.clear();
        state.interaction.points.clear();
        return true;
    }
    match state.wall_shape.as_mut() {
        Some(d) if !d.session.typed.is_empty() || d.session.typed.angle_active => {
            d.session.typed.clear();
            true
        }
        Some(d) if !d.session.picks.is_empty() => {
            d.session.picks.clear();
            true
        }
        _ => false,
    }
}

/// Pick the wall tool's mode: the points of the shape being drawn are dropped.
pub fn set_mode(state: &mut AppState, mode: WallMode) {
    if state.wall_mode == mode {
        return;
    }
    state.wall_mode = mode;
    state.wall_shape = None;
    state.interaction.points.clear();
    state.interaction.targets.clear();
    // A spline goes on from where a chain stands; other modes start afresh after one.
    if mode == WallMode::Spline {
        state.interaction.points.extend(state.interaction.anchor.take());
    } else {
        state.interaction.anchor = None;
    }
    state.interaction.prev_dir = None;
}

/// The options bar's fields of the shape modes: offset, radius, sides,
/// joins and chain.
pub fn options(ui: &mut Ui, state: &mut AppState) {
    let mode = state.wall_mode;
    let opts = &mut state.wall_draw;
    if !matches!(mode, WallMode::Fillet | WallMode::Spline) {
        ui.label(t!("opt-offset"));
        ui.add(crate::properties::length_drag(&mut opts.offset).speed(0.01).range(-100.0..=100.0));
        ui.separator();
    }
    match mode {
        WallMode::Rect | WallMode::Polygon | WallMode::PolygonOut | WallMode::Circle => {
            ui.label(t!("opt-radius"));
            ui.add(crate::properties::length_drag(&mut opts.radius).speed(0.01).range(0.0..=100.0));
            ui.separator();
        }
        WallMode::Fillet => {
            ui.label(t!("opt-radius"));
            ui.add(crate::properties::length_drag(&mut opts.fillet_radius).speed(0.01).range(0.01..=100.0));
            ui.separator();
        }
        _ => {}
    }
    if matches!(mode, WallMode::Polygon | WallMode::PolygonOut) {
        ui.label(t!("sketch-sides"));
        ui.add(egui::DragValue::new(&mut opts.sides).range(3..=64));
        ui.separator();
    }
    if mode == WallMode::PickLines {
        ui.checkbox(&mut opts.chain, t!("sketch-chain"));
        ui.separator();
    }
    if handles(mode) && mode != WallMode::Fillet {
        ui.checkbox(&mut opts.join, t!("opt-wall-join"));
        ui.separator();
    }
}

/// The hint of a mode [`handles`], by how far the shape has got.
pub fn hint(state: &AppState) -> String {
    let picks = state.wall_shape.as_ref().map_or(0, |d| d.session.picks.len());
    let draw = state.wall_mode.shape();
    match (state.wall_mode, draw, picks) {
        (WallMode::Fillet, _, _) if state.interaction.targets.is_empty() => t!("hint-wall-fillet"),
        (WallMode::Fillet, _, _) => t!("hint-wall-fillet-second"),
        (WallMode::PickLines, _, _) => t!("hint-wall-pick-lines"),
        (WallMode::Circle, _, 0) if state.wall_draw.radius > 0.0 => t!("hint-wall-circle-radius"),
        (_, Some(Draw::Rect), 0) => t!("hint-sketch-rect"),
        (_, Some(Draw::Rect), _) => t!("hint-sketch-rect-next"),
        (_, Some(Draw::Polygon | Draw::PolygonOut | Draw::Circle | Draw::ArcCenter | Draw::Ellipse), 0) => {
            t!("hint-sketch-center")
        }
        (_, Some(Draw::Polygon), _) => t!("hint-sketch-corner"),
        (_, Some(Draw::PolygonOut), _) => t!("hint-sketch-side"),
        (_, Some(Draw::Circle), _) => t!("hint-sketch-radius"),
        (_, Some(Draw::ArcCenter), 1) => t!("hint-sketch-arc-start"),
        (_, Some(Draw::ArcCenter), _) => t!("hint-sketch-arc-end"),
        (_, Some(Draw::ArcTangent), 0) => t!("hint-wall-arc-tangent"),
        (_, Some(Draw::ArcTangent), _) => t!("hint-sketch-arc-end"),
        (_, Some(Draw::Ellipse), 1) => t!("hint-sketch-axis"),
        (_, Some(Draw::Ellipse), _) => t!("hint-sketch-other-axis"),
        _ => String::new(),
    }
}
