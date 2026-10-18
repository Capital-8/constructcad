//! Modal and floating dialogs.

use crate::actions;
use crate::state::AppState;
use crate::t;
use cc_core::Material;
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
    crate::type_dialog::show(ctx, state);
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

pub(crate) fn material_combo(
    ui: &mut Ui,
    state: &AppState,
    id: impl Hash + std::fmt::Debug,
    current: &mut String,
) -> bool {
    let before = current.clone();
    egui::ComboBox::from_id_salt(id).selected_text(current.as_str()).width(140.0).show_ui(ui, |ui| {
        for m in &state.project().materials {
            ui.selectable_value(current, m.name.clone(), &m.name);
        }
    });
    *current != before
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
    use crate::theme::{self, PALETTE};
    use egui::RichText;
    if !state.dialogs.about {
        return;
    }
    let repository = env!("CARGO_PKG_REPOSITORY");
    let modal = egui::Modal::new(egui::Id::new("about")).frame(
        egui::Frame::popup(&ctx.style_of(egui::Theme::Light)).inner_margin(egui::Margin::same(24)).corner_radius(10),
    );
    let response = modal.show(ctx, |ui| {
        ui.set_width(440.0);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
            if let Some(logo) = theme::logo(ui.ctx()) {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(logo.id(), rect, uv, egui::Color32::WHITE);
            }
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("ConstructCAD").family(theme::semibold()).size(22.0).color(PALETTE.text));
                let version = format!("{} {}", t!("about-version"), env!("CARGO_PKG_VERSION"));
                ui.label(RichText::new(version).color(PALETTE.text_muted));
            });
        });
        ui.add_space(12.0);
        ui.label(t!("about-text"));
        ui.label(RichText::new(t!("about-familiar")).color(PALETTE.text_muted));
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);
        egui::Grid::new("about-info").num_columns(2).striped(false).spacing([16.0, 6.0]).show(ui, |ui| {
            ui.label(theme::muted(t!("about-author")));
            ui.label(env!("CARGO_PKG_AUTHORS"));
            ui.end_row();
            ui.label(theme::muted(t!("about-license-label")));
            ui.hyperlink_to("GPL-3.0-or-later", "https://www.gnu.org/licenses/gpl-3.0.html");
            ui.end_row();
            ui.label(theme::muted(t!("about-repository")));
            ui.hyperlink_to(repository.trim_start_matches("https://"), repository);
            ui.end_row();
        });
        ui.add_space(12.0);
        ui.label(theme::title(t!("about-credits")));
        ui.add_space(2.0);
        for line in [t!("about-built-with"), t!("about-font-credit"), t!("about-icon-credit")] {
            ui.label(RichText::new(line).size(12.0).color(PALETTE.text_muted));
        }
        ui.add_space(12.0);
        ui.label(RichText::new(t!("about-independence")).size(11.5).color(PALETTE.text_faint));
        ui.add_space(16.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let close = ui.add(
                egui::Button::new(RichText::new(t!("action-close")).color(egui::Color32::WHITE)).fill(PALETTE.accent),
            );
            if ui.button(t!("about-open-repository")).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(repository));
            }
            close.clicked()
        })
        .inner
    });
    if response.inner || response.should_close() {
        state.dialogs.about = false;
    }
}
