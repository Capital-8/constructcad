//! Ribbon, options bar and status bar.

use crate::actions;
use crate::state::{AppState, PendingAction, RibbonTab};
use crate::t;
use crate::tools::{Tool, walls_loop};
use cc_core::{Category, ViewCategory};
use cc_views::format;
use egui::{Button, DragValue, RichText, Ui};

fn tool_button(ui: &mut Ui, state: &mut AppState, label: String, shortcut: &str, category: Category) {
    let active = state.tool.category() == Some(category) && state.tool != Tool::Section;
    let r = ui.add(Button::selectable(active, label)).on_hover_text(format!("{} ({shortcut})", t!("hint-shortcut")));
    if r.clicked() {
        actions::activate_category_tool(state, category);
    }
}

pub fn ribbon(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("ConstructCAD").strong().color(egui::Color32::from_rgb(30, 90, 170)));
        ui.separator();
        for (tab, key) in [
            (RibbonTab::File, t!("ribbon-file")),
            (RibbonTab::Architecture, t!("ribbon-architecture")),
            (RibbonTab::Modify, t!("ribbon-modify")),
            (RibbonTab::View, t!("ribbon-view")),
            (RibbonTab::Manage, t!("ribbon-manage")),
        ] {
            ui.selectable_value(&mut state.ribbon_tab, tab, key);
        }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| match state.ribbon_tab {
        RibbonTab::File => {
            if ui.button(t!("action-new")).on_hover_text("Ctrl+N").clicked() {
                actions::guarded(state, PendingAction::New);
            }
            if ui.button(t!("action-open")).on_hover_text("Ctrl+O").clicked() {
                actions::guarded(state, PendingAction::Open);
            }
            if ui.button(t!("action-save")).on_hover_text("Ctrl+S").clicked() {
                actions::save(state, false);
            }
            if ui.button(t!("action-save-as")).on_hover_text("Ctrl+Shift+S").clicked() {
                actions::save(state, true);
            }
            ui.separator();
            ui.label(t!("settings-language"));
            let mut lang = crate::i18n::language();
            egui::ComboBox::from_id_salt("language").selected_text(lang.native_name()).show_ui(ui, |ui| {
                for l in crate::i18n::Language::ALL {
                    ui.selectable_value(&mut lang, l, l.native_name());
                }
            });
            actions::set_language(state, lang);
            ui.separator();
            if ui.button(t!("dlg-about")).clicked() {
                state.dialogs.about = true;
            }
        }
        RibbonTab::Architecture => {
            tool_button(ui, state, t!("tool-wall"), "WA", Category::Wall);
            tool_button(ui, state, t!("tool-door"), "DR", Category::Door);
            tool_button(ui, state, t!("tool-window"), "WN", Category::Window);
            ui.separator();
            tool_button(ui, state, t!("tool-floor"), "SB", Category::Floor);
            tool_button(ui, state, t!("tool-ceiling"), "FO", Category::Ceiling);
            tool_button(ui, state, t!("tool-roof"), "TE", Category::Roof);
            ui.separator();
            if ui
                .add(Button::selectable(state.tool == Tool::Select, t!("tool-select")))
                .on_hover_text("MD / Esc")
                .clicked()
            {
                state.set_tool(Tool::Select);
            }
        }
        RibbonTab::Modify => {
            if ui.add(Button::selectable(state.tool == Tool::Select, t!("tool-select"))).clicked() {
                state.set_tool(Tool::Select);
            }
            let has_sel = !state.selection.is_empty();
            if ui.add_enabled(has_sel, Button::new(t!("action-copy"))).on_hover_text(t!("hint-copy")).clicked() {
                let ids: Vec<_> = state.selection.iter().copied().collect();
                crate::tools::duplicate(state, &ids, glam::DVec2::new(1.0, -1.0));
            }
            if ui.add_enabled(has_sel, Button::new(t!("action-delete"))).on_hover_text("Delete").clicked() {
                state.delete_selection();
            }
            ui.separator();
            let undo = state.doc.undo_name().map(|n| format!(" ({n})")).unwrap_or_default();
            if ui
                .add_enabled(state.doc.can_undo(), Button::new(format!("{}{undo}", t!("action-undo"))))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                state.doc.undo();
            }
            if ui.add_enabled(state.doc.can_redo(), Button::new(t!("action-redo"))).on_hover_text("Ctrl+Y").clicked() {
                state.doc.redo();
            }
        }
        RibbonTab::View => {
            let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
            for (ceiling, label) in [(false, t!("view-new-floor-plan")), (true, t!("view-new-ceiling-plan"))] {
                ui.menu_button(label, |ui| {
                    for (id, name) in &levels {
                        if ui.button(name).clicked() {
                            actions::new_plan(state, *id, ceiling);
                            ui.close();
                        }
                    }
                });
            }
            if ui.button(t!("view-new-3d")).clicked() {
                actions::new_3d(state);
            }
            if ui.add(Button::selectable(state.tool == Tool::Section, t!("tool-section"))).on_hover_text("CT").clicked()
            {
                state.set_tool(Tool::Section);
                state.message = t!("msg-section-hint");
            }
            ui.menu_button(t!("view-elevations"), |ui| {
                let elevations: Vec<_> =
                    state.project().views_of(ViewCategory::Elevation).map(|v| (v.id, v.name.clone())).collect();
                for (id, name) in elevations {
                    if ui.button(name).clicked() {
                        state.open_view(id);
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui.button(t!("view-new-legend")).clicked() {
                actions::new_legend(state);
            }
            ui.menu_button(t!("view-new-schedule"), |ui| {
                for c in Category::ALL {
                    if ui.button(actions::t_category(c)).clicked() {
                        actions::new_schedule(state, c);
                        ui.close();
                    }
                }
            });
            if ui.button(t!("view-new-sheet")).clicked() {
                actions::new_sheet(state);
            }
            ui.separator();
            if ui.button(t!("action-zoom-fit")).on_hover_text("ZE").clicked() {
                crate::app::zoom_fit(state);
            }
        }
        RibbonTab::Manage => {
            if ui.button(t!("dlg-project-info")).clicked() {
                state.dialogs.project_info = true;
            }
            if ui.button(t!("dlg-levels")).clicked() {
                state.dialogs.levels = true;
            }
            if ui.button(t!("dlg-materials")).clicked() {
                state.dialogs.materials = true;
            }
        }
    });
}

/// Context options for the active tool (options bar).
pub fn options_bar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(crate::i18n::tr(state.tool.key())).strong());
        ui.separator();
        if let (Some(category), Some(type_id)) = (state.tool.category(), state.tool.type_id()) {
            let types: Vec<_> = state.project().types_of(category).map(|t| (t.id, t.name.clone())).collect();
            let mut selected = type_id;
            egui::ComboBox::from_id_salt("tool_type")
                .selected_text(state.project().type_name(type_id).to_string())
                .width(180.0)
                .show_ui(ui, |ui| {
                    for (id, name) in types {
                        ui.selectable_value(&mut selected, id, name);
                    }
                });
            if selected != type_id {
                state.tool = state.tool.with_type(selected);
            }
            if ui.small_button(t!("props-edit-type")).clicked() {
                state.dialogs.type_editor = Some(type_id);
            }
            ui.separator();
        }
        match state.tool.clone() {
            Tool::Select => {
                ui.label(RichText::new(t!("hint-select")).weak());
            }
            Tool::Wall { .. } => {
                ui.label(t!("prop-height"));
                ui.add(
                    DragValue::new(&mut state.wall_height).speed(0.01).range(0.1..=50.0).fixed_decimals(2).suffix(" m"),
                );
                ui.separator();
                ui.label(RichText::new(t!("hint-wall")).weak());
            }
            Tool::Hosted { .. } => {
                ui.checkbox(&mut state.interaction.flip_hand, t!("prop-flip-hand"));
                ui.label(RichText::new(t!("hint-hosted")).weak());
            }
            Tool::Sketch { category, type_id } => {
                let n = state.interaction.points.len();
                if ui.add_enabled(n >= 3, Button::new(t!("sketch-finish"))).clicked()
                    && let Some(level) = state.active_level()
                {
                    let pts = state.interaction.points.clone();
                    crate::tools::finish_sketch(state, level, category, type_id, pts);
                }
                if ui.button(t!("sketch-from-walls")).on_hover_text(t!("hint-from-walls")).clicked() {
                    match state.active_level() {
                        Some(level) => {
                            let outward = if category == Category::Ceiling { -1.0 } else { 1.0 };
                            match walls_loop(state.project(), level, outward) {
                                Some(pts) => crate::tools::finish_sketch(state, level, category, type_id, pts),
                                None => state.message = t!("msg-no-wall-loop"),
                            }
                        }
                        None => state.message = t!("msg-open-plan"),
                    }
                }
                if ui.add_enabled(n > 0, Button::new(t!("action-cancel"))).clicked() {
                    state.interaction.points.clear();
                }
                ui.label(RichText::new(t!("hint-sketch")).weak());
            }
            Tool::Section => {
                ui.label(RichText::new(t!("msg-section-hint")).weak());
            }
        }
    });
}

pub fn status_bar(ui: &mut Ui, state: &AppState) {
    ui.horizontal(|ui| {
        if let Some(c) = state.cursor {
            ui.monospace(format!("X {:>9}  Y {:>9}", format::number(c.x, 3), format::number(c.y, 3)));
            ui.separator();
        }
        if !state.length_input.is_empty() {
            ui.label(RichText::new(t!("status-length", "value" => state.length_input)).strong());
            ui.separator();
        }
        let n = state.selection.len();
        if n > 0 {
            ui.label(t!("status-selected", "count" => n));
            ui.separator();
        }
        ui.label(&state.message);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let file = state
                .doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| t!("status-unsaved-file"));
            let dirty = if state.doc.is_dirty() { " *" } else { "" };
            ui.label(RichText::new(format!("{file}{dirty}")).weak());
        });
    });
}
