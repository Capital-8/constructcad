//! Schedule (table) view, its definition dialog (fields, sorting, filter,
//! totals) and CSV export.

use crate::dialog::{Dialog, session};
use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use crate::theme::{self, PALETTE};
use cc_core::{Category, ScheduleDef, ScheduleField, ViewId, ViewKind};
use cc_views::Cell;
use egui::{Context, RichText, Ui};
use std::collections::BTreeSet;

fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Text(s) => s.clone(),
        Cell::Key(k) => tr(k),
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState, view_id: ViewId) {
    let Some(view) = state.project().view(view_id).cloned() else { return };
    let ViewKind::Schedule { category, def } = view.kind.clone() else { return };
    let table = cc_views::schedule(state.project(), state.session.geom(), category, &def);

    egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&view.name).family(theme::semibold()).size(17.0).color(PALETTE.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t!("schedule-export-csv")).clicked() {
                    export_csv(state, view_id, &view.name);
                }
                if ui.button(t!("schedule-fields")).clicked() {
                    state.dialogs.schedule_def = Some(view_id);
                }
            });
        });
        ui.label(RichText::new(t!("schedule-hint")).color(PALETTE.text_faint));
        ui.add_space(6.0);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(("schedule", view_id.0)).striped(true).min_col_width(70.0).spacing([16.0, 6.0]).show(
                ui,
                |ui| {
                    for h in &table.headers {
                        ui.label(RichText::new(tr(h)).family(theme::semibold()));
                    }
                    ui.end_row();
                    for row in &table.rows {
                        let selected = state.selection.contains(&row.element);
                        for (i, cell) in row.cells.iter().enumerate() {
                            let text = cell_text(cell);
                            let text = if text.is_empty() && i == 0 { "—".to_string() } else { text };
                            if ui.selectable_label(selected, text).clicked() {
                                state.selection = BTreeSet::from([row.element]);
                            }
                        }
                        ui.end_row();
                    }
                    for (i, cell) in table.totals.iter().enumerate() {
                        let text = if i == 0 { t!("schedule-total", "count" => cell) } else { cell.clone() };
                        ui.label(RichText::new(text).family(theme::semibold()));
                    }
                    ui.end_row();
                },
            );
        });
    });
}

fn export_csv(state: &mut AppState, view: ViewId, name: &str) {
    let dialog =
        rfd::FileDialog::new().add_filter(t!("file-filter-csv"), &["csv"]).set_file_name(format!("{name}.csv"));
    let Some(mut path) = dialog.save_file() else { return };
    if path.extension().is_none() {
        path.set_extension("csv");
    }
    let command = cc_engine::commands::io::ScheduleExportCsv { view: cc_engine::Ref::Id(view.0), path: path.clone() };
    match state.session.run(command) {
        Ok(_) => state.message = t!("msg-exported", "path" => path.display()),
        Err(e) => state.dialogs.error = Some(t!("msg-export-failed", "error" => e.message)),
    }
}

/// Edit the schedule definition of `view` through `f` (one undo step per change).
fn update(state: &mut AppState, view: ViewId, f: impl FnOnce(&mut ScheduleDef)) {
    state.session.doc.edit(t!("tx-modify-view"), |e| {
        if let Some(ViewKind::Schedule { def, .. }) = e.view_mut(view).map(|v| &mut v.kind) {
            f(def);
        }
    });
}

/// Fields, sorting, filter and totals of the schedule being edited.
pub fn dialog(ctx: &Context, state: &mut AppState) {
    let Some(view_id) = state.dialogs.schedule_def else { return };
    let Some(ViewKind::Schedule { category, def }) = state.project().view(view_id).map(|v| v.kind.clone()) else {
        state.dialogs.schedule_def = None;
        return;
    };
    session::begin(state);
    let mut ok = 0;
    let response = Dialog::new("schedule-def", t!("dlg-schedule")).width(560.0).show(
        ctx,
        |ui| body(ui, state, view_id, category, &def),
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    if crate::dialogs::ok_cancel(state, &response, ok) {
        state.dialogs.schedule_def = None;
    }
}

fn body(ui: &mut Ui, state: &mut AppState, view: ViewId, category: Category, def: &ScheduleDef) {
    let chosen = def.fields_for(category);
    let available: Vec<ScheduleField> =
        ScheduleField::available(category).into_iter().filter(|f| !chosen.contains(f)).collect();
    let set_fields = |state: &mut AppState, fields: Vec<ScheduleField>| update(state, view, |d| d.fields = fields);

    ui.columns(2, |cols| {
        cols[0].label(theme::muted(t!("schedule-available")));
        egui::Frame::new().stroke(egui::Stroke::new(1.0, PALETTE.border)).inner_margin(6).show(&mut cols[0], |ui| {
            ui.set_min_height(220.0);
            ui.set_width(ui.available_width());
            for f in &available {
                if ui.selectable_label(false, format!("+  {}", tr(f.key()))).on_hover_text(t!("schedule-add")).clicked()
                {
                    let mut fields = chosen.clone();
                    fields.push(*f);
                    set_fields(state, fields);
                }
            }
        });
        cols[1].label(theme::muted(t!("schedule-chosen")));
        egui::Frame::new().stroke(egui::Stroke::new(1.0, PALETTE.border)).inner_margin(6).show(&mut cols[1], |ui| {
            ui.set_min_height(220.0);
            ui.set_width(ui.available_width());
            let n = chosen.len();
            for (i, f) in chosen.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(tr(f.key()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if ui
                            .add_enabled(n > 1, egui::Button::new("×").small())
                            .on_hover_text(t!("schedule-remove"))
                            .clicked()
                        {
                            let mut fields = chosen.clone();
                            fields.remove(i);
                            set_fields(state, fields);
                        }
                        if ui.add_enabled(i + 1 < n, egui::Button::new("↓").small()).clicked() {
                            let mut fields = chosen.clone();
                            fields.swap(i, i + 1);
                            set_fields(state, fields);
                        }
                        if ui.add_enabled(i > 0, egui::Button::new("↑").small()).clicked() {
                            let mut fields = chosen.clone();
                            fields.swap(i, i - 1);
                            set_fields(state, fields);
                        }
                    });
                });
            }
        });
    });
    ui.add_space(8.0);

    let none = t!("schedule-none");
    let label = |f: Option<ScheduleField>| f.map_or(none.clone(), |f| tr(f.key()));
    egui::Grid::new("schedule-options").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
        ui.label(theme::muted(t!("schedule-sort")));
        ui.horizontal(|ui| {
            let mut sort = def.sort_by;
            egui::ComboBox::from_id_salt("schedule-sort").width(200.0).selected_text(label(sort)).show_ui(ui, |ui| {
                ui.selectable_value(&mut sort, None, none.clone());
                for f in &chosen {
                    ui.selectable_value(&mut sort, Some(*f), tr(f.key()));
                }
            });
            if sort != def.sort_by {
                update(state, view, |d| d.sort_by = sort);
            }
            let mut descending = def.descending;
            if ui.checkbox(&mut descending, t!("schedule-descending")).changed() {
                update(state, view, |d| d.descending = descending);
            }
        });
        ui.end_row();

        ui.label(theme::muted(t!("schedule-filter")));
        ui.horizontal(|ui| {
            let mut field = def.filter.as_ref().map(|(f, _)| *f);
            let mut text = def.filter.as_ref().map(|(_, s)| s.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("schedule-filter").width(200.0).selected_text(label(field)).show_ui(
                ui,
                |ui| {
                    ui.selectable_value(&mut field, None, none.clone());
                    for f in &chosen {
                        ui.selectable_value(&mut field, Some(*f), tr(f.key()));
                    }
                },
            );
            ui.label(theme::muted(t!("schedule-contains")));
            let r = ui.add_enabled(field.is_some(), egui::TextEdit::singleline(&mut text).desired_width(160.0));
            let new = field.map(|f| (f, text));
            if new != def.filter {
                state.session.doc.edit_merged(t!("tx-modify-view"), view.0 ^ 0x5c4e, |e| {
                    if let Some(ViewKind::Schedule { def, .. }) = e.view_mut(view).map(|v| &mut v.kind) {
                        def.filter = new;
                    }
                });
            }
            if r.lost_focus() {
                state.session.doc.break_merge();
            }
        });
        ui.end_row();

        ui.label("");
        let mut totals = !def.hide_totals;
        if ui.checkbox(&mut totals, t!("schedule-show-totals")).changed() {
            update(state, view, |d| d.hide_totals = !totals);
        }
        ui.end_row();
    });
}
