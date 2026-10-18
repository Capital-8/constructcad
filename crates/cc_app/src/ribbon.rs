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
const BUTTON_HEIGHT: f32 = 66.0;
const ICON_SIZE: f32 = 22.0;
/// Icon size of the small stacked ribbon buttons.
const STACK_ICON_SIZE: f32 = 20.0;

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
        Category::Component => Icon::Component,
    }
}

fn tool_icon(tool: &Tool) -> Icon {
    match tool {
        Tool::Select => Icon::Select,
        Tool::Section => Icon::Section,
        Tool::Move => Icon::Move,
        Tool::Copy => Icon::Copy,
        Tool::Rotate => Icon::Rotate,
        Tool::Mirror { draw: false } => Icon::MirrorPick,
        Tool::Mirror { draw: true } => Icon::MirrorDraw,
        Tool::Array => Icon::Array,
        Tool::TrimCorner => Icon::TrimCorner,
        Tool::TrimExtend => Icon::TrimExtend,
        Tool::Split => Icon::Split,
        Tool::Offset => Icon::Offset,
        Tool::Align => Icon::Align,
        Tool::ToggleJoin => Icon::Join,
        Tool::Paste => Icon::Paste,
        Tool::MatchType => Icon::MatchType,
        _ => tool.category().map_or(Icon::Select, category_icon),
    }
}

/// Labels wider than this (points) wrap onto two lines at a space.
const WRAP_WIDTH: f32 = 64.0;

/// Split `label` into one or two lines: long labels break at the space
/// that best balances the two lines.
fn label_lines(ui: &Ui, label: &str, font: &egui::FontId) -> Vec<String> {
    let width = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font.clone(), PALETTE.text).size().x;
    if width(label) <= WRAP_WIDTH || !label.contains(' ') {
        return vec![label.to_string()];
    }
    let best = label
        .match_indices(' ')
        .map(|(i, _)| (i, width(&label[..i]).max(width(&label[i + 1..]))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    match best {
        Some(i) => vec![label[..i].to_string(), label[i + 1..].to_string()],
        None => vec![label.to_string()],
    }
}

/// A large ribbon button: icon above a label of up to two lines. `dropdown` adds a chevron.
fn big_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool, dropdown: bool) -> Response {
    let text_color = if enabled { PALETTE.text } else { PALETTE.text_faint };
    let font = egui::FontId::proportional(12.0);
    let galleys: Vec<_> = label_lines(ui, label, &font)
        .into_iter()
        .map(|line| ui.painter().layout_no_wrap(line, font.clone(), text_color))
        .collect();
    let chevron = if dropdown { 11.0 } else { 0.0 };
    let text_w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let width = (text_w + chevron + 14.0).max(50.0);
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
        let icon_center = egui::pos2(rect.center().x, rect.top() + 6.0 + ICON_SIZE / 2.0);
        icons::paint(painter, egui::Rect::from_center_size(icon_center, vec2(ICON_SIZE, ICON_SIZE)), icon, icon_color);
        // Lines start right under the icon, each centered; the chevron follows the last one.
        let text_color = if selected { PALETTE.accent_text } else { text_color };
        let mut y = rect.top() + 6.0 + ICON_SIZE + 3.0;
        let n = galleys.len();
        for (k, galley) in galleys.into_iter().enumerate() {
            let size = galley.size();
            let extra = if k + 1 == n { chevron } else { 0.0 };
            let pos = egui::pos2(rect.center().x - (size.x + extra) / 2.0, y);
            if dropdown && k + 1 == n {
                let side = size.y - 3.0;
                let c = egui::Rect::from_min_size(egui::pos2(pos.x + size.x + 2.0, pos.y + 2.0), vec2(side, side));
                icons::paint(painter, c, Icon::ChevronDown, icon_color);
            }
            painter.galley_with_override_text_color(pos, galley, text_color);
            y += size.y - 1.0;
        }
    }
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

/// A small icon-only ribbon button for stacks; its name is the tooltip.
pub struct StackButton {
    pub icon: Icon,
    pub tooltip: String,
    pub enabled: bool,
    pub selected: bool,
}

/// Rows of a stacked family: families of 2 or 4 use two rows (one column,
/// or 2 x 2), everything else three.
fn stack_rows(n: usize) -> usize {
    if n == 2 || n == 4 { 2 } else { 3 }
}

/// One family of related commands as small icon buttons in columns of
/// two or three (see [`stack_rows`]); each family starts its own column,
/// so unrelated commands never share one. Two-row columns are centered on
/// the large buttons' height. Returns the index of the clicked button.
fn stacked(ui: &mut Ui, buttons: &[StackButton]) -> Option<usize> {
    let mut clicked = None;
    let rows = stack_rows(buttons.len());
    let size = vec2(28.0, (BUTTON_HEIGHT - 4.0) / 3.0);
    let column_h = rows as f32 * size.y + (rows as f32 - 1.0) * 2.0;
    for (column, chunk) in buttons.chunks(rows).enumerate() {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(((BUTTON_HEIGHT - column_h) / 2.0).max(0.0));
            for (row, b) in chunk.iter().enumerate() {
                let sense = if b.enabled { Sense::click() } else { Sense::hover() };
                let (rect, r) = ui.allocate_exact_size(size, sense);
                let painter = ui.painter();
                let radius = theme::RADIUS as f32;
                if b.selected {
                    painter.rect_filled(rect, radius, PALETTE.accent_soft);
                    painter.rect_stroke(
                        rect,
                        radius,
                        Stroke::new(1.0, PALETTE.accent.gamma_multiply(0.55)),
                        StrokeKind::Inside,
                    );
                } else if b.enabled && r.hovered() {
                    let fill = if r.is_pointer_button_down_on() { PALETTE.pressed } else { PALETTE.hover };
                    painter.rect_filled(rect, radius, fill);
                }
                let color = match (b.enabled, b.selected) {
                    (false, _) => PALETTE.text_faint,
                    (true, true) => PALETTE.accent_text,
                    (true, false) => PALETTE.text,
                };
                let icon_rect = egui::Rect::from_center_size(rect.center(), vec2(STACK_ICON_SIZE, STACK_ICON_SIZE));
                icons::paint(painter, icon_rect, b.icon, color);
                let r = if b.enabled { r.on_hover_cursor(egui::CursorIcon::PointingHand) } else { r };
                if r.on_hover_text(&b.tooltip).clicked() && b.enabled {
                    clicked = Some(column * rows + row);
                }
            }
        });
    }
    clicked
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
        // Frequent commands that already live in the ribbon.
        let (div, _) = ui.allocate_exact_size(vec2(9.0, 18.0), Sense::hover());
        ui.painter().vline(div.center().x, div.y_range(), Stroke::new(1.0, PALETTE.border));
        let sheet = actions::active_sheet(state).is_some();
        let pdf_hint = if sheet { t!("action-export-pdf") } else { t!("hint-export-sheet") };
        if icon_button(ui, Icon::Export, sheet).on_hover_text(pdf_hint).clicked() {
            actions::export_sheet(state, actions::SheetFormat::Pdf);
        }
        let dim = format!("{} (DI)", t!("tool-dimension"));
        if icon_button(ui, Icon::Dimension, true).on_hover_text(dim).clicked() {
            actions::activate_category_tool(state, Category::Dimension);
        }
        if icon_button(ui, Icon::View3D, true).on_hover_text(t!("action-default-3d")).clicked() {
            actions::open_default_3d(state);
        }
        if icon_button(ui, Icon::Section, true).on_hover_text(format!("{} (CT)", t!("tool-section"))).clicked() {
            state.set_tool(Tool::Section);
            state.message = t!("msg-section-hint");
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
    group(ui, t!("group-import"), |ui| {
        if big_button(ui, Icon::Open, &t!("action-import-ifc"), false, true, false).clicked() {
            actions::guarded(state, PendingAction::ImportIfc);
        }
    });
    group(ui, t!("group-export"), |ui| {
        if big_button(ui, Icon::Export, &t!("action-export-ifc"), false, true, false).clicked() {
            actions::export_ifc(state);
        }
        let sheet = actions::active_sheet(state).is_some();
        for (label, format) in
            [(t!("action-export-pdf"), actions::SheetFormat::Pdf), (t!("action-export-svg"), actions::SheetFormat::Svg)]
        {
            let r = big_button(ui, Icon::Sheet, &label, false, sheet, false);
            let r = if sheet { r } else { r.on_hover_text(t!("hint-export-sheet")) };
            if sheet && r.clicked() {
                actions::export_sheet(state, format);
            }
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
    group(ui, t!("group-model"), |ui| {
        tool_button(ui, state, t!("tool-component"), "CM", Category::Component);
        if big_button(ui, Icon::Plus, &t!("action-new-component"), false, true, false).clicked() {
            state.dialogs.component_editor = Some(crate::component_editor::Editor::new_component());
        }
        if big_button(ui, Icon::Open, &t!("action-load-component"), false, true, false).clicked() {
            actions::load_component(state);
        }
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
        let edit = [StackButton {
            icon: Icon::Delete,
            tooltip: format!("{} (Delete)", t!("action-delete")),
            enabled: has_sel,
            selected: false,
        }];
        if stacked(ui, &edit).is_some() {
            state.delete_selection();
        }
        let undo_name = state.doc.undo_name().map(|n| format!(": {n}")).unwrap_or_default();
        let history = [
            StackButton {
                icon: Icon::Undo,
                tooltip: format!("{}{undo_name} (Ctrl+Z)", t!("action-undo")),
                enabled: state.doc.can_undo(),
                selected: false,
            },
            StackButton {
                icon: Icon::Redo,
                tooltip: format!("{} (Ctrl+Y)", t!("action-redo")),
                enabled: state.doc.can_redo(),
                selected: false,
            },
        ];
        match stacked(ui, &history) {
            Some(0) => {
                state.doc.undo();
            }
            Some(1) => {
                state.doc.redo();
            }
            _ => {}
        }
    });
    group(ui, t!("group-clipboard"), |ui| clipboard_group(ui, state, has_sel));
    group(ui, t!("group-modify"), |ui| {
        let tip = |key: &str, keys: &str| format!("{} ({keys})", crate::i18n::tr(key));
        let tools = [
            (Tool::Move, Icon::Move, tip("tool-move", "MV")),
            (Tool::Copy, Icon::Copy, tip("tool-copy", "CO")),
            (Tool::Rotate, Icon::Rotate, tip("tool-rotate", "RO")),
            (Tool::Mirror { draw: false }, Icon::MirrorPick, tip("tool-mirror-pick", "MM")),
            (Tool::Mirror { draw: true }, Icon::MirrorDraw, tip("tool-mirror-draw", "DM")),
            (Tool::Array, Icon::Array, tip("tool-array", "AR")),
        ];
        let buttons: Vec<StackButton> = tools
            .iter()
            .map(|(tool, icon, tooltip)| StackButton {
                icon: *icon,
                tooltip: tooltip.clone(),
                enabled: has_sel || state.tool == *tool,
                selected: state.tool == *tool,
            })
            .collect();
        if let Some(i) = stacked(ui, &buttons)
            && let Some((tool, _, _)) = tools.get(i)
        {
            crate::modify::start(state, tool.clone());
        }
        // Geometry edits pick their elements, no selection needed.
        let picks = [
            (Tool::Align, Icon::Align, tip("tool-align", "AL")),
            (Tool::Offset, Icon::Offset, tip("tool-offset", "OF")),
            (Tool::TrimCorner, Icon::TrimCorner, tip("tool-trim-corner", "TR")),
            (Tool::TrimExtend, Icon::TrimExtend, tip("tool-trim-extend", "TX")),
            (Tool::Split, Icon::Split, tip("tool-split", "SL")),
            (Tool::ToggleJoin, Icon::Join, crate::i18n::tr("tool-toggle-join")),
        ];
        let buttons: Vec<StackButton> = picks
            .iter()
            .map(|(tool, icon, tooltip)| StackButton {
                icon: *icon,
                tooltip: tooltip.clone(),
                enabled: true,
                selected: state.tool == *tool,
            })
            .collect();
        if let Some(i) = stacked(ui, &buttons)
            && let Some((tool, _, _)) = picks.get(i)
        {
            crate::modify::start_pick(state, tool.clone());
        }
        let pins = [
            StackButton { icon: Icon::Pin, tooltip: tip("action-pin", "PN"), enabled: has_sel, selected: false },
            StackButton { icon: Icon::Unpin, tooltip: tip("action-unpin", "UP"), enabled: has_sel, selected: false },
        ];
        match stacked(ui, &pins) {
            Some(0) => crate::modify::set_pinned(state, true),
            Some(1) => crate::modify::set_pinned(state, false),
            _ => {}
        }
    });
}

/// Paste (a menu of where to paste), then cut, copy and match type.
fn clipboard_group(ui: &mut Ui, state: &mut AppState, has_sel: bool) {
    let can_paste = !state.clipboard.is_empty();
    let pasting = state.tool == Tool::Paste;
    let r = big_button(ui, Icon::Paste, &t!("action-paste"), pasting, can_paste, true);
    egui::Popup::menu(&r).show(|ui| {
        if ui.button(format!("{} (Ctrl+V)", t!("paste-click"))).clicked() {
            crate::clipboard::paste(state);
        }
        if ui.button(t!("paste-same-place")).clicked() {
            crate::clipboard::paste_aligned(state, false);
        }
        let plan = state.active_level().is_some();
        if ui.add_enabled(plan, Button::new(t!("paste-current-level"))).clicked() {
            crate::clipboard::paste_aligned(state, true);
        }
    });
    let buttons = [
        StackButton {
            icon: Icon::Cut,
            tooltip: format!("{} (Ctrl+X)", t!("action-cut")),
            enabled: has_sel,
            selected: false,
        },
        StackButton {
            icon: Icon::Copy,
            tooltip: format!("{} (Ctrl+C)", t!("action-copy")),
            enabled: has_sel,
            selected: false,
        },
        StackButton {
            icon: Icon::MatchType,
            tooltip: format!("{} (MA)", t!("tool-match-type")),
            enabled: true,
            selected: state.tool == Tool::MatchType,
        },
    ];
    match stacked(ui, &buttons) {
        Some(0) => crate::clipboard::cut(ui.ctx(), state),
        Some(1) => {
            crate::clipboard::copy(ui.ctx(), state);
        }
        Some(2) => crate::clipboard::start_match_type(state),
        _ => {}
    }
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
        if big_button(ui, Icon::Dimension, &t!("dlg-units"), false, true, false).clicked() {
            state.dialogs.units = true;
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
                    ui.add(crate::properties::length_drag(&mut state.wall_height).speed(0.01).range(0.1..=50.0));
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
            Tool::Component { .. } => {
                ui.label(t!("prop-rotation"));
                let rotation = &mut state.interaction.rotation;
                if ui.add(DragValue::new(rotation).speed(1.0).suffix("°")).changed() {
                    *rotation = rotation.rem_euclid(360.0);
                }
                ui.label(theme::muted(t!("hint-component")));
            }
            Tool::Move => {
                ui.label(theme::muted(t!("hint-base-point")));
            }
            Tool::Copy => {
                ui.checkbox(&mut state.modify.multiple, t!("opt-multiple"));
                ui.label(theme::muted(t!("hint-base-point")));
            }
            Tool::Array => {
                ui.label(t!("opt-count"));
                ui.add(DragValue::new(&mut state.modify.count).range(2..=200));
                ui.label(theme::muted(t!("hint-array")));
            }
            Tool::Rotate => {
                ui.checkbox(&mut state.modify.rotate_copy, t!("opt-copy"));
                ui.separator();
                ui.label(t!("prop-rotation"));
                ui.add(DragValue::new(&mut state.modify.angle).speed(1.0).suffix("°"));
                if ui.button(t!("action-apply")).clicked()
                    && let Some(center) = state.interaction.center
                {
                    let targets = state.interaction.targets.iter().copied().collect();
                    crate::modify::apply_rotation(state, &targets, center, state.modify.angle.to_radians());
                }
                if ui.button(t!("opt-set-center")).clicked() {
                    state.interaction.placing_center = true;
                }
                ui.label(theme::muted(t!("hint-rotate")));
            }
            Tool::Offset => {
                ui.label(t!("opt-distance"));
                ui.add(crate::properties::length_drag(&mut state.modify.offset).range(0.001..=100.0));
                ui.label(theme::muted(t!("hint-offset")));
            }
            Tool::TrimCorner | Tool::TrimExtend | Tool::Split | Tool::Align | Tool::ToggleJoin => {
                let key = match state.tool {
                    Tool::TrimCorner => "hint-trim-corner",
                    Tool::TrimExtend => "hint-trim-extend",
                    Tool::Split => "hint-split",
                    Tool::ToggleJoin => "hint-toggle-join",
                    _ => "hint-align",
                };
                ui.label(theme::muted(crate::i18n::tr(key)));
            }
            Tool::Paste => {
                ui.label(theme::muted(t!("hint-paste")));
            }
            Tool::MatchType => {
                ui.label(theme::muted(t!("hint-match-type")));
            }
            Tool::Mirror { draw } => {
                ui.checkbox(&mut state.modify.mirror_copy, t!("opt-copy"));
                ui.label(theme::muted(if draw { t!("hint-mirror-draw") } else { t!("hint-mirror-pick") }));
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
            let coords = format!("X {:>9}  Y {:>9}", format::meters(c.x), format::meters(c.y));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_labels_wrap_on_two_balanced_lines() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut lines = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = egui::FontId::proportional(12.0);
            lines = [
                "Níveis",
                "Unidades do projeto",
                "Informações do projeto",
                "Relatório de perda de pressão da tubulação",
                "Supercalifragilístico",
            ]
            .map(|l| label_lines(ui, l, &font))
            .to_vec();
        });
        out.textures_delta.clear();
        assert_eq!(lines[0], vec!["Níveis"]);
        // Long labels break at a space into two lines that keep all the words.
        for (i, label) in [
            (1, "Unidades do projeto"),
            (2, "Informações do projeto"),
            (3, "Relatório de perda de pressão da tubulação"),
        ] {
            assert_eq!(lines[i].len(), 2, "{label}");
            assert_eq!(lines[i].join(" "), label);
            assert!(lines[i].iter().all(|l| l.len() < label.len()));
        }
        // A single long word cannot wrap.
        assert_eq!(lines[4], vec!["Supercalifragilístico"]);
    }

    #[test]
    fn families_of_two_or_four_use_two_rows() {
        assert_eq!([1, 2, 3, 4, 5, 6, 7].map(stack_rows), [3, 2, 3, 2, 3, 3, 3]);
    }
}
