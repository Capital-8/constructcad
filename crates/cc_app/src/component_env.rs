//! The component modeling environment: a workspace of its own where one
//! component is shaped. It replaces the project's views with the
//! component's plan, front, side and 3D views, the ribbon with the *Create*
//! tab and the left panel with the component's parameters, types and the
//! selected reference plane or dimension.
//!
//! Every change is an engine command on the component (`component_*`), so
//! it applies to every type of the component and undoes like any edit.
//! *Finish* keeps the changes; *Cancel* undoes everything done since the
//! environment opened.

use crate::paint::{PaintOptions, Xf, paint_drawing};
use crate::state::AppState;
use crate::t;
use crate::theme::{self, PALETTE};
use cc_core::component::SketchPlane;
use cc_core::{ElementTypeId, PlaneAxis, TypeData};
use cc_engine::Ref;
use cc_engine::commands::component as cmd;
use cc_views::{ComponentView, ComponentViewKind, component_view};
use egui::{Color32, Pos2, Sense, Stroke, Ui, vec2};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use glam::DVec2;
use std::collections::BTreeMap;

/// Scale the views are drawn at (text and dimension sizes).
const VIEW_SCALE: u32 = 10;
/// Pick distance, screen points.
const PICK_PX: f32 = 6.0;

/// What is selected in the environment.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    Plane(String),
    Dimension(usize),
}

/// The tool of the environment.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum EnvTool {
    #[default]
    Select,
    /// Two clicks along the plane's line in a view.
    RefPlane { first: Option<(ComponentViewKind, DVec2)> },
    /// Two parallel planes, then the line's place.
    Dimension { first: Option<(ComponentViewKind, String)> },
}

/// Pan and zoom of one view.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Camera {
    center: DVec2,
    /// Screen points per meter.
    scale: f64,
}

pub struct Env {
    /// The type the environment shows (its parameter values).
    pub type_id: ElementTypeId,
    pub dock: DockState<ComponentViewKind>,
    pub pick: Option<Pick>,
    pub tool: EnvTool,
    /// Undo depth when the environment opened (Cancel goes back to it).
    pub start_depth: usize,
    cameras: BTreeMap<u8, Camera>,
    /// Text being typed in the panel's fields, until committed.
    drafts: BTreeMap<String, String>,
}

impl Env {
    fn new(type_id: ElementTypeId, start_depth: usize) -> Self {
        let mut dock = DockState::new(vec![ComponentViewKind::Plan]);
        let surface = dock.main_surface_mut();
        let [left, right] = surface.split_right(NodeIndex::root(), 0.5, vec![ComponentViewKind::Front]);
        surface.split_below(left, 0.5, vec![ComponentViewKind::Side]);
        surface.split_below(right, 0.5, vec![ComponentViewKind::ThreeD]);
        Self {
            type_id,
            dock,
            pick: None,
            tool: EnvTool::Select,
            start_depth,
            cameras: BTreeMap::new(),
            drafts: BTreeMap::new(),
        }
    }
}

/// Open the environment on the component of type `type_id`.
pub fn open(state: &mut AppState, type_id: ElementTypeId) {
    if !matches!(state.project().element_type(type_id).map(|t| &t.data), Some(TypeData::Component { .. })) {
        return;
    }
    state.set_tool(crate::tools::Tool::Select);
    state.selection.clear();
    state.component_env = Some(Env::new(type_id, state.session.doc.undo_depth()));
}

/// The type of the selected component element, or the one the component tool places.
pub fn target_type(state: &AppState) -> Option<ElementTypeId> {
    let p = state.project();
    let from_selection = state.selection.iter().find_map(|id| {
        let e = p.element(*id)?;
        matches!(p.element_type(e.type_id).map(|t| &t.data), Some(TypeData::Component { .. })).then_some(e.type_id)
    });
    from_selection.or(match state.tool {
        crate::tools::Tool::Component { type_id } => Some(type_id),
        _ => None,
    })
}

/// Save the component as a `.ccomp` file.
pub fn save_file(state: &mut AppState) {
    if let Some((name, _)) = component(state) {
        crate::actions::save_component(state, &name);
    }
}

/// Leave the environment keeping the changes.
pub fn finish(state: &mut AppState) {
    state.component_env = None;
}

/// Leave the environment undoing every change made in it.
pub fn cancel(state: &mut AppState) {
    if let Some(env) = state.component_env.take() {
        state.session.undo_to(env.start_depth);
    }
}

/// The component's definition, name and parameter values of the shown type.
fn component(state: &AppState) -> Option<(String, cc_core::ComponentDef)> {
    let env = state.component_env.as_ref()?;
    match state.project().element_type(env.type_id).map(|t| &t.data) {
        Some(TypeData::Component { component, def }) => Some((component.clone(), def.clone())),
        _ => None,
    }
}

fn type_ref(state: &AppState) -> Ref {
    Ref::Id(state.component_env.as_ref().map_or(0, |e| e.type_id.0))
}

/// The view of the environment's component in `kind`.
fn view(state: &AppState, kind: ComponentViewKind) -> Option<ComponentView> {
    let (_, def) = component(state)?;
    Some(component_view(&def, &BTreeMap::new(), kind, VIEW_SCALE))
}

// --- Picking (pure, tested) ---

/// The plane a two-click line in view `kind` makes: the axis it is square
/// to and its position. Mostly vertical lines make planes of constant u.
pub fn plane_from_line(kind: ComponentViewKind, a: DVec2, b: DVec2) -> Option<(PlaneAxis, f64)> {
    let d = b - a;
    if d.length() < 1e-9 {
        return None;
    }
    let vertical = d.y.abs() >= d.x.abs();
    let axis = match (kind, vertical) {
        (ComponentViewKind::Plan | ComponentViewKind::Front, true) => PlaneAxis::X,
        (ComponentViewKind::Side, true) => PlaneAxis::Y,
        (ComponentViewKind::Plan, false) => PlaneAxis::Y,
        (ComponentViewKind::Front | ComponentViewKind::Side, false) => PlaneAxis::Z,
        (ComponentViewKind::ThreeD, _) => return None,
    };
    let at = if vertical { (a.x + b.x) / 2.0 } else { (a.y + b.y) / 2.0 };
    // To the millimeter.
    Some((axis, (at * 1000.0).round() / 1000.0))
}

/// The plane of `view` whose line passes within `tol` (model units) of `p`.
pub fn plane_at(view: &ComponentView, p: DVec2, tol: f64) -> Option<String> {
    view.planes
        .iter()
        .map(|pl| (cc_geom::polygon::closest_on_segment(pl.a, pl.b, p).0.distance(p), pl))
        .filter(|(d, _)| *d <= tol)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, pl)| pl.name.clone())
}

/// The dimension whose text is within `tol` of `p`.
pub fn dimension_at(view: &ComponentView, p: DVec2, tol: f64) -> Option<usize> {
    view.dimensions
        .iter()
        .map(|d| (d.text_pos.distance(p), d.index))
        .filter(|(dist, _)| *dist <= tol)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
}

fn view_name(kind: ComponentViewKind) -> &'static str {
    match kind.plane() {
        Some(SketchPlane::Plan) => "plan",
        Some(SketchPlane::Front) => "front",
        _ => "side",
    }
}

// --- Layout ---

/// The *Create* tab of the ribbon.
pub fn ribbon(ui: &mut Ui, state: &mut AppState) {
    use crate::commands::UiCommand::*;
    let title = component(state).map(|(name, _)| name).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(theme::title(t!("comp-env-title", "name" => title)));
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        crate::ribbon::env_group(ui, state, t!("group-selection"), &[Select]);
        crate::ribbon::env_group(ui, state, t!("comp-env-group-datum"), &[CompRefPlane, CompDimension]);
        crate::ribbon::env_group(ui, state, t!("comp-env-group-component"), &[CompSaveFile]);
        crate::ribbon::env_group(ui, state, t!("comp-env-group-editor"), &[CompFinish, CompCancel]);
    });
}

/// The hint of the active tool, for the options bar.
pub fn options(ui: &mut Ui, state: &AppState) {
    let Some(env) = &state.component_env else { return };
    let hint = match &env.tool {
        EnvTool::Select => t!("comp-env-hint-select"),
        EnvTool::RefPlane { first: None } => t!("comp-env-hint-plane"),
        EnvTool::RefPlane { first: Some(_) } => t!("comp-env-hint-plane-second"),
        EnvTool::Dimension { first: None } => t!("comp-env-hint-dimension"),
        EnvTool::Dimension { first: Some(_) } => t!("comp-env-hint-dimension-second"),
    };
    ui.label(theme::muted(hint));
}

/// The central area: the component's views in tabs.
pub fn central(ui: &mut Ui, state: &mut AppState) {
    let Some(mut dock) = state.component_env.as_mut().map(|e| std::mem::replace(&mut e.dock, DockState::new(vec![])))
    else {
        return;
    };
    DockArea::new(&mut dock)
        .id(egui::Id::new("component-env-dock"))
        .style(theme::dock_style(ui.style()))
        .show_leaf_collapse_buttons(false)
        .show_leaf_close_all_buttons(false)
        .show_close_buttons(false)
        .show_inside(ui, &mut Tabs { state });
    if let Some(env) = state.component_env.as_mut() {
        env.dock = dock;
    }
}

struct Tabs<'a> {
    state: &'a mut AppState,
}

impl TabViewer for Tabs<'_> {
    type Tab = ComponentViewKind;

    fn id(&mut self, tab: &mut ComponentViewKind) -> egui::Id {
        egui::Id::new(("component-env-view", *tab as u8))
    }

    fn title(&mut self, tab: &mut ComponentViewKind) -> egui::WidgetText {
        crate::i18n::tr(tab.key()).into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut ComponentViewKind) {
        canvas(ui, self.state, *tab);
    }

    fn scroll_bars(&self, _tab: &ComponentViewKind) -> [bool; 2] {
        [false, false]
    }
}

/// One view: pan (middle or right drag), zoom (wheel), picking and the tools.
fn canvas(ui: &mut Ui, state: &mut AppState, kind: ComponentViewKind) {
    let Some(view) = view(state, kind) else { return };
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, PALETTE.paper);
    let key = kind as u8;
    let Some(env) = state.component_env.as_mut() else { return };
    let mut cam = *env.cameras.entry(key).or_insert_with(|| fit(&view, rect));
    if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
        let d = resp.drag_delta();
        cam.center -= DVec2::new(d.x as f64, -d.y as f64) / cam.scale;
    }
    if let Some(hover) = resp.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let before = to_world(cam, rect, hover);
            cam.scale = (cam.scale * (1.0 + scroll as f64 * 0.002)).clamp(5.0, 50_000.0);
            cam.center += before - to_world(cam, rect, hover);
        }
    }
    if resp.double_clicked_by(egui::PointerButton::Middle) {
        cam = fit(&view, rect);
    }
    env.cameras.insert(key, cam);
    let xf = Xf { screen: rect.center(), origin: cam.center, scale: cam.scale };
    // Text stays readable when zoomed out.
    let px_per_mm = (cam.scale * f64::from(VIEW_SCALE) / 1000.0).max(4.5);
    let none = Default::default();
    paint_drawing(&painter, &view.drawing, &xf, &PaintOptions { highlight: &none, px_per_mm, mono: None, only: None });
    if !view.errors.is_empty() {
        painter.text(
            rect.left_bottom() + vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            view.errors.join(" · "),
            egui::FontId::proportional(12.0),
            PALETTE.danger,
        );
    }
    let tol = f64::from(PICK_PX) / cam.scale;
    let hover = resp.hover_pos().map(|p| xf.to_world(p));
    let highlight = |name: &str, color: Color32| {
        if let Some(pl) = view.planes.iter().find(|p| p.name == name) {
            painter.line_segment([xf.to_screen(pl.a), xf.to_screen(pl.b)], Stroke::new(2.0, color));
        }
    };
    let pick = env.pick.clone();
    if let Some(Pick::Plane(name)) = &pick {
        highlight(name, PALETTE.accent);
    }
    if let Some(Pick::Dimension(i)) = pick
        && let Some(d) = view.dimensions.iter().find(|d| d.index == i)
    {
        painter.circle_stroke(xf.to_screen(d.text_pos), 10.0, Stroke::new(1.5, PALETTE.accent));
    }
    let hovered_plane = hover.and_then(|p| plane_at(&view, p, tol));
    if let Some(name) = &hovered_plane {
        highlight(name, PALETTE.accent.gamma_multiply(0.5));
    }
    let tool = env.tool.clone();
    let clicked = resp.clicked_by(egui::PointerButton::Primary);
    match tool {
        EnvTool::Select => {
            if clicked && let Some(p) = hover {
                let picked = dimension_at(&view, p, tol * 2.0).map(Pick::Dimension).or(hovered_plane.map(Pick::Plane));
                if let Some(env) = state.component_env.as_mut() {
                    env.pick = picked;
                }
            }
        }
        EnvTool::RefPlane { first } => {
            let Some(p) = hover else { return };
            match first {
                Some((k, a)) if k == kind => {
                    painter.line_segment([xf.to_screen(a), xf.to_screen(p)], Stroke::new(1.5, PALETTE.accent));
                    if clicked && let Some((axis, at)) = plane_from_line(kind, a, p) {
                        add_plane(state, axis, at);
                        set_tool(state, EnvTool::RefPlane { first: None });
                    }
                }
                _ if clicked && kind != ComponentViewKind::ThreeD => {
                    set_tool(state, EnvTool::RefPlane { first: Some((kind, p)) });
                }
                _ => {}
            }
        }
        EnvTool::Dimension { first } => {
            if let Some((k, name)) = &first
                && *k == kind
            {
                highlight(name, PALETTE.accent);
            }
            if !clicked {
                return;
            }
            let (Some(p), Some(name)) = (hover, hovered_plane) else { return };
            match first {
                Some((k, from)) if k == kind && from != name => {
                    let place = match view.planes.iter().find(|pl| pl.name == name) {
                        Some(pl) if (pl.b - pl.a).x.abs() < 1e-9 => p.y,
                        _ => p.x,
                    };
                    let command = cmd::ComponentDimensionAdd {
                        type_ref: type_ref(state),
                        view: view_name(kind).into(),
                        from,
                        to: name,
                        label: None,
                        place: Some(place),
                    };
                    if let Some(out) = crate::actions::run_command(state, command)
                        && let Some(env) = state.component_env.as_mut()
                    {
                        env.pick = out["index"].as_u64().map(|i| Pick::Dimension(i as usize));
                    }
                    set_tool(state, EnvTool::Dimension { first: None });
                }
                _ => set_tool(state, EnvTool::Dimension { first: Some((kind, name)) }),
            }
        }
    }
}

fn to_world(cam: Camera, rect: egui::Rect, p: Pos2) -> DVec2 {
    Xf { screen: rect.center(), origin: cam.center, scale: cam.scale }.to_world(p)
}

/// The camera that shows the whole view.
fn fit(view: &ComponentView, rect: egui::Rect) -> Camera {
    let (lo, hi) = view.bounds.unwrap_or((DVec2::splat(-0.5), DVec2::splat(0.5)));
    let size = (hi - lo).max(DVec2::splat(0.2)) * 1.4;
    let scale = (f64::from(rect.width()) / size.x).min(f64::from(rect.height()) / size.y).max(5.0);
    Camera { center: (lo + hi) / 2.0, scale }
}

pub fn set_tool(state: &mut AppState, tool: EnvTool) {
    if let Some(env) = state.component_env.as_mut() {
        env.tool = tool;
    }
}

fn add_plane(state: &mut AppState, axis: PlaneAxis, at: f64) {
    let axis = match axis {
        PlaneAxis::X => "x",
        PlaneAxis::Y => "y",
        PlaneAxis::Z => "z",
    };
    let command =
        cmd::ComponentRefPlaneAdd { type_ref: type_ref(state), axis: axis.into(), at: format!("{at}"), name: None };
    if let Some(out) = crate::actions::run_command(state, command)
        && let (Some(env), Some(name)) = (state.component_env.as_mut(), out["name"].as_str())
    {
        env.pick = Some(Pick::Plane(name.to_string()));
    }
}

/// Esc steps back: the tool's first pick, the tool, the selection.
pub fn escape(state: &mut AppState) {
    let Some(env) = state.component_env.as_mut() else { return };
    env.tool = match &env.tool {
        EnvTool::RefPlane { first: Some(_) } => EnvTool::RefPlane { first: None },
        EnvTool::Dimension { first: Some(_) } => EnvTool::Dimension { first: None },
        EnvTool::Select => {
            env.pick = None;
            EnvTool::Select
        }
        _ => EnvTool::Select,
    };
}

/// Delete removes the selected plane or dimension.
pub fn delete_pick(state: &mut AppState) {
    let Some(pick) = state.component_env.as_ref().and_then(|e| e.pick.clone()) else { return };
    let t = type_ref(state);
    let done = match pick {
        Pick::Plane(name) => crate::actions::run_command(state, cmd::ComponentRefPlaneDelete { type_ref: t, name }),
        Pick::Dimension(index) => {
            crate::actions::run_command(state, cmd::ComponentDimensionDelete { type_ref: t, index })
        }
    };
    if done.is_some()
        && let Some(env) = state.component_env.as_mut()
    {
        env.pick = None;
    }
}

// --- Left panel ---

/// Text field that commits on Enter or when it loses focus; `None` until then.
fn draft_field(ui: &mut Ui, state: &mut AppState, key: &str, current: &str, width: f32) -> Option<String> {
    let env = state.component_env.as_mut()?;
    let text = env.drafts.entry(key.to_string()).or_insert_with(|| current.to_string());
    let r = ui.add(egui::TextEdit::singleline(text).desired_width(width));
    let commit = r.lost_focus();
    if !r.has_focus() && !commit && *text != current {
        *text = current.to_string();
    }
    if commit {
        let out = text.trim().to_string();
        env.drafts.remove(key);
        return (out != current).then_some(out);
    }
    None
}

/// The component's types, parameters and the selection's properties.
pub fn panel(ui: &mut Ui, state: &mut AppState) {
    let Some((name, def)) = component(state) else {
        finish(state);
        return;
    };
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(6.0);
        ui.label(theme::title(name.clone()));
        types_row(ui, state);
        ui.add_space(6.0);
        crate::table::section(ui, "comp-env-params", t!("comp-params"), |ui| params(ui, state, &def));
        let pick = state.component_env.as_ref().and_then(|e| e.pick.clone());
        match pick {
            Some(Pick::Plane(plane)) => {
                crate::table::section(ui, "comp-env-plane", t!("comp-env-plane"), |ui| {
                    plane_props(ui, state, &def, &plane)
                });
            }
            Some(Pick::Dimension(index)) => {
                crate::table::section(ui, "comp-env-dimension", t!("comp-env-dimension"), |ui| {
                    dimension_props(ui, state, &def, index);
                });
            }
            None => {}
        }
        let problems = def.ref_problems();
        if !problems.is_empty() {
            ui.add_space(6.0);
            for p in problems {
                ui.label(egui::RichText::new(p).color(PALETTE.danger).size(12.0));
            }
        }
    });
}

/// The type whose values the views show.
fn types_row(ui: &mut Ui, state: &mut AppState) {
    let Some(env) = state.component_env.as_ref() else { return };
    let current = env.type_id;
    let p = state.project();
    let Some(TypeData::Component { component, .. }) = p.element_type(current).map(|t| &t.data) else { return };
    let types: Vec<(ElementTypeId, String)> = p
        .types
        .values()
        .filter(|t| matches!(&t.data, TypeData::Component { component: c, .. } if c == component))
        .map(|t| (t.id, t.name.clone()))
        .collect();
    let mut chosen = current;
    crate::table::row(ui, &t!("comp-env-type"), |ui| {
        egui::ComboBox::from_id_salt("comp-env-type")
            .selected_text(types.iter().find(|(id, _)| *id == current).map_or(String::new(), |(_, n)| n.clone()))
            .show_ui(ui, |ui| {
                for (id, n) in &types {
                    ui.selectable_value(&mut chosen, *id, n);
                }
            });
    });
    if chosen != current
        && let Some(env) = state.component_env.as_mut()
    {
        env.type_id = chosen;
        env.cameras.clear();
    }
}

fn params(ui: &mut Ui, state: &mut AppState, def: &cc_core::ComponentDef) {
    for p in &def.params {
        let mut value = p.value;
        let mut instance = p.instance;
        let mut rename = None;
        let mut delete = false;
        let r = ui.horizontal(|ui| {
            rename = draft_field(ui, state, &format!("param:{}", p.name), &p.name, 110.0);
            let r = ui.add(crate::properties::param_value(&mut value, p.kind));
            ui.checkbox(&mut instance, "").on_hover_text(t!("comp-param-instance"));
            delete = ui.small_button("×").on_hover_text(t!("action-delete")).clicked();
            r
        });
        let t = type_ref(state);
        if r.inner.changed() || instance != p.instance {
            let command = cmd::ComponentParamSet {
                type_ref: t.clone(),
                name: p.name.clone(),
                value: r.inner.changed().then_some(value),
                kind: None,
                instance: (instance != p.instance).then_some(instance),
                rename: None,
            };
            crate::actions::run_command(state, command);
        }
        if let Some(new) = rename {
            let command = cmd::ComponentParamSet {
                type_ref: t.clone(),
                name: p.name.clone(),
                value: None,
                kind: None,
                instance: None,
                rename: Some(new),
            };
            crate::actions::run_command(state, command);
        }
        if delete {
            crate::actions::run_command(state, cmd::ComponentParamDelete { type_ref: t, name: p.name.clone() });
        }
    }
    if ui.button(format!("+ {}", t!("comp-env-add-param"))).clicked() {
        let name = (1..).map(|i| format!("Param{i}")).find(|n| !def.name_taken(n)).unwrap_or_default();
        let command =
            cmd::ComponentParamAdd { type_ref: type_ref(state), name, value: Some(1.0), kind: None, instance: None };
        crate::actions::run_command(state, command);
    }
}

fn plane_props(ui: &mut Ui, state: &mut AppState, def: &cc_core::ComponentDef, name: &str) {
    let Some(plane) = def.ref_plane(name) else { return };
    let origin = cc_core::component_ref::ORIGIN_PLANES.iter().any(|(n, _)| *n == name);
    let scope = def.scope(&BTreeMap::new(), &[]);
    crate::table::readonly_row(ui, &t!("comp-env-axis"), crate::i18n::tr(plane.axis.key()));
    if origin {
        crate::table::readonly_row(ui, &t!("prop-name"), name.to_string());
        ui.label(theme::muted(t!("msg-comp-origin-plane")));
        return;
    }
    let t = type_ref(state);
    let mut rename = None;
    crate::table::row(ui, &t!("prop-name"), |ui| {
        rename = draft_field(ui, state, &format!("plane-name:{name}"), name, 120.0)
    });
    let mut at = None;
    crate::table::row(ui, &t!("comp-env-at"), |ui| {
        at = draft_field(ui, state, &format!("plane-at:{name}"), &plane.at, 160.0)
    });
    crate::table::readonly_row(ui, &t!("prop-value"), scope.get(name).map_or(String::new(), cc_views::format::length));
    if at.is_some() || rename.is_some() {
        let command =
            cmd::ComponentRefPlaneSet { type_ref: t.clone(), name: name.to_string(), at, rename: rename.clone() };
        if crate::actions::run_command(state, command).is_some()
            && let (Some(new), Some(env)) = (rename, state.component_env.as_mut())
        {
            env.pick = Some(Pick::Plane(new));
        }
    }
    if ui.button(t!("action-delete")).clicked() {
        delete_pick(state);
    }
}

fn dimension_props(ui: &mut Ui, state: &mut AppState, def: &cc_core::ComponentDef, index: usize) {
    let Some(d) = def.dimensions.get(index).cloned() else { return };
    let scope = def.scope(&BTreeMap::new(), &[]);
    let t = type_ref(state);
    crate::table::readonly_row(ui, &t!("comp-env-between"), format!("{} — {}", d.from, d.to));
    if let Ok(mut value) = def.dimension_value(&scope, index) {
        let before = value;
        let r = crate::table::row(ui, &t!("prop-value"), |ui| ui.add(crate::properties::length_drag(&mut value)));
        if r.changed() && value > 0.0 && (value - before).abs() > 1e-12 {
            let command =
                cmd::ComponentDimensionSet { type_ref: t.clone(), index, label: None, value: Some(value), place: None };
            crate::actions::run_command(state, command);
        }
    }
    // The label: none, or a length parameter.
    let none = t!("comp-env-no-label");
    let mut label = d.label.clone().unwrap_or_default();
    let before = label.clone();
    crate::table::row(ui, &t!("comp-env-label"), |ui| {
        egui::ComboBox::from_id_salt("comp-env-label")
            .selected_text(if label.is_empty() { none.clone() } else { label.clone() })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut label, String::new(), none.clone());
                for p in def.params.iter().filter(|p| p.kind == cc_core::ParamKind::Length) {
                    ui.selectable_value(&mut label, p.name.clone(), p.name.clone());
                }
            });
    });
    if label != before {
        let command =
            cmd::ComponentDimensionSet { type_ref: t.clone(), index, label: Some(label), value: None, place: None };
        crate::actions::run_command(state, command);
    }
    // A new parameter named here labels it at once.
    let mut new_label = None;
    crate::table::row(ui, &t!("comp-env-new-label"), |ui| {
        new_label = draft_field(ui, state, &format!("dim-new:{index}"), "", 120.0)
    });
    if let Some(name) = new_label.filter(|n| !n.is_empty()) {
        let command = cmd::ComponentDimensionSet { type_ref: t, index, label: Some(name), value: None, place: None };
        crate::actions::run_command(state, command);
    }
    if ui.button(t!("action-delete")).clicked() {
        delete_pick(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::Document;

    fn table_state() -> (AppState, ElementTypeId) {
        let state = AppState::new(Document::new(crate::actions::new_project()));
        let table = state
            .project()
            .types_of(cc_core::Category::Component)
            .find(|t| matches!(&t.data, TypeData::Component { component, .. } if component == "Mesa"))
            .map(|t| t.id)
            .unwrap();
        (state, table)
    }

    #[test]
    fn two_clicks_make_a_plane_square_to_the_view() {
        use ComponentViewKind::*;
        let (a, b) = (DVec2::new(0.3, -1.0), DVec2::new(0.3004, 1.0));
        assert_eq!(plane_from_line(Plan, a, b), Some((PlaneAxis::X, 0.3)));
        assert_eq!(plane_from_line(Side, a, b), Some((PlaneAxis::Y, 0.3)));
        let (a, b) = (DVec2::new(-1.0, 0.75), DVec2::new(1.0, 0.75));
        assert_eq!(plane_from_line(Front, a, b), Some((PlaneAxis::Z, 0.75)));
        assert_eq!(plane_from_line(Plan, a, b), Some((PlaneAxis::Y, 0.75)));
        assert_eq!(plane_from_line(ThreeD, a, b), None);
        assert_eq!(plane_from_line(Plan, a, a), None);
    }

    #[test]
    fn planes_and_dimensions_are_picked_in_their_views() {
        let (mut state, table) = table_state();
        open(&mut state, table);
        add_plane(&mut state, PlaneAxis::X, 0.4);
        assert_eq!(state.component_env.as_ref().unwrap().pick, Some(Pick::Plane("Plane1".into())));
        let plan = view(&state, ComponentViewKind::Plan).unwrap();
        assert_eq!(plane_at(&plan, DVec2::new(0.401, 0.1), 0.005), Some("Plane1".into()));
        assert_eq!(plane_at(&plan, DVec2::new(0.45, 0.1), 0.005), None);
        let command = cmd::ComponentDimensionAdd {
            type_ref: type_ref(&state),
            view: "plan".into(),
            from: "CenterX".into(),
            to: "Plane1".into(),
            label: Some("Half".into()),
            place: Some(-0.6),
        };
        crate::actions::run_command(&mut state, command).unwrap();
        let plan = view(&state, ComponentViewKind::Plan).unwrap();
        let text = plan.dimensions[0].text_pos;
        assert_eq!(dimension_at(&plan, text + DVec2::splat(0.001), 0.01), Some(0));
        // Delete takes the selected dimension away; Cancel undoes the whole session.
        state.component_env.as_mut().unwrap().pick = Some(Pick::Dimension(0));
        delete_pick(&mut state);
        assert!(component(&state).unwrap().1.dimensions.is_empty());
        cancel(&mut state);
        assert!(state.component_env.is_none());
        let TypeData::Component { def, .. } = &state.project().element_type(table).unwrap().data else { panic!() };
        assert!(def.ref_planes.is_empty() && def.param("Half").is_none());
    }

    #[test]
    fn finish_keeps_the_changes_and_esc_steps_back() {
        let (mut state, table) = table_state();
        open(&mut state, table);
        set_tool(&mut state, EnvTool::RefPlane { first: Some((ComponentViewKind::Plan, DVec2::ZERO)) });
        escape(&mut state);
        assert_eq!(state.component_env.as_ref().unwrap().tool, EnvTool::RefPlane { first: None });
        escape(&mut state);
        assert_eq!(state.component_env.as_ref().unwrap().tool, EnvTool::Select);
        add_plane(&mut state, PlaneAxis::Z, 0.75);
        finish(&mut state);
        let TypeData::Component { def, .. } = &state.project().element_type(table).unwrap().data else { panic!() };
        assert_eq!(def.ref_planes.len(), 1);
    }
}
