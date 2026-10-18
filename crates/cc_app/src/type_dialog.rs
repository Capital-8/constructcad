//! Type Properties dialog: switch, duplicate and rename types, edit the
//! compound structure (layers) and the type parameters in sections.

use crate::actions;
use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use crate::table::{readonly_row, row, section};
use crate::theme::{self, PALETTE};
use cc_core::{ElementType, ElementTypeId, Layer, LayerFunction, TypeData, TypeInfo, WallFunction};
use cc_views::format;
use egui::{Context, DragValue, RichText, Ui};
use std::hash::{Hash, Hasher};

fn key(parts: impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

fn finish(state: &mut AppState, r: &egui::Response) {
    if r.drag_stopped() || r.lost_focus() {
        state.doc.break_merge();
    }
}

/// Apply `f` to type `tid`, merging consecutive edits of the same field.
fn update(state: &mut AppState, tid: ElementTypeId, field: &str, f: impl FnOnce(&mut ElementType)) {
    state.doc.edit_merged(t!("tx-modify-type"), key((tid, field)), |e| {
        if let Some(t) = e.type_mut(tid) {
            f(t);
        }
    });
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(tid) = state.dialogs.type_editor else { return };
    let Some(ty) = state.project().element_type(tid).cloned() else {
        state.dialogs.type_editor = None;
        return;
    };
    crate::dialog::session::begin(state);
    let mut ok = 0;
    let max_height = (ctx.content_rect().height() - 320.0).clamp(240.0, 560.0);
    let response = crate::dialog::Dialog::new("type-editor", t!("dlg-type")).width(580.0).show(
        ctx,
        |ui| {
            header(ui, state, &ty);
            ui.add_space(6.0);
            ui.label(theme::muted(t!("type-parameters")).size(12.0));
            egui::Frame::new().stroke(egui::Stroke::new(1.0, PALETTE.border)).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    parameters(ui, state, &ty);
                });
            });
        },
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    // While renaming, Esc only cancels the rename.
    let renaming = state.dialogs.type_rename.is_some();
    let response = crate::dialog::DialogResponse { dismissed: response.dismissed && !renaming, ..response };
    if crate::dialogs::ok_cancel(state, &response, ok) {
        state.dialogs.type_editor = None;
        state.dialogs.type_rename = None;
    }
}

/// Family (category), type selector and the Duplicate / Rename buttons.
fn header(ui: &mut Ui, state: &mut AppState, ty: &ElementType) {
    let tid = ty.id;
    let category = ty.category();
    egui::Grid::new("type-header").num_columns(3).striped(false).spacing([10.0, 8.0]).show(ui, |ui| {
        ui.label(theme::muted(t!("type-category")));
        ui.label(RichText::new(actions::t_category(category)).color(PALETTE.text));
        ui.label("");
        ui.end_row();

        ui.label(theme::muted(t!("type-type")));
        let button_w = 110.0;
        let field_w = (ui.available_width() - button_w - 12.0).max(200.0);
        match state.dialogs.type_rename.take() {
            Some(mut name) => {
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(field_w));
                r.request_focus();
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                if (r.lost_focus() || enter) && !escape {
                    let name = name.trim().to_string();
                    if !name.is_empty() && name != ty.name {
                        state.doc.edit(t!("tx-modify-type"), |e| {
                            if let Some(t) = e.type_mut(tid) {
                                t.name = name;
                            }
                        });
                    }
                } else if !escape {
                    state.dialogs.type_rename = Some(name);
                }
            }
            None => {
                let mut selected = tid;
                let types: Vec<_> = state.project().types_of(category).map(|t| (t.id, t.name.clone())).collect();
                egui::ComboBox::from_id_salt("type-switch").width(field_w).selected_text(ty.name.clone()).show_ui(
                    ui,
                    |ui| {
                        for (id, name) in types {
                            ui.selectable_value(&mut selected, id, name);
                        }
                    },
                );
                if selected != tid {
                    state.dialogs.type_editor = Some(selected);
                }
            }
        }
        ui.vertical(|ui| {
            if ui.add_sized([button_w, 24.0], egui::Button::new(t!("type-duplicate"))).clicked() {
                duplicate(state, ty);
            }
            if ui.add_sized([button_w, 24.0], egui::Button::new(t!("type-rename"))).clicked() {
                state.dialogs.type_rename = Some(ty.name.clone());
            }
        });
        ui.end_row();
    });
}

/// Copy the type under a new name; selected instances of it switch to the copy.
fn duplicate(state: &mut AppState, ty: &ElementType) {
    let tid = ty.id;
    let selection: Vec<_> = state.selection.iter().copied().collect();
    let new_id = state.doc.edit(t!("tx-duplicate-type"), |e| {
        let id = ElementTypeId(e.alloc_id());
        e.add_type(ElementType {
            id,
            name: t!("copy-of", "name" => ty.name),
            data: ty.data.clone(),
            info: ty.info.clone(),
            uid: Default::default(),
        });
        for el in selection {
            if let Some(x) = e.element_mut(el).filter(|x| x.type_id == tid) {
                x.type_id = id;
            }
        }
        id
    });
    state.dialogs.type_editor = Some(new_id);
    state.dialogs.type_rename = Some(t!("copy-of", "name" => ty.name));
}

fn length_row(
    ui: &mut Ui,
    state: &mut AppState,
    label: &str,
    tid: ElementTypeId,
    field: &str,
    value: f64,
    set: impl FnOnce(&mut TypeData, f64),
) {
    let mut v = value;
    let r = row(ui, label, |ui| {
        ui.add(DragValue::new(&mut v).speed(0.005).range(0.001..=20.0).fixed_decimals(3).suffix(" m"))
    });
    if r.changed() {
        update(state, tid, field, |t| set(&mut t.data, v));
    }
    finish(state, &r);
}

/// Component name and kind, the editor button and this type's parameter values.
fn component_parameters(ui: &mut Ui, state: &mut AppState, ty: &ElementType) {
    let TypeData::Component { component, def } = &ty.data else { return };
    let tid = ty.id;
    section(ui, "type-component", t!("section-component"), |ui| {
        readonly_row(ui, &t!("col-component"), component.clone());
        readonly_row(ui, &t!("col-component-kind"), tr(def.kind.key()));
        if row(ui, &t!("comp-definition"), |ui| ui.button(t!("action-edit-component")).clicked()) {
            state.dialogs.component_editor = crate::component_editor::Editor::open(state, tid);
        }
    });
    section(ui, "type-comp-params", t!("comp-params"), |ui| {
        for (i, p) in def.params.iter().enumerate() {
            let mut v = p.value;
            let label = if p.instance { format!("{} ({})", p.name, t!("comp-param-instance")) } else { p.name.clone() };
            let r = row(ui, &label, |ui| ui.add(crate::properties::param_value(&mut v, p.kind)));
            if r.changed() {
                update(state, tid, &format!("comp-param-{i}"), |t| {
                    if let TypeData::Component { def, .. } = &mut t.data
                        && let Some(p) = def.params.get_mut(i)
                    {
                        p.value = v;
                    }
                });
            }
            finish(state, &r);
        }
    });
}

fn parameters(ui: &mut Ui, state: &mut AppState, ty: &ElementType) {
    let tid = ty.id;
    if matches!(ty.data, TypeData::Component { .. }) {
        component_parameters(ui, state, ty);
        identity(ui, state, tid, &ty.info);
        return;
    }
    let layered = !ty.layers().is_empty();

    section(ui, "type-construction", t!("section-construction"), |ui| {
        if layered {
            let editing = state.dialogs.type_layers_open;
            let label = if editing { t!("action-hide") } else { t!("action-edit") };
            if row(ui, &t!("prop-structure"), |ui| ui.button(label).clicked()) {
                state.dialogs.type_layers_open = !editing;
            }
            if state.dialogs.type_layers_open {
                layers_editor(ui, state, ty);
            }
        }
        if let Some(function) = ty.wall_function() {
            let mut f = function;
            row(ui, &t!("prop-function"), |ui| {
                egui::ComboBox::from_id_salt("wall-function").selected_text(tr(f.key())).show_ui(ui, |ui| {
                    for option in WallFunction::ALL {
                        ui.selectable_value(&mut f, option, tr(option.key()));
                    }
                });
            });
            if f != function {
                update(state, tid, "function", |t| {
                    if let TypeData::Wall { function, .. } = &mut t.data {
                        *function = f;
                    }
                });
            }
        }
        match ty.data {
            TypeData::Ceiling { thickness, .. } => {
                length_row(ui, state, &t!("prop-thickness"), tid, "thk", thickness, |d, v| {
                    if let TypeData::Ceiling { thickness, .. } = d {
                        *thickness = v;
                    }
                });
            }
            _ if layered => {
                readonly_row(ui, &t!("prop-thickness"), format!("{} m", format::meters(ty.thickness())));
            }
            _ => {}
        }
    });

    structure_parameters(ui, state, ty);

    if let TypeData::Door { width, height, .. } | TypeData::Window { width, height, .. } = ty.data {
        section(ui, "type-dimensions", t!("section-dimensions"), |ui| {
            length_row(ui, state, &t!("col-width"), tid, "w", width, |d, v| {
                if let TypeData::Door { width, .. } | TypeData::Window { width, .. } = d {
                    *width = v.max(0.1);
                }
            });
            length_row(ui, state, &t!("col-height"), tid, "h", height, |d, v| {
                if let TypeData::Door { height, .. } | TypeData::Window { height, .. } = d {
                    *height = v.max(0.1);
                }
            });
            if let TypeData::Window { default_sill, .. } = ty.data {
                length_row(ui, state, &t!("prop-sill"), tid, "sill", default_sill, |d, v| {
                    if let TypeData::Window { default_sill, .. } = d {
                        *default_sill = v.max(0.0);
                    }
                });
            }
        });
    }

    if let TypeData::Door { material, .. }
    | TypeData::Window { material, .. }
    | TypeData::Ceiling { material, .. }
    | TypeData::Column { material, .. }
    | TypeData::Beam { material, .. }
    | TypeData::Stair { material, .. }
    | TypeData::Railing { material, .. } = &ty.data
    {
        section(ui, "type-materials", t!("section-materials"), |ui| {
            let mut m = material.clone();
            let changed =
                row(ui, &t!("prop-material"), |ui| crate::dialogs::material_combo(ui, state, "type-material", &mut m));
            if changed {
                update(state, tid, "mat", |t| match &mut t.data {
                    TypeData::Door { material, .. }
                    | TypeData::Window { material, .. }
                    | TypeData::Ceiling { material, .. }
                    | TypeData::Column { material, .. }
                    | TypeData::Beam { material, .. }
                    | TypeData::Stair { material, .. }
                    | TypeData::Railing { material, .. } => *material = m,
                    _ => {}
                });
            }
        });
    }

    section(ui, "type-identity", t!("section-identity"), |ui| identity(ui, state, tid, &ty.info));

    section(ui, "type-ifc", t!("section-ifc"), |ui| {
        readonly_row(ui, &t!("prop-ifc-guid"), cc_ifc::guid::encode(ty.uid.0));
    });
}

/// Text fields of the type information.
fn identity(ui: &mut Ui, state: &mut AppState, tid: ElementTypeId, info: &TypeInfo) {
    type Field = (&'static str, fn(&mut TypeInfo) -> &mut String);
    let fields: [Field; 9] = [
        ("info-type-mark", |i| &mut i.type_mark),
        ("info-description", |i| &mut i.description),
        ("info-manufacturer", |i| &mut i.manufacturer),
        ("info-model", |i| &mut i.model),
        ("info-url", |i| &mut i.url),
        ("info-keynote", |i| &mut i.keynote),
        ("info-classification", |i| &mut i.classification),
        ("info-fire-rating", |i| &mut i.fire_rating),
        ("info-type-comments", |i| &mut i.comments),
    ];
    for (label, field) in fields {
        let mut text = field(&mut info.clone()).clone();
        let r = row(ui, &tr(label), |ui| ui.text_edit_singleline(&mut text));
        if r.changed() {
            update(state, tid, label, |t| *field(&mut t.info) = text);
        }
        finish(state, &r);
    }
    let mut cost = info.cost.unwrap_or(0.0);
    let r = row(ui, &t!("info-cost"), |ui| {
        ui.add(DragValue::new(&mut cost).speed(1.0).range(0.0..=f64::MAX).fixed_decimals(2))
    });
    if r.changed() {
        update(state, tid, "cost", |t| t.info.cost = Some(cost));
    }
    finish(state, &r);
}

/// Layers table: function, material and thickness, with reorder and removal.
fn layers_editor(ui: &mut Ui, state: &mut AppState, ty: &ElementType) {
    let tid = ty.id;
    let layers = ty.layers().to_vec();
    let set_layers = |state: &mut AppState, field: &str, layers: Vec<Layer>| {
        update(state, tid, field, |t| match &mut t.data {
            TypeData::Wall { layers: l, .. } | TypeData::Floor { layers: l } | TypeData::Roof { layers: l } => {
                *l = layers
            }
            _ => {}
        });
    };
    egui::Frame::new().fill(PALETTE.panel).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        ui.label(theme::muted(t!("type-layers")).size(12.0));
        egui::Grid::new(("layers", tid.0)).num_columns(5).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
            for h in ["", "col-function", "prop-material", "prop-thickness", ""] {
                ui.label(theme::muted(if h.is_empty() { String::new() } else { tr(h) }).size(11.5));
            }
            ui.end_row();
            for (i, layer) in layers.iter().enumerate() {
                ui.label(theme::muted((i + 1).to_string()));
                let mut f = layer.function;
                egui::ComboBox::from_id_salt(("lfn", tid.0, i)).width(120.0).selected_text(tr(f.key())).show_ui(
                    ui,
                    |ui| {
                        for option in LayerFunction::ALL {
                            ui.selectable_value(&mut f, option, tr(option.key()));
                        }
                    },
                );
                if f != layer.function {
                    let mut l = layers.clone();
                    l[i].function = f;
                    set_layers(state, "lfn", l);
                    state.doc.break_merge();
                }
                let mut m = layer.material.clone();
                if crate::dialogs::material_combo(ui, state, ("lmat", tid.0, i), &mut m) {
                    let mut l = layers.clone();
                    l[i].material = m;
                    set_layers(state, "lmat", l);
                    state.doc.break_merge();
                }
                let mut thk = layer.thickness;
                let r = ui.add(DragValue::new(&mut thk).speed(0.002).range(0.001..=2.0).fixed_decimals(3).suffix(" m"));
                if r.changed() {
                    let mut l = layers.clone();
                    l[i].thickness = thk;
                    set_layers(state, &format!("lthk{i}"), l);
                }
                finish(state, &r);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let n = layers.len();
                    let moves = [("↑", i > 0, i.wrapping_sub(1), "layer-up"), ("↓", i + 1 < n, i + 1, "layer-down")];
                    for (glyph, enabled, target, tip) in moves {
                        if ui.add_enabled(enabled, egui::Button::new(glyph).small()).on_hover_text(tr(tip)).clicked() {
                            let mut l = layers.clone();
                            l.swap(i, target);
                            set_layers(state, "lmove", l);
                            state.doc.break_merge();
                        }
                    }
                    let remove =
                        ui.add_enabled(n > 1, egui::Button::new("×").small()).on_hover_text(t!("layer-remove"));
                    if remove.clicked() {
                        let mut l = layers.clone();
                        l.remove(i);
                        set_layers(state, "lremove", l);
                        state.doc.break_merge();
                    }
                });
                ui.end_row();
            }
        });
        ui.horizontal(|ui| {
            if ui.button(t!("type-add-layer")).clicked() {
                let mut l = layers.clone();
                let material = state.project().materials.first().map(|m| m.name.clone()).unwrap_or_default();
                l.push(Layer { material, thickness: 0.02, function: LayerFunction::Finish });
                set_layers(state, "ladd", l);
                state.doc.break_merge();
            }
            let total = format!("{}: {} m", t!("type-total-thickness"), format::number(ty.thickness(), 3));
            ui.label(theme::muted(total));
        });
    });
}

/// Section and layout parameters of columns, beams, stairs and railings.
fn structure_parameters(ui: &mut Ui, state: &mut AppState, ty: &ElementType) {
    let tid = ty.id;
    match ty.data {
        TypeData::Column { section, .. } => {
            section_rows(ui, state, tid, section);
        }
        TypeData::Beam { width, height, .. } => {
            crate::table::section(ui, "type-dimensions", t!("section-dimensions"), |ui| {
                length_row(ui, state, &t!("col-width"), tid, "bw", width, |d, v| {
                    if let TypeData::Beam { width, .. } = d {
                        *width = v.max(0.01);
                    }
                });
                length_row(ui, state, &t!("col-height"), tid, "bh", height, |d, v| {
                    if let TypeData::Beam { height, .. } = d {
                        *height = v.max(0.01);
                    }
                });
            });
        }
        TypeData::Stair { width, max_riser, tread, .. } => {
            crate::table::section(ui, "type-dimensions", t!("section-dimensions"), |ui| {
                length_row(ui, state, &t!("col-width"), tid, "sw", width, |d, v| {
                    if let TypeData::Stair { width, .. } = d {
                        *width = v.max(0.3);
                    }
                });
                length_row(ui, state, &t!("prop-max-riser"), tid, "sr", max_riser, |d, v| {
                    if let TypeData::Stair { max_riser, .. } = d {
                        *max_riser = v.clamp(0.05, 0.5);
                    }
                });
                length_row(ui, state, &t!("col-tread"), tid, "st", tread, |d, v| {
                    if let TypeData::Stair { tread, .. } = d {
                        *tread = v.clamp(0.1, 1.0);
                    }
                });
            });
        }
        TypeData::Railing { height, post_spacing, .. } => {
            crate::table::section(ui, "type-dimensions", t!("section-dimensions"), |ui| {
                length_row(ui, state, &t!("col-height"), tid, "rh", height, |d, v| {
                    if let TypeData::Railing { height, .. } = d {
                        *height = v.clamp(0.3, 3.0);
                    }
                });
                length_row(ui, state, &t!("prop-post-spacing"), tid, "rp", post_spacing, |d, v| {
                    if let TypeData::Railing { post_spacing, .. } = d {
                        *post_spacing = v.clamp(0.1, 5.0);
                    }
                });
            });
        }
        _ => {}
    }
}

fn section_rows(ui: &mut Ui, state: &mut AppState, tid: ElementTypeId, section: cc_core::ColumnSection) {
    use cc_core::ColumnSection;
    crate::table::section(ui, "type-dimensions", t!("section-dimensions"), |ui| match section {
        ColumnSection::Rectangular { width, depth } => {
            length_row(ui, state, &t!("col-width"), tid, "cw", width, |d, v| {
                if let TypeData::Column { section: ColumnSection::Rectangular { width, .. }, .. } = d {
                    *width = v.max(0.01);
                }
            });
            length_row(ui, state, &t!("prop-depth-section"), tid, "cd", depth, |d, v| {
                if let TypeData::Column { section: ColumnSection::Rectangular { depth, .. }, .. } = d {
                    *depth = v.max(0.01);
                }
            });
        }
        ColumnSection::Round { diameter } => {
            length_row(ui, state, &t!("prop-diameter"), tid, "cdia", diameter, |d, v| {
                if let TypeData::Column { section: ColumnSection::Round { diameter }, .. } = d {
                    *diameter = v.max(0.01);
                }
            });
        }
    });
}
