//! Sketching pieces in the component modeling environment: an outline
//! drawn in the plan, front or side view (lines with optional arcs, a
//! rectangle or a circle), extruded or revolved into a solid or void piece.
//!
//! A point that lands on a reference plane seen edge-on is locked to it:
//! its coordinate is written as the plane's name, so the piece follows the
//! plane when a parameter moves it. Other coordinates are rounded to the
//! millimeter.

use crate::component_env::{EnvTool, Pick};
use crate::paint::Xf;
use crate::state::AppState;
use crate::t;
use crate::theme::{self, PALETTE};
use cc_engine::commands::component_piece::{ComponentPieceAdd, ExprArg, SketchPointArg};
use cc_views::{ComponentView, ComponentViewKind};
use egui::{Painter, Stroke, Ui};
use glam::DVec2;

/// How clicks make the outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SketchMode {
    /// Points one by one, closed by clicking the first again; a radius makes
    /// the next segment an arc.
    #[default]
    Line,
    /// Two opposite corners.
    Rect,
    /// The center, then a point on the circle.
    Circle,
}

/// What the outline becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PieceKind {
    #[default]
    Extrusion,
    /// Turned about an axis picked after the outline closes.
    Revolution,
}

/// A point of the sketch: where it is and how its coordinates are written.
#[derive(Debug, Clone, PartialEq)]
pub struct SketchPt {
    pub pos: DVec2,
    pub u: String,
    pub v: String,
    /// The segment to the next point is an arc of this radius (empty: straight).
    pub radius: String,
}

/// The sketch being drawn.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sketch {
    pub mode: SketchMode,
    pub kind: PieceKind,
    pub void: bool,
    /// Radius of the next segment's arc (0: straight).
    pub arc_radius: f64,
    /// The view the sketch is drawn in (set by its first point).
    pub view: Option<ComponentViewKind>,
    pub points: Vec<SketchPt>,
    /// A closed outline of a revolution, waiting for its axis.
    pub closed: bool,
}

impl Sketch {
    pub fn new(kind: PieceKind) -> Self {
        Self { kind, ..Self::default() }
    }
}

/// A number rounded to the millimeter, as an expression.
fn mm(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

/// `e + d`, folded into a number when `e` is one.
pub fn offset(e: &str, d: f64) -> String {
    if d.abs() < 1e-12 {
        return e.to_string();
    }
    if let Ok(v) = e.trim().parse::<f64>() {
        return mm(v + d);
    }
    if d > 0.0 { format!("{e} + {}", mm(d)) } else { format!("{e} - {}", mm(-d)) }
}

/// The point at `p` in `view`, locked to the planes whose lines pass within
/// `tol`: a vertical line locks u, a horizontal one v.
pub fn snap(view: &ComponentView, p: DVec2, tol: f64) -> SketchPt {
    let mut pos = p;
    let (mut u, mut v) = (None, None);
    let (mut du, mut dv) = (tol, tol);
    for plane in &view.planes {
        let vertical = (plane.b.x - plane.a.x).abs() < 1e-9;
        if vertical && (p.x - plane.a.x).abs() <= du {
            du = (p.x - plane.a.x).abs();
            pos.x = plane.a.x;
            u = Some(plane.name.clone());
        } else if !vertical && (p.y - plane.a.y).abs() <= dv {
            dv = (p.y - plane.a.y).abs();
            pos.y = plane.a.y;
            v = Some(plane.name.clone());
        }
    }
    let u = u.unwrap_or_else(|| {
        pos.x = (pos.x * 1000.0).round() / 1000.0;
        mm(pos.x)
    });
    let v = v.unwrap_or_else(|| {
        pos.y = (pos.y * 1000.0).round() / 1000.0;
        mm(pos.y)
    });
    SketchPt { pos, u, v, radius: String::new() }
}

/// The four corners of the rectangle with opposite corners `a` and `b`,
/// counter-clockwise.
pub fn rect_points(a: &SketchPt, b: &SketchPt) -> Vec<SketchPt> {
    let pt = |x: &SketchPt, y: &SketchPt| SketchPt {
        pos: DVec2::new(x.pos.x, y.pos.y),
        u: x.u.clone(),
        v: y.v.clone(),
        radius: String::new(),
    };
    let mut pts = vec![pt(a, a), pt(b, a), pt(b, b), pt(a, b)];
    // Counter-clockwise, so arcs bulge outwards.
    let area: f64 = (0..4).map(|i| pts[i].pos.perp_dot(pts[(i + 1) % 4].pos)).sum();
    if area < 0.0 {
        pts.reverse();
    }
    pts
}

/// A circle about `center` through a point `r` away: two half arcs.
pub fn circle_points(center: &SketchPt, r: f64) -> Vec<SketchPt> {
    let r = (r * 1000.0).round() / 1000.0;
    let radius = mm(r);
    [-1.0, 1.0]
        .iter()
        .map(|s| SketchPt {
            pos: center.pos + DVec2::new(s * r, 0.0),
            u: offset(&center.u, s * r),
            v: center.v.clone(),
            radius: radius.clone(),
        })
        .collect()
}

fn args(points: &[SketchPt]) -> Vec<SketchPointArg> {
    points
        .iter()
        .map(|p| SketchPointArg {
            u: ExprArg::Text(p.u.clone()),
            v: ExprArg::Text(p.v.clone()),
            radius: (!p.radius.trim().is_empty()).then(|| ExprArg::Text(p.radius.clone())),
        })
        .collect()
}

fn plane_name(kind: ComponentViewKind) -> &'static str {
    match kind {
        ComponentViewKind::Front => "front",
        ComponentViewKind::Side => "side",
        _ => "plan",
    }
}

/// Make the piece of a closed sketch (`axis`: the revolution's axis). The
/// new piece is selected and the tool starts a new sketch of the same kind.
fn create(state: &mut AppState, sketch: &Sketch, axis: Option<String>) {
    let (Some(view), Some(env)) = (sketch.view, state.component_env.as_ref()) else { return };
    let kind = match sketch.kind {
        PieceKind::Extrusion => "extrusion",
        PieceKind::Revolution => "revolution",
    };
    let command = ComponentPieceAdd {
        type_ref: cc_engine::Ref::Id(env.type_id.0),
        kind: Some(kind.into()),
        plane: plane_name(view).into(),
        points: args(&sketch.points),
        start: None,
        end: None,
        axis: axis.map(ExprArg::Text),
        angle: None,
        void: sketch.void,
        name: None,
        material: None,
    };
    let out = crate::actions::run_command(state, command);
    if let Some(env) = state.component_env.as_mut() {
        if let Some(i) = out.and_then(|o| o["index"].as_u64()) {
            env.pick = Some(Pick::Piece(i as usize));
        }
        env.tool = EnvTool::Sketch(Sketch { view: None, points: Vec::new(), closed: false, ..sketch.clone() });
    }
}

fn set(state: &mut AppState, sketch: Sketch) {
    if let Some(env) = state.component_env.as_mut() {
        env.tool = EnvTool::Sketch(sketch);
    }
}

/// The sketch tool in one view: preview, snapping and clicks.
#[allow(clippy::too_many_arguments)]
pub fn handle(
    state: &mut AppState,
    sketch: Sketch,
    view: &ComponentView,
    kind: ComponentViewKind,
    xf: &Xf,
    painter: &Painter,
    hover: Option<DVec2>,
    clicked: bool,
    tol: f64,
) {
    if kind == ComponentViewKind::ThreeD || sketch.view.is_some_and(|v| v != kind) {
        return;
    }
    let stroke = Stroke::new(1.5, PALETTE.accent);
    let line = |a: DVec2, b: DVec2| painter.line_segment([xf.to_screen(a), xf.to_screen(b)], stroke);
    for w in sketch.points.windows(2) {
        line(w[0].pos, w[1].pos);
    }
    let Some(p) = hover else { return };
    // A closed revolution outline waits for its axis: a vertical plane, or any vertical line.
    if sketch.closed {
        if let (Some(first), Some(last)) = (sketch.points.first(), sketch.points.last()) {
            line(last.pos, first.pos);
        }
        let s = snap(view, p, tol);
        let (lo, hi) = view.bounds.unwrap_or((DVec2::splat(-1.0), DVec2::splat(1.0)));
        painter.line_segment(
            [xf.to_screen(DVec2::new(s.pos.x, lo.y)), xf.to_screen(DVec2::new(s.pos.x, hi.y))],
            Stroke::new(1.0, PALETTE.accent.gamma_multiply(0.6)),
        );
        if clicked {
            create(state, &sketch, Some(s.u));
        }
        return;
    }
    let s = snap(view, p, tol);
    // Locked coordinates show as a square, free ones as a circle.
    let at = xf.to_screen(s.pos);
    let locked = s.u.parse::<f64>().is_err() || s.v.parse::<f64>().is_err();
    if locked {
        painter.rect_stroke(
            egui::Rect::from_center_size(at, egui::vec2(9.0, 9.0)),
            0.0,
            stroke,
            egui::StrokeKind::Middle,
        );
    } else {
        painter.circle_stroke(at, 4.0, stroke);
    }
    let mut sketch = sketch;
    match sketch.mode {
        SketchMode::Line => {
            let closes = sketch.points.len() >= 2
                && sketch.points.first().is_some_and(|f| xf.to_screen(f.pos).distance(at) < 8.0);
            if let Some(last) = sketch.points.last() {
                line(last.pos, s.pos);
            }
            if !clicked {
                return;
            }
            if closes {
                finish(state, sketch);
                return;
            }
            if let Some(last) = sketch.points.last_mut()
                && sketch.arc_radius > 0.0
            {
                last.radius = mm(sketch.arc_radius);
            }
            sketch.view = Some(kind);
            sketch.points.push(s);
            set(state, sketch);
        }
        SketchMode::Rect => match sketch.points.first() {
            Some(a) => {
                let pts = rect_points(a, &s);
                for i in 0..4 {
                    line(pts[i].pos, pts[(i + 1) % 4].pos);
                }
                if clicked && (s.pos - a.pos).abs().min_element() > 1e-3 {
                    sketch.points = pts;
                    finish(state, sketch);
                }
            }
            None if clicked => {
                sketch.view = Some(kind);
                sketch.points.push(s);
                set(state, sketch);
            }
            None => {}
        },
        SketchMode::Circle => match sketch.points.first() {
            Some(c) => {
                let r = c.pos.distance(p);
                painter.circle_stroke(xf.to_screen(c.pos), (r * xf.scale) as f32, stroke);
                if clicked && r > 1e-3 {
                    sketch.points = circle_points(c, r);
                    finish(state, sketch);
                }
            }
            None if clicked => {
                sketch.view = Some(kind);
                sketch.points.push(s);
                set(state, sketch);
            }
            None => {}
        },
    }
}

/// A closed outline: an extrusion is made at once, a revolution asks for its axis.
fn finish(state: &mut AppState, sketch: Sketch) {
    match sketch.kind {
        PieceKind::Extrusion => create(state, &sketch, None),
        PieceKind::Revolution => set(state, Sketch { closed: true, ..sketch }),
    }
}

#[cfg(test)]
pub fn finish_for_tests(state: &mut AppState, sketch: Sketch) {
    finish(state, sketch);
}

/// The sketch's options: how clicks draw, solid or void, the arc radius.
pub fn options(ui: &mut Ui, state: &mut AppState) {
    let Some(EnvTool::Sketch(mut sketch)) = state.component_env.as_ref().map(|e| e.tool.clone()) else { return };
    let before = sketch.clone();
    ui.horizontal(|ui| {
        for (mode, key) in [
            (SketchMode::Line, "comp-sketch-line"),
            (SketchMode::Rect, "comp-sketch-rect"),
            (SketchMode::Circle, "comp-sketch-circle"),
        ] {
            if ui.selectable_label(sketch.mode == mode, crate::i18n::tr(key)).clicked() && sketch.mode != mode {
                sketch = Sketch { mode, view: None, points: Vec::new(), closed: false, ..sketch.clone() };
            }
        }
        ui.separator();
        ui.selectable_value(&mut sketch.void, false, t!("comp-sketch-solid"));
        ui.selectable_value(&mut sketch.void, true, t!("comp-sketch-void"));
        if sketch.mode == SketchMode::Line {
            ui.separator();
            ui.label(t!("comp-sketch-arc"));
            ui.add(crate::properties::length_drag(&mut sketch.arc_radius).range(0.0..=1000.0))
                .on_hover_text(t!("comp-sketch-arc-tip"));
        }
        ui.separator();
        let hint = match (sketch.closed, sketch.mode, sketch.points.is_empty()) {
            (true, ..) => t!("comp-sketch-hint-axis"),
            (_, SketchMode::Line, true) => t!("comp-sketch-hint-line"),
            (_, SketchMode::Line, false) => t!("comp-sketch-hint-line-next"),
            (_, SketchMode::Rect, _) => t!("comp-sketch-hint-rect"),
            (_, SketchMode::Circle, _) => t!("comp-sketch-hint-circle"),
        };
        ui.label(theme::muted(hint));
    });
    if sketch != before {
        set(state, sketch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_views::component_view::ViewPlane;

    fn view() -> ComponentView {
        let plane = |name: &str, a: (f64, f64), b: (f64, f64)| ViewPlane {
            name: name.into(),
            a: DVec2::new(a.0, a.1),
            b: DVec2::new(b.0, b.1),
            origin: false,
        };
        ComponentView {
            planes: vec![plane("Left", (-0.6, -1.0), (-0.6, 1.0)), plane("Front", (-1.0, -0.4), (1.0, -0.4))],
            ..Default::default()
        }
    }

    #[test]
    fn points_lock_to_planes_within_reach() {
        let v = view();
        let s = snap(&v, DVec2::new(-0.603, -0.398), 0.01);
        assert_eq!((s.u.as_str(), s.v.as_str()), ("Left", "Front"));
        assert_eq!(s.pos, DVec2::new(-0.6, -0.4));
        let s = snap(&v, DVec2::new(0.12345, -0.398), 0.01);
        assert_eq!((s.u.as_str(), s.v.as_str()), ("0.123", "Front"));
    }

    #[test]
    fn rectangles_and_circles_keep_their_locks() {
        let v = view();
        let a = snap(&v, DVec2::new(-0.6, -0.4), 0.01);
        let b = snap(&v, DVec2::new(0.2, 0.3), 0.01);
        let r = rect_points(&b, &a);
        let us: Vec<&str> = r.iter().map(|p| p.u.as_str()).collect();
        let vs: Vec<&str> = r.iter().map(|p| p.v.as_str()).collect();
        assert!(us.contains(&"Left") && us.contains(&"0.2") && vs.contains(&"Front") && vs.contains(&"0.3"));
        // Counter-clockwise whichever corner came first.
        let area: f64 = (0..4).map(|i| r[i].pos.perp_dot(r[(i + 1) % 4].pos)).sum();
        assert!(area > 0.0);
        let c = circle_points(&a, 0.05);
        assert_eq!((c[0].u.as_str(), c[1].u.as_str()), ("Left - 0.05", "Left + 0.05"));
        assert_eq!(c[0].radius, "0.05");
        assert_eq!(offset("0.5", -0.25), "0.25");
    }
}
