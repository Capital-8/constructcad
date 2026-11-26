//! Construction phases in the application: the Phases dialog (phases,
//! phase filters and the graphics of each status), the phase rows of view
//! and element properties, and the Demolish tool. Every change goes through
//! the engine's commands; OK keeps the dialog's changes, Cancel undoes them.

use crate::graphics_dialog::{Edits, header, key, override_cells, override_headers, overrides_json, run};
use crate::i18n::tr;
use crate::icons::Icon;
use crate::settings;
use crate::state::AppState;
use crate::t;
use crate::table::{readonly_row, row, section};
use crate::theme::PALETTE;
use cc_core::{Element, ElementId, PhaseDisplay, PhaseFilterId, PhaseId, PhaseStatus, ViewId};
use cc_engine::Ref;
use cc_engine::commands::phases as cmd;
use egui::{Context, RichText, Ui};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Phases,
    Filters,
    Graphics,
}

/// A text being typed, sent when the field loses focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Draft {
    PhaseName(PhaseId),
    PhaseDescription(PhaseId),
    FilterName(PhaseFilterId),
}

/// The open Phases dialog.
pub struct PhasesDialog {
    tab: Tab,
    selected: Option<PhaseId>,
    /// Where the selected phase's elements and views go when it is deleted.
    merge_into: Option<PhaseId>,
    drafts: BTreeMap<Draft, String>,
    edits: Edits,
}

pub fn open(state: &mut AppState) {
    let selected = state.project().phases.last().map(|p| p.id);
    let edits = Edits::begin(state);
    state.dialogs.phases =
        Some(PhasesDialog { tab: Tab::Phases, selected, merge_into: None, drafts: BTreeMap::new(), edits });
}

/// A text field holding a draft while focused; returns the text to commit
/// when it loses focus with a change.
fn draft_field(
    ui: &mut Ui,
    drafts: &mut BTreeMap<Draft, String>,
    which: Draft,
    current: &str,
    width: f32,
) -> Option<String> {
    let mut text = drafts.get(&which).cloned().unwrap_or_else(|| current.to_string());
    settings::framed(ui);
    let r = ui.add(settings::text_edit(&mut text, width));
    if r.changed() {
        drafts.insert(which, text);
    }
    if r.lost_focus() {
        return drafts.remove(&which).filter(|t| t.trim() != current.trim());
    }
    None
}

fn display_label(d: PhaseDisplay) -> String {
    tr(d.key())
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(d) = state.dialogs.phases.as_mut() else { return };
    let (mut tab, mut selected, mut merge_into) = (d.tab, d.selected, d.merge_into);
    let mut drafts = std::mem::take(&mut d.drafts);
    selected = selected.filter(|id| state.project().phase(*id).is_some());
    let mut ok = 0;
    let response = crate::dialog::Dialog::new("phases", t!("dlg-phases")).width(940.0).show(
        ctx,
        |ui| {
            let tabs = [
                (Tab::Phases, t!("ph-tab-phases")),
                (Tab::Filters, t!("ph-tab-filters")),
                (Tab::Graphics, t!("ph-tab-graphics")),
            ];
            settings::tabs(ui, &mut tab, &tabs);
            egui::ScrollArea::vertical().max_height(460.0).auto_shrink([false, true]).show(ui, |ui| match tab {
                Tab::Phases => phases_tab(ui, state, &mut selected, &mut merge_into, &mut drafts),
                Tab::Filters => filters_tab(ui, state, &mut drafts),
                Tab::Graphics => graphics_tab(ui, state),
            });
        },
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    if let Some(d) = state.dialogs.phases.as_mut() {
        (d.tab, d.selected, d.merge_into, d.drafts) = (tab, selected, merge_into, drafts);
    }
    let close = match response.clicked {
        Some(i) => Some(i == ok),
        None if response.dismissed => Some(false),
        None => None,
    };
    if let Some(keep) = close
        && let Some(d) = state.dialogs.phases.take()
    {
        d.edits.finish(state, keep);
    }
}

fn phases_tab(
    ui: &mut Ui,
    state: &mut AppState,
    selected: &mut Option<PhaseId>,
    merge_into: &mut Option<PhaseId>,
    drafts: &mut BTreeMap<Draft, String>,
) {
    ui.label(RichText::new(t!("ph-phases-hint")).size(12.0).color(PALETTE.text_muted));
    ui.add_space(10.0);
    let phases = state.project().phases.clone();
    let counts: Vec<(usize, usize)> = {
        let p = state.project();
        let phased = || p.elements.values().filter(|e| e.category().is_phased());
        (0..phases.len())
            .map(|i| {
                let built = phased().filter(|e| p.created_index(e) == i).count();
                let razed = phased().filter(|e| p.demolished_index(e) == Some(i)).count();
                (built, razed)
            })
            .collect()
    };
    timeline(ui, &phases, selected);
    ui.add_space(16.0);
    let index = selected.and_then(|id| phases.iter().position(|p| p.id == id));
    let new_name = || {
        let names: Vec<String> = phases.iter().map(|p| p.name.to_lowercase()).collect();
        (1..1000)
            .map(|n| t!("ph-default-name", "n" => n))
            .find(|n| !names.contains(&n.to_lowercase()))
            .unwrap_or_default()
    };
    settings::list_title(ui, &t!("ph-tab-phases"), Some(phases.len()), |ui| {
        let insert = settings::button(ui, index.is_some(), &t!("ph-insert")).on_hover_text(t!("ph-insert-tip"));
        if insert.clicked()
            && let Some(id) = *selected
        {
            let command = cmd::PhaseCreate { name: new_name(), description: None, before: Some(Ref::Id(id.0)) };
            if let Some(out) = crate::actions::run_command(state, command) {
                *selected = out["id"].as_u64().map(PhaseId);
            }
        }
        if settings::button(ui, true, &t!("ph-add")).on_hover_text(t!("ph-add-tip")).clicked() {
            let command = cmd::PhaseCreate { name: new_name(), description: None, before: None };
            if let Some(out) = crate::actions::run_command(state, command) {
                *selected = out["id"].as_u64().map(PhaseId);
            }
        }
    });
    let w = [28.0, 190.0, 0.0, 96.0, 96.0];
    settings::card(ui, |ui| {
        let flexible = (ui.available_width()
            - 2.0 * settings::ROW_PAD_X
            - w.iter().sum::<f32>()
            - 64.0
            - 6.0 * settings::CELL_GAP)
            .max(120.0);
        let widths = [w[0], w[1], flexible, w[3], w[4]];
        let mut x = 0.0;
        let names = ["#".to_string(), t!("prop-name"), t!("ph-description"), t!("ph-built"), t!("ph-demolished")];
        let columns: Vec<(f32, String)> = names
            .into_iter()
            .zip(widths)
            .map(|(n, width)| {
                let at = x;
                x += width + settings::CELL_GAP;
                (at, n)
            })
            .collect();
        settings::card_header(ui, &columns);
        let count = phases.len();
        for (i, ph) in phases.iter().enumerate() {
            let is_selected = *selected == Some(ph.id);
            let (_, row) = settings::card_row(ui, ph.id.0, 48.0, is_selected, i + 1 == count, |ui| {
                settings::cell(ui, widths[0], |ui| number_dot(ui, i + 1, is_selected));
                settings::cell(ui, widths[1], |ui| {
                    if let Some(name) = draft_field(ui, drafts, Draft::PhaseName(ph.id), &ph.name, widths[1]) {
                        let command = cmd::PhaseSet { phase: Ref::Id(ph.id.0), name: Some(name), ..Default::default() };
                        run(state, None, command);
                    }
                });
                settings::cell(ui, widths[2], |ui| {
                    let current = &ph.description;
                    if let Some(text) = draft_field(ui, drafts, Draft::PhaseDescription(ph.id), current, widths[2]) {
                        let command =
                            cmd::PhaseSet { phase: Ref::Id(ph.id.0), description: Some(text), ..Default::default() };
                        run(state, None, command);
                    }
                });
                let (built, razed) = counts.get(i).copied().unwrap_or_default();
                settings::cell(ui, widths[3], |ui| {
                    let color = if built > 0 { PALETTE.success } else { PALETTE.text_faint };
                    settings::badge(ui, &built.to_string(), color);
                });
                settings::cell(ui, widths[4], |ui| {
                    let color = if razed > 0 { PALETTE.danger } else { PALETTE.text_faint };
                    settings::badge(ui, &razed.to_string(), color);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let later = format!("{}: {}", t!("ph-later"), ph.name);
                    if settings::icon_button(ui, Icon::ChevronDown, &later, i + 1 < count, false).clicked() {
                        run(
                            state,
                            None,
                            cmd::PhaseSet { phase: Ref::Id(ph.id.0), index: Some(i + 1), ..Default::default() },
                        );
                        *selected = Some(ph.id);
                    }
                    let earlier = format!("{}: {}", t!("ph-earlier"), ph.name);
                    if settings::icon_button(ui, Icon::ChevronUp, &earlier, i > 0, false).clicked() {
                        run(
                            state,
                            None,
                            cmd::PhaseSet { phase: Ref::Id(ph.id.0), index: Some(i - 1), ..Default::default() },
                        );
                        *selected = Some(ph.id);
                    }
                });
            });
            if row.clicked() {
                *selected = Some(ph.id);
            }
        }
    });
    // Deleting: a phase in use hands its elements and views to another.
    let Some((i, ph)) = index.and_then(|i| phases.get(i).map(|p| (i, p.clone()))) else { return };
    let others: Vec<(PhaseId, String)> =
        phases.iter().filter(|p| p.id != ph.id).map(|p| (p.id, p.name.clone())).collect();
    if merge_into.is_none_or(|m| !others.iter().any(|(id, _)| *id == m)) {
        // The previous phase by default, else the next.
        *merge_into = i.checked_sub(1).and_then(|j| phases.get(j)).or(phases.get(i + 1)).map(|p| p.id);
    }
    ui.add_space(14.0);
    egui::Frame::new()
        .fill(PALETTE.danger.gamma_multiply(0.06))
        .stroke(egui::Stroke::new(1.0, PALETTE.danger.gamma_multiply(0.25)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(t!("ph-merge-into"));
                if let Some(mut target) = *merge_into {
                    settings::combo_items(ui, "ph-merge", 180.0, &mut target, &others);
                    *merge_into = Some(target);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = t!("ph-delete", "name" => ph.name.clone());
                    let r = ui.add_enabled_ui(!others.is_empty(), |ui| {
                        crate::dialog::styled_button(ui, &label, crate::dialog::Style::Danger)
                    });
                    if r.inner.clicked() {
                        let command =
                            cmd::PhaseDelete { phase: Ref::Id(ph.id.0), merge_into: merge_into.map(|m| Ref::Id(m.0)) };
                        if run(state, None, command) {
                            *selected = *merge_into;
                        }
                    }
                });
            });
        });
}

/// The phases from the oldest to the newest as numbered stops on a line;
/// a click on one picks it.
fn timeline(ui: &mut Ui, phases: &[cc_core::Phase], selected: &mut Option<PhaseId>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 74.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 8.0, PALETTE.surface);
    let n = phases.len();
    if n == 0 {
        return;
    }
    let step = rect.width() / n as f32;
    let y = rect.top() + 26.0;
    let x = |i: usize| rect.left() + step * (i as f32 + 0.5);
    if n > 1 {
        ui.painter().line_segment(
            [egui::pos2(x(0), y), egui::pos2(x(n - 1), y)],
            egui::Stroke::new(2.0, PALETTE.border_strong),
        );
    }
    for (i, ph) in phases.iter().enumerate() {
        let center = egui::pos2(x(i), y);
        let hit = egui::Rect::from_center_size(egui::pos2(center.x, rect.center().y), egui::vec2(step, rect.height()));
        let r = ui.interact(hit, ui.id().with(("phase-stop", ph.id.0)), egui::Sense::click());
        if r.clicked() {
            *selected = Some(ph.id);
        }
        let on = *selected == Some(ph.id);
        let (fill, text) = if on {
            (PALETTE.accent, egui::Color32::WHITE)
        } else if r.hovered() {
            (PALETTE.hover, PALETTE.text)
        } else {
            (PALETTE.canvas, PALETTE.text_muted)
        };
        let stroke = egui::Stroke::new(2.0, if on { PALETTE.accent } else { PALETTE.border_strong });
        ui.painter().circle(center, 12.0, fill, stroke);
        let font = egui::FontId::new(12.0, crate::theme::semibold());
        ui.painter().text(center, egui::Align2::CENTER_CENTER, (i + 1).to_string(), font, text);
        let name_font =
            if on { egui::FontId::new(12.0, crate::theme::semibold()) } else { egui::FontId::proportional(12.0) };
        let galley = ui.painter().layout(ph.name.clone(), name_font, PALETTE.text, step - 8.0);
        let at = egui::pos2(center.x - galley.size().x / 2.0, y + 18.0);
        ui.painter().galley(at, galley, if on { PALETTE.accent_text } else { PALETTE.text });
        r.on_hover_text(&ph.name);
    }
}

/// A phase number in a circle, filled when its phase is picked.
fn number_dot(ui: &mut Ui, n: usize, on: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
    let (fill, text) = if on { (PALETTE.accent, egui::Color32::WHITE) } else { (PALETTE.surface, PALETTE.text_muted) };
    ui.painter().circle_filled(rect.center(), 11.0, fill);
    let font = egui::FontId::new(11.5, crate::theme::semibold());
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, n.to_string(), font, text);
}

fn filters_tab(ui: &mut Ui, state: &mut AppState, drafts: &mut BTreeMap<Draft, String>) {
    ui.label(RichText::new(t!("ph-filters-hint")).size(12.0).color(PALETTE.text_muted));
    ui.add_space(12.0);
    let filters: Vec<cc_core::PhaseFilter> = state.project().phase_filters.values().cloned().collect();
    settings::list_title(ui, &t!("ph-tab-filters"), Some(filters.len()), |ui| {
        if settings::button(ui, true, &t!("ph-new-filter")).clicked() {
            let names: Vec<String> = state.project().phase_filters.values().map(|f| f.name.to_lowercase()).collect();
            let name = (1..1000)
                .map(|n| t!("ph-default-filter-name", "n" => n))
                .find(|n| !names.contains(&n.to_lowercase()))
                .unwrap_or_default();
            run(state, None, cmd::PhaseFilterCreate { name, ..Default::default() });
        }
    });
    let (name_w, status_w) = (200.0, 140.0);
    settings::card(ui, |ui| {
        let mut columns = vec![(0.0, t!("prop-name"))];
        for (i, st) in PhaseStatus::ALL.iter().enumerate() {
            columns.push((name_w + settings::CELL_GAP + i as f32 * (status_w + settings::CELL_GAP), tr(st.key())));
        }
        settings::card_header(ui, &columns);
        let count = filters.len();
        for (i, f) in filters.into_iter().enumerate() {
            settings::card_row(ui, ("phase-filter", f.id.0), 46.0, false, i + 1 == count, |ui| {
                settings::cell(ui, name_w, |ui| {
                    if let Some(name) = draft_field(ui, drafts, Draft::FilterName(f.id), &f.name, name_w) {
                        let command =
                            cmd::PhaseFilterSet { filter: Ref::Id(f.id.0), name: Some(name), ..Default::default() };
                        run(state, None, command);
                    }
                });
                let mut set = cmd::PhaseFilterSet { filter: Ref::Id(f.id.0), ..Default::default() };
                for st in PhaseStatus::ALL {
                    settings::cell(ui, status_w, |ui| {
                        let mut d = f.display(st);
                        settings::combo(
                            ui,
                            ("phase-filter", f.id.0, st),
                            status_w - 8.0,
                            &mut d,
                            PhaseDisplay::ALL,
                            display_label,
                        );
                        if d != f.display(st) {
                            let value = Some(cmd::display_name(d).to_string());
                            match st {
                                PhaseStatus::Existing => set.existing = value,
                                PhaseStatus::Demolished => set.demolished = value,
                                PhaseStatus::New => set.new = value,
                                PhaseStatus::Temporary => set.temporary = value,
                            }
                        }
                    });
                }
                if set.existing.is_some() || set.demolished.is_some() || set.new.is_some() || set.temporary.is_some() {
                    run(state, None, set);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = format!("{}: {}", t!("ph-delete-filter"), f.name);
                    if settings::icon_button(ui, Icon::Delete, &label, true, true).clicked() {
                        run(state, None, cmd::PhaseFilterDelete { filter: Ref::Id(f.id.0) });
                    }
                });
            });
        }
    });
}

fn graphics_tab(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new(t!("ph-graphics-hint")).size(12.0).color(PALETTE.text_muted));
    ui.add_space(12.0);
    let graphics = state.project().phase_graphics;
    let choices = crate::fill_patterns_dialog::choices(state.project());
    settings::card(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            egui::Grid::new("phase-graphics").num_columns(8).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                header(ui, t!("ph-status"));
                override_headers(ui);
                ui.label("");
                ui.end_row();
                for st in PhaseStatus::ALL {
                    let mut g = *graphics.get(st);
                    ui.label(tr(st.key()));
                    let mut changed = override_cells(ui, &mut g, true, &choices);
                    if ui.add_enabled(!g.is_empty(), egui::Button::new(t!("vg-reset")).small()).clicked() {
                        g = cc_core::GraphicOverride::default();
                        changed = true;
                    }
                    ui.end_row();
                    if changed {
                        let command = cmd::PhaseGraphicsSet {
                            status: cmd::status_name(st).to_string(),
                            overrides: Some(overrides_json(&g)),
                            reset: true,
                        };
                        run(state, Some(key(("phase-graphics", st))), command);
                    }
                }
            });
        });
    });
}

// ---------------------------------------------------------------- properties

/// The Phasing section of a view's properties (plans, elevations, sections,
/// 3D views and schedules).
pub fn view_section(ui: &mut Ui, state: &mut AppState, view: ViewId) {
    let phased = state.project().view(view).is_some_and(|v| v.kind.is_phased());
    if phased && !state.project().phases.is_empty() {
        section(ui, "view-phasing", t!("section-phasing"), |ui| view_rows(ui, state, view));
    }
}

/// A view's phase and phase filter; the filter is read-only when the view's
/// template controls it.
fn view_rows(ui: &mut Ui, state: &mut AppState, view: ViewId) {
    let Some(v) = state.project().view(view).cloned() else { return };
    let p = state.project();
    let phases: Vec<(PhaseId, String)> = p.phases.iter().map(|ph| (ph.id, ph.name.clone())).collect();
    let filters: Vec<(PhaseFilterId, String)> = p.phase_filters.values().map(|f| (f.id, f.name.clone())).collect();
    let current = p.view_phase(&v);
    let locked = v.template.and_then(|t| p.template(t)).filter(|t| t.include.phase_filter).map(|t| t.name.clone());
    let mut phase = current;
    row(ui, &t!("prop-phase"), |ui| {
        let name = |id: Option<PhaseId>| id.and_then(|i| phases.iter().find(|(p, _)| *p == i)).map(|(_, n)| n.clone());
        egui::ComboBox::from_id_salt(("view-phase", view.0)).selected_text(name(phase).unwrap_or_default()).show_ui(
            ui,
            |ui| {
                for (id, n) in &phases {
                    ui.selectable_value(&mut phase, Some(*id), n);
                }
            },
        );
    });
    if phase != current
        && let Some(ph) = phase
    {
        crate::engine_edit::view(state, view, None, |x| x.phase = ph);
    }
    let filter_name = |id: Option<PhaseFilterId>| {
        id.and_then(|i| filters.iter().find(|(f, _)| *f == i)).map_or(t!("ph-filter-none"), |(_, n)| n.clone())
    };
    match locked {
        Some(_) => readonly_row(ui, &t!("prop-phase-filter"), filter_name(v.phase_filter)),
        None => {
            let mut chosen = v.phase_filter;
            row(ui, &t!("prop-phase-filter"), |ui| {
                egui::ComboBox::from_id_salt(("view-phase-filter", view.0)).selected_text(filter_name(chosen)).show_ui(
                    ui,
                    |ui| {
                        ui.selectable_value(&mut chosen, None, t!("ph-filter-none"));
                        for (id, n) in &filters {
                            ui.selectable_value(&mut chosen, Some(*id), n);
                        }
                    },
                );
            });
            if chosen != v.phase_filter {
                crate::engine_edit::view(state, view, None, |x| x.phase_filter = chosen);
            }
        }
    }
}

/// The phases an element is built and demolished in.
pub fn element_rows(ui: &mut Ui, state: &mut AppState, el: &Element) {
    let p = state.project();
    if !el.category().is_phased() || p.phases.is_empty() {
        return;
    }
    let phases: Vec<(PhaseId, String)> = p.phases.iter().map(|ph| (ph.id, ph.name.clone())).collect();
    let created_index = p.created_index(el);
    let created = phases.get(created_index).map(|(id, _)| *id);
    let demolished = el.phase_demolished;
    let id = el.id;
    section(ui, "phasing", t!("section-phasing"), |ui| {
        let name = |x: Option<PhaseId>| {
            x.and_then(|i| phases.iter().find(|(p, _)| *p == i)).map_or(t!("ph-not-demolished"), |(_, n)| n.clone())
        };
        let mut c = created;
        row(ui, &t!("col-phase-created"), |ui| {
            egui::ComboBox::from_id_salt(("phase-created", id.0)).selected_text(name(c)).show_ui(ui, |ui| {
                for (pid, n) in &phases {
                    ui.selectable_value(&mut c, Some(*pid), n);
                }
            });
        });
        let mut d = demolished;
        row(ui, &t!("col-phase-demolished"), |ui| {
            egui::ComboBox::from_id_salt(("phase-demolished", id.0)).selected_text(name(d)).show_ui(ui, |ui| {
                ui.selectable_value(&mut d, None, t!("ph-not-demolished"));
                // Not before it is built.
                for (pid, n) in phases.iter().skip(created_index) {
                    ui.selectable_value(&mut d, Some(*pid), n);
                }
            });
        });
        if c != created || d != demolished {
            let command = cmd::ElementsPhaseSet {
                ids: vec![id.0],
                created: (c != created).then(|| c.map(|x| Ref::Id(x.0))).flatten(),
                demolished: (d != demolished).then(|| d.map(|x| Ref::Id(x.0))),
            };
            crate::actions::run_command(state, command);
        }
    });
}

// ---------------------------------------------------------------- Demolish tool

/// The elements of `ids` standing in the view's phase (existing or new
/// there), which the view can demolish.
pub fn demolishable(state: &AppState, view: ViewId, ids: impl IntoIterator<Item = ElementId>) -> Vec<ElementId> {
    let p = state.project();
    let Some(phase) = p.view(view).and_then(|v| p.view_phase(v)) else { return Vec::new() };
    ids.into_iter()
        .filter(|id| {
            p.element(*id).is_some_and(|e| {
                matches!(p.phase_state(e, phase), cc_core::PhaseState::Status(PhaseStatus::Existing | PhaseStatus::New))
            })
        })
        .collect()
}

/// Demolish `ids` in the view's phase; returns whether it was done.
pub fn demolish(state: &mut AppState, view: ViewId, ids: &[ElementId]) -> bool {
    if ids.is_empty() {
        state.message = t!("hint-demolish");
        return false;
    }
    let command =
        cmd::ElementsDemolish { ids: ids.iter().map(|i| i.0).collect(), phase: None, view: Some(Ref::Id(view.0)) };
    if crate::actions::run_command(state, command).is_none() {
        return false;
    }
    let p = state.project();
    let phase = p.view(view).and_then(|v| p.view_phase(v)).map(|ph| p.phase_name(ph)).unwrap_or_default();
    state.message = t!("msg-demolished", "phase" => phase);
    true
}

/// Starting the Demolish command: a selection standing in the active view's
/// phase is demolished at once; otherwise the tool waits for clicks or a window.
pub fn start_demolish(state: &mut AppState) {
    if let Some(view) = crate::commands::phase_view(state) {
        let ids = demolishable(state, view, state.selection.iter().copied());
        if !ids.is_empty() {
            if demolish(state, view, &ids) {
                state.selection.clear();
            }
            return;
        }
    }
    state.set_tool(crate::tools::Tool::Demolish);
    state.message = t!("hint-demolish");
}

/// Click elements, or drag a window around them (left to right: inside it;
/// right to left: touching it), to demolish them in the active view's phase.
pub fn demolish_tool(
    resp: &egui::Response,
    painter: &egui::Painter,
    state: &mut AppState,
    view: &cc_core::View,
    xf: &crate::paint::Xf,
    drawing: &cc_views::Drawing2D,
) {
    use crate::tools::Drag;
    if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(start) = resp.interact_pointer_pos()
    {
        state.interaction.drag = Some(Drag::Box { start });
    }
    if let Some(Drag::Box { start }) = state.interaction.drag {
        let end = resp.interact_pointer_pos().or(resp.hover_pos()).unwrap_or(start);
        let (a, b) = (xf.to_world(start), xf.to_world(end));
        let inside = end.x >= start.x;
        let ids = demolishable(state, view.id, drawing.elements_in(a.min(b), a.max(b), inside));
        let lit: std::collections::BTreeSet<ElementId> = ids.iter().copied().collect();
        crate::modify::paint_ghost(painter, xf, drawing, &lit, &cc_core::PlanTransform::IDENTITY);
        let rect = egui::Rect::from_two_pos(start, end);
        let color = PALETTE.danger;
        painter.rect_filled(rect, 0.0, color.gamma_multiply(0.08));
        if inside {
            painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Middle);
        } else {
            let c = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
            painter.extend(egui::Shape::dashed_line(&c, egui::Stroke::new(1.0, color), 5.0, 3.0));
        }
        if resp.drag_stopped() {
            state.interaction.drag = None;
            demolish(state, view.id, &ids);
        }
        return;
    }
    let Some(raw) = resp.hover_pos().map(|p| xf.to_world(p)) else { return };
    let hovered = drawing.hit_test(raw, 6.0 / xf.scale).and_then(|id| demolishable(state, view.id, [id]).pop());
    let lit: std::collections::BTreeSet<ElementId> = hovered.into_iter().collect();
    crate::modify::paint_ghost(painter, xf, drawing, &lit, &cc_core::PlanTransform::IDENTITY);
    if resp.clicked() {
        demolish(state, view.id, &lit.into_iter().collect::<Vec<_>>());
    }
}
