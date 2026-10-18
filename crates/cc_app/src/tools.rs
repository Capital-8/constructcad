//! Modeling tools: selection, walls, hosted openings, sketches and sections.

use crate::paint::{HIGHLIGHT, PREVIEW, PaintOptions, Xf, paint_drawing};
use crate::state::AppState;
use crate::t;
use cc_core::{
    Category, Ceiling, Element, ElementId, ElementKind, ElementTypeId, Floor, Hosted, LevelId, Project, Roof,
    RoofShape, View, ViewId, ViewKind, Wall,
};
use cc_geom::{ModelGeometry, polygon};
use cc_views::{Drawing2D, format};
use egui::{Color32, Key, Painter, Pos2, Response, Stroke, Ui};
use glam::DVec2;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Tool {
    Select,
    Wall { type_id: ElementTypeId },
    Hosted { category: Category, type_id: ElementTypeId },
    Sketch { category: Category, type_id: ElementTypeId },
    Section,
}

impl Tool {
    /// Translation key of the tool name.
    pub fn key(&self) -> &'static str {
        match self {
            Tool::Select => "tool-select",
            Tool::Wall { .. } => "tool-wall",
            Tool::Hosted { category: Category::Door, .. } => "tool-door",
            Tool::Hosted { .. } => "tool-window",
            Tool::Sketch { category: Category::Ceiling, .. } => "tool-ceiling",
            Tool::Sketch { category: Category::Roof, .. } => "tool-roof",
            Tool::Sketch { .. } => "tool-floor",
            Tool::Section => "tool-section",
        }
    }

    pub fn type_id(&self) -> Option<ElementTypeId> {
        match self {
            Tool::Wall { type_id } | Tool::Hosted { type_id, .. } | Tool::Sketch { type_id, .. } => Some(*type_id),
            _ => None,
        }
    }

    pub fn category(&self) -> Option<Category> {
        match self {
            Tool::Wall { .. } => Some(Category::Wall),
            Tool::Hosted { category, .. } | Tool::Sketch { category, .. } => Some(*category),
            _ => None,
        }
    }

    pub fn with_type(&self, id: ElementTypeId) -> Tool {
        match self.clone() {
            Tool::Wall { .. } => Tool::Wall { type_id: id },
            Tool::Hosted { category, .. } => Tool::Hosted { category, type_id: id },
            Tool::Sketch { category, .. } => Tool::Sketch { category, type_id: id },
            t => t,
        }
    }

    /// Tool for creating elements of `category` with its default type.
    pub fn for_category(project: &Project, category: Category) -> Option<Tool> {
        let type_id = project.default_type(category)?;
        Some(match category {
            Category::Wall => Tool::Wall { type_id },
            Category::Door | Category::Window => Tool::Hosted { category, type_id },
            Category::Floor | Category::Ceiling | Category::Roof => Tool::Sketch { category, type_id },
        })
    }
}

#[derive(Debug, Clone)]
pub enum Drag {
    Box { start: Pos2 },
    Move { start: DVec2, ids: Vec<ElementId>, copy: bool },
}

/// Transient state of the active tool.
#[derive(Debug, Clone, Default)]
pub struct Interaction {
    /// Wall chain anchor or section start.
    pub anchor: Option<DVec2>,
    /// Sketch points.
    pub points: Vec<DVec2>,
    pub flip_hand: bool,
    pub drag: Option<Drag>,
}

impl Interaction {
    pub fn is_idle(&self) -> bool {
        self.anchor.is_none() && self.points.is_empty() && self.drag.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SnapKind {
    Endpoint,
    Midpoint,
    Nearest,
    Free,
}

#[derive(Debug, Clone, Copy)]
struct Snap {
    point: DVec2,
    kind: SnapKind,
}

const SNAP_PX: f64 = 10.0;

/// Increment for rounding free coordinates at the current zoom.
fn round_step(scale: f64) -> f64 {
    [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0].into_iter().find(|s| s * scale >= 6.0).unwrap_or(10.0)
}

fn snap(
    project: &Project,
    level: Option<LevelId>,
    raw: DVec2,
    anchor: Option<DVec2>,
    extra: &[DVec2],
    scale: f64,
    ortho: bool,
) -> Snap {
    let r = SNAP_PX / scale;
    let mut ends: Vec<DVec2> = extra.to_vec();
    let mut mids = Vec::new();
    let mut lines = Vec::new();
    for e in project.elements.values().filter(|e| Some(e.level) == level) {
        if let Some(w) = e.as_wall() {
            ends.extend([w.start, w.end]);
            mids.push((w.start + w.end) * 0.5);
            lines.push((w.start, w.end));
        } else if let Some(s) = e.sketch() {
            ends.extend_from_slice(s);
        }
    }
    let nearest = |pts: &[DVec2]| {
        pts.iter().map(|p| (*p, p.distance(raw))).filter(|(_, d)| *d <= r).min_by(|a, b| a.1.total_cmp(&b.1))
    };
    if let Some((p, _)) = nearest(&ends) {
        return Snap { point: p, kind: SnapKind::Endpoint };
    }
    if let Some((p, _)) = nearest(&mids) {
        return Snap { point: p, kind: SnapKind::Midpoint };
    }
    let step = round_step(scale);
    if let Some(a) = anchor {
        let v = raw - a;
        let mut dir = v.normalize_or_zero();
        if ortho {
            dir = if v.x.abs() >= v.y.abs() { DVec2::new(v.x.signum(), 0.0) } else { DVec2::new(0.0, v.y.signum()) };
        } else {
            let ang = v.y.atan2(v.x).to_degrees();
            let snapped = (ang / 15.0).round() * 15.0;
            if (ang - snapped).abs() < 3.0 {
                dir = DVec2::from_angle(snapped.to_radians());
            }
        }
        let len = (v.dot(dir) / step).round() * step;
        let p = a + dir * len;
        // Prefer landing exactly on a wall line when close (T-junctions).
        if let Some(q) = on_lines(&lines, raw, r) {
            return Snap { point: q, kind: SnapKind::Nearest };
        }
        return Snap { point: p, kind: SnapKind::Free };
    }
    if let Some(q) = on_lines(&lines, raw, r) {
        return Snap { point: q, kind: SnapKind::Nearest };
    }
    Snap { point: (raw / step).round() * step, kind: SnapKind::Free }
}

fn on_lines(lines: &[(DVec2, DVec2)], raw: DVec2, r: f64) -> Option<DVec2> {
    lines
        .iter()
        .map(|(a, b)| polygon::closest_on_segment(*a, *b, raw).0)
        .map(|q| (q, q.distance(raw)))
        .filter(|(_, d)| *d <= r)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(q, _)| q)
}

fn draw_snap(painter: &Painter, xf: &Xf, s: Snap) {
    let c = xf.to_screen(s.point);
    let stroke = Stroke::new(1.5, PREVIEW);
    match s.kind {
        SnapKind::Endpoint => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        SnapKind::Midpoint => {
            let pts = vec![c + egui::vec2(0.0, -6.0), c + egui::vec2(6.0, 5.0), c + egui::vec2(-6.0, 5.0)];
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        SnapKind::Nearest => {
            painter.line_segment([c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)], stroke);
            painter.line_segment([c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)], stroke);
        }
        SnapKind::Free => {
            painter.circle_stroke(c, 3.0, Stroke::new(1.0, PREVIEW));
        }
    }
}

fn label(painter: &Painter, pos: Pos2, text: String) {
    let galley = painter.layout_no_wrap(text, egui::FontId::proportional(13.0), Color32::BLACK);
    let rect = egui::Rect::from_min_size(pos + egui::vec2(12.0, -24.0), galley.size()).expand(3.0);
    painter.rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(255, 250, 225, 235));
    painter.galley(rect.min + egui::vec2(3.0, 3.0), galley, Color32::BLACK);
}

/// Parse the typed length ("3,5" or "3.5").
fn typed_length(input: &str) -> Option<f64> {
    input.trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v > 0.0)
}

pub fn handle(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    view: &View,
    xf: &Xf,
    drawing: &Drawing2D,
) {
    let is_plan = matches!(view.kind, ViewKind::FloorPlan { .. } | ViewKind::CeilingPlan { .. });
    let tool = state.tool.clone();
    match (&tool, view.level()) {
        (Tool::Select, _) => select(ui, resp, painter, state, is_plan, xf, drawing),
        (_, None) | (_, Some(_)) if !is_plan => {
            if resp.clicked() {
                state.message = t!("msg-plan-only");
            }
            select(ui, resp, painter, state, false, xf, drawing);
        }
        (Tool::Wall { type_id }, Some(level)) => wall_tool(ui, resp, painter, state, level, *type_id, xf),
        (Tool::Hosted { category, type_id }, Some(level)) => {
            hosted_tool(ui, resp, painter, state, level, *category, *type_id, xf)
        }
        (Tool::Sketch { category, type_id }, Some(level)) => {
            sketch_tool(ui, resp, painter, state, level, *category, *type_id, xf)
        }
        (Tool::Section, level) => section_tool(ui, resp, painter, state, level, xf),
        _ => {}
    }
}

fn wall_tool(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    type_id: ElementTypeId,
    xf: &Xf,
) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let shift = ui.input(|i| i.modifiers.shift);
    let anchor = state.interaction.anchor;
    let s = snap(state.project(), Some(level), raw, anchor, &[], xf.scale, shift);
    let mut p = s.point;
    draw_snap(painter, xf, s);

    if let Some(a) = anchor {
        if let Some(len) = typed_length(&state.length_input) {
            p = a + (p - a).normalize_or(DVec2::X) * len;
        }
        let half = state.project().element_type(type_id).map_or(0.075, |t| t.thickness() * 0.5);
        let n = (p - a).normalize_or_zero().perp() * half;
        let stroke = Stroke::new(1.5, PREVIEW);
        painter.line_segment([xf.to_screen(a), xf.to_screen(p)], Stroke::new(1.0, PREVIEW));
        painter.line_segment([xf.to_screen(a + n), xf.to_screen(p + n)], stroke);
        painter.line_segment([xf.to_screen(a - n), xf.to_screen(p - n)], stroke);
        let text = if state.length_input.is_empty() {
            format::meters(a.distance(p))
        } else {
            format!("{} m [Enter]", state.length_input)
        };
        label(painter, xf.to_screen(p), text);
    }

    let enter = ui.input(|i| i.key_pressed(Key::Enter)) && !state.length_input.is_empty();
    if resp.clicked() || enter {
        match anchor {
            None => state.interaction.anchor = Some(p),
            Some(a) if a.distance(p) > 0.01 => {
                let wall = Wall { start: a, end: p, base_offset: 0.0, height: state.wall_height, ..Default::default() };
                state.create_element(t!("tx-create-wall"), level, type_id, ElementKind::Wall(wall));
                state.interaction.anchor = Some(p);
                state.length_input.clear();
            }
            _ => {}
        }
    }
}

/// Nearest wall on `level` to `raw`: (wall, u along wall, signed v).
fn wall_under(
    project: &Project,
    geom: &ModelGeometry,
    level: LevelId,
    raw: DVec2,
    slack: f64,
) -> Option<(ElementId, f64, f64)> {
    geom.walls
        .values()
        .filter(|g| project.element(g.element).is_some_and(|e| e.level == level))
        .filter_map(|g| {
            let rel = raw - g.origin;
            let (u, v) = (rel.dot(g.dir), rel.dot(g.normal));
            (u >= 0.0 && u <= g.length && v.abs() <= g.half + slack).then_some((g.element, u, v, v.abs()))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(id, u, v, _)| (id, u, v))
}

#[allow(clippy::too_many_arguments)]
fn hosted_tool(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    category: Category,
    type_id: ElementTypeId,
    xf: &Xf,
) {
    if ui.input(|i| i.key_pressed(Key::Space)) {
        state.interaction.flip_hand = !state.interaction.flip_hand;
    }
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let project = &state.doc.project;
    let Some(ty) = project.element_type(type_id) else { return };
    let Some((width, _)) = ty.opening_size() else { return };
    let sill = match ty.data {
        cc_core::TypeData::Window { default_sill, .. } => default_sill,
        _ => 0.0,
    };
    let Some((host, u, v)) = wall_under(project, &state.geom, level, raw, 12.0 / xf.scale) else {
        painter.circle_stroke(xf.to_screen(raw), 4.0, Stroke::new(1.0, Color32::GRAY));
        return;
    };
    let g = &state.geom.walls[&host];
    let margin = 0.01;
    if g.length < width + 2.0 * margin {
        label(painter, xf.to_screen(raw), t!("msg-wall-too-short"));
        return;
    }
    let offset = u.clamp(width * 0.5 + margin, g.length - width * 0.5 - margin);
    let flip_facing = v < 0.0;
    let (u0, u1) = (offset - width * 0.5, offset + width * 0.5);
    let rect = [g.world(u0, -g.half), g.world(u1, -g.half), g.world(u1, g.half), g.world(u0, g.half)];
    let pts: Vec<Pos2> = rect.iter().map(|p| xf.to_screen(*p)).collect();
    painter.add(egui::Shape::closed_line(pts, Stroke::new(2.0, PREVIEW)));
    if category == Category::Door {
        let side = if flip_facing { -1.0 } else { 1.0 };
        let hinge_u = if state.interaction.flip_hand { u1 } else { u0 };
        let hinge = g.world(hinge_u, side * g.half);
        painter.line_segment(
            [xf.to_screen(hinge), xf.to_screen(hinge + g.normal * side * width)],
            Stroke::new(1.5, PREVIEW),
        );
    }
    label(painter, xf.to_screen(raw), format!("{} {}", t!("label-offset"), format::meters(offset)));

    if resp.clicked() {
        let hosted = Hosted { host, offset, sill_height: sill, flip_facing, flip_hand: state.interaction.flip_hand };
        let host_level = project.element(host).map_or(level, |e| e.level);
        let (kind, name) = if category == Category::Door {
            (ElementKind::Door(hosted), t!("tx-create-door"))
        } else {
            (ElementKind::Window(hosted), t!("tx-create-window"))
        };
        state.create_element(name, host_level, type_id, kind);
    }
}

#[allow(clippy::too_many_arguments)]
fn sketch_tool(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    category: Category,
    type_id: ElementTypeId,
    xf: &Xf,
) {
    if ui.input(|i| i.key_pressed(Key::Backspace)) && state.length_input.is_empty() {
        state.interaction.points.pop();
    }
    let finish_key = ui.input(|i| i.key_pressed(Key::Enter));
    let pts = state.interaction.points.clone();
    let stroke = Stroke::new(1.8, Color32::from_rgb(200, 40, 160));
    for w in pts.windows(2) {
        painter.line_segment([xf.to_screen(w[0]), xf.to_screen(w[1])], stroke);
    }
    let mut point = None;
    if let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) {
        let shift = ui.input(|i| i.modifiers.shift);
        let s = snap(state.project(), Some(level), raw, pts.last().copied(), &pts, xf.scale, shift);
        draw_snap(painter, xf, s);
        if let Some(last) = pts.last() {
            painter.line_segment([xf.to_screen(*last), xf.to_screen(s.point)], Stroke::new(1.0, PREVIEW));
            label(painter, xf.to_screen(s.point), format::meters(last.distance(s.point)));
        }
        if pts.len() >= 2 {
            painter.extend(egui::Shape::dashed_line(
                &[xf.to_screen(s.point), xf.to_screen(pts[0])],
                Stroke::new(1.0, PREVIEW),
                5.0,
                4.0,
            ));
        }
        point = Some(s.point);
    }
    if finish_key && pts.len() >= 3 {
        finish_sketch(state, level, category, type_id, pts);
        return;
    }
    if resp.clicked()
        && let Some(p) = point
    {
        if pts.len() >= 3 && p.distance(pts[0]) * xf.scale < SNAP_PX {
            finish_sketch(state, level, category, type_id, pts);
        } else if pts.last().is_none_or(|l| l.distance(p) > 1e-3) {
            state.interaction.points.push(p);
        }
    }
}

/// Default roof base: the top of the tallest wall on the level.
fn default_roof_base(project: &Project, level: LevelId) -> f64 {
    project
        .walls()
        .filter(|(e, _)| e.level == level)
        .map(|(e, w)| project.wall_top_z(e, w) - project.level_elevation(level))
        .fold(None, |m: Option<f64>, h| Some(m.map_or(h, |m| m.max(h))))
        .unwrap_or(3.0)
}

pub fn finish_sketch(
    state: &mut AppState,
    level: LevelId,
    category: Category,
    type_id: ElementTypeId,
    pts: Vec<DVec2>,
) {
    if pts.len() < 3 || polygon::area(&pts) < 1e-4 {
        state.message = t!("msg-sketch-invalid");
        return;
    }
    let (kind, name) = match category {
        Category::Floor => (ElementKind::Floor(Floor { boundary: pts, height_offset: 0.0 }), t!("tx-create-floor")),
        Category::Ceiling => {
            (ElementKind::Ceiling(Ceiling { boundary: pts, height_offset: 2.6 }), t!("tx-create-ceiling"))
        }
        _ => {
            let base_offset = default_roof_base(state.project(), level);
            let roof = Roof { footprint: pts, base_offset, overhang: 0.5, shape: RoofShape::Flat };
            (ElementKind::Roof(roof), t!("tx-create-roof"))
        }
    };
    state.create_element(name, level, type_id, kind);
    state.interaction.points.clear();
}

/// Closed loop following the walls of `level`, offset to the exterior faces
/// (`outward > 0`) or interior faces (`outward < 0`).
pub fn walls_loop(project: &Project, level: LevelId, outward: f64) -> Option<Vec<DVec2>> {
    let walls: Vec<(&Element, &Wall)> = project.walls().filter(|(e, _)| e.level == level).collect();
    const TOL: f64 = 0.01;
    for start in 0..walls.len() {
        let mut used = vec![false; walls.len()];
        used[start] = true;
        let (first, w0) = (walls[start].1.start, walls[start].1.end);
        let mut pts = vec![first, w0];
        let mut cur = w0;
        let mut half = project.thickness_of(walls[start].0) * 0.5;
        loop {
            if cur.distance(first) < TOL && pts.len() > 3 {
                pts.pop();
                let pts = polygon::ccw(&pts);
                return Some(polygon::offset(&pts, half * outward));
            }
            let next = (0..walls.len())
                .find(|&i| !used[i] && (walls[i].1.start.distance(cur) < TOL || walls[i].1.end.distance(cur) < TOL));
            let Some(i) = next else { break };
            used[i] = true;
            let w = walls[i].1;
            cur = if w.start.distance(cur) < TOL { w.end } else { w.start };
            half = half.max(project.thickness_of(walls[i].0) * 0.5);
            pts.push(cur);
        }
    }
    None
}

fn section_tool(ui: &Ui, resp: &Response, painter: &Painter, state: &mut AppState, level: Option<LevelId>, xf: &Xf) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let shift = ui.input(|i| i.modifiers.shift);
    let s = snap(state.project(), level, raw, state.interaction.anchor, &[], xf.scale, shift);
    draw_snap(painter, xf, s);
    if let Some(a) = state.interaction.anchor {
        let stroke = Stroke::new(1.5, PREVIEW);
        painter.line_segment([xf.to_screen(a), xf.to_screen(s.point)], stroke);
        let look = -(s.point - a).normalize_or_zero().perp();
        let mid = (a + s.point) * 0.5;
        painter.arrow(xf.to_screen(mid), (xf.to_screen(mid + look) - xf.to_screen(mid)).normalized() * 30.0, stroke);
    }
    if resp.clicked() {
        match state.interaction.anchor {
            None => state.interaction.anchor = Some(s.point),
            Some(a) if a.distance(s.point) > 0.1 => {
                let end = s.point;
                let id = state.doc.edit(t!("tx-create-section"), |e| {
                    let n = e.project().views_of(cc_core::ViewCategory::Section).count() + 1;
                    let id = ViewId(e.alloc_id());
                    e.add_view(View {
                        id,
                        name: t!("default-section-name", "n" => n),
                        scale: 50,
                        kind: ViewKind::Section { start: a, end, depth: 8.0 },
                    })
                });
                state.set_tool(Tool::Select);
                state.open_view(id);
            }
            _ => {}
        }
    }
}

fn select(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    movable: bool,
    xf: &Xf,
    drawing: &Drawing2D,
) {
    let raw = resp.hover_pos().map(|p| xf.to_world(p));
    let tol = 5.0 / xf.scale;
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));

    // Pre-selection highlight.
    if state.interaction.drag.is_none()
        && let Some(hit) = raw.and_then(|p| drawing.hit_test(p, tol))
        && !state.selection.contains(&hit)
    {
        let only = BTreeSet::from([hit]);
        let opts = PaintOptions {
            highlight: &only,
            px_per_mm: 0.0,
            mono: Some(HIGHLIGHT.gamma_multiply(0.7)),
            only: Some(&only),
        };
        paint_drawing(painter, drawing, xf, &opts);
    }

    if resp.drag_started_by(egui::PointerButton::Primary) {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
        let w = xf.to_world(origin);
        match drawing.hit_test(w, tol) {
            Some(hit) if movable => {
                if !state.selection.contains(&hit) {
                    state.selection = BTreeSet::from([hit]);
                }
                let ids = state.selection.iter().copied().collect();
                state.interaction.drag = Some(Drag::Move { start: w, ids, copy: ctrl });
            }
            _ => state.interaction.drag = Some(Drag::Box { start: origin }),
        }
    }

    let delta_of = |start: DVec2, raw: DVec2| {
        let step = round_step(xf.scale);
        let mut d = raw - start;
        if shift {
            d = if d.x.abs() >= d.y.abs() { DVec2::new(d.x, 0.0) } else { DVec2::new(0.0, d.y) };
        }
        (d / step).round() * step
    };

    match (&state.interaction.drag, raw) {
        (Some(Drag::Move { start, ids, copy }), Some(raw)) => {
            let delta = delta_of(*start, raw);
            let only: BTreeSet<_> = ids.iter().copied().collect();
            let ghost = Xf { origin: xf.origin - delta, ..*xf };
            let opts = PaintOptions { highlight: &only, px_per_mm: 0.0, mono: Some(PREVIEW), only: Some(&only) };
            paint_drawing(painter, drawing, &ghost, &opts);
            let verb = if *copy { t!("label-copy") } else { t!("label-move") };
            label(painter, xf.to_screen(raw), format!("{verb} {} m", format::meters(delta.length())));
        }
        (Some(Drag::Box { start }), _) => {
            if let Some(cur) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                let rect = egui::Rect::from_two_pos(*start, cur);
                let crossing = cur.x < start.x;
                let color = if crossing { Color32::from_rgb(30, 160, 60) } else { HIGHLIGHT };
                painter.rect_filled(rect, 0.0, color.gamma_multiply(0.08));
                if crossing {
                    let c =
                        [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
                    painter.extend(egui::Shape::dashed_line(&c, Stroke::new(1.0, color), 5.0, 3.0));
                } else {
                    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, color), egui::StrokeKind::Middle);
                }
            }
        }
        _ => {}
    }

    if resp.drag_stopped() {
        match state.interaction.drag.take() {
            Some(Drag::Move { start, ids, copy }) => {
                if let Some(raw) = raw.or(state.cursor) {
                    let delta = delta_of(start, raw);
                    if delta.length() > 1e-6 {
                        if copy {
                            duplicate(state, &ids, delta);
                        } else {
                            move_elements(state, &ids, delta);
                        }
                    }
                }
            }
            Some(Drag::Box { start }) => {
                let end = resp.interact_pointer_pos().or(resp.hover_pos()).unwrap_or(start);
                let (a, b) = (xf.to_world(start), xf.to_world(end));
                let ids = drawing.elements_in(a.min(b), a.max(b), end.x >= start.x);
                if !ctrl {
                    state.selection.clear();
                }
                state.selection.extend(ids);
            }
            None => {}
        }
    }

    if resp.clicked() {
        match raw.and_then(|p| drawing.hit_test(p, tol)) {
            Some(hit) if ctrl => {
                if !state.selection.remove(&hit) {
                    state.selection.insert(hit);
                }
            }
            Some(hit) => state.selection = BTreeSet::from([hit]),
            None if !ctrl => state.selection.clear(),
            None => {}
        }
    }
}

/// Translate elements; hosted elements slide along their host unless it moves too.
pub fn move_elements(state: &mut AppState, ids: &[ElementId], delta: DVec2) {
    let moving: BTreeSet<_> = ids.iter().copied().collect();
    state.doc.edit(t!("tx-move"), |e| {
        for id in ids {
            let Some(el) = e.project().element(*id) else { continue };
            if let Some(h) = el.as_hosted() {
                if moving.contains(&h.host) {
                    continue;
                }
                let Some(dir) = e.project().element(h.host).and_then(|w| w.as_wall()).map(|w| w.direction()) else {
                    continue;
                };
                if let Some(h) = e.element_mut(*id).and_then(|el| el.as_hosted_mut()) {
                    h.offset += delta.dot(dir);
                }
            } else if let Some(el) = e.element_mut(*id) {
                el.translate(delta);
            }
        }
    });
}

/// Copy elements by `delta`; hosted elements follow copied hosts.
pub fn duplicate(state: &mut AppState, ids: &[ElementId], delta: DVec2) {
    let selected: BTreeSet<_> = ids.iter().copied().collect();
    let new_ids = state.doc.edit(t!("tx-copy"), |e| {
        let originals: Vec<Element> = ids.iter().filter_map(|id| e.project().element(*id).cloned()).collect();
        let mut remap = std::collections::BTreeMap::new();
        let mut created = Vec::new();
        // Hosts first so hosted copies can be remapped.
        for el in originals.iter().filter(|el| el.as_hosted().is_none()) {
            let mut c = el.clone();
            c.id = ElementId(e.alloc_id());
            c.translate(delta);
            remap.insert(el.id, c.id);
            created.push(e.add_element(c));
        }
        for el in originals.iter().filter(|el| el.as_hosted().is_some()) {
            let mut c = el.clone();
            c.id = ElementId(e.alloc_id());
            c.mark = e.project().next_mark(c.category());
            let host = el.as_hosted().map(|h| h.host).unwrap_or(ElementId(0));
            let dir = e.project().element(host).and_then(|w| w.as_wall()).map(|w| w.direction());
            if let Some(h) = c.as_hosted_mut() {
                if let Some(new_host) = remap.get(&h.host) {
                    h.host = *new_host;
                } else if selected.contains(&host) {
                    continue;
                } else if let Some(dir) = dir {
                    h.offset += delta.dot(dir);
                }
            }
            created.push(e.add_element(c));
        }
        created
    });
    state.selection = new_ids.into_iter().collect();
}
