//! Schedule (table) view.

use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use cc_core::{ViewId, ViewKind};
use egui::{RichText, Ui};
use std::collections::BTreeSet;

pub fn show(ui: &mut Ui, state: &mut AppState, view_id: ViewId) {
    let Some(view) = state.project().view(view_id).cloned() else { return };
    let ViewKind::Schedule { category } = view.kind else { return };
    let table = cc_views::schedule(state.project(), &state.geom, category);

    ui.add_space(6.0);
    ui.heading(&view.name);
    ui.label(RichText::new(t!("schedule-hint")).weak());
    ui.add_space(6.0);
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new(("schedule", view_id.0)).striped(true).min_col_width(70.0).spacing([16.0, 6.0]).show(
            ui,
            |ui| {
                for h in &table.headers {
                    ui.label(RichText::new(tr(h)).strong());
                }
                ui.end_row();
                for row in &table.rows {
                    let selected = state.selection.contains(&row.element);
                    for (i, cell) in row.cells.iter().enumerate() {
                        let text = if cell.is_empty() && i == 0 { "—".to_string() } else { cell.clone() };
                        let r = ui.selectable_label(selected, text);
                        if r.clicked() {
                            state.selection = BTreeSet::from([row.element]);
                        }
                    }
                    ui.end_row();
                }
                for (i, cell) in table.totals.iter().enumerate() {
                    let text = if i == 0 { t!("schedule-total", "count" => cell) } else { cell.clone() };
                    ui.label(RichText::new(text).strong());
                }
                ui.end_row();
            },
        );
    });
}
