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
    if !matches!(view.kind, ViewKind::Schedule { .. }) {
        return;
    }
    // The elements the schedule's phase and phase filter show.
    let table = state.session.schedule(view_id).unwrap_or_default();

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
        let widths = column_widths(ui, state.session.doc.revision(), view_id, &table);
        let row_height = ui.spacing().interact_size.y.max(ui.text_style_height(&egui::TextStyle::Body));
        // The header stays on top; only the rows in sight are laid out, so long schedules stay cheap.
        egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(COLUMN_GAP, ROW_GAP);
            row_cells(ui, &widths, row_height, None, |ui, i| {
                let h = table.headers.get(i).map(|h| tr(h)).unwrap_or_default();
                ui.label(RichText::new(h).family(theme::semibold()));
            });
            // The rows, then the totals as one more row.
            let total = table.rows.len() + 1;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_height, total, |ui, range| {
                for r in range {
                    let stripe = (r % 2 == 1).then(|| ui.visuals().faint_bg_color);
                    let Some(row) = table.rows.get(r) else {
                        row_cells(ui, &widths, row_height, stripe, |ui, i| {
                            let Some(cell) = table.totals.get(i) else { return };
                            let text = if i == 0 { t!("schedule-total", "count" => cell) } else { cell.clone() };
                            ui.label(RichText::new(text).family(theme::semibold()));
                        });
                        continue;
                    };
                    let selected = state.selection.contains(&row.element);
                    let mut clicked = false;
                    row_cells(ui, &widths, row_height, stripe, |ui, i| {
                        let Some(cell) = row.cells.get(i) else { return };
                        let text = cell_text(cell);
                        let text = if text.is_empty() && i == 0 { "—".to_string() } else { text };
                        clicked |= ui.selectable_label(selected, text).clicked();
                    });
                    if clicked {
                        state.selection = BTreeSet::from([row.element]);
                    }
                }
            });
        });
    });
}

/// Space between columns and between rows of the table.
const COLUMN_GAP: f32 = 16.0;
const ROW_GAP: f32 = 6.0;
const MIN_COLUMN: f32 = 70.0;

/// The column widths of the schedule shown: the widest of each column's header, cells and total.
/// Measuring every cell is the costly part, so the widths are kept until the document, the language
/// or the text size changes.
fn column_widths(ui: &Ui, revision: u64, view: ViewId, table: &cc_views::Table) -> Vec<f32> {
    #[derive(Clone, PartialEq)]
    struct Key(u64, crate::i18n::Language, u32, usize);
    let body = ui.text_style_height(&egui::TextStyle::Body);
    let key = Key(revision, crate::i18n::language(), body.to_bits(), table.headers.len());
    let id = egui::Id::new(("schedule-widths", view.0));
    if let Some((k, widths)) = ui.ctx().data(|d| d.get_temp::<(Key, Vec<f32>)>(id))
        && k == key
    {
        return widths;
    }
    let painter = ui.painter();
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    let bold_font = egui::FontId::new(body_font.size, theme::semibold());
    let measure = |text: String, font: &egui::FontId| painter.layout_no_wrap(text, font.clone(), PALETTE.text).size().x;
    // A selectable label pads its text on both sides.
    let pad = 2.0 * ui.spacing().button_padding.x;
    let mut widths: Vec<f32> = table.headers.iter().map(|h| measure(tr(h), &bold_font).max(MIN_COLUMN)).collect();
    for row in &table.rows {
        for (i, cell) in row.cells.iter().enumerate() {
            let w = measure(cell_text(cell), &body_font) + pad;
            if let Some(slot) = widths.get_mut(i) {
                *slot = slot.max(w);
            } else {
                widths.push(w.max(MIN_COLUMN));
            }
        }
    }
    for (i, cell) in table.totals.iter().enumerate() {
        let text = if i == 0 { t!("schedule-total", "count" => cell) } else { cell.clone() };
        let w = measure(text, &bold_font);
        if let Some(slot) = widths.get_mut(i) {
            *slot = slot.max(w);
        } else {
            widths.push(w.max(MIN_COLUMN));
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, (key, widths.clone())));
    widths
}

/// One table row: each column at its width, left aligned, over an optional stripe.
fn row_cells(
    ui: &mut Ui,
    widths: &[f32],
    height: f32,
    stripe: Option<egui::Color32>,
    mut cell: impl FnMut(&mut Ui, usize),
) {
    let gap = ui.spacing().item_spacing.x;
    let width = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    if let Some(color) = stripe {
        ui.painter().rect_filled(rect.expand2(egui::vec2(gap / 2.0, ROW_GAP / 2.0)), 0.0, color);
    }
    let mut x = rect.left();
    for (i, w) in widths.iter().enumerate() {
        let cell_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(*w, height));
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(cell_rect).layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| cell(ui, i),
        );
        x += w + gap;
    }
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
    crate::engine_edit::view(state, view, None, |x| {
        if let ViewKind::Schedule { def, .. } = &mut x.kind {
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
                if ui
                    .selectable_label(false, format!("+  {}", crate::properties::field_label(*f)))
                    .on_hover_text(t!("schedule-add"))
                    .clicked()
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
                    ui.label(crate::properties::field_label(*f));
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
    let label = |f: Option<ScheduleField>| f.map_or(none.clone(), crate::properties::field_label);
    egui::Grid::new("schedule-options").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
        ui.label(theme::muted(t!("schedule-sort")));
        ui.horizontal(|ui| {
            let mut sort = def.sort_by;
            egui::ComboBox::from_id_salt("schedule-sort").width(200.0).selected_text(label(sort)).show_ui(ui, |ui| {
                ui.selectable_value(&mut sort, None, none.clone());
                for f in &chosen {
                    ui.selectable_value(&mut sort, Some(*f), crate::properties::field_label(*f));
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
                        ui.selectable_value(&mut field, Some(*f), crate::properties::field_label(*f));
                    }
                },
            );
            ui.label(theme::muted(t!("schedule-contains")));
            let enabled = field.is_some();
            let r = crate::table::live_text(ui, ("schedule-filter", view), &mut text, |ui, t| {
                ui.add_enabled(enabled, egui::TextEdit::singleline(t).desired_width(160.0))
            });
            let new = field.map(|f| (f, text));
            if new != def.filter {
                crate::engine_edit::view(state, view, Some(view.0 ^ 0x5c4e), |x| {
                    if let ViewKind::Schedule { def, .. } = &mut x.kind {
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
