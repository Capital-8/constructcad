//! Project Browser: the tree of views, schedules and sheets.

use crate::actions;
use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use crate::tools::Tool;
use cc_core::{Category, ViewCategory, ViewId, ViewKind};
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(t!("browser-title")).strong());
        ui.label(RichText::new(&state.project().info.name).weak());
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("browser").auto_shrink([false, false]).show(ui, |ui| {
        for category in ViewCategory::ALL {
            egui::CollapsingHeader::new(tr(category.key()))
                .id_salt(("browser", category))
                .default_open(!matches!(category, ViewCategory::Legend))
                .show(ui, |ui| {
                    category_items(ui, state, category);
                    add_button(ui, state, category);
                });
        }
    });
}

fn category_items(ui: &mut Ui, state: &mut AppState, category: ViewCategory) {
    let mut views: Vec<(ViewId, String)> = state
        .project()
        .views_of(category)
        .map(|v| {
            let label = match &v.kind {
                ViewKind::Sheet { number, .. } => format!("{number} - {}", v.name),
                _ => v.name.clone(),
            };
            (v.id, label)
        })
        .collect();
    if matches!(category, ViewCategory::FloorPlan | ViewCategory::CeilingPlan) {
        // Order plans by level elevation like the levels themselves.
        let project = state.project();
        views.sort_by(|a, b| {
            let elev =
                |id: ViewId| project.view(id).and_then(|v| v.level()).map_or(0.0, |l| project.level_elevation(l));
            elev(a.0).total_cmp(&elev(b.0)).then(a.1.cmp(&b.1))
        });
    } else {
        views.sort_by(|a, b| a.1.cmp(&b.1));
    }

    for (id, label) in views {
        if let Some((rid, text)) = &mut state.rename
            && *rid == id
        {
            let r = ui.text_edit_singleline(text);
            r.request_focus();
            if r.lost_focus() {
                let (rid, text) = state.rename.take().unwrap();
                if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    actions::rename_view(state, rid, text);
                }
            }
            continue;
        }
        let active = state.active_view == Some(id);
        let r = ui.selectable_label(active, &label);
        if r.double_clicked() || r.clicked() {
            state.open_view(id);
        }
        r.context_menu(|ui| {
            if ui.button(t!("browser-open")).clicked() {
                state.open_view(id);
                ui.close();
            }
            if ui.button(t!("browser-rename")).clicked() {
                let name = state.project().view(id).map(|v| v.name.clone()).unwrap_or_default();
                state.rename = Some((id, name));
                ui.close();
            }
            if ui.button(t!("browser-duplicate")).clicked() {
                actions::duplicate_view(state, id);
                ui.close();
            }
            if let Some(sheet) = state.active_view.filter(|s| {
                matches!(state.project().view(*s).map(|v| &v.kind), Some(ViewKind::Sheet { .. })) && *s != id
            }) {
                let placeable = state.project().view(id).is_some_and(|v| v.placeable());
                if placeable && ui.button(t!("browser-place-on-sheet")).clicked() {
                    actions::place_on_sheet(state, sheet, id);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button(t!("browser-delete")).clicked() {
                actions::delete_view(state, id);
                ui.close();
            }
        });
    }
}

fn add_button(ui: &mut Ui, state: &mut AppState, category: ViewCategory) {
    let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
    match category {
        ViewCategory::FloorPlan | ViewCategory::CeilingPlan => {
            ui.menu_button(RichText::new(t!("browser-new")).small(), |ui| {
                for (id, name) in levels {
                    if ui.button(name).clicked() {
                        actions::new_plan(state, id, category == ViewCategory::CeilingPlan);
                        ui.close();
                    }
                }
            });
        }
        ViewCategory::ThreeD => {
            if ui.small_button(t!("browser-new")).clicked() {
                actions::new_3d(state);
            }
        }
        ViewCategory::Section => {
            if ui.small_button(t!("browser-new")).clicked() {
                state.set_tool(Tool::Section);
                state.message = t!("msg-section-hint");
            }
        }
        ViewCategory::Legend => {
            if ui.small_button(t!("browser-new")).clicked() {
                actions::new_legend(state);
            }
        }
        ViewCategory::Schedule => {
            ui.menu_button(RichText::new(t!("browser-new")).small(), |ui| {
                for c in Category::ALL {
                    if ui.button(actions::t_category(c)).clicked() {
                        actions::new_schedule(state, c);
                        ui.close();
                    }
                }
            });
        }
        ViewCategory::Sheet => {
            if ui.small_button(t!("browser-new")).clicked() {
                actions::new_sheet(state);
            }
        }
        ViewCategory::Elevation => {}
    }
}
