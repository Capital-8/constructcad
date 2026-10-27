//! Component editor: the parameters and pieces of a parametric component
//! (boxes, cylinders, profiles of lines and arcs extruded or revolved on the
//! plan, front or side plane, voids), with expression checking and plan,
//! front and 3D previews.
//!
//! The editor works on a copy of the definition. OK applies it to every
//! type of the component in one undo step (each type keeps its own values
//! of the parameters it already had); Cancel discards it.

use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use crate::theme::{self, PALETTE};
use cc_core::component::PartError;
use cc_core::{
    ComponentDef, ComponentItem, ComponentKind, ComponentParam, ComponentShape, ElementTypeId, ParamKind, ProfilePoint,
    SketchPlane, TypeData, expr,
};
use egui::{Color32, Context, Pos2, RichText, Sense, Stroke, Ui, vec2};
use glam::{DVec2, DVec3};

/// An open component editor.
#[derive(Debug, Clone)]
pub struct Editor {
    /// Type being edited; `None` creates a new component.
    pub type_id: Option<ElementTypeId>,
    pub name: String,
    pub def: ComponentDef,
    pub selected: usize,
    /// Parameter renames made in the editor (old, new), applied to sibling types.
    renames: Vec<(String, String)>,
    /// Name each parameter has in the piece expressions. It follows the
    /// edited name only while that name is valid and unique, so clearing a
    /// name while typing does not erase its references.
    refs: Vec<String>,
    error: Option<String>,
}

impl Editor {
    /// Edit the component of type `tid`.
    pub fn open(state: &AppState, tid: ElementTypeId) -> Option<Editor> {
        let ty = state.project().element_type(tid)?;
        let TypeData::Component { component, def } = &ty.data else { return None };
        Some(Editor {
            type_id: Some(tid),
            name: component.clone(),
            refs: def.params.iter().map(|p| p.name.clone()).collect(),
            def: def.clone(),
            selected: 0,
            renames: Vec::new(),
            error: None,
        })
    }

    /// A new component: one box sized by width, depth and height.
    pub fn new_component() -> Editor {
        let (w, d, h) = (t!("tpl-param-width"), t!("tpl-param-depth"), t!("tpl-param-height"));
        let param = |name: &str, value| ComponentParam {
            name: name.to_string(),
            value,
            kind: ParamKind::Length,
            instance: false,
        };
        let def = ComponentDef {
            kind: ComponentKind::Generic,
            ifc: None,
            mep: None,
            opening: None,
            connectors: Vec::new(),
            params: vec![param(&w, 1.0), param(&d, 0.6), param(&h, 0.9)],
            items: vec![ComponentItem {
                name: t!("tpl-part-body"),
                shape: ComponentShape::Box {
                    x: format!("-{w} / 2"),
                    y: format!("-{d} / 2"),
                    z: "0".into(),
                    width: w.clone(),
                    depth: d.clone(),
                    height: h.clone(),
                },
                material: String::new(),
                in_plan: true,
                void: false,
                plan_only: false,
            }],
        };
        Editor {
            type_id: None,
            name: t!("comp-new-name"),
            refs: def.params.iter().map(|p| p.name.clone()).collect(),
            def,
            selected: 0,
            renames: Vec::new(),
            error: None,
        }
    }
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(mut ed) = state.dialogs.component_editor.take() else { return };
    let materials: Vec<String> = state.project().materials.iter().map(|m| m.name.clone()).collect();
    let mut ok = 0;
    let title = if ed.type_id.is_some() { t!("dlg-component") } else { t!("dlg-component-new") };
    let response = crate::dialog::Dialog::new("component-editor", title).width(900.0).show(
        ctx,
        |ui| body(ui, &mut ed, &materials),
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    match response.clicked {
        Some(i) if i == ok => match validate(&ed) {
            Ok(()) => apply(state, ed),
            Err(message) => {
                ed.error = Some(message);
                state.dialogs.component_editor = Some(ed);
            }
        },
        Some(_) => {}
        None if response.dismissed => {}
        None => state.dialogs.component_editor = Some(ed),
    }
}

/// Parameter names must be valid and unique, and every piece must resolve.
fn validate(ed: &Editor) -> Result<(), String> {
    if ed.name.trim().is_empty() {
        return Err(t!("comp-err-name"));
    }
    for (i, p) in ed.def.params.iter().enumerate() {
        if !expr::is_valid_name(&p.name) {
            return Err(t!("comp-err-param-name", "name" => p.name));
        }
        if ed.def.params[..i].iter().any(|q| q.name == p.name) {
            return Err(t!("comp-err-param-duplicate", "name" => p.name));
        }
    }
    if ed.def.items.is_empty() {
        return Err(t!("comp-err-no-pieces"));
    }
    if matches!(ed.def.kind, ComponentKind::Door | ComponentKind::Window) && ed.def.opening_size().is_none() {
        return Err(t!("comp-err-opening"));
    }
    let resolved = ed.def.resolve_connectors(&Default::default());
    if let Some((_, c)) = ed.def.connectors.iter().enumerate().find(|(i, _)| !resolved.iter().any(|k| k.index == *i)) {
        return Err(t!("comp-err-connector", "name" => c.name));
    }
    let (_, errors) = ed.def.resolve_in(&Default::default(), PREVIEW_WALL);
    if let Some((i, e)) = errors.first() {
        let piece = ed.def.items.get(*i).map_or(String::new(), |it| it.name.clone());
        return Err(t!("comp-err-piece", "piece" => piece, "error" => part_error(e)));
    }
    Ok(())
}

/// The wall a door or window component is previewed and checked in: 15 cm thick.
const PREVIEW_WALL: &[(&str, f64)] = &[(cc_core::component::WALL_THICKNESS, 0.15)];

fn part_error(e: &PartError) -> String {
    match e {
        PartError::Expr { field, error } => format!("{}: {error}", tr(&format!("comp-field-{field}"))),
        PartError::Degenerate => t!("comp-err-degenerate"),
    }
}

/// Write the definition to every type of the component (or create a type),
/// through the engine.
fn apply(state: &mut AppState, ed: Editor) {
    let name = ed.name.trim().to_string();
    let def = serde_json::to_value(&ed.def).unwrap_or_default();
    let Some(tid) = ed.type_id else {
        let command = cc_engine::commands::types::ComponentCreate { name: name.clone(), def };
        let Some(id) = crate::actions::run_command(state, command).and_then(|o| o["id"].as_u64()) else { return };
        let type_id = ElementTypeId(id);
        // Doors and windows are placed in walls.
        let category = state.project().element_type(type_id).map(|t| t.category());
        state.set_tool(match category {
            Some(category @ (cc_core::Category::Door | cc_core::Category::Window)) => {
                crate::tools::Tool::Hosted { category, type_id }
            }
            _ => crate::tools::Tool::Component { type_id },
        });
        state.message = t!("msg-component-created", "name" => name);
        return;
    };
    let command = cc_engine::commands::types::TypeSet {
        type_ref: cc_engine::Ref::Id(tid.0),
        name: None,
        params: Some(serde_json::json!({ "component": name, "def": def })),
        info: None,
        renames: ed.renames.into_iter().map(|(a, b)| [a, b]).collect(),
    };
    crate::actions::run_command(state, command);
}

/// The opening a door or window cuts in its wall: width, height and default
/// sill as expressions of the parameters, and whether plans draw its pieces.
fn opening(ui: &mut Ui, ed: &mut Editor) {
    if !matches!(ed.def.kind, ComponentKind::Door | ComponentKind::Window) {
        ed.def.opening = None;
        return;
    }
    let first = |i: usize| ed.def.params.get(i).map_or_else(|| "1".to_string(), |p| p.name.clone());
    let (w, h) = (first(0), first(1));
    let o = ed.def.opening.get_or_insert_with(|| {
        Box::new(cc_core::component::HostOpening { width: w, height: h, sill: String::new(), own_symbol: false })
    });
    ui.add_space(4.0);
    egui::Grid::new("component-opening").num_columns(8).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label(theme::muted(t!("comp-opening")));
        for (label, text) in
            [(t!("col-width"), &mut o.width), (t!("col-height"), &mut o.height), (t!("prop-sill"), &mut o.sill)]
        {
            ui.label(theme::muted(label));
            ui.add(egui::TextEdit::singleline(text).desired_width(90.0));
        }
        ui.checkbox(&mut o.own_symbol, t!("comp-own-symbol")).on_hover_text(t!("comp-own-symbol-tip"));
        ui.end_row();
    });
    ui.label(theme::muted(t!("comp-opening-hint", "name" => cc_core::component::WALL_THICKNESS)).size(11.5));
}

fn body(ui: &mut Ui, ed: &mut Editor, materials: &[String]) {
    egui::Grid::new("component-head").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label(theme::muted(t!("comp-name")));
        ui.add(egui::TextEdit::singleline(&mut ed.name).desired_width(260.0));
        ui.label(theme::muted(t!("comp-kind")));
        egui::ComboBox::from_id_salt("comp-kind").selected_text(tr(ed.def.kind.key())).width(180.0).show_ui(ui, |ui| {
            for k in ComponentKind::ALL {
                ui.selectable_value(&mut ed.def.kind, k, tr(k.key()));
            }
        });
        ui.end_row();
    });
    opening(ui, ed);
    ui.add_space(6.0);
    ui.columns(2, |cols| {
        let left = &mut cols[0];
        params_table(left, ed);
        left.add_space(8.0);
        pieces(left, ed, materials);
        left.add_space(8.0);
        connectors(left, ed);
        preview(&mut cols[1], ed);
    });
    if let Some(e) = &ed.error {
        ui.add_space(4.0);
        ui.label(RichText::new(e).color(PALETTE.danger));
    }
}

/// Where pipes and conduits connect: name, system kind, position and
/// direction in the component's frame, diameter (expressions).
fn connectors(ui: &mut Ui, ed: &mut Editor) {
    use cc_core::component::{ComponentConnector, ConnectorDirection};
    ui.label(theme::title(t!("comp-connectors")));
    ui.label(theme::muted(t!("comp-connectors-hint")).size(11.5));
    let mut remove = None;
    egui::Grid::new("component-connectors").num_columns(8).spacing([6.0, 4.0]).show(ui, |ui| {
        for (i, c) in ed.def.connectors.iter_mut().enumerate() {
            ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(90.0));
            egui::ComboBox::from_id_salt(("conn-system", i)).selected_text(tr(c.system.key())).width(110.0).show_ui(
                ui,
                |ui| {
                    for k in cc_core::SystemKind::ALL {
                        ui.selectable_value(&mut c.system, k, tr(k.key()));
                    }
                },
            );
            for (axis, text) in [("x", &mut c.x), ("y", &mut c.y), ("z", &mut c.z)] {
                ui.add(egui::TextEdit::singleline(text).desired_width(70.0).hint_text(axis));
            }
            egui::ComboBox::from_id_salt(("conn-dir", i)).selected_text(tr(c.direction.key())).width(90.0).show_ui(
                ui,
                |ui| {
                    for d in ConnectorDirection::ALL {
                        ui.selectable_value(&mut c.direction, d, tr(d.key()));
                    }
                },
            );
            ui.add(egui::TextEdit::singleline(&mut c.diameter).desired_width(60.0).hint_text("Ø"));
            if ui.small_button("×").on_hover_text(t!("action-remove")).clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        ed.def.connectors.remove(i);
    }
    if ui.button(t!("comp-add-connector")).clicked() {
        let n = ed.def.connectors.len() + 1;
        ed.def.connectors.push(ComponentConnector {
            name: format!("{} {n}", t!("comp-connector")),
            system: cc_core::SystemKind::ColdWater,
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            direction: ConnectorDirection::Down,
            diameter: String::new(),
        });
    }
}

fn params_table(ui: &mut Ui, ed: &mut Editor) {
    ui.label(theme::muted(t!("comp-params")).size(12.0));
    let mut remove = None;
    egui::Grid::new("comp-params").num_columns(5).striped(true).spacing([6.0, 4.0]).show(ui, |ui| {
        ui.label(theme::muted(t!("comp-param-name")));
        ui.label(theme::muted(t!("comp-param-kind")));
        ui.label(theme::muted(t!("comp-param-value")));
        ui.label(theme::muted(t!("comp-param-instance")));
        ui.label("");
        ui.end_row();
        let n = ed.def.params.len();
        for i in 0..n {
            let Some(p) = ed.def.params.get_mut(i) else { continue };
            let valid = expr::is_valid_name(&p.name);
            let edit = egui::TextEdit::singleline(&mut p.name).desired_width(120.0);
            let edit = if valid { edit } else { edit.text_color(PALETTE.danger) };
            ui.add(edit);
            egui::ComboBox::from_id_salt(("comp-param-kind", i)).selected_text(tr(p.kind.key())).width(100.0).show_ui(
                ui,
                |ui| {
                    for k in ParamKind::ALL {
                        ui.selectable_value(&mut p.kind, k, tr(k.key()));
                    }
                },
            );
            ui.add(crate::properties::param_value(&mut p.value, p.kind));
            ui.checkbox(&mut p.instance, "");
            if ui.add(egui::Button::new("×").small()).on_hover_text(t!("comp-param-remove")).clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        ed.def.params.remove(i);
        if i < ed.refs.len() {
            ed.refs.remove(i);
        }
    }
    sync_refs(ed);
    if ui.button(t!("comp-param-add")).clicked() {
        let mut k = ed.def.params.len() + 1;
        let mut name = format!("P{k}");
        while ed.def.param(&name).is_some() {
            k += 1;
            name = format!("P{k}");
        }
        ed.refs.push(name.clone());
        ed.def.params.push(ComponentParam { name, value: 0.5, kind: ParamKind::Length, instance: false });
    }
}

/// Carry valid, unique parameter names into the piece expressions.
fn sync_refs(ed: &mut Editor) {
    ed.refs.resize(ed.def.params.len(), String::new());
    for i in 0..ed.def.params.len() {
        let name = ed.def.params[i].name.clone();
        let unique = ed.def.params.iter().filter(|p| p.name == name).count() == 1
            && !ed.refs.iter().enumerate().any(|(j, r)| j != i && *r == name);
        if name != ed.refs[i] && expr::is_valid_name(&name) && unique {
            let old = std::mem::replace(&mut ed.refs[i], name.clone());
            if !old.is_empty() {
                rename_in_items(&mut ed.def, &old, &name);
                ed.renames.push((old, name));
            }
        }
    }
}

/// Rename references in the pieces only (the parameter itself was edited in place).
fn rename_in_items(def: &mut ComponentDef, old: &str, new: &str) {
    let params = std::mem::take(&mut def.params);
    def.rename_param(old, new);
    def.params = params;
}

fn pieces(ui: &mut Ui, ed: &mut Editor, materials: &[String]) {
    ui.label(theme::muted(t!("comp-pieces")).size(12.0));
    let (_, errors) = ed.def.resolve_in(&Default::default(), PREVIEW_WALL);
    ui.horizontal_wrapped(|ui| {
        for (i, item) in ed.def.items.iter().enumerate() {
            let label = if errors.iter().any(|(e, _)| *e == i) {
                RichText::new(format!("⚠ {}", item.name)).color(PALETTE.danger)
            } else {
                RichText::new(&item.name)
            };
            if ui.selectable_label(ed.selected == i, label).clicked() {
                ed.selected = i;
            }
        }
    });
    ui.horizontal(|ui| {
        let defaults = |kind: usize| -> ComponentShape {
            match kind {
                0 => ComponentShape::Box {
                    x: "0".into(),
                    y: "0".into(),
                    z: "0".into(),
                    width: "0.5".into(),
                    depth: "0.5".into(),
                    height: "0.5".into(),
                },
                1 => ComponentShape::Cylinder {
                    x: "0".into(),
                    y: "0".into(),
                    z: "0".into(),
                    radius: "0.25".into(),
                    height: "0.5".into(),
                },
                2 => ComponentShape::Extrusion {
                    points: vec![("0".into(), "0".into()), ("0.5".into(), "0".into()), ("0".into(), "0.5".into())],
                    z: "0".into(),
                    height: "0.5".into(),
                },
                // A 0.5 m square seen from the front, extruded 0.5 m back.
                3 => ComponentShape::Profile {
                    plane: SketchPlane::Front,
                    points: square(),
                    offset: "0".into(),
                    depth: "0.5".into(),
                },
                // The same square turned about the vertical axis through the origin.
                _ => ComponentShape::Revolution {
                    plane: SketchPlane::Front,
                    points: square(),
                    offset: "0".into(),
                    axis: "0".into(),
                    angle: "360".into(),
                },
            }
        };
        let keys =
            ["comp-add-box", "comp-add-cylinder", "comp-add-extrusion", "comp-add-profile", "comp-add-revolution"];
        for (k, key) in keys.iter().enumerate() {
            if ui.button(tr(key)).clicked() {
                let name = format!("{} {}", t!("comp-piece"), ed.def.items.len() + 1);
                ed.def.items.push(ComponentItem {
                    name,
                    shape: defaults(k),
                    material: String::new(),
                    in_plan: true,
                    void: false,
                    plan_only: false,
                });
                ed.selected = ed.def.items.len() - 1;
            }
        }
        let n = ed.def.items.len();
        let i = ed.selected;
        if ui.add_enabled(i > 0 && i < n, egui::Button::new("↑")).clicked() {
            ed.def.items.swap(i, i - 1);
            ed.selected -= 1;
        }
        if ui.add_enabled(i + 1 < n, egui::Button::new("↓")).clicked() {
            ed.def.items.swap(i, i + 1);
            ed.selected += 1;
        }
        if ui.add_enabled(i < n, egui::Button::new(t!("comp-piece-remove"))).clicked() {
            ed.def.items.remove(i);
            ed.selected = ed.selected.min(ed.def.items.len().saturating_sub(1));
        }
    });
    let values = ed.def.values(&Default::default());
    let Some(item) = ed.def.items.get_mut(ed.selected) else { return };
    ui.add_space(4.0);
    egui::Grid::new("comp-piece").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label(theme::muted(t!("comp-piece-name")));
        ui.add(egui::TextEdit::singleline(&mut item.name).desired_width(200.0));
        ui.end_row();
        ui.label(theme::muted(t!("comp-piece-material")));
        let none = t!("schedule-none");
        let shown = if item.material.is_empty() { none.clone() } else { item.material.clone() };
        egui::ComboBox::from_id_salt("comp-material").selected_text(shown).width(200.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut item.material, String::new(), none);
            for m in materials {
                ui.selectable_value(&mut item.material, m.clone(), m);
            }
        });
        ui.end_row();
        ui.label("");
        ui.checkbox(&mut item.in_plan, t!("comp-piece-in-plan"));
        ui.checkbox(&mut item.plan_only, t!("comp-piece-plan-only")).on_hover_text(t!("comp-piece-plan-only-tip"));
        ui.end_row();
        ui.label("");
        ui.checkbox(&mut item.void, t!("comp-piece-void")).on_hover_text(t!("comp-piece-void-tip"));
        ui.end_row();
    });
    let lookup = |name: &str| values.get(name).copied();
    let field = |ui: &mut Ui, label: &str, text: &mut String| {
        ui.label(theme::muted(label));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(text).desired_width(180.0).font(egui::TextStyle::Monospace));
            match expr::eval(text, &lookup) {
                Ok(v) => ui.label(theme::muted(format!("= {}", cc_views::format::number(v, 3)))),
                Err(e) => ui.label(RichText::new(e.to_string()).color(PALETTE.danger).size(11.0)),
            };
        });
        ui.end_row();
    };
    egui::Grid::new("comp-shape").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| match &mut item.shape {
        ComponentShape::Box { x, y, z, width, depth, height } => {
            field(ui, &t!("comp-field-x"), x);
            field(ui, &t!("comp-field-y"), y);
            field(ui, &t!("comp-field-z"), z);
            field(ui, &t!("comp-field-width"), width);
            field(ui, &t!("comp-field-depth"), depth);
            field(ui, &t!("comp-field-height"), height);
        }
        ComponentShape::Cylinder { x, y, z, radius, height } => {
            field(ui, &t!("comp-field-x"), x);
            field(ui, &t!("comp-field-y"), y);
            field(ui, &t!("comp-field-z"), z);
            field(ui, &t!("comp-field-radius"), radius);
            field(ui, &t!("comp-field-height"), height);
        }
        ComponentShape::Extrusion { points, z, height } => {
            field(ui, &t!("comp-field-z"), z);
            field(ui, &t!("comp-field-height"), height);
            let mut remove = None;
            let n = points.len();
            for (i, (px, py)) in points.iter_mut().enumerate() {
                field(ui, &format!("{} {} X", t!("comp-point"), i + 1), px);
                ui.label(theme::muted(format!("{} {} Y", t!("comp-point"), i + 1)));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(py).desired_width(180.0).font(egui::TextStyle::Monospace));
                    if let Err(e) = expr::eval(py, &lookup) {
                        ui.label(RichText::new(e.to_string()).color(PALETTE.danger).size(11.0));
                    }
                    if n > 3 && ui.add(egui::Button::new("×").small()).clicked() {
                        remove = Some(i);
                    }
                });
                ui.end_row();
            }
            if let Some(i) = remove {
                points.remove(i);
            }
            ui.label("");
            if ui.button(t!("comp-point-add")).clicked() {
                let last = points.last().cloned().unwrap_or_else(|| ("0".into(), "0".into()));
                points.push(last);
            }
            ui.end_row();
        }
        ComponentShape::Profile { plane, points, offset, depth } => {
            plane_combo(ui, plane);
            let normal = plane_axes(*plane).2;
            field(ui, &format!("{} ({normal})", t!("comp-field-offset")), offset);
            field(ui, &format!("{} (+{normal})", t!("comp-field-extrusion")), depth);
            profile_points(ui, *plane, points, &lookup);
        }
        ComponentShape::Revolution { plane, points, offset, axis, angle } => {
            plane_combo(ui, plane);
            let (u, _, normal) = plane_axes(*plane);
            field(ui, &format!("{} ({normal})", t!("comp-field-offset")), offset);
            field(ui, &format!("{} ({u})", t!("comp-field-axis")), axis);
            field(ui, &t!("comp-field-angle"), angle);
            profile_points(ui, *plane, points, &lookup);
        }
        ComponentShape::Mesh { vertices, faces } => {
            ui.label(theme::muted(t!("comp-field-mesh")));
            ui.label(t!("comp-mesh-size", "vertices" => vertices.len(), "faces" => faces.len()));
            ui.end_row();
        }
    });
}

/// A 0.5 m square profile from the origin.
fn square() -> Vec<ProfilePoint> {
    vec![
        ProfilePoint::new("0", "0"),
        ProfilePoint::new("0.5", "0"),
        ProfilePoint::new("0.5", "0.5"),
        ProfilePoint::new("0", "0.5"),
    ]
}

/// Names of a plane's U and V axes and of its normal axis in the component frame.
fn plane_axes(plane: SketchPlane) -> (&'static str, &'static str, &'static str) {
    match plane {
        SketchPlane::Plan => ("X", "Y", "Z"),
        SketchPlane::Front => ("X", "Z", "Y"),
        SketchPlane::Side => ("Y", "Z", "X"),
    }
}

fn plane_combo(ui: &mut Ui, plane: &mut SketchPlane) {
    ui.label(theme::muted(t!("comp-field-plane")));
    egui::ComboBox::from_id_salt("comp-plane").selected_text(tr(plane.key())).width(180.0).show_ui(ui, |ui| {
        for p in SketchPlane::ALL {
            ui.selectable_value(plane, p, tr(p.key()));
        }
    });
    ui.end_row();
}

/// The points of a profile: U and V on the plane and, for an arc to the
/// next point, its radius (empty for a straight segment).
fn profile_points(
    ui: &mut Ui,
    plane: SketchPlane,
    points: &mut Vec<ProfilePoint>,
    lookup: &dyn Fn(&str) -> Option<f64>,
) {
    let (u, v, _) = plane_axes(plane);
    ui.label("");
    ui.label(theme::muted(format!("{u} · {v} · {}", t!("comp-field-arc"))).size(11.0))
        .on_hover_text(t!("comp-field-arc-tip"));
    ui.end_row();
    let mut remove = None;
    let n = points.len();
    for (i, p) in points.iter_mut().enumerate() {
        ui.label(theme::muted(format!("{} {}", t!("comp-point"), i + 1)));
        ui.horizontal(|ui| {
            let mut error = None;
            for (text, width) in [(&mut p.u, 120.0), (&mut p.v, 120.0), (&mut p.radius, 70.0)] {
                // An empty radius is a straight segment; coordinates are always checked.
                let check = !text.trim().is_empty() || width > 100.0;
                let result = if check { expr::eval(text, lookup).err() } else { None };
                let edit = egui::TextEdit::singleline(text).desired_width(width).font(egui::TextStyle::Monospace);
                ui.add(if result.is_some() { edit.text_color(PALETTE.danger) } else { edit });
                error = error.or(result);
            }
            if n > 3 && ui.add(egui::Button::new("×").small()).on_hover_text(t!("comp-point-remove")).clicked() {
                remove = Some(i);
            }
            if let Some(e) = error {
                ui.label(RichText::new(e.to_string()).color(PALETTE.danger).size(11.0));
            }
        });
        ui.end_row();
    }
    if let Some(i) = remove {
        points.remove(i);
    }
    ui.label("");
    if ui.button(t!("comp-point-add")).clicked() {
        let last = points.last().map(|p| ProfilePoint::new(p.u.clone(), p.v.clone())).unwrap_or_default();
        points.push(last);
    }
    ui.end_row();
}

/// How a preview looks at the pieces: a rotation taking the viewer's
/// direction to +Z (so the edges seen from above are the ones it shows).
fn view_transform(view: usize) -> impl Fn(DVec3) -> DVec3 {
    move |p: DVec3| match view {
        0 => p,
        // From the front (-Y), Z up.
        1 => DVec3::new(p.x, p.z, -p.y),
        // From the front, right and above.
        _ => {
            let eye = DVec3::new(1.0, -1.0, 1.0).normalize();
            let right = (-eye).cross(DVec3::Z).normalize();
            let up = right.cross(-eye);
            DVec3::new(p.dot(right), p.dot(up), p.dot(eye))
        }
    }
}

/// A piece's index, whether it is a void, and its edges in a preview.
type PieceEdges = (usize, bool, Vec<(DVec2, DVec2)>);

/// Plan (top), front (from -Y) and 3D views of the pieces, voids cut out
/// (and drawn in the warning color); the selected piece is highlighted.
fn preview(ui: &mut Ui, ed: &Editor) {
    ui.label(theme::muted(t!("comp-preview")).size(12.0));
    let mut solids = cc_geom::resolve_component_in(&ed.def, &Default::default(), PREVIEW_WALL);
    solids.retain(|p| !p.plan_only);
    let (all, _) = ed.def.resolve_in(&Default::default(), PREVIEW_WALL);
    let pieces: Vec<(usize, bool, Vec<Vec<DVec3>>)> = solids
        .iter()
        .map(|p| (p.item, false, cc_geom::component::faces_of(p)))
        .chain(all.iter().filter(|p| p.void).map(|p| (p.item, true, cc_geom::component::faces_of(p))))
        .collect();
    let width = ui.available_width();
    let size = vec2(width, 180.0);
    let labels = [t!("comp-preview-plan"), t!("comp-preview-front"), t!("comp-preview-3d")];
    for (view, label) in labels.iter().enumerate() {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, PALETTE.sheet_bg);
        painter.text(
            rect.left_top() + vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(11.0),
            PALETTE.text_muted,
        );
        // Edges of every piece seen in this view, in drawing coordinates.
        let look = view_transform(view);
        let edges: Vec<PieceEdges> = pieces
            .iter()
            .map(|(item, void, faces)| {
                let turned: Vec<Vec<DVec3>> = faces.iter().map(|f| f.iter().map(|p| look(*p)).collect()).collect();
                let lines =
                    cc_geom::plan_edges(&turned).into_iter().map(|(a, b)| (a.truncate(), b.truncate())).collect();
                (*item, *void, lines)
            })
            .collect();
        let Some((lo, hi)) = edges
            .iter()
            .filter(|(_, void, _)| !*void)
            .flat_map(|(_, _, l)| l.iter().flat_map(|(a, b)| [*a, *b]))
            .fold(None, |acc: Option<(DVec2, DVec2)>, p| Some(acc.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p)))))
        else {
            continue;
        };
        let inner = rect.shrink(22.0);
        let extent = (hi - lo).max(DVec2::splat(1e-3));
        let k = (inner.width() as f64 / extent.x).min(inner.height() as f64 / extent.y);
        let center = (lo + hi) * 0.5;
        let to = |p: DVec2| -> Pos2 {
            let q = (p - center) * k;
            Pos2::new(inner.center().x + q.x as f32, inner.center().y - q.y as f32)
        };
        // Origin cross.
        let o = to(look(DVec3::ZERO).truncate());
        if inner.expand(10.0).contains(o) {
            let s = Stroke::new(1.0, PALETTE.text_faint);
            painter.line_segment([o - vec2(6.0, 0.0), o + vec2(6.0, 0.0)], s);
            painter.line_segment([o - vec2(0.0, 6.0), o + vec2(0.0, 6.0)], s);
        }
        let selected = ed.selected;
        let order = edges.iter().filter(|(i, ..)| *i != selected).chain(edges.iter().filter(|(i, ..)| *i == selected));
        for (item, void, lines) in order {
            let color = match (*item == selected, *void) {
                (true, _) => PALETTE.accent,
                (false, true) => PALETTE.danger,
                (false, false) => Color32::from_gray(60),
            };
            let stroke = Stroke::new(if *item == selected { 1.6 } else { 1.0 }, color);
            for (a, b) in lines {
                painter.line_segment([to(*a), to(*b)], stroke);
            }
        }
        // Connectors: a dot in the color of their system.
        for k in ed.def.resolve_connectors(&Default::default()) {
            let [r, g, b] = k.system.default_color();
            let at = to(look(k.point).truncate());
            painter.circle(at, 4.0, Color32::from_rgb(r, g, b), Stroke::new(1.0, Color32::WHITE));
        }
        if view < 2 {
            let dims = format!("{} × {}", cc_views::format::length(extent.x), cc_views::format::length(extent.y));
            painter.text(
                rect.right_bottom() - vec2(6.0, 4.0),
                egui::Align2::RIGHT_BOTTOM,
                dims,
                egui::FontId::proportional(11.0),
                PALETTE.text_muted,
            );
        }
        ui.add_space(6.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::{Category, Document};

    fn table_type(state: &AppState) -> ElementTypeId {
        state
            .project()
            .types_of(Category::Component)
            .find(|t| matches!(&t.data, TypeData::Component { def, .. } if def.items.len() == 5))
            .unwrap()
            .id
    }

    fn def_of(state: &AppState, id: ElementTypeId) -> (String, ComponentDef) {
        match state.project().element_type(id).map(|t| &t.data) {
            Some(TypeData::Component { component, def }) => (component.clone(), def.clone()),
            _ => panic!("not a component type"),
        }
    }

    #[test]
    fn editing_applies_to_every_type_and_keeps_their_values() {
        let mut state = AppState::new(Document::new(crate::actions::new_project()));
        let table = table_type(&state);
        // A second type of the same component, 2 m wide.
        let wide = state.session.doc.edit("dup", |e| {
            let mut ty = e.project().element_type(table).unwrap().clone();
            ty.id = ElementTypeId(e.alloc_id());
            ty.name = "Wide".into();
            if let TypeData::Component { def, .. } = &mut ty.data {
                def.params[0].value = 2.0;
            }
            e.add_type(ty)
        });
        let width = def_of(&state, table).1.params[0].name.clone();

        let mut ed = Editor::open(&state, table).unwrap();
        ed.name = "Mesa nova".into();
        // Typing the new name passes through an empty, invalid name.
        ed.def.params[0].name = String::new();
        sync_refs(&mut ed);
        ed.def.params[0].name = "W".into();
        sync_refs(&mut ed);
        ed.def.params[0].value = 1.5;
        ed.def.params[0].instance = true;
        assert_eq!(validate(&ed), Ok(()));
        let depth = state.session.doc.undo_depth();
        apply(&mut state, ed);
        assert_eq!(state.session.doc.undo_depth(), depth + 1);

        let (name, def) = def_of(&state, table);
        assert_eq!(name, "Mesa nova");
        assert_eq!((def.params[0].name.as_str(), def.params[0].value, def.params[0].instance), ("W", 1.5, true));
        assert!(def.resolve(&Default::default()).1.is_empty());
        // The sibling follows the rename but keeps its own width.
        let (name, def) = def_of(&state, wide);
        assert_eq!(name, "Mesa nova");
        assert_eq!((def.params[0].name.as_str(), def.params[0].value), ("W", 2.0));
        // One undo restores both types.
        state.session.doc.undo();
        assert_eq!(def_of(&state, wide).1.params[0].name, width);
    }

    #[test]
    fn validation_rejects_bad_names_and_pieces() {
        let state = AppState::new(Document::new(crate::actions::new_project()));
        let mut ed = Editor::open(&state, table_type(&state)).unwrap();
        ed.def.params[1].name = "2x".into();
        assert!(validate(&ed).is_err());
        ed.def.params[1].name = ed.def.params[0].name.clone();
        assert!(validate(&ed).is_err());
        let mut ed = Editor::new_component();
        assert_eq!(validate(&ed), Ok(()));
        if let ComponentShape::Box { height, .. } = &mut ed.def.items[0].shape {
            *height = "Missing + 1".into();
        }
        assert!(validate(&ed).is_err());
        ed.def.items.clear();
        assert!(validate(&ed).is_err());
    }

    #[test]
    fn new_component_creates_a_type_and_selects_its_tool() {
        let mut state = AppState::new(Document::new(crate::actions::new_project()));
        let before = state.project().types_of(Category::Component).count();
        let mut ed = Editor::new_component();
        ed.name = "Bancada".into();
        apply(&mut state, ed);
        assert_eq!(state.project().types_of(Category::Component).count(), before + 1);
        let crate::tools::Tool::Component { type_id } = state.tool.clone() else { panic!("tool") };
        assert_eq!(def_of(&state, type_id).0, "Bancada");
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use cc_core::{Category, Document};

    #[test]
    fn loading_a_component_file_adds_renamed_types_in_one_step() {
        let mut state = AppState::new(Document::new(crate::actions::new_project()));
        let file = cc_core::component_file::extract(state.project(), "Mesa").unwrap();
        let text = cc_core::component_file::to_ron(&file).unwrap();
        let path = std::env::temp_dir().join(format!("ccad-mesa-{}.ccomp", std::process::id()));
        std::fs::write(&path, text).unwrap();
        let before = state.project().types_of(Category::Component).count();
        let depth = state.session.doc.undo_depth();
        crate::actions::load_component_path(&mut state, path);
        let crate::tools::Tool::Component { type_id: first } = state.tool.clone() else { panic!("tool") };
        assert_eq!(state.session.doc.undo_depth(), depth + 1);
        assert_eq!(state.project().types_of(Category::Component).count(), before + 1);
        assert_eq!(state.project().type_name(first), "Mesa 120 x 80 (2)");
        assert!(matches!(state.tool, crate::tools::Tool::Component { type_id } if type_id == first));
        state.session.doc.undo();
        assert_eq!(state.project().types_of(Category::Component).count(), before);
    }
}
