//! The modify tools in the 3D view: move, copy, array, rotate and mirror
//! the selection, align and join geometry. Points are picked on a
//! horizontal working plane (the level of the elements changed, then the
//! height of the first point picked) or snapped to model points near the
//! pointer on screen: corners of solids, column and component insertion
//! points, wall ends and grid crossings. Every result is a plan transform
//! run through the same engine commands as in plans, so elements keep their
//! levels: a level 2 column can be moved or aligned onto a ground-floor one.

use crate::paint::PREVIEW;
use crate::state::AppState;
use crate::t;
use crate::tools::Tool;
use cc_core::{CameraState, ElementId, ElementKind, PlanTransform, Project, ViewId, ViewKind};
use cc_geom::{ModelGeometry, Ray};
use cc_views::format;
use egui::{Painter, Pos2, Rect, Response, Stroke, Ui};
use glam::{DVec2, DVec3, Mat4, Vec4};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Snap radius on screen, points.
const SNAP_PX: f32 = 12.0;
/// Solids with more vertices than this give no corner snaps (imported meshes).
const MAX_SOLID_VERTS: usize = 512;
/// Preview edges drawn at most.
const MAX_GHOST_EDGES: usize = 20_000;
/// Working-plane hits farther than this from the eye are ignored (a plane seen edge-on).
const MAX_PLANE_DISTANCE: f64 = 10_000.0;

/// Snap points, keyed by (revision, temporary visibility revision).
type SnapCache = ((u64, u64), Arc<Vec<DVec3>>);
/// Outline edges, keyed by (revision, elements).
type GhostCache = ((u64, Vec<ElementId>), Arc<Vec<(DVec3, DVec3)>>);

/// Cached pick data of one 3D view.
#[derive(Default)]
pub struct PickCache {
    snaps: Option<SnapCache>,
    /// Outline edges of the elements a tool acts on.
    ghost: Option<GhostCache>,
}

/// Whether `tool` works in the 3D view (it then takes the clicks there).
pub fn works_in_3d(tool: &Tool) -> bool {
    matches!(
        tool,
        Tool::Move
            | Tool::Copy
            | Tool::Array
            | Tool::Rotate
            | Tool::Mirror { .. }
            | Tool::Align
            | Tool::JoinGeometry { .. }
    )
}

/// Whether the active view is a 3D view.
pub fn active_3d(state: &AppState) -> bool {
    let view = state.active_view.and_then(|v| state.project().view(v));
    view.is_some_and(|v| matches!(v.kind, ViewKind::ThreeD { .. }))
}

/// The camera of a 3D view as drawn in `rect`.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub rect: Rect,
    pub camera: CameraState,
    aspect: f32,
    view_proj: Mat4,
}

impl Frame {
    pub fn new(rect: Rect, camera: CameraState) -> Self {
        let aspect = rect.width() / rect.height().max(1.0);
        Self { rect, camera, aspect, view_proj: cc_render::view_proj(&camera, aspect) }
    }

    /// The ray through a screen position.
    pub fn ray(&self, pos: Pos2) -> Ray {
        let nx = (pos.x - self.rect.left()) / self.rect.width().max(1.0) * 2.0 - 1.0;
        let ny = 1.0 - (pos.y - self.rect.top()) / self.rect.height().max(1.0) * 2.0;
        cc_render::pick_ray(&self.camera, self.aspect, nx, ny)
    }

    /// Where a world point shows on screen; `None` behind the eye.
    pub fn screen(&self, p: DVec3) -> Option<Pos2> {
        let clip = self.view_proj * Vec4::new(p.x as f32, p.y as f32, p.z as f32, 1.0);
        if clip.w <= 1e-6 {
            return None;
        }
        let (x, y) = (clip.x / clip.w, clip.y / clip.w);
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let r = self.rect;
        Some(Pos2::new(r.left() + (x + 1.0) * 0.5 * r.width(), r.top() + (1.0 - y) * 0.5 * r.height()))
    }
}

/// Where `ray` meets the horizontal plane at height `z` (in front of its origin).
pub fn ray_plane_z(ray: Ray, z: f64) -> Option<DVec3> {
    if ray.dir.z.abs() < 1e-9 {
        return None;
    }
    let t = (z - ray.origin.z) / ray.dir.z;
    (t > 0.0 && t < MAX_PLANE_DISTANCE && t.is_finite()).then(|| ray.origin + ray.dir * t)
}

/// The point that shows nearest to `cursor`, within `radius` points; of
/// points showing (about) as near, the one closest to the eye.
pub fn nearest_on_screen(
    frame: &Frame,
    points: impl IntoIterator<Item = DVec3>,
    cursor: Pos2,
    radius: f32,
) -> Option<DVec3> {
    let eye = frame.camera.eye();
    let mut best: Option<(f32, f64, DVec3)> = None;
    for p in points {
        let Some(s) = frame.screen(p) else { continue };
        let d = s.distance(cursor);
        if d > radius {
            continue;
        }
        let depth = p.distance(eye);
        let better = best.is_none_or(|(bd, bdepth, _)| d < bd - 1.0 || (d <= bd + 1.0 && depth < bdepth));
        if better {
            best = Some((d, depth, p));
        }
    }
    best.map(|(_, _, p)| p)
}

/// Model points to snap to: corners of the solids shown, the insertion
/// points of columns and components and the ends of walls, at the bottom
/// and top of each.
pub fn snap_points(project: &Project, geom: &ModelGeometry, shown: &mut dyn FnMut(ElementId) -> bool) -> Vec<DVec3> {
    let mut out = Vec::new();
    for s in geom.solids.iter().filter(|s| shown(s.element)) {
        let count: usize = s.faces.iter().map(|f| f.verts.len()).sum();
        if count <= MAX_SOLID_VERTS {
            out.extend(s.faces.iter().flat_map(|f| f.verts.iter().copied()));
        }
    }
    for e in project.elements.values().filter(|e| shown(e.id)) {
        let plan: Vec<DVec2> = match &e.kind {
            ElementKind::Column(c) => vec![c.position],
            ElementKind::Component(c) => vec![c.position],
            ElementKind::Wall(w) => vec![w.start, w.end],
            _ => continue,
        };
        let Some((lo, hi)) = element_bounds(geom, e.id) else { continue };
        for p in plan {
            out.extend([p.extend(lo.z), p.extend(hi.z)]);
        }
    }
    out.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
    out.dedup_by(|a, b| a.distance(*b) < 1e-9);
    out
}

/// The box around an element's solids.
fn element_bounds(geom: &ModelGeometry, id: ElementId) -> Option<(DVec3, DVec3)> {
    geom.solids_of(id).filter_map(|s| s.bounds()).reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
}

/// Keep the plan offset from `a` to `p` along its main axis (Shift).
pub fn ortho(a: DVec2, p: DVec2) -> DVec2 {
    let v = p - a;
    if v.x.abs() >= v.y.abs() { DVec2::new(p.x, a.y) } else { DVec2::new(a.x, p.y) }
}

/// The element under `pos` among those the view shows.
fn pick_element(state: &AppState, view_id: ViewId, ray: Ray) -> Option<(ElementId, DVec3)> {
    let geom = state.session.geom_for(view_id);
    let project = &state.session.doc.project;
    let hit = match project.view(view_id) {
        Some(view) => {
            let temporary = state.session.temporary(view_id);
            let mut resolver = cc_views::graphics::Resolver::new(project, geom, view, temporary);
            geom.pick_where(ray, &mut |e| resolver.style(e).is_some())
        }
        None => geom.pick(ray, &[]),
    };
    hit.map(|(id, t)| (id, ray.origin + ray.dir * t))
}

fn cache_mut(state: &mut AppState, view_id: ViewId) -> Option<&mut PickCache> {
    state.view3d.get_mut(&view_id).map(|v| &mut v.picks)
}

/// The snap points of the view, built once per model revision.
fn cached_snaps(state: &mut AppState, view_id: ViewId) -> Arc<Vec<DVec3>> {
    let key = (state.session.doc.revision(), state.session.temporary_revision());
    if let Some((k, points)) = state.view3d.get(&view_id).and_then(|v| v.picks.snaps.as_ref())
        && *k == key
    {
        return points.clone();
    }
    let geom = state.session.geom_for(view_id);
    let project = &state.session.doc.project;
    let points = Arc::new(match project.view(view_id) {
        Some(view) => {
            let temporary = state.session.temporary(view_id);
            let mut resolver = cc_views::graphics::Resolver::new(project, geom, view, temporary);
            snap_points(project, geom, &mut |e| resolver.style(e).is_some())
        }
        None => snap_points(project, geom, &mut |_| true),
    });
    if let Some(cache) = cache_mut(state, view_id) {
        cache.snaps = Some((key, points.clone()));
    }
    points
}

/// The outline edges of `ids`, built once per model revision.
fn cached_ghost(state: &mut AppState, view_id: ViewId, ids: &BTreeSet<ElementId>) -> Arc<Vec<(DVec3, DVec3)>> {
    let key = (state.session.doc.revision(), ids.iter().copied().collect::<Vec<_>>());
    if let Some((k, edges)) = state.view3d.get(&view_id).and_then(|v| v.picks.ghost.as_ref())
        && *k == key
    {
        return edges.clone();
    }
    let geom = state.session.geom_for(view_id);
    let mut edges = Vec::new();
    for s in geom.solids.iter().filter(|s| ids.contains(&s.element)) {
        edges.extend(s.feature_edges().into_iter().flatten());
        if edges.len() > MAX_GHOST_EDGES {
            edges.truncate(MAX_GHOST_EDGES);
            break;
        }
    }
    let edges = Arc::new(edges);
    if let Some(cache) = cache_mut(state, view_id) {
        cache.ghost = Some((key, edges.clone()));
    }
    edges
}

/// Height of the working plane: the first point picked, else the level of
/// the elements the tool acts on, else zero.
fn plane_z(state: &AppState) -> f64 {
    if let Some(z) = state.interaction.plane_z {
        return z;
    }
    let project = state.project();
    let level = state.interaction.targets.iter().find_map(|id| project.element(*id)).map(|e| e.level);
    level.map_or(0.0, |l| project.level_elevation(l))
}

/// A picked point and whether it snapped to a model point.
#[derive(Debug, Clone, Copy)]
struct Picked {
    point: DVec3,
    snapped: bool,
}

/// The point under the pointer: a model point near it on screen, else its
/// hit on the working plane (along the main axis from `anchor` with Shift).
fn pick_point(
    state: &mut AppState,
    view_id: ViewId,
    frame: &Frame,
    cursor: Pos2,
    ortho_from: Option<DVec2>,
) -> Option<Picked> {
    let z = plane_z(state);
    let snaps = cached_snaps(state, view_id);
    let grids: Vec<DVec3> = crate::tools::grid_crossings(state.project()).into_iter().map(|p| p.extend(z)).collect();
    if let Some(p) = nearest_on_screen(frame, snaps.iter().chain(&grids).copied(), cursor, SNAP_PX) {
        return Some(Picked { point: p, snapped: true });
    }
    let hit = ray_plane_z(frame.ray(cursor), z)?;
    let plan = match ortho_from {
        Some(a) => ortho(a, hit.truncate()),
        None => hit.truncate(),
    };
    Some(Picked { point: plan.extend(z), snapped: false })
}

/// Everything a 3D tool draws with.
struct Ctx<'a> {
    ui: &'a Ui,
    resp: &'a Response,
    painter: &'a Painter,
    frame: Frame,
    view_id: ViewId,
}

impl Ctx<'_> {
    fn line(&self, a: DVec3, b: DVec3, width: f32) {
        if let (Some(a), Some(b)) = (self.frame.screen(a), self.frame.screen(b)) {
            self.painter.line_segment([a, b], Stroke::new(width, PREVIEW));
        }
    }

    fn dashed(&self, a: DVec3, b: DVec3) {
        // Past both ends a little, as a reference line.
        let d = b - a;
        if let (Some(a), Some(b)) = (self.frame.screen(a - d * 0.25), self.frame.screen(b + d * 0.25)) {
            self.painter.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(1.5, PREVIEW), 8.0, 4.0));
        }
    }

    fn marker(&self, p: Picked) {
        let Some(c) = self.frame.screen(p.point) else { return };
        let stroke = Stroke::new(1.5, PREVIEW);
        if p.snapped {
            let r = egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0));
            self.painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
        } else {
            self.painter.line_segment([c - egui::vec2(6.0, 0.0), c + egui::vec2(6.0, 0.0)], stroke);
            self.painter.line_segment([c - egui::vec2(0.0, 6.0), c + egui::vec2(0.0, 6.0)], stroke);
        }
    }

    /// The targets' outline, transformed in plan.
    fn ghost(&self, edges: &[(DVec3, DVec3)], t: &PlanTransform) {
        let moved = |p: DVec3| t.apply(p.truncate()).extend(p.z);
        for (a, b) in edges {
            self.line(moved(*a), moved(*b), 1.0);
        }
    }
}

/// Run the active tool in a 3D view (after the scene is painted).
pub fn handle(ui: &Ui, resp: &Response, painter: &Painter, state: &mut AppState, view_id: ViewId, frame: Frame) {
    let ctx = Ctx { ui, resp, painter, frame, view_id };
    let tool = state.tool.clone();
    match tool {
        Tool::Align => align(&ctx, state),
        Tool::JoinGeometry { unjoin } => join(&ctx, state, unjoin),
        Tool::Move | Tool::Copy | Tool::Array | Tool::Rotate | Tool::Mirror { .. } => {
            let targets: BTreeSet<ElementId> = state.interaction.targets.iter().copied().collect();
            if targets.is_empty() {
                state.set_tool(Tool::Select);
                return;
            }
            match tool {
                Tool::Rotate => rotate(&ctx, state, &targets),
                Tool::Mirror { draw } => mirror(&ctx, state, draw, &targets),
                _ => base_target(&ctx, state, &tool, &targets),
            }
        }
        _ => {}
    }
}

/// Move, copy and array: a base point, then a target point (or a typed distance).
fn base_target(ctx: &Ctx, state: &mut AppState, tool: &Tool, targets: &BTreeSet<ElementId>) {
    let Some(cursor) = ctx.resp.hover_pos() else { return };
    let shift = ctx.ui.input(|i| i.modifiers.shift);
    let anchor = state.interaction.anchor;
    let Some(picked) = pick_point(state, ctx.view_id, &ctx.frame, cursor, anchor.filter(|_| shift)) else { return };
    ctx.marker(picked);
    let Some(a) = anchor else {
        if ctx.resp.clicked() {
            state.interaction.anchor = Some(picked.point.truncate());
            state.interaction.plane_z = Some(picked.point.z);
            state.message = t!("hint-target-point");
        }
        return;
    };
    let z = plane_z(state);
    let mut p = picked.point.truncate();
    let mut end = picked.point;
    if !state.typed.is_empty() {
        p = state.typed.apply(a, p, None);
        end = p.extend(z);
    }
    let count = state.modify.count.max(2);
    let edges = cached_ghost(state, ctx.view_id, targets);
    let steps = if *tool == Tool::Array { count - 1 } else { 1 };
    for k in 1..=steps {
        ctx.ghost(&edges, &PlanTransform::translate((p - a) * k as f64));
    }
    let start = a.extend(z);
    ctx.line(start, end, 1.5);
    if let (Some(s), Some(e)) = (ctx.frame.screen(start), ctx.frame.screen(end)) {
        crate::tools::label(ctx.painter, s + (e - s) * 0.5, format::length(a.distance(p)));
    }
    let enter = ctx.ui.input(|i| i.key_pressed(egui::Key::Enter)) && !state.typed.is_empty();
    if (ctx.resp.clicked() || enter) && a.distance(p) > 1e-6 {
        crate::modify::apply_base_target(state, tool, targets, p - a);
    }
}

/// Center of the targets in plan (of their solids' box).
fn center_of(state: &AppState, view_id: ViewId, targets: &BTreeSet<ElementId>) -> Option<DVec2> {
    let geom = state.session.geom_for(view_id);
    let boxes = targets.iter().filter_map(|id| element_bounds(geom, *id));
    let (lo, hi) = boxes.reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))?;
    Some(((lo + hi) * 0.5).truncate())
}

/// Rotate: a reference direction from the center, then the target direction.
fn rotate(ctx: &Ctx, state: &mut AppState, targets: &BTreeSet<ElementId>) {
    let Some(center) = state.interaction.center.or_else(|| center_of(state, ctx.view_id, targets)) else { return };
    state.interaction.center = Some(center);
    let z = plane_z(state);
    let c3 = center.extend(z);
    ctx.marker(Picked { point: c3, snapped: false });
    if let Some(c) = ctx.frame.screen(c3) {
        ctx.painter.circle_stroke(c, 5.0, Stroke::new(1.5, PREVIEW));
    }
    let Some(cursor) = ctx.resp.hover_pos() else { return };
    let Some(picked) = pick_point(state, ctx.view_id, &ctx.frame, cursor, None) else { return };
    ctx.marker(picked);
    let p = picked.point.truncate();
    if state.interaction.placing_center {
        if ctx.resp.clicked() {
            state.interaction.center = Some(p);
            state.interaction.plane_z = Some(picked.point.z);
            state.interaction.placing_center = false;
            state.interaction.points.clear();
        }
        return;
    }
    let Some(reference) = state.interaction.points.first().copied() else {
        ctx.line(c3, p.extend(z), 1.0);
        if ctx.resp.clicked() && p.distance(center) > 1e-6 {
            state.interaction.points.push(p);
            state.message = t!("hint-rotate-target");
        }
        return;
    };
    let r = (reference - center).normalize_or(DVec2::X);
    let v = (p - center).normalize_or(r);
    let angle = r.angle_to(v);
    let edges = cached_ghost(state, ctx.view_id, targets);
    ctx.ghost(&edges, &PlanTransform::rotate_about(center, angle));
    let radius = reference.distance(center).max(1e-6);
    ctx.line(c3, reference.extend(z), 1.0);
    ctx.line(c3, (center + v * radius).extend(z), 1.0);
    if let Some(s) = ctx.frame.screen(picked.point) {
        crate::tools::label(ctx.painter, s, format::angle(angle.to_degrees()));
    }
    if ctx.resp.clicked() {
        crate::modify::apply_rotation(state, targets, center, angle);
    }
}

/// Mirror across a picked wall, grid, beam or railing, or across an axis drawn with two clicks.
fn mirror(ctx: &Ctx, state: &mut AppState, draw: bool, targets: &BTreeSet<ElementId>) {
    let Some(cursor) = ctx.resp.hover_pos() else { return };
    let (axis, z) = if draw {
        let Some(picked) = pick_point(state, ctx.view_id, &ctx.frame, cursor, None) else { return };
        ctx.marker(picked);
        let p = picked.point.truncate();
        match state.interaction.anchor {
            None => {
                if ctx.resp.clicked() {
                    state.interaction.anchor = Some(p);
                    state.interaction.plane_z = Some(picked.point.z);
                }
                return;
            }
            Some(a) if a.distance(p) > 1e-6 => (Some((a, p)), plane_z(state)),
            Some(_) => return,
        }
    } else {
        let hit = pick_element(state, ctx.view_id, ctx.frame.ray(cursor));
        let axis = hit.and_then(|(id, _)| crate::modify::axis_of(state, id));
        (axis, hit.map_or(0.0, |(_, h)| h.z))
    };
    let Some((a, b)) = axis else { return };
    ctx.dashed(a.extend(z), b.extend(z));
    let edges = cached_ghost(state, ctx.view_id, targets);
    ctx.ghost(&edges, &PlanTransform::mirror_across(a, b - a));
    if ctx.resp.clicked() {
        crate::modify::apply_mirror(state, targets, a, b);
    }
}

/// The plan line an element aligns by when picked at `at`: its location
/// line, or the edge of its outline nearest to the pick.
fn align_line(state: &AppState, view_id: ViewId, id: ElementId, at: DVec2) -> Option<(DVec2, DVec2)> {
    match state.project().element(id).and_then(cc_core::wall_ops::path_of) {
        Some(cc_core::wall_ops::Path::Line(a, b)) => Some((a, b)),
        Some(cc_core::wall_ops::Path::Arc(_)) => None,
        None => state.session.geom_for(view_id).nearest_plan_edge(id, at),
    }
}

/// Align: click the reference element's face (or line), then the face of the
/// element to move; it moves in plan onto the reference, whatever its level.
fn align(ctx: &Ctx, state: &mut AppState) {
    let Some(cursor) = ctx.resp.hover_pos() else { return };
    let hit =
        pick_element(state, ctx.view_id, ctx.frame.ray(cursor)).filter(|(id, _)| crate::modify::alignable(state, *id));
    let first = state.interaction.targets.first().copied();
    let first_pick = state.interaction.points.first().copied();
    if let (Some(r), Some(rp)) = (first, first_pick)
        && let Some((a, b)) = align_line(state, ctx.view_id, r, rp)
    {
        let z = plane_z(state);
        ctx.dashed(a.extend(z), b.extend(z));
    }
    if let Some((id, h)) = hit
        && let Some((a, b)) = align_line(state, ctx.view_id, id, h.truncate())
    {
        ctx.line(a.extend(h.z), b.extend(h.z), 2.5);
    }
    if !ctx.resp.clicked() {
        return;
    }
    let Some((id, h)) = hit else {
        state.message = if first.is_some() { t!("hint-align-3d-second") } else { t!("hint-align-3d") };
        return;
    };
    match (first, first_pick) {
        (Some(r), Some(rp)) if r != id => {
            if state.project().is_pinned(id) {
                state.message = t!("msg-pinned-skipped", "count" => 1);
                return;
            }
            crate::actions::run_command(state, crate::modify::align_command(id, h.truncate(), r, rp));
            state.interaction.targets.clear();
            state.interaction.points.clear();
            state.interaction.plane_z = None;
            if state.message.is_empty() {
                state.message = t!("hint-align-3d");
            }
        }
        (Some(_), Some(_)) => {}
        _ => {
            state.interaction.targets = vec![id];
            state.interaction.points = vec![h.truncate()];
            state.interaction.plane_z = Some(h.z);
            state.message = t!("hint-align-3d-second");
        }
    }
}

/// Join or unjoin geometry: the first element clicked stays, each further one
/// is joined with it (or unjoined from it) until Esc.
fn join(ctx: &Ctx, state: &mut AppState, unjoin: bool) {
    let Some(cursor) = ctx.resp.hover_pos() else { return };
    let joinable =
        |id: ElementId| state.project().element(id).is_some_and(|e| cc_core::can_join_geometry(e.category()));
    let hovered = pick_element(state, ctx.view_id, ctx.frame.ray(cursor)).map(|(id, _)| id).filter(|id| joinable(*id));
    let lit: BTreeSet<ElementId> = state.interaction.targets.first().copied().into_iter().collect();
    if !lit.is_empty() {
        let edges = cached_ghost(state, ctx.view_id, &lit);
        ctx.ghost(&edges, &PlanTransform::IDENTITY);
    }
    if ctx.resp.clicked() {
        crate::modify::join_pick(state, hovered, unjoin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        let rect = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(800.0, 600.0));
        Frame::new(rect, CameraState::default())
    }

    #[test]
    fn rays_meet_horizontal_planes() {
        let ray = Ray { origin: DVec3::new(1.0, 2.0, 10.0), dir: DVec3::new(0.0, 0.6, -0.8) };
        let hit = ray_plane_z(ray, 2.0).unwrap();
        assert!(hit.distance(DVec3::new(1.0, 8.0, 2.0)) < 1e-9, "{hit}");
        // Behind the origin, or along a ray parallel to the plane: no hit.
        assert!(ray_plane_z(ray, 12.0).is_none());
        let flat = Ray { origin: DVec3::ZERO, dir: DVec3::X };
        assert!(ray_plane_z(flat, 1.0).is_none());
    }

    /// Projecting a point and casting the ray back through its screen position finds it again.
    #[test]
    fn screen_projection_inverts_the_pick_ray() {
        let f = frame();
        // The target is at the center of the view.
        let c = f.screen(f.camera.target).unwrap();
        assert!(c.distance(f.rect.center()) < 0.5, "{c:?}");
        for p in [DVec3::new(3.0, -2.0, 0.0), DVec3::new(-4.0, 5.0, 3.0), DVec3::new(0.5, 0.5, 6.0)] {
            let s = f.screen(p).unwrap();
            let hit = ray_plane_z(f.ray(s), p.z).unwrap();
            assert!(hit.distance(p) < 1e-3, "{p} came back as {hit}");
        }
        // Behind the eye nothing shows.
        let eye = f.camera.eye();
        let behind = eye + (eye - f.camera.target);
        assert!(f.screen(behind).is_none());
    }

    #[test]
    fn snapping_takes_the_nearest_point_on_screen() {
        let f = frame();
        let (a, b) = (DVec3::new(0.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 0.0));
        let sa = f.screen(a).unwrap();
        let near_a = sa + egui::vec2(4.0, -3.0);
        assert_eq!(nearest_on_screen(&f, [a, b], near_a, SNAP_PX), Some(a));
        // Near the other point on screen, that one.
        let sb = f.screen(b).unwrap();
        assert_eq!(nearest_on_screen(&f, [a, b], sb + egui::vec2(-2.0, 5.0), SNAP_PX), Some(b));
        assert_eq!(nearest_on_screen(&f, [a, b], sa + egui::vec2(0.0, 200.0), SNAP_PX), None);
        // Two points on one line of sight: the one nearer the eye wins.
        let eye = f.camera.eye();
        let near = eye + (a - eye) * 0.5;
        assert_eq!(nearest_on_screen(&f, [a, near], sa, SNAP_PX), Some(near));
    }

    #[test]
    fn shift_keeps_the_main_axis() {
        let a = DVec2::new(1.0, 1.0);
        assert_eq!(ortho(a, DVec2::new(4.0, 1.5)), DVec2::new(4.0, 1.0));
        assert_eq!(ortho(a, DVec2::new(0.5, -3.0)), DVec2::new(1.0, -3.0));
    }

    /// A column gives its corners and its center at its base and top.
    #[test]
    fn columns_snap_at_corners_and_center() {
        let mut s = cc_engine::Session::default();
        let out = s.run(cc_engine::commands::create::ColumnCreate { position: [3.0, 4.0], ..Default::default() });
        let id = ElementId(out.unwrap()["id"].as_u64().unwrap());
        let (project, geom) = s.project_and_geometry();
        let points = snap_points(project, geom, &mut |e| e == id);
        let (lo, hi) = element_bounds(geom, id).unwrap();
        for z in [lo.z, hi.z] {
            assert!(points.iter().any(|p| p.distance(DVec3::new(3.0, 4.0, z)) < 1e-9), "center at {z}");
            assert!(points.iter().any(|p| p.distance(DVec3::new(lo.x, lo.y, z)) < 1e-9), "corner at {z}");
        }
        assert!(snap_points(project, geom, &mut |_| false).is_empty());
    }
}
