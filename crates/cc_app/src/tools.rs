//! Modeling tools: selection, walls, hosted openings, sketches, sections,
//! rooms, grids and dimensions.

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
    Room { type_id: ElementTypeId },
    Grid { type_id: ElementTypeId },
    Dimension { type_id: ElementTypeId },
    Column { type_id: ElementTypeId },
    Beam { type_id: ElementTypeId },
    Stair { type_id: ElementTypeId },
    Railing { type_id: ElementTypeId },
    Component { type_id: ElementTypeId },
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
            Tool::Room { .. } => "tool-room",
            Tool::Grid { .. } => "tool-grid",
            Tool::Dimension { .. } => "tool-dimension",
            Tool::Column { .. } => "tool-column",
            Tool::Beam { .. } => "tool-beam",
            Tool::Stair { .. } => "tool-stair",
            Tool::Railing { .. } => "tool-railing",
            Tool::Component { .. } => "tool-component",
        }
    }

    pub fn type_id(&self) -> Option<ElementTypeId> {
        match self {
            Tool::Wall { type_id }
            | Tool::Hosted { type_id, .. }
            | Tool::Sketch { type_id, .. }
            | Tool::Room { type_id }
            | Tool::Grid { type_id }
            | Tool::Dimension { type_id }
            | Tool::Column { type_id }
            | Tool::Beam { type_id }
            | Tool::Stair { type_id }
            | Tool::Railing { type_id }
            | Tool::Component { type_id } => Some(*type_id),
            _ => None,
        }
    }

    pub fn category(&self) -> Option<Category> {
        match self {
            Tool::Wall { .. } => Some(Category::Wall),
            Tool::Hosted { category, .. } | Tool::Sketch { category, .. } => Some(*category),
            Tool::Room { .. } => Some(Category::Room),
            Tool::Grid { .. } => Some(Category::Grid),
            Tool::Dimension { .. } => Some(Category::Dimension),
            Tool::Column { .. } => Some(Category::Column),
            Tool::Beam { .. } => Some(Category::Beam),
            Tool::Stair { .. } => Some(Category::Stair),
            Tool::Railing { .. } => Some(Category::Railing),
            Tool::Component { .. } => Some(Category::Component),
            _ => None,
        }
    }

    pub fn with_type(&self, id: ElementTypeId) -> Tool {
        match self.clone() {
            Tool::Wall { .. } => Tool::Wall { type_id: id },
            Tool::Hosted { category, .. } => Tool::Hosted { category, type_id: id },
            Tool::Sketch { category, .. } => Tool::Sketch { category, type_id: id },
            Tool::Room { .. } => Tool::Room { type_id: id },
            Tool::Grid { .. } => Tool::Grid { type_id: id },
            Tool::Dimension { .. } => Tool::Dimension { type_id: id },
            Tool::Column { .. } => Tool::Column { type_id: id },
            Tool::Beam { .. } => Tool::Beam { type_id: id },
            Tool::Stair { .. } => Tool::Stair { type_id: id },
            Tool::Railing { .. } => Tool::Railing { type_id: id },
            Tool::Component { .. } => Tool::Component { type_id: id },
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
            Category::Room => Tool::Room { type_id },
            Category::Grid => Tool::Grid { type_id },
            Category::Dimension => Tool::Dimension { type_id },
            Category::Column => Tool::Column { type_id },
            Category::Beam => Tool::Beam { type_id },
            Category::Stair => Tool::Stair { type_id },
            Category::Railing => Tool::Railing { type_id },
            Category::Component => Tool::Component { type_id },
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
    /// Points acquired for alignment guides (see `snap`).
    pub tracked: Vec<DVec2>,
    /// Direction back along the previous segment of a chain (angle reference).
    pub prev_dir: Option<DVec2>,
    /// Rotation of the next component, degrees.
    pub rotation: f64,
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
    /// On a guide from an acquired point.
    Alignment,
    /// Where two guides cross.
    Intersection,
    Free,
}

#[derive(Debug, Clone, Copy)]
struct Snap {
    point: DVec2,
    kind: SnapKind,
    /// Dashed guides to draw: from the point that casts them to `point`.
    guides: [Option<(DVec2, DVec2)>; 2],
}

impl Snap {
    fn at(point: DVec2, kind: SnapKind) -> Self {
        Self { point, kind, guides: [None, None] }
    }
}

/// Points acquired for alignment, most recent first.
const MAX_TRACKED: usize = 6;

/// Snap through [`snap`] with the interaction's acquired points, and
/// acquire the point when it is a model point.
#[allow(clippy::too_many_arguments)]
fn snap_at(
    state: &mut AppState,
    level: Option<LevelId>,
    raw: DVec2,
    anchor: Option<DVec2>,
    extra: &[DVec2],
    scale: f64,
    ortho: bool,
) -> Snap {
    let s = snap(&state.doc.project, level, raw, anchor, extra, &state.interaction.tracked, scale, ortho);
    if matches!(s.kind, SnapKind::Endpoint | SnapKind::Midpoint) {
        let tracked = &mut state.interaction.tracked;
        tracked.retain(|p| p.distance(s.point) > 1e-9);
        tracked.insert(0, s.point);
        tracked.truncate(MAX_TRACKED);
    }
    s
}

const SNAP_PX: f64 = 10.0;

/// Increment for rounding free coordinates at the current zoom.
fn round_step(scale: f64) -> f64 {
    [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0].into_iter().find(|s| s * scale >= 6.0).unwrap_or(10.0)
}

/// Snap `raw` to model points, wall lines, alignments and angle/length
/// increments. `tracked` are points acquired by hovering them earlier:
/// with the anchor and `extra` they cast horizontal, vertical and wall-line
/// guides; a crossing of two guides, or of a guide with the current
/// direction from the anchor, is a snap point.
#[allow(clippy::too_many_arguments)]
fn snap(
    project: &Project,
    level: Option<LevelId>,
    raw: DVec2,
    anchor: Option<DVec2>,
    extra: &[DVec2],
    tracked: &[DVec2],
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
        return Snap::at(p, SnapKind::Endpoint);
    }
    if let Some((p, _)) = nearest(&mids) {
        return Snap::at(p, SnapKind::Midpoint);
    }

    // Guides (origin, unit direction) passing near the pointer.
    let mut origins: Vec<DVec2> = tracked.iter().chain(extra).copied().chain(anchor).collect();
    origins.dedup_by(|a, b| a.distance(*b) < 1e-9);
    let mut guides: Vec<(DVec2, DVec2)> = Vec::new();
    for o in &origins {
        guides.push((*o, DVec2::X));
        guides.push((*o, DVec2::Y));
        for (a, b) in &lines {
            if a.distance(*o) < 1e-9 || b.distance(*o) < 1e-9 {
                let d = (*b - *a).normalize_or_zero();
                if d.x.abs() > 1e-6 && d.y.abs() > 1e-6 {
                    guides.push((*o, d));
                }
            }
        }
    }
    let off = |g: &(DVec2, DVec2), q: DVec2| (q - g.0).perp_dot(g.1).abs();
    guides.retain(|g| off(g, raw) <= r && g.0.distance(raw) > r);
    // Two guides crossing near the pointer.
    let mut best: Option<(Snap, f64)> = None;
    for (i, g) in guides.iter().enumerate() {
        for h in guides.iter().skip(i + 1) {
            if g.1.perp_dot(h.1).abs() < 1e-6 {
                continue;
            }
            let Some(t) = polygon::line_intersect(g.0, g.1, h.0, h.1) else { continue };
            let q = g.0 + g.1 * t;
            let d = q.distance(raw);
            if d <= r * 1.5 && best.is_none_or(|b| d < b.1) {
                let guides = [Some((g.0, q)), Some((h.0, q))];
                best = Some((Snap { point: q, kind: SnapKind::Intersection, guides }, d));
            }
        }
    }
    if let Some((s, _)) = best {
        return s;
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
            return Snap::at(q, SnapKind::Nearest);
        }
        // The current direction meeting a guide from another point.
        let aligned = guides
            .iter()
            .filter(|g| g.0.distance(a) > 1e-9 && dir.perp_dot(g.1).abs() > 1e-6)
            .filter_map(|g| polygon::line_intersect(a, dir, g.0, g.1).map(|t| (a + dir * t, *g)))
            .filter(|(q, _)| (*q - a).dot(dir) > 0.0 && q.distance(raw) <= r * 1.5)
            .min_by(|x, y| x.0.distance(raw).total_cmp(&y.0.distance(raw)));
        if let Some((q, g)) = aligned {
            return Snap { point: q, kind: SnapKind::Alignment, guides: [Some((g.0, q)), None] };
        }
        return Snap::at(p, SnapKind::Free);
    }
    if let Some(q) = on_lines(&lines, raw, r) {
        return Snap::at(q, SnapKind::Nearest);
    }
    if let Some(g) = guides.iter().min_by(|x, y| off(x, raw).total_cmp(&off(y, raw))) {
        let t = ((raw - g.0).dot(g.1) / step).round() * step;
        let q = g.0 + g.1 * t;
        return Snap { point: q, kind: SnapKind::Alignment, guides: [Some((g.0, q)), None] };
    }
    Snap::at((raw / step).round() * step, SnapKind::Free)
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
    for (from, to) in s.guides.iter().flatten() {
        // Extend the guide a little past the snapped point.
        let (a, b) = (xf.to_screen(*from), xf.to_screen(*to));
        let past = b + (b - a).normalized() * 40.0;
        painter.extend(egui::Shape::dashed_line(&[a, past], Stroke::new(1.0, TEMP_DIM.gamma_multiply(0.7)), 6.0, 4.0));
        painter.circle_stroke(a, 3.0, Stroke::new(1.0, TEMP_DIM));
    }
    match s.kind {
        SnapKind::Intersection => {
            painter.line_segment([c + egui::vec2(-6.0, -6.0), c + egui::vec2(6.0, 6.0)], Stroke::new(2.0, PREVIEW));
            painter.line_segment([c + egui::vec2(-6.0, 6.0), c + egui::vec2(6.0, -6.0)], Stroke::new(2.0, PREVIEW));
        }
        SnapKind::Alignment => {
            painter.circle_stroke(c, 4.0, stroke);
        }
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

/// Color of temporary dimensions.
const TEMP_DIM: Color32 = Color32::from_rgb(40, 110, 200);

/// Text on a light background, centered at `pos` and rotated by `angle`
/// (radians, screen orientation), kept upright.
fn dim_text(painter: &Painter, pos: Pos2, text: String, angle: f32, strong: bool) {
    let angle =
        if angle.abs() > std::f32::consts::FRAC_PI_2 { angle - angle.signum() * std::f32::consts::PI } else { angle };
    let font =
        if strong { egui::FontId::new(12.5, crate::theme::semibold()) } else { egui::FontId::proportional(12.0) };
    let galley = painter.layout_no_wrap(text, font, TEMP_DIM);
    let size = galley.size();
    let rot = egui::emath::Rot2::from_angle(angle);
    let top_left = pos - rot * (size / 2.0);
    let corners = [
        egui::vec2(-2.0, -1.0),
        egui::vec2(size.x + 2.0, -1.0),
        size + egui::vec2(2.0, 1.0),
        egui::vec2(-2.0, size.y + 1.0),
    ];
    let bg: Vec<Pos2> = corners.iter().map(|c| top_left + rot * *c).collect();
    painter.add(egui::Shape::convex_polygon(bg, Color32::from_rgba_unmultiplied(255, 255, 255, 230), Stroke::NONE));
    painter.add(egui::epaint::TextShape::new(top_left, galley, TEMP_DIM).with_angle(angle));
}

/// Temporary dimension of a segment being drawn from `a` to `p`: a
/// dimension line beside it with its length (or the typed value) and an
/// arc with the angle to `reference` (the previous segment, or the X axis).
fn temp_dimension(painter: &Painter, xf: &Xf, a: DVec2, p: DVec2, reference: Option<DVec2>, typed: &str, side: f64) {
    let len = a.distance(p);
    let Some(dir) = (p - a).try_normalize() else { return };
    let thin = Stroke::new(1.0, TEMP_DIM);
    // Dimension line, offset to the left of the segment.
    let n = dir.perp();
    let offset = (side + 26.0 / xf.scale) * n;
    let gap = n * (4.0 / xf.scale);
    let over = n * (5.0 / xf.scale);
    let (da, dp) = (a + offset, p + offset);
    for q in [a, p] {
        painter.line_segment([xf.to_screen(q + n * side + gap), xf.to_screen(q + offset + over)], thin);
    }
    painter.line_segment([xf.to_screen(da), xf.to_screen(dp)], thin);
    // Architectural ticks.
    let tick = (dir + n).normalize_or_zero() * (5.0 / xf.scale);
    for q in [da, dp] {
        painter.line_segment([xf.to_screen(q - tick), xf.to_screen(q + tick)], Stroke::new(1.5, TEMP_DIM));
    }
    let text = if typed.is_empty() { format::meters(len) } else { format!("{typed} m ⏎") };
    let screen_dir = xf.to_screen(dp) - xf.to_screen(da);
    let mid = xf.to_screen((da + dp) * 0.5) + (xf.to_screen(da + n) - xf.to_screen(da)).normalized() * 9.0;
    dim_text(painter, mid, text, screen_dir.y.atan2(screen_dir.x), true);

    // Angle to the reference direction.
    let reference = reference.and_then(|r| r.try_normalize()).unwrap_or(DVec2::X);
    let angle = reference.angle_to(dir);
    let degrees = angle.to_degrees();
    let radius_px = (len * xf.scale * 0.45).clamp(18.0, 70.0);
    if len * xf.scale < 30.0 || degrees.abs() < 0.5 {
        return;
    }
    let radius = radius_px / xf.scale;
    painter.extend(egui::Shape::dashed_line(
        &[xf.to_screen(a), xf.to_screen(a + reference * radius * 1.25)],
        thin,
        4.0,
        3.0,
    ));
    let steps = 24;
    let arc: Vec<Pos2> = (0..=steps)
        .map(|i| xf.to_screen(a + DVec2::from_angle(angle * i as f64 / steps as f64).rotate(reference) * radius))
        .collect();
    painter.add(egui::Shape::line(arc, thin));
    let label_at = xf.to_screen(a + DVec2::from_angle(angle / 2.0).rotate(reference) * (radius + 14.0 / xf.scale));
    dim_text(painter, label_at, format!("{}°", format::number(degrees.abs(), 2)), 0.0, false);
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
        // Dimensions work in any 2D view.
        (Tool::Dimension { type_id }, level) => {
            dimension_tool(ui, resp, painter, state, view, level, *type_id, xf, drawing)
        }
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
        (Tool::Room { type_id }, Some(level)) => room_tool(resp, painter, state, level, *type_id, xf),
        (Tool::Grid { type_id }, Some(level)) => grid_tool(ui, resp, painter, state, level, *type_id, xf),
        (Tool::Column { type_id }, Some(level)) => column_tool(ui, resp, painter, state, level, *type_id, xf),
        (Tool::Beam { type_id }, Some(level)) => beam_tool(ui, resp, painter, state, level, *type_id, xf),
        (Tool::Stair { type_id }, Some(level)) => stair_tool(ui, resp, painter, state, level, *type_id, xf),
        (Tool::Railing { type_id }, Some(level)) => railing_tool(ui, resp, painter, state, level, *type_id, xf),
        (Tool::Component { type_id }, Some(level)) => component_tool(ui, resp, painter, state, level, *type_id, xf),
        _ => {}
    }
}

/// Top constraint for new columns and stairs: the level above, if any.
fn default_top(project: &Project, level: LevelId) -> cc_core::WallTop {
    match project.level_above(level) {
        Some(above) => cc_core::WallTop::Level { level: above.id, offset: 0.0 },
        None => cc_core::WallTop::Unconnected,
    }
}

/// Click to place a column (snaps to wall ends and grid crossings through the usual snaps).
fn column_tool(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    type_id: ElementTypeId,
    xf: &Xf,
) {
    if resp.hovered() && ui.input(|i| i.key_pressed(Key::Space)) {
        state.interaction.rotation = (state.interaction.rotation + 90.0).rem_euclid(360.0);
    }
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let shift = ui.input(|i| i.modifiers.shift);
    let s = snap_at(state, Some(level), raw, None, &grid_crossings(state.project()), xf.scale, shift);
    draw_snap(painter, xf, s);
    if let Some(cc_core::TypeData::Column { section, .. }) = state.project().element_type(type_id).map(|t| &t.data) {
        let outline: Vec<Pos2> = cc_geom::column_outline(*section, s.point, state.interaction.rotation)
            .iter()
            .map(|p| xf.to_screen(*p))
            .collect();
        painter.add(egui::Shape::closed_line(outline, Stroke::new(1.5, PREVIEW)));
    }
    if resp.clicked() {
        let project = state.project();
        let column = cc_core::Column {
            position: s.point,
            rotation_deg: state.interaction.rotation,
            base_offset: 0.0,
            height: project.level_height(level, 3.0),
            top: default_top(project, level),
        };
        state.create_element(t!("tx-create-column"), level, type_id, ElementKind::Column(column));
    }
}

/// Click to place a component; Space turns it by 90°.
fn component_tool(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    type_id: ElementTypeId,
    xf: &Xf,
) {
    if resp.hovered() && ui.input(|i| i.key_pressed(Key::Space)) {
        state.interaction.rotation = (state.interaction.rotation + 90.0).rem_euclid(360.0);
    }
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let shift = ui.input(|i| i.modifiers.shift);
    let s = snap_at(state, Some(level), raw, None, &[], xf.scale, shift);
    draw_snap(painter, xf, s);
    let rotation = state.interaction.rotation;
    if let Some(cc_core::TypeData::Component { def, .. }) = state.project().element_type(type_id).map(|t| &t.data) {
        let rot = DVec2::from_angle(rotation.to_radians());
        let (parts, _) = def.resolve(&Default::default());
        for part in parts.iter().filter(|p| p.in_plan) {
            let outline: Vec<Pos2> = part.footprint.iter().map(|p| xf.to_screen(s.point + rot.rotate(*p))).collect();
            painter.add(egui::Shape::closed_line(outline, Stroke::new(1.5, PREVIEW)));
        }
    }
    if resp.clicked() {
        let component = cc_core::Component {
            position: s.point,
            rotation_deg: rotation,
            base_offset: 0.0,
            overrides: Default::default(),
        };
        state.create_element(t!("tx-create-component"), level, type_id, ElementKind::Component(component));
    }
}

/// Intersections of grid lines, offered as snap points.
fn grid_crossings(project: &Project) -> Vec<DVec2> {
    let grids: Vec<(DVec2, DVec2)> = project
        .elements
        .values()
        .filter_map(|e| match &e.kind {
            ElementKind::Grid(g) => Some((g.start, g.end)),
            _ => None,
        })
        .collect();
    let mut points = Vec::new();
    for (i, &(a, b)) in grids.iter().enumerate() {
        for &(c, d) in grids.iter().skip(i + 1) {
            if let Some(t) = polygon::line_intersect(a, b - a, c, d - c)
                && (0.0..=1.0).contains(&t)
            {
                points.push(a + (b - a) * t);
            }
        }
    }
    points
}

/// Two clicks: start and end of a beam. Its top is at the level above by default.
fn beam_tool(
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
    let crossings = grid_crossings(state.project());
    let anchor = state.interaction.anchor;
    let s = snap_at(state, Some(level), raw, anchor, &crossings, xf.scale, shift);
    draw_snap(painter, xf, s);
    if let Some(a) = state.interaction.anchor {
        let width = match state.project().element_type(type_id).map(|t| &t.data) {
            Some(cc_core::TypeData::Beam { width, .. }) => *width,
            _ => 0.15,
        };
        let n = (s.point - a).normalize_or_zero().perp() * (width / 2.0);
        let pts = vec![xf.to_screen(a - n), xf.to_screen(s.point - n), xf.to_screen(s.point + n), xf.to_screen(a + n)];
        painter.add(egui::Shape::closed_line(pts, Stroke::new(1.5, PREVIEW)));
        temp_dimension(painter, xf, a, s.point, None, "", width / 2.0);
    }
    if resp.clicked() {
        match state.interaction.anchor {
            None => state.interaction.anchor = Some(s.point),
            Some(a) if a.distance(s.point) > 0.05 => {
                let beam =
                    cc_core::Beam { start: a, end: s.point, top_offset: state.project().level_height(level, 3.0) };
                state.create_element(t!("tx-create-beam"), level, type_id, ElementKind::Beam(beam));
                state.interaction.anchor = Some(s.point);
            }
            _ => {}
        }
    }
}

/// Two clicks: the foot of the stair, then a point giving its direction.
fn stair_tool(
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
    let s = snap_at(state, Some(level), raw, state.interaction.anchor, &[], xf.scale, shift);
    draw_snap(painter, xf, s);
    let project = state.project();
    let Some(cc_core::TypeData::Stair { width, max_riser, tread, .. }) =
        project.element_type(type_id).map(|t| t.data.clone())
    else {
        return;
    };
    let height = match default_top(project, level) {
        cc_core::WallTop::Level { level: top, .. } => project.level_elevation(top) - project.level_elevation(level),
        cc_core::WallTop::Unconnected => 3.0,
    };
    let (risers, _, run) = cc_core::stair_layout(height, max_riser, tread);
    if let Some(a) = state.interaction.anchor {
        let u = (s.point - a).normalize_or(DVec2::X);
        let v = u.perp() * (width / 2.0);
        let end = a + u * run;
        let pts = vec![xf.to_screen(a - v), xf.to_screen(end - v), xf.to_screen(end + v), xf.to_screen(a + v)];
        painter.add(egui::Shape::closed_line(pts, Stroke::new(1.5, PREVIEW)));
        for k in 1..risers.saturating_sub(1) {
            let p = a + u * (k as f64 * tread);
            painter.line_segment([xf.to_screen(p - v), xf.to_screen(p + v)], Stroke::new(1.0, PREVIEW));
        }
        label(painter, xf.to_screen(end), t!("label-stair", "risers" => risers, "run" => format::meters(run)));
    }
    if resp.clicked() {
        match state.interaction.anchor {
            None => state.interaction.anchor = Some(s.point),
            Some(a) if a.distance(s.point) > 0.05 => {
                let d = s.point - a;
                let stair = cc_core::Stair {
                    start: a,
                    direction_deg: d.y.atan2(d.x).to_degrees(),
                    base_offset: 0.0,
                    height,
                    top: default_top(state.project(), level),
                };
                state.create_element(t!("tx-create-stair"), level, type_id, ElementKind::Stair(stair));
                state.interaction.anchor = None;
            }
            _ => {}
        }
    }
}

/// Click the points of the railing path; Enter, a double click or Finish ends it.
fn railing_tool(
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
    let last = state.interaction.points.last().copied();
    let s = snap_at(state, Some(level), raw, last, &state.interaction.points.clone(), xf.scale, shift);
    draw_snap(painter, xf, s);
    let pts: Vec<Pos2> =
        state.interaction.points.iter().map(|p| xf.to_screen(*p)).chain([xf.to_screen(s.point)]).collect();
    if pts.len() >= 2 {
        painter.add(egui::Shape::line(pts, Stroke::new(1.5, PREVIEW)));
    }
    let finish = resp.double_clicked() || ui.input(|i| i.key_pressed(Key::Enter));
    if resp.clicked() && !resp.double_clicked() {
        state.interaction.points.push(s.point);
    }
    if finish {
        finish_railing(state, level, type_id);
    }
}

/// Create the railing from the points clicked so far.
pub fn finish_railing(state: &mut AppState, level: LevelId, type_id: ElementTypeId) {
    let mut path = std::mem::take(&mut state.interaction.points);
    path.dedup_by(|a, b| a.distance(*b) < 1e-3);
    if path.len() >= 2 {
        let railing = cc_core::Railing { path, base_offset: 0.0 };
        state.create_element(t!("tx-create-railing"), level, type_id, ElementKind::Railing(railing));
    }
}

/// Next free number among `existing` names (1 when none is numeric).
fn next_number<'a>(existing: impl Iterator<Item = &'a str>) -> String {
    (existing.filter_map(|s| s.trim().parse::<u32>().ok()).max().unwrap_or(0) + 1).to_string()
}

/// Click inside walls to create a room; the enclosed region under the cursor is previewed.
fn room_tool(
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    level: LevelId,
    type_id: ElementTypeId,
    xf: &Xf,
) {
    let Some(p) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let boundary = state.geom.room_at(state.project(), level, p);
    if let Some(b) = &boundary {
        let pts: Vec<Pos2> = b.iter().map(|q| xf.to_screen(*q)).collect();
        crate::paint::fill_polygon(painter, pts.clone(), PREVIEW.gamma_multiply(0.18));
        painter.add(egui::Shape::closed_line(pts, Stroke::new(1.5, PREVIEW)));
    }
    if resp.clicked() {
        if boundary.is_none() {
            state.message = t!("msg-room-not-enclosed");
            return;
        }
        let project = state.project();
        let number = next_number(project.elements.values().filter_map(|e| match &e.kind {
            ElementKind::Room(r) => Some(r.number.as_str()),
            _ => None,
        }));
        let room = cc_core::Room {
            position: p,
            number,
            name: t!("room-default-name"),
            height: project.level_height(level, 3.0),
        };
        state.create_element(t!("tx-create-room"), level, type_id, ElementKind::Room(room));
    }
}

/// Two clicks: start and end of a grid line.
fn grid_tool(
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
    let s = snap_at(state, Some(level), raw, state.interaction.anchor, &[], xf.scale, shift);
    draw_snap(painter, xf, s);
    if let Some(a) = state.interaction.anchor {
        painter.add(egui::Shape::dashed_line(
            &[xf.to_screen(a), xf.to_screen(s.point)],
            Stroke::new(1.5, PREVIEW),
            10.0,
            4.0,
        ));
        temp_dimension(painter, xf, a, s.point, None, "", 0.0);
    }
    if resp.clicked() {
        match state.interaction.anchor {
            None => state.interaction.anchor = Some(s.point),
            Some(a) if a.distance(s.point) > 0.1 => {
                let name = next_number(state.project().elements.values().filter_map(|e| match &e.kind {
                    ElementKind::Grid(g) => Some(g.name.as_str()),
                    _ => None,
                }));
                let grid = cc_core::Grid { start: a, end: s.point, name };
                state.create_element(t!("tx-create-grid"), level, type_id, ElementKind::Grid(grid));
                state.interaction.anchor = None;
            }
            _ => {}
        }
    }
}

/// Three clicks: the two measured points, then the position of the dimension line.
#[allow(clippy::too_many_arguments)]
fn dimension_tool(
    ui: &Ui,
    resp: &Response,
    painter: &Painter,
    state: &mut AppState,
    view: &View,
    level: Option<LevelId>,
    type_id: ElementTypeId,
    xf: &Xf,
    drawing: &Drawing2D,
) {
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let shift = ui.input(|i| i.modifiers.shift);
    // Snap to the visible linework of the view.
    let ends: Vec<DVec2> = drawing
        .prims
        .iter()
        .filter_map(|p| match p {
            cc_views::Prim::Line { a, b, .. } => Some([*a, *b]),
            _ => None,
        })
        .flatten()
        .collect();
    let anchor = state.interaction.points.last().copied();
    let s = snap_at(state, None, raw, anchor, &ends, xf.scale, shift);
    let stroke = Stroke::new(1.5, PREVIEW);
    let points = state.interaction.points.clone();
    match points.as_slice() {
        [] => draw_snap(painter, xf, s),
        [a] => {
            draw_snap(painter, xf, s);
            painter.line_segment([xf.to_screen(*a), xf.to_screen(s.point)], stroke);
            label(painter, xf.to_screen(s.point), format::number(a.distance(s.point), 2));
        }
        [a, b, ..] => {
            let n = (*b - *a).normalize_or_zero().perp();
            let offset = (raw - *a).dot(n);
            let (p, q) = (*a + n * offset, *b + n * offset);
            painter.line_segment([xf.to_screen(*a), xf.to_screen(p)], Stroke::new(1.0, PREVIEW));
            painter.line_segment([xf.to_screen(*b), xf.to_screen(q)], Stroke::new(1.0, PREVIEW));
            painter.line_segment([xf.to_screen(p), xf.to_screen(q)], stroke);
            label(painter, xf.to_screen((p + q) * 0.5), format::number(a.distance(*b), 2));
        }
    }
    if resp.clicked() {
        match points.as_slice() {
            [] => state.interaction.points.push(s.point),
            [a] if a.distance(s.point) > 1e-3 => state.interaction.points.push(s.point),
            [a, b] => {
                let n = (*b - *a).normalize_or_zero().perp();
                let dim = cc_core::Dimension { view: view.id, a: *a, b: *b, offset: (raw - *a).dot(n) };
                let level = level.or_else(|| state.project().levels.first().map(|l| l.id));
                if let Some(level) = level {
                    state.create_element(t!("tx-create-dimension"), level, type_id, ElementKind::Dimension(dim));
                }
                state.interaction.points.clear();
            }
            _ => {}
        }
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
    let s = snap_at(state, Some(level), raw, anchor, &[], xf.scale, shift);
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
        temp_dimension(painter, xf, a, p, state.interaction.prev_dir, &state.length_input, half);
    }

    let enter = ui.input(|i| i.key_pressed(Key::Enter)) && !state.length_input.is_empty();
    if resp.clicked() || enter {
        match anchor {
            None => state.interaction.anchor = Some(p),
            Some(a) if a.distance(p) > 0.01 => {
                let top = match state.wall_top_level {
                    Some(level) => cc_core::WallTop::Level { level, offset: 0.0 },
                    None => cc_core::WallTop::Unconnected,
                };
                let wall = Wall {
                    start: a,
                    end: p,
                    base_offset: 0.0,
                    height: state.wall_height,
                    top,
                    location_line: state.wall_location_line,
                    ..Default::default()
                };
                state.create_element(t!("tx-create-wall"), level, type_id, ElementKind::Wall(wall));
                state.interaction.anchor = Some(p);
                // The next wall's angle is measured from this one (as an interior angle).
                state.interaction.prev_dir = Some(a - p);
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
        let s = snap_at(state, Some(level), raw, pts.last().copied(), &pts, xf.scale, shift);
        draw_snap(painter, xf, s);
        if let Some(last) = pts.last() {
            painter.line_segment([xf.to_screen(*last), xf.to_screen(s.point)], Stroke::new(1.0, PREVIEW));
            let previous = pts.len().checked_sub(2).and_then(|i| pts.get(i)).map(|q| *q - *last);
            temp_dimension(painter, xf, *last, s.point, previous, "", 0.0);
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
    let s = snap_at(state, level, raw, state.interaction.anchor, &[], xf.scale, shift);
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

/// Space on a selection: components and columns turn 90° about their
/// insertion point, doors and windows flip to the other side of the wall.
/// Returns whether anything changed (one undo step).
pub fn rotate_selection(state: &mut AppState) -> bool {
    let ids: Vec<ElementId> = state.selection.iter().copied().collect();
    let turnable = ids.iter().any(|id| {
        matches!(
            state.project().element(*id).map(|e| &e.kind),
            Some(ElementKind::Component(_) | ElementKind::Column(_) | ElementKind::Door(_) | ElementKind::Window(_))
        )
    });
    if !turnable {
        return false;
    }
    state.doc.edit(t!("tx-rotate"), |e| {
        for id in ids {
            let Some(el) = e.element_mut(id) else { continue };
            match &mut el.kind {
                ElementKind::Component(c) => c.rotation_deg = (c.rotation_deg + 90.0).rem_euclid(360.0),
                ElementKind::Column(c) => c.rotation_deg = (c.rotation_deg + 90.0).rem_euclid(360.0),
                ElementKind::Door(h) | ElementKind::Window(h) => h.flip_facing = !h.flip_facing,
                _ => {}
            }
        }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100 px per meter: the snap radius is 0.1 m.
    const SCALE: f64 = 100.0;

    fn empty() -> (Project, LevelId) {
        let p = crate::actions::new_project();
        let level = p.levels[0].id;
        (p, level)
    }

    #[test]
    fn two_guides_cross_at_an_alignment() {
        let (p, level) = empty();
        // Acquired points (0, 0) and (5, 3): their vertical and horizontal guides cross at (5, 0) and (0, 3).
        let tracked = [DVec2::new(0.0, 0.0), DVec2::new(5.0, 3.0)];
        let s = snap(&p, Some(level), DVec2::new(5.04, 0.03), None, &[], &tracked, SCALE, false);
        assert_eq!(s.kind, SnapKind::Intersection);
        assert!(s.point.distance(DVec2::new(5.0, 0.0)) < 1e-12, "{}", s.point);
        assert!(s.guides.iter().all(Option::is_some));
    }

    #[test]
    fn the_drawing_direction_meets_a_guide() {
        let (p, level) = empty();
        // Drawing from (0, 0) at 30°: the line meets the vertical guide of (4, 9) at x = 4.
        let anchor = DVec2::ZERO;
        let dir = DVec2::from_angle(30f64.to_radians());
        let target = anchor + dir * (4.0 / dir.x);
        let raw = target + dir.perp() * 0.02 + DVec2::new(0.03, 0.0);
        let s = snap(&p, Some(level), raw, Some(anchor), &[], &[DVec2::new(4.0, 9.0)], SCALE, false);
        assert_eq!(s.kind, SnapKind::Alignment);
        assert!(s.point.distance(target) < 1e-9, "{} vs {target}", s.point);
    }

    #[test]
    fn space_turns_components_and_flips_doors() {
        let mut state = AppState::new(cc_core::Document::new(crate::home::sample_project("casa_simples").unwrap()));
        let pick = |state: &AppState, f: fn(&ElementKind) -> bool| {
            state.project().elements.values().find(|e| f(&e.kind)).map(|e| e.id).unwrap()
        };
        let comp = pick(&state, |k| matches!(k, ElementKind::Component(_)));
        let door = pick(&state, |k| matches!(k, ElementKind::Door(_)));
        let wall = pick(&state, |k| matches!(k, ElementKind::Wall(_)));
        let before = state.project().element(comp).cloned().unwrap();
        state.selection = [comp, door, wall].into_iter().collect();
        let depth = state.doc.undo_depth();
        assert!(rotate_selection(&mut state));
        assert_eq!(state.doc.undo_depth(), depth + 1);
        let (ElementKind::Component(a), Some(ElementKind::Component(b))) =
            (&before.kind, state.project().element(comp).map(|e| &e.kind))
        else {
            panic!("component")
        };
        assert_eq!(b.rotation_deg, (a.rotation_deg + 90.0).rem_euclid(360.0));
        assert_eq!(a.position, b.position);
        assert!(matches!(state.project().element(door).map(|e| &e.kind), Some(ElementKind::Door(h)) if h.flip_facing));
        // Nothing turnable selected: no undo step.
        state.selection = [wall].into_iter().collect();
        assert!(!rotate_selection(&mut state));
    }

    #[test]
    fn far_from_guides_the_pointer_is_free() {
        let (p, level) = empty();
        let s = snap(&p, Some(level), DVec2::new(2.33, 1.71), None, &[], &[DVec2::new(5.0, 5.0)], SCALE, false);
        assert_eq!(s.kind, SnapKind::Free);
        assert!(s.guides.iter().all(Option::is_none));
        // Endpoints still win over alignments.
        let s =
            snap(&p, Some(level), DVec2::new(5.01, 5.0), None, &[DVec2::new(5.0, 5.0)], &[DVec2::ZERO], SCALE, false);
        assert_eq!(s.kind, SnapKind::Endpoint);
    }
}
