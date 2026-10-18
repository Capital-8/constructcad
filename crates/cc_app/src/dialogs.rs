//! Modal and floating dialogs.

use crate::actions;
use crate::state::AppState;
use crate::t;
use cc_core::{ElementType, ElementTypeId, Layer, Material, TypeData};
use egui::{Context, DragValue, RichText, Ui};
use std::hash::{Hash, Hasher};

fn key(parts: impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

fn done(state: &mut AppState, r: &egui::Response) {
    if r.drag_stopped() || r.lost_focus() {
        state.doc.break_merge();
    }
}

pub fn show(ctx: &Context, state: &mut AppState) {
    project_info(ctx, state);
    levels(ctx, state);
    materials(ctx, state);
    type_editor(ctx, state);
    confirm(ctx, state);
    error(ctx, state);
    about(ctx, state);
}

fn project_info(ctx: &Context, state: &mut AppState) {
    let mut open = state.dialogs.project_info;
    egui::Window::new(t!("dlg-project-info")).open(&mut open).resizable(false).show(ctx, |ui| {
        let info = state.project().info.clone();
        egui::Grid::new("info").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            let fields = [
                (t!("info-name"), "name", info.name),
                (t!("info-number"), "number", info.number),
                (t!("info-client"), "client", info.client),
                (t!("info-address"), "address", info.address),
                (t!("info-author"), "author", info.author),
            ];
            for (label, field, value) in fields {
                ui.label(label);
                let mut text = value;
                let r = ui.text_edit_singleline(&mut text);
                if r.changed() {
                    state.doc.edit_merged(t!("tx-project-info"), key(("info", field)), |e| {
                        let info = e.info_mut();
                        match field {
                            "name" => info.name = text,
                            "number" => info.number = text,
                            "client" => info.client = text,
                            "address" => info.address = text,
                            _ => info.author = text,
                        }
                    });
                }
                done(state, &r);
                ui.end_row();
            }
        });
    });
    state.dialogs.project_info = open;
}

fn levels(ctx: &Context, state: &mut AppState) {
    let mut open = state.dialogs.levels;
    egui::Window::new(t!("dlg-levels")).open(&mut open).resizable(false).show(ctx, |ui| {
        let levels = state.project().levels.clone();
        egui::Grid::new("levels").num_columns(3).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label(RichText::new(t!("prop-name")).strong());
            ui.label(RichText::new(t!("level-elevation")).strong());
            ui.label("");
            ui.end_row();
            for level in levels.iter().rev() {
                let mut name = level.name.clone();
                let r = ui.text_edit_singleline(&mut name);
                if r.changed() {
                    state.doc.edit_merged(t!("tx-modify-level"), key((level.id, "name")), |e| {
                        if let Some(l) = e.levels_mut().iter_mut().find(|l| l.id == level.id) {
                            l.name = name;
                        }
                    });
                }
                done(state, &r);
                let mut elev = level.elevation;
                let r = ui.add(DragValue::new(&mut elev).speed(0.01).fixed_decimals(2).suffix(" m"));
                if r.changed() {
                    state.doc.edit_merged(t!("tx-modify-level"), key((level.id, "elev")), |e| {
                        if let Some(l) = e.levels_mut().iter_mut().find(|l| l.id == level.id) {
                            l.elevation = elev;
                        }
                        e.levels_mut().sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
                    });
                }
                done(state, &r);
                if ui.add_enabled(levels.len() > 1, egui::Button::new(t!("action-delete"))).clicked() {
                    actions::delete_level(state, level.id);
                }
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        if ui.button(t!("level-add")).clicked() {
            actions::add_level(state);
        }
    });
    state.dialogs.levels = open;
}

fn materials(ctx: &Context, state: &mut AppState) {
    let mut open = state.dialogs.materials;
    egui::Window::new(t!("dlg-materials")).open(&mut open).resizable(false).show(ctx, |ui| {
        let mats = state.project().materials.clone();
        egui::Grid::new("materials").num_columns(2).striped(true).show(ui, |ui| {
            for (i, m) in mats.iter().enumerate() {
                let mut color = m.color;
                let r = ui.color_edit_button_srgb(&mut color);
                if r.changed() {
                    state.doc.edit_merged(t!("tx-modify-material"), key(("mat", i)), |e| {
                        if let Some(m) = e.materials_mut().get_mut(i) {
                            m.color = color;
                        }
                    });
                }
                // Names are referenced by layers, so they stay fixed here.
                ui.label(&m.name);
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        if ui.button(t!("material-add")).clicked() {
            let n = mats.len() + 1;
            state.doc.edit(t!("tx-modify-material"), |e| {
                e.materials_mut()
                    .push(Material { name: t!("default-material-name", "n" => n), color: [180, 180, 180] });
            });
        }
    });
    state.dialogs.materials = open;
}

fn material_combo(ui: &mut Ui, state: &AppState, id: impl Hash + std::fmt::Debug, current: &mut String) -> bool {
    let before = current.clone();
    egui::ComboBox::from_id_salt(id).selected_text(current.as_str()).width(140.0).show_ui(ui, |ui| {
        for m in &state.project().materials {
            ui.selectable_value(current, m.name.clone(), &m.name);
        }
    });
    *current != before
}

fn type_editor(ctx: &Context, state: &mut AppState) {
    let Some(tid) = state.dialogs.type_editor else { return };
    let Some(ty) = state.project().element_type(tid).cloned() else {
        state.dialogs.type_editor = None;
        return;
    };
    let mut open = true;
    egui::Window::new(t!("dlg-type")).id(egui::Id::new("type_editor")).open(&mut open).resizable(false).show(
        ctx,
        |ui| {
            ui.label(RichText::new(actions::t_category(ty.category())).weak());
            let update = |state: &mut AppState, field: &str, f: &dyn Fn(&mut ElementType)| {
                state.doc.edit_merged(t!("tx-modify-type"), key((tid, field)), |e| {
                    if let Some(t) = e.type_mut(tid) {
                        f(t);
                    }
                });
            };
            egui::Grid::new("type").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(t!("prop-name"));
                let mut name = ty.name.clone();
                let r = ui.text_edit_singleline(&mut name);
                if r.changed() {
                    update(state, "name", &|t| t.name = name.clone());
                }
                done(state, &r);
                ui.end_row();
                match &ty.data {
                    TypeData::Door { width, height, material } | TypeData::Window { width, height, material, .. } => {
                        for (label, field, value) in [(t!("col-width"), "w", *width), (t!("col-height"), "h", *height)]
                        {
                            ui.label(label);
                            let mut v = value;
                            let r = ui.add(
                                DragValue::new(&mut v).speed(0.01).range(0.1..=10.0).fixed_decimals(2).suffix(" m"),
                            );
                            if r.changed() {
                                update(state, field, &|t| match &mut t.data {
                                    TypeData::Door { width, height, .. } | TypeData::Window { width, height, .. } => {
                                        if field == "w" {
                                            *width = v;
                                        } else {
                                            *height = v;
                                        }
                                    }
                                    _ => {}
                                });
                            }
                            done(state, &r);
                            ui.end_row();
                        }
                        if let TypeData::Window { default_sill, .. } = ty.data {
                            ui.label(t!("prop-sill"));
                            let mut v = default_sill;
                            let r = ui.add(
                                DragValue::new(&mut v).speed(0.01).range(0.0..=10.0).fixed_decimals(2).suffix(" m"),
                            );
                            if r.changed() {
                                update(state, "sill", &|t| {
                                    if let TypeData::Window { default_sill, .. } = &mut t.data {
                                        *default_sill = v;
                                    }
                                });
                            }
                            done(state, &r);
                            ui.end_row();
                        }
                        ui.label(t!("prop-material"));
                        let mut m = material.clone();
                        if material_combo(ui, state, ("tmat", tid.0), &mut m) {
                            update(state, "mat", &|t| match &mut t.data {
                                TypeData::Door { material, .. } | TypeData::Window { material, .. } => {
                                    *material = m.clone()
                                }
                                _ => {}
                            });
                        }
                        ui.end_row();
                    }
                    TypeData::Ceiling { thickness, material } => {
                        ui.label(t!("prop-thickness"));
                        let mut v = *thickness;
                        let r = ui
                            .add(DragValue::new(&mut v).speed(0.005).range(0.005..=1.0).fixed_decimals(3).suffix(" m"));
                        if r.changed() {
                            update(state, "thk", &|t| {
                                if let TypeData::Ceiling { thickness, .. } = &mut t.data {
                                    *thickness = v;
                                }
                            });
                        }
                        done(state, &r);
                        ui.end_row();
                        ui.label(t!("prop-material"));
                        let mut m = material.clone();
                        if material_combo(ui, state, ("tmat", tid.0), &mut m) {
                            update(state, "mat", &|t| {
                                if let TypeData::Ceiling { material, .. } = &mut t.data {
                                    *material = m.clone();
                                }
                            });
                        }
                        ui.end_row();
                    }
                    _ => {}
                }
            });

            let layers = ty.layers().to_vec();
            if !layers.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(t!("type-layers")).strong());
                let set_layers = |state: &mut AppState, field: &str, layers: Vec<Layer>| {
                    state.doc.edit_merged(t!("tx-modify-type"), key((tid, field)), |e| {
                        if let Some(t) = e.type_mut(tid) {
                            match &mut t.data {
                                TypeData::Wall { layers: l }
                                | TypeData::Floor { layers: l }
                                | TypeData::Roof { layers: l } => *l = layers,
                                _ => {}
                            }
                        }
                    });
                };
                egui::Grid::new("layers").num_columns(3).striped(true).show(ui, |ui| {
                    for (i, layer) in layers.iter().enumerate() {
                        let mut m = layer.material.clone();
                        if material_combo(ui, state, ("lmat", tid.0, i), &mut m) {
                            let mut l = layers.clone();
                            l[i].material = m;
                            set_layers(state, "lmat", l);
                        }
                        let mut thk = layer.thickness;
                        let r = ui.add(
                            DragValue::new(&mut thk).speed(0.002).range(0.001..=2.0).fixed_decimals(3).suffix(" m"),
                        );
                        if r.changed() {
                            let mut l = layers.clone();
                            l[i].thickness = thk;
                            set_layers(state, &format!("lthk{i}"), l);
                        }
                        done(state, &r);
                        if ui.add_enabled(layers.len() > 1, egui::Button::new("x")).clicked() {
                            let mut l = layers.clone();
                            l.remove(i);
                            set_layers(state, "lremove", l);
                            state.doc.break_merge();
                        }
                        ui.end_row();
                    }
                });
                ui.label(format!("{}: {} m", t!("type-total-thickness"), cc_views::format::number(ty.thickness(), 3)));
                if ui.button(t!("type-add-layer")).clicked() {
                    let mut l = layers.clone();
                    let material = state.project().materials.first().map(|m| m.name.clone()).unwrap_or_default();
                    l.push(Layer { material, thickness: 0.02 });
                    set_layers(state, "ladd", l);
                    state.doc.break_merge();
                }
            }

            ui.add_space(8.0);
            ui.separator();
            if ui.button(t!("type-duplicate")).clicked() {
                let selection: Vec<_> = state.selection.iter().copied().collect();
                let new_id = state.doc.edit(t!("tx-duplicate-type"), |e| {
                    let id = ElementTypeId(e.alloc_id());
                    e.add_type(ElementType { id, name: t!("copy-of", "name" => ty.name), data: ty.data.clone() });
                    // Like Revit, the selected instances switch to the new type.
                    for el in selection {
                        if let Some(x) = e.element_mut(el).filter(|x| x.type_id == tid) {
                            x.type_id = id;
                        }
                    }
                    id
                });
                state.dialogs.type_editor = Some(new_id);
            }
        },
    );
    if !open {
        state.dialogs.type_editor = None;
    }
}

fn confirm(ctx: &Context, state: &mut AppState) {
    let Some(action) = state.dialogs.confirm.clone() else { return };
    let modal = egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
        ui.set_width(340.0);
        ui.heading(t!("dlg-unsaved"));
        ui.label(t!("dlg-unsaved-text"));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(t!("action-save")).clicked() {
                state.dialogs.confirm = None;
                if actions::save(state, false) {
                    actions::run_pending(state, action.clone());
                }
            }
            if ui.button(t!("action-discard")).clicked() {
                state.dialogs.confirm = None;
                actions::run_pending(state, action.clone());
            }
            if ui.button(t!("action-cancel")).clicked() {
                state.dialogs.confirm = None;
            }
        });
    });
    if modal.should_close() {
        state.dialogs.confirm = None;
    }
}

fn error(ctx: &Context, state: &mut AppState) {
    let Some(msg) = state.dialogs.error.clone() else { return };
    let modal = egui::Modal::new(egui::Id::new("error")).show(ctx, |ui| {
        ui.set_width(380.0);
        ui.heading(t!("dlg-error"));
        ui.label(msg);
        ui.add_space(8.0);
        if ui.button(t!("action-ok")).clicked() {
            state.dialogs.error = None;
        }
    });
    if modal.should_close() {
        state.dialogs.error = None;
    }
}

fn about(ctx: &Context, state: &mut AppState) {
    let mut open = state.dialogs.about;
    egui::Window::new(t!("dlg-about")).open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.heading("ConstructCAD");
        ui.label(format!("{} {}", t!("about-version"), env!("CARGO_PKG_VERSION")));
        ui.label(t!("about-text"));
        ui.label(t!("about-license"));
        ui.label(env!("CARGO_PKG_AUTHORS"));
    });
    state.dialogs.about = open;
}
