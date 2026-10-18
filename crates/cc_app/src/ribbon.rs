//! Ribbon, options bar and status bar.

use crate::actions;
use crate::icons::{self, Icon};
use crate::state::{AppState, PendingAction, RibbonTab};
use crate::t;
use crate::theme::{self, PALETTE};
use crate::tools::{Tool, walls_loop};
use cc_core::{Category, ViewCategory};
use cc_views::format;
use egui::{Align, Button, DragValue, Layout, Response, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

/// Height of the large ribbon buttons.
const BUTTON_HEIGHT: f32 = 56.0;
const ICON_SIZE: f32 = 22.0;

/// Icon of a model category, shared by the ribbon, options bar and browser.
pub fn category_icon(category: Category) -> Icon {
    match category {
        Category::Wall => Icon::Wall,
        Category::Door => Icon::Door,
        Category::Window => Icon::Window,
        Category::Floor => Icon::Floor,
        Category::Ceiling => Icon::Ceiling,
        Category::Roof => Icon::Roof,
        Category::Room => Icon::Room,
        Category::Grid => Icon::Grid,
        Category::Dimension => Icon::Dimension,
        Category::Column => Icon::Column,
        Category::Beam => Icon::Beam,
        Category::Stair => Icon::Stair,
        Category::Railing => Icon::Railing,
    }
}

fn tool_icon(tool: &Tool) -> Icon {
    match tool {
        Tool::Select => Icon::Select,
        Tool::Section => Icon::Section,
        _ => tool.category().map_or(Icon::Select, category_icon),
    }
}

/// A large ribbon button: icon above a label. `dropdown` adds a chevron.
fn big_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool, dropdown: bool) -> Response {
    let text_color = if enabled { PALETTE.text } else { PALETTE.text_faint };
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(12.0), text_color);
    let chevron = if dropdown { 11.0 } else { 0.0 };
    let width = (galley.size().x + chevron + 16.0).max(54.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(width, BUTTON_HEIGHT), sense);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let radius = theme::RADIUS as f32;
        let pressed = response.is_pointer_button_down_on();
        if selected {
            painter.rect_filled(rect, radius, PALETTE.accent_soft);
            let border = Stroke::new(1.0, PALETTE.accent.gamma_multiply(0.55));
            painter.rect_stroke(rect, radius, border, StrokeKind::Inside);
        } else if enabled && (response.hovered() || pressed) {
            painter.rect_filled(rect, radius, if pressed { PALETTE.pressed } else { PALETTE.hover });
        }
        let icon_color = match (enabled, selected) {
            (false, _) => PALETTE.text_faint,
            (true, true) => PALETTE.accent_text,
            (true, false) => PALETTE.text,
        };
        let icon_center = egui::pos2(rect.center().x, rect.top() + 7.0 + ICON_SIZE / 2.0);
        icons::paint(painter, egui::Rect::from_center_size(icon_center, vec2(ICON_SIZE, ICON_SIZE)), icon, icon_color);
        let text_size = galley.size();
        let text_pos = egui::pos2(rect.center().x - (text_size.x + chevron) / 2.0, rect.bottom() - 7.0 - text_size.y);
        if dropdown {
            let side = text_size.y - 3.0;
            let c = egui::Rect::from_min_size(
                egui::pos2(text_pos.x + text_size.x + 2.0, text_pos.y + 2.0),
                vec2(side, side),
            );
            icons::paint(painter, c, Icon::ChevronDown, icon_color);
        }
        let text_color = if selected { PALETTE.accent_text } else { text_color };
        painter.galley_with_override_text_color(text_pos, galley, text_color);
    }
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

/// Small square icon button (quick access toolbar).
fn icon_button(ui: &mut Ui, icon: Icon, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 26.0), sense);
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, theme::RADIUS as f32, PALETTE.hover);
    }
    let color = if enabled { PALETTE.text_muted } else { PALETTE.text_faint.gamma_multiply(0.6) };
    icons::paint(ui.painter(), rect.shrink(5.0), icon, color);
    response
}

/// A captioned group of ribbon buttons followed by a vertical divider.
fn group(ui: &mut Ui, caption: String, add: impl FnOnce(&mut Ui)) {
    let caption_w =
        ui.painter().layout_no_wrap(caption.clone(), egui::FontId::proportional(11.0), PALETTE.text).size().x;
    let inner = ui.vertical(|ui| {
        // At least as wide as the caption, so it never wraps.
        ui.set_min_width(caption_w + 8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            add(ui);
        });
        let width = ui.min_rect().width();
        ui.allocate_ui_with_layout(vec2(width, 14.0), Layout::top_down(Align::Center), |ui| {
            ui.label(theme::caption(caption));
        });
    });
    let height = inner.response.rect.height();
    let (r, _) = ui.allocate_exact_size(vec2(13.0, height), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range().shrink(4.0), Stroke::new(1.0, PALETTE.border));
}

/// A ribbon tab title in the top strip.
fn tab_title(ui: &mut Ui, label: &str, active: bool) -> Response {
    let family = if active { theme::semibold() } else { egui::FontFamily::Proportional };
    let color = if active { PALETTE.accent_text } else { PALETTE.text_muted };
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::new(13.0, family), color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(20.0, 12.0), Sense::click());
    if response.hovered() && !active {
        ui.painter().rect_filled(rect.shrink2(vec2(2.0, 3.0)), theme::RADIUS as f32, PALETTE.hover);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    if active {
        let y = rect.bottom() - 1.0;
        let line = [egui::pos2(rect.left() + 8.0, y), egui::pos2(rect.right() - 8.0, y)];
        ui.painter().line_segment(line, Stroke::new(2.0, PALETTE.accent));
    }
    response
}

fn tool_button(ui: &mut Ui, state: &mut AppState, label: String, shortcut: &str, category: Category) {
    let active = state.tool.category() == Some(category) && state.tool != Tool::Section;
    let r = big_button(ui, category_icon(category), &label, active, true, false)
        .on_hover_text(format!("{label} — {} {shortcut}", t!("hint-shortcut")));
    if r.clicked() {
        actions::activate_category_tool(state, category);
    }
}

fn select_button(ui: &mut Ui, state: &mut AppState) {
    let r = big_button(ui, Icon::Select, &t!("tool-select"), state.tool == Tool::Select, true, false)
        .on_hover_text(format!("{} — MD / Esc", t!("tool-select")));
    if r.clicked() {
        state.set_tool(Tool::Select);
    }
}

pub fn ribbon(ui: &mut Ui, state: &mut AppState) {
    // Quick access toolbar at the top left, where desktop applications keep it.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.add_space(2.0);
        let (mark, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
        if let Some(logo) = theme::logo(ui.ctx()) {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(logo.id(), mark, uv, egui::Color32::WHITE);
        }
        ui.add_space(8.0);
        if icon_button(ui, Icon::Home, true).on_hover_text(t!("action-home")).clicked() {
            state.show_home = true;
        }
        if icon_button(ui, Icon::New, true).on_hover_text(format!("{} (Ctrl+N)", t!("action-new"))).clicked() {
            actions::guarded(state, PendingAction::New);
        }
        if icon_button(ui, Icon::Open, true).on_hover_text(format!("{} (Ctrl+O)", t!("action-open"))).clicked() {
            actions::guarded(state, PendingAction::Open);
        }
        if icon_button(ui, Icon::Save, true).on_hover_text(format!("{} (Ctrl+S)", t!("action-save"))).clicked() {
            actions::save(state, false);
        }
        let (div, _) = ui.allocate_exact_size(vec2(9.0, 18.0), Sense::hover());
        ui.painter().vline(div.center().x, div.y_range(), Stroke::new(1.0, PALETTE.border));
        let undo_name = state.doc.undo_name().map(|n| format!(": {n}")).unwrap_or_default();
        let undo = icon_button(ui, Icon::Undo, state.doc.can_undo());
        if undo.on_hover_text(format!("{}{undo_name} (Ctrl+Z)", t!("action-undo"))).clicked() {
            state.doc.undo();
        }
        let redo = icon_button(ui, Icon::Redo, state.doc.can_redo());
        if redo.on_hover_text(format!("{} (Ctrl+Y)", t!("action-redo"))).clicked() {
            state.doc.redo();
        }
    });
    // Ribbon tab titles.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (tab, label) in [
            (RibbonTab::File, t!("ribbon-file")),
            (RibbonTab::Architecture, t!("ribbon-architecture")),
            (RibbonTab::Structure, t!("ribbon-structure")),
            (RibbonTab::Modify, t!("ribbon-modify")),
            (RibbonTab::Annotate, t!("ribbon-annotate")),
            (RibbonTab::View, t!("ribbon-view")),
            (RibbonTab::Manage, t!("ribbon-manage")),
        ] {
            if tab_title(ui, &label, state.ribbon_tab == tab).clicked() {
                state.ribbon_tab = tab;
            }
        }
    });
    let strip_bottom = ui.min_rect().bottom();
    ui.painter().hline(ui.max_rect().x_range(), strip_bottom + 3.0, Stroke::new(1.0, PALETTE.border));
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        match state.ribbon_tab {
            RibbonTab::File => file_tab(ui, state),
            RibbonTab::Architecture => architecture_tab(ui, state),
            RibbonTab::Structure => structure_tab(ui, state),
            RibbonTab::Modify => modify_tab(ui, state),
            RibbonTab::Annotate => annotate_tab(ui, state),
            RibbonTab::View => view_tab(ui, state),
            RibbonTab::Manage => manage_tab(ui, state),
        }
    });
    ui.add_space(2.0);
}

fn file_tab(ui: &mut Ui, state: &mut AppState) {
    group(ui, t!("group-project"), |ui| {
        if big_button(ui, Icon::New, &t!("action-new"), false, true, false).on_hover_text("Ctrl+N").clicked() {
            actions::guarded(state, PendingAction::New);
        }
        if big_button(ui, Icon::Open, &t!("action-open"), false, true, false).on_hover_text("Ctrl+O").clicked() {
            actions::guarded(state, PendingAction::Open);
        }
        if big_button(ui, Icon::Save, &t!("action-save"), false, true, false).on_hover_text("Ctrl+S").clicked() {
            actions::save(state, false);
        }
        let save_as = big_button(ui, Icon::SaveAs, &t!("action-save-as"), false, true, false);
        if save_as.on_hover_text("Ctrl+Shift+S").clicked() {
            actions::save(state, true);
        }
        let close = big_button(ui, Icon::CloseProject, &t!("action-close-project"), false, true, false);
        if close.on_hover_text("Ctrl+W").clicked() {
            actions::guarded(state, PendingAction::Close);
        }
    });
    group(ui, t!("group-export"), |ui| {
        if big_button(ui, Icon::Export, &t!("action-export-ifc"), false, true, false).clicked() {
            actions::export_ifc(state);
        }
    });
    group(ui, t!("group-settings"), |ui| {
        ui.allocate_ui_with_layout(vec2(170.0, BUTTON_HEIGHT), Layout::top_down(Align::Min), |ui| {
            ui.add_space(6.0);
            ui.label(theme::muted(t!("settings-language")).size(12.0));
            let mut lang = crate::i18n::language();
            egui::ComboBox::from_id_salt("language").width(150.0).selected_text(lang.native_name()).show_ui(ui, |ui| {
                for l in crate::i18n::Language::ALL {
                    ui.selectable_value(&mut lang, l, l.native_name());
                }
            });
            actions::set_language(state, lang);
        });
    });
    group(ui, t!("group-help"), |ui| {
        if big_button(ui, Icon::About, &t!("dlg-about"), false, true, false).clicked() {
            state.dialogs.about = true;
        }
    });
}

fn architecture_tab(ui: &mut Ui, state: &mut AppState) {
    group(ui, t!("group-build"), |ui| {
        tool_button(ui, state, t!("tool-wall"), "WA", Category::Wall);
        tool_button(ui, state, t!("tool-door"), "DR", Category::Door);
        tool_button(ui, state, t!("tool-window"), "WN", Category::Window);
    });
    group(ui, t!("group-horizontal"), |ui| {
        tool_button(ui, state, t!("tool-floor"), "SB", Category::Floor);
        tool_button(ui, state, t!("tool-ceiling"), "FO", Category::Ceiling);
        tool_button(ui, state, t!("tool-roof"), "TE", Category::Roof);
    });
    group(ui, t!("group-circulation"), |ui| {
        tool_button(ui, state, t!("tool-stair"), "ST", Category::Stair);
        tool_button(ui, state, t!("tool-railing"), "RA", Category::Railing);
    });
    group(ui, t!("group-room"), |ui| {
        tool_button(ui, state, t!("tool-room"), "RM", Category::Room);
    });
    group(ui, t!("group-datum"), |ui| {
        tool_button(ui, state, t!("tool-grid"), "GR", Category::Grid);
    });
    group(ui, t!("group-selection"), |ui| select_button(ui, state));
}

fn structure_tab(ui: &mut Ui, state: &mut AppState) {
    group(ui, t!("group-structure"), |ui| {
        tool_button(ui, state, t!("tool-column"), "CL", Category::Column);
        tool_button(ui, state, t!("tool-beam"), "BM", Category::Beam);
    });
    group(ui, t!("group-datum"), |ui| {
        tool_button(ui, state, t!("tool-grid"), "GR", Category::Grid);
    });
    group(ui, t!("group-selection"), |ui| select_button(ui, state));
}

fn annotate_tab(ui: &mut Ui, state: &mut AppState) {
    group(ui, t!("group-dimension"), |ui| {
        tool_button(ui, state, t!("tool-dimension"), "DI", Category::Dimension);
    });
    group(ui, t!("group-selection"), |ui| select_button(ui, state));
}

fn modify_tab(ui: &mut Ui, state: &mut AppState) {
    let has_sel = !state.selection.is_empty();
    group(ui, t!("group-selection"), |ui| {
        select_button(ui, state);
        let copy = big_button(ui, Icon::Copy, &t!("action-copy"), false, has_sel, false);
        if copy.on_hover_text(t!("hint-copy")).clicked() {
            let ids: Vec<_> = state.selection.iter().copied().collect();
            crate::tools::duplicate(state, &ids, glam::DVec2::new(1.0, -1.0));
        }
        let delete = big_button(ui, Icon::Delete, &t!("action-delete"), false, has_sel, false);
        if delete.on_hover_text("Delete").clicked() {
            state.delete_selection();
        }
    });
    group(ui, t!("group-history"), |ui| {
        let undo_name = state.doc.undo_name().map(|n| format!(": {n}")).unwrap_or_default();
        let undo = big_button(ui, Icon::Undo, &t!("action-undo"), false, state.doc.can_undo(), false);
        if undo.on_hover_text(format!("{}{undo_name} (Ctrl+Z)", t!("action-undo"))).clicked() {
            state.doc.undo();
        }
        let redo = big_button(ui, Icon::Redo, &t!("action-redo"), false, state.doc.can_redo(), false);
        if redo.on_hover_text("Ctrl+Y").clicked() {
            state.doc.redo();
        }
    });
}

fn view_tab(ui: &mut Ui, state: &mut AppState) {
    let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
    group(ui, t!("group-create-views"), |ui| {
        for (ceiling, icon, label) in [
            (false, Icon::FloorPlan, t!("view-new-floor-plan")),
            (true, Icon::CeilingPlan, t!("view-new-ceiling-plan")),
        ] {
            let r = big_button(ui, icon, &label, false, true, true);
            egui::Popup::menu(&r).show(|ui| {
                for (id, name) in &levels {
                    if ui.button(name).clicked() {
                        actions::new_plan(state, *id, ceiling);
                    }
                }
            });
        }
        if big_button(ui, Icon::View3D, &t!("view-new-3d"), false, true, false).clicked() {
            actions::new_3d(state);
        }
        let section = big_button(ui, Icon::Section, &t!("tool-section"), state.tool == Tool::Section, true, false);
        if section.on_hover_text(format!("{} — CT", t!("tool-section"))).clicked() {
            state.set_tool(Tool::Section);
            state.message = t!("msg-section-hint");
        }
        let r = big_button(ui, Icon::Elevation, &t!("view-elevations"), false, true, true);
        egui::Popup::menu(&r).show(|ui| {
            let elevations: Vec<_> =
                state.project().views_of(ViewCategory::Elevation).map(|v| (v.id, v.name.clone())).collect();
            for (id, name) in elevations {
                if ui.button(name).clicked() {
                    state.open_view(id);
                }
            }
        });
    });
    group(ui, t!("group-documentation"), |ui| {
        if big_button(ui, Icon::Legend, &t!("view-new-legend"), false, true, false).clicked() {
            actions::new_legend(state);
        }
        let r = big_button(ui, Icon::Schedule, &t!("view-new-schedule"), false, true, true);
        egui::Popup::menu(&r).show(|ui| {
            for c in Category::SCHEDULABLE {
                if ui.button(actions::t_category(c)).clicked() {
                    actions::new_schedule(state, c);
                }
            }
        });
        if big_button(ui, Icon::Sheet, &t!("view-new-sheet"), false, true, false).clicked() {
            actions::new_sheet(state);
        }
    });
    group(ui, t!("group-navigate"), |ui| {
        if big_button(ui, Icon::ZoomFit, &t!("action-zoom-fit"), false, true, false).on_hover_text("ZE").clicked() {
            crate::app::zoom_fit(state);
        }
    });
}

fn manage_tab(ui: &mut Ui, state: &mut AppState) {
    group(ui, t!("group-project"), |ui| {
        if big_button(ui, Icon::ProjectInfo, &t!("dlg-project-info"), false, true, false).clicked() {
            state.dialogs.project_info = true;
        }
        if big_button(ui, Icon::Levels, &t!("dlg-levels"), false, true, false).clicked() {
            state.dialogs.levels = true;
        }
        if big_button(ui, Icon::Materials, &t!("dlg-materials"), false, true, false).clicked() {
            state.dialogs.materials = true;
        }
    });
}

/// Context options for the active tool (options bar).
pub fn options_bar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(PALETTE.accent_soft)
            .corner_radius(theme::RADIUS)
            .inner_margin(egui::Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (r, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                icons::paint(ui.painter(), r, tool_icon(&state.tool), PALETTE.accent_text);
                let name = crate::i18n::tr(state.tool.key());
                ui.label(RichText::new(name).family(theme::semibold()).color(PALETTE.accent_text));
            });
        ui.separator();
        if let (Some(category), Some(type_id)) = (state.tool.category(), state.tool.type_id()) {
            let types: Vec<_> = state.project().types_of(category).map(|t| (t.id, t.name.clone())).collect();
            let mut selected = type_id;
            egui::ComboBox::from_id_salt("tool_type")
                .selected_text(state.project().type_name(type_id).to_string())
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (id, name) in types {
                        ui.selectable_value(&mut selected, id, name);
                    }
                });
            if selected != type_id {
                state.tool = state.tool.with_type(selected);
            }
            if ui.button(t!("props-edit-type")).clicked() {
                state.dialogs.type_editor = Some(type_id);
            }
            ui.separator();
        }
        match state.tool.clone() {
            Tool::Select => {
                ui.label(theme::muted(t!("hint-select")));
            }
            Tool::Wall { .. } => {
                ui.label(t!("prop-top-constraint"));
                let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
                let current = match state.wall_top_level.and_then(|id| levels.iter().find(|l| l.0 == id)) {
                    Some((_, name)) => t!("top-up-to", "level" => name),
                    None => t!("top-unconnected"),
                };
                egui::ComboBox::from_id_salt("wall_top").selected_text(current).width(170.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.wall_top_level, None, t!("top-unconnected"));
                    for (id, name) in &levels {
                        ui.selectable_value(&mut state.wall_top_level, Some(*id), t!("top-up-to", "level" => name));
                    }
                });
                if state.wall_top_level.is_none() {
                    ui.add(
                        DragValue::new(&mut state.wall_height)
                            .speed(0.01)
                            .range(0.1..=50.0)
                            .fixed_decimals(2)
                            .suffix(" m"),
                    );
                }
                ui.separator();
                ui.label(t!("prop-location-line"));
                let location = &mut state.wall_location_line;
                egui::ComboBox::from_id_salt("wall_location")
                    .selected_text(crate::i18n::tr(location.key()))
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for l in cc_core::LocationLine::ALL {
                            ui.selectable_value(location, l, crate::i18n::tr(l.key()));
                        }
                    });
                ui.separator();
                ui.label(theme::muted(t!("hint-wall")));
            }
            Tool::Hosted { .. } => {
                ui.checkbox(&mut state.interaction.flip_hand, t!("prop-flip-hand"));
                ui.label(theme::muted(t!("hint-hosted")));
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
                ui.label(theme::muted(t!("hint-sketch")));
            }
            Tool::Section => {
                ui.label(theme::muted(t!("msg-section-hint")));
            }
            Tool::Room { .. } => {
                ui.label(theme::muted(t!("hint-room")));
            }
            Tool::Grid { .. } => {
                ui.label(theme::muted(t!("hint-grid")));
            }
            Tool::Dimension { .. } => {
                ui.label(theme::muted(t!("hint-dimension")));
            }
            Tool::Column { .. } => {
                ui.label(theme::muted(t!("hint-column")));
            }
            Tool::Beam { .. } => {
                ui.label(theme::muted(t!("hint-beam")));
            }
            Tool::Stair { .. } => {
                ui.label(theme::muted(t!("hint-stair")));
            }
            Tool::Railing { type_id } => {
                let n = state.interaction.points.len();
                if ui.add_enabled(n >= 2, Button::new(t!("sketch-finish"))).clicked()
                    && let Some(level) = state.active_level()
                {
                    crate::tools::finish_railing(state, level, type_id);
                }
                if ui.add_enabled(n > 0, Button::new(t!("action-cancel"))).clicked() {
                    state.interaction.points.clear();
                }
                ui.label(theme::muted(t!("hint-railing")));
            }
        }
    });
}

pub fn status_bar(ui: &mut Ui, state: &AppState) {
    ui.horizontal(|ui| {
        if let Some(c) = state.cursor {
            let coords = format!("X {:>9}  Y {:>9}", format::number(c.x, 3), format::number(c.y, 3));
            ui.label(RichText::new(coords).monospace().color(PALETTE.text_muted));
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
        ui.label(theme::muted(&state.message));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let file = state
                .doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| t!("status-unsaved-file"));
            let dirty = if state.doc.is_dirty() { " *" } else { "" };
            ui.label(RichText::new(format!("{file}{dirty}")).color(PALETTE.text_faint));
        });
    });
}
