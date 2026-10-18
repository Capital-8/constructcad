//! Project Browser: the tree of views, schedules and sheets.

use crate::actions;
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::state::AppState;
use crate::t;
use crate::theme::{self, PALETTE};
use crate::tools::Tool;
use cc_core::{Category, ViewCategory, ViewId, ViewKind};
use egui::{Rect, Response, Sense, Ui, vec2};

const ROW_HEIGHT: f32 = 24.0;
const INDENT: f32 = 16.0;

pub fn category_icon(category: ViewCategory) -> Icon {
    match category {
        ViewCategory::FloorPlan => Icon::FloorPlan,
        ViewCategory::CeilingPlan => Icon::CeilingPlan,
        ViewCategory::ThreeD => Icon::View3D,
        ViewCategory::Elevation => Icon::Elevation,
        ViewCategory::Section => Icon::Section,
        ViewCategory::Legend => Icon::Legend,
        ViewCategory::Schedule => Icon::Schedule,
        ViewCategory::Sheet => Icon::Sheet,
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    theme::panel_header(ui, &t!("browser-title"), Some(&state.project().info.name));
    egui::ScrollArea::vertical().id_salt("browser").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(4.0);
        for category in ViewCategory::ALL {
            let id = ui.make_persistent_id(("browser-open", category));
            let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(id, !matches!(category, ViewCategory::Legend)));
            let count = state.project().views_of(category).count();
            let label = format!("{}  ", tr(category.key()));
            let row = tree_row(ui, 0, Some(open), category_icon(category), &label, Some(count), false);
            if add_button(ui, state, category, &row) {
                open = true;
            } else if row.clicked() {
                open = !open;
            }
            ui.data_mut(|d| d.insert_persisted(id, open));
            if open {
                category_items(ui, state, category);
            }
        }
        ui.add_space(6.0);
    });
}

/// One row of the tree. `expander` is `Some(open)` for collapsible rows.
fn tree_row(
    ui: &mut Ui,
    depth: usize,
    expander: Option<bool>,
    icon: Icon,
    label: &str,
    count: Option<usize>,
    selected: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let bg = rect.shrink2(vec2(4.0, 1.0));
    if selected {
        painter.rect_filled(bg, theme::RADIUS as f32, PALETTE.accent_soft);
    } else if response.hovered() {
        painter.rect_filled(bg, theme::RADIUS as f32, PALETTE.hover);
    }
    let mut x = rect.left() + 10.0 + depth as f32 * INDENT;
    let cy = rect.center().y;
    if let Some(open) = expander {
        let chevron = Rect::from_center_size(egui::pos2(x + 5.0, cy), vec2(10.0, 10.0));
        icons::paint(painter, chevron, if open { Icon::ChevronDown } else { Icon::ChevronRight }, PALETTE.text_faint);
        x += 15.0;
    }
    let color = if selected { PALETTE.accent_text } else { PALETTE.text };
    let icon_color = if selected { PALETTE.accent_text } else { PALETTE.text_muted };
    icons::paint(painter, Rect::from_center_size(egui::pos2(x + 7.0, cy), vec2(14.0, 14.0)), icon, icon_color);
    x += 21.0;
    let family = if expander.is_some() { theme::semibold() } else { egui::FontFamily::Proportional };
    let galley = painter.layout_no_wrap(label.to_string(), egui::FontId::new(13.0, family), color);
    let width = galley.size().x;
    painter.galley(egui::pos2(x, cy - galley.size().y / 2.0), galley, color);
    if let Some(n) = count {
        let font = egui::FontId::proportional(11.5);
        painter.text(egui::pos2(x + width, cy), egui::Align2::LEFT_CENTER, n.to_string(), font, PALETTE.text_faint);
    }
    response
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
        if state.rename.as_ref().is_some_and(|(rid, _)| *rid == id) {
            let finished = ui
                .horizontal(|ui| {
                    ui.add_space(10.0 + INDENT + 21.0);
                    let text = &mut state.rename.as_mut().unwrap().1;
                    let r = ui.add(egui::TextEdit::singleline(text).desired_width(ui.available_width() - 8.0));
                    r.request_focus();
                    r.lost_focus().then(|| !ui.input(|i| i.key_pressed(egui::Key::Escape)))
                })
                .inner;
            if let Some(accept) = finished {
                let (rid, text) = state.rename.take().unwrap();
                if accept {
                    actions::rename_view(state, rid, text);
                }
            }
            continue;
        }
        let active = state.active_view == Some(id);
        let r = tree_row(ui, 1, None, category_icon(category), &label, None, active);
        if r.clicked() {
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

/// "+" button at the right of a category row. Returns true when it was used.
fn add_button(ui: &mut Ui, state: &mut AppState, category: ViewCategory, row: &Response) -> bool {
    if category == ViewCategory::Elevation {
        return false;
    }
    let rect = Rect::from_center_size(egui::pos2(row.rect.right() - 18.0, row.rect.center().y), vec2(20.0, 20.0));
    let r = ui.interact(rect, row.id.with("add"), Sense::click()).on_hover_text(t!("browser-new"));
    if r.hovered() {
        ui.painter().rect_filled(rect, theme::RADIUS as f32, PALETTE.pressed);
    }
    if row.hovered() || r.hovered() || r.has_focus() {
        icons::paint(ui.painter(), rect.shrink(4.0), Icon::Plus, PALETTE.text_muted);
    }
    let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
    let mut used = false;
    match category {
        ViewCategory::FloorPlan | ViewCategory::CeilingPlan => {
            egui::Popup::menu(&r).show(|ui| {
                for (id, name) in levels {
                    if ui.button(name).clicked() {
                        actions::new_plan(state, id, category == ViewCategory::CeilingPlan);
                        used = true;
                    }
                }
            });
        }
        ViewCategory::Schedule => {
            egui::Popup::menu(&r).show(|ui| {
                for c in Category::ALL {
                    if ui.button(actions::t_category(c)).clicked() {
                        actions::new_schedule(state, c);
                        used = true;
                    }
                }
            });
        }
        _ if r.clicked() => {
            match category {
                ViewCategory::ThreeD => {
                    actions::new_3d(state);
                }
                ViewCategory::Section => {
                    state.set_tool(Tool::Section);
                    state.message = t!("msg-section-hint");
                }
                ViewCategory::Legend => {
                    actions::new_legend(state);
                }
                ViewCategory::Sheet => {
                    actions::new_sheet(state);
                }
                _ => {}
            }
            used = true;
        }
        _ => {}
    }
    used || r.clicked()
}
