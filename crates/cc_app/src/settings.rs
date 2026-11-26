//! Building blocks of settings pages: a dialog body with a sidebar of pages
//! beside the page shown (the Preferences window), a page title with what
//! the page holds, groups of setting rows (the setting's name and what it
//! does on the left, its control on the right, a divider between rows),
//! switches, segmented choices, framed combo boxes and buttons, folder
//! fields and the picture cards of the theme.
//!
//! Every control reports its role and label (`Response::widget_info`), so
//! the UI tests find a switch as a checkbox by the setting's name.

use crate::icons::{self, Icon};
use crate::theme::{self, PALETTE, Palette, ThemeChoice};
use egui::{Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder};
use egui::{pos2, vec2};
use std::path::Path;

/// Width of the sidebar of pages.
pub const SIDEBAR_WIDTH: f32 = 212.0;
/// Space around a page's content.
const PAGE_PAD_X: f32 = 28.0;
const PAGE_PAD_TOP: f32 = 22.0;
/// Space between a setting's text and its control.
const GAP: f32 = 24.0;
/// Space above and below a setting's text.
const ROW_PAD_Y: f32 = 11.0;
/// Width a control is first given, before its own width is known.
const CONTROL_GUESS: f32 = 160.0;
/// Corner radius of the dialog frame (see `dialog.rs`).
const DIALOG_RADIUS: u8 = 8;

/// Text sizes follow the font size preference.
fn size(points: f32) -> f32 {
    points * theme::font_scale()
}

/// Which side of a [`sidebar_layout`] is being drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Sidebar,
    Page,
}

/// A dialog body (of a `Dialog::flush` dialog) as a sidebar of pages under
/// `title` beside the page: `pane` draws the sidebar's items and then the
/// page, both `height` tall. The page is padded; a long page scrolls in a
/// [`page`].
pub fn sidebar_layout(ui: &mut Ui, title: &str, height: f32, mut pane: impl FnMut(&mut Ui, Pane)) {
    let rect = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), height));
    let side = Rect::from_min_max(rect.min, pos2(rect.left() + SIDEBAR_WIDTH, rect.bottom()));
    let radius = CornerRadius { nw: DIALOG_RADIUS, ..CornerRadius::ZERO };
    // The page on the content color, the sidebar a shade darker.
    let page_rect = Rect::from_min_max(pos2(side.right(), rect.top()), rect.max);
    ui.painter().rect_filled(page_rect, CornerRadius { ne: DIALOG_RADIUS, ..CornerRadius::ZERO }, PALETTE.canvas);
    ui.painter().rect_filled(side, radius, PALETTE.surface);
    ui.painter().vline(side.right(), side.y_range(), Stroke::new(1.0, PALETTE.border));
    let font = FontId::new(size(16.0), theme::semibold());
    ui.painter().text(pos2(side.left() + 20.0, side.top() + 26.0), Align2::LEFT_CENTER, title, font, PALETTE.text);

    let items = Rect::from_min_max(side.min + vec2(10.0, 52.0), side.max - vec2(10.0, 10.0));
    let mut sidebar = ui.new_child(UiBuilder::new().max_rect(items).layout(Layout::top_down(Align::Min)));
    sidebar.spacing_mut().item_spacing.y = 2.0;
    pane(&mut sidebar, Pane::Sidebar);

    let content = Rect::from_min_max(
        pos2(side.right() + PAGE_PAD_X, rect.top() + PAGE_PAD_TOP),
        pos2(rect.right() - PAGE_PAD_X / 2.0, rect.bottom()),
    );
    let mut page = ui.new_child(UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)));
    page.set_clip_rect(content.intersect(ui.clip_rect()));
    page.spacing_mut().item_spacing.y = 0.0;
    pane(&mut page, Pane::Page);
    ui.allocate_rect(rect, Sense::hover());
}

/// An item of the sidebar: its icon and name, the page shown highlighted.
pub fn nav_item(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), size(13.0) + 21.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 6.0, PALETTE.accent_soft);
    } else if r.hovered() {
        painter.rect_filled(rect, 6.0, PALETTE.hover);
    }
    let color = if selected { PALETTE.accent_text } else { PALETTE.text };
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(18.0, 18.0));
    icons::paint(painter, icon_rect, icon, if selected { color } else { PALETTE.text_muted });
    let font = if selected { FontId::new(size(13.0), theme::semibold()) } else { FontId::proportional(size(13.0)) };
    painter.text(pos2(rect.left() + 38.0, rect.center().y), Align2::LEFT_CENTER, label, font, color);
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label));
    r
}

/// The page: its title and what it holds, a divider, and `add` drawing the
/// page's settings in a scroll area (`scroll`: false when `add` scrolls a
/// part of the page itself, under a search field for instance).
pub fn page(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    description: &str,
    scroll: bool,
    add: impl FnOnce(&mut Ui),
) {
    let width = ui.available_width() - PAGE_PAD_X / 2.0;
    ui.add(egui::Label::new(RichText::new(title).family(theme::semibold()).size(size(19.0)).color(PALETTE.text)));
    ui.add_space(4.0);
    ui.allocate_ui(vec2(width, 0.0), |ui| {
        ui.set_width(width);
        ui.add(egui::Label::new(RichText::new(description).size(size(12.5)).color(PALETTE.text_muted)).wrap());
    });
    ui.add_space(14.0);
    let (line, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, PALETTE.border));
    if !scroll {
        ui.add_space(12.0);
        ui.allocate_ui(vec2(width, ui.available_height()), |ui| {
            ui.set_width(width);
            add(ui);
        });
        return;
    }
    egui::ScrollArea::vertical().id_salt(("settings-page", id)).auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui);
        ui.add_space(20.0);
    });
}

/// A group heading: small muted capitals with space above.
pub fn group(ui: &mut Ui, title: &str) {
    ui.add_space(22.0);
    let text = RichText::new(title.to_uppercase()).family(theme::semibold()).size(size(11.0)).color(PALETTE.text_faint);
    ui.add(egui::Label::new(text).selectable(false));
    ui.add_space(2.0);
}

/// A thin line across the page, between settings.
pub fn divider(ui: &mut Ui) {
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, PALETTE.border));
}

/// A setting: its name and what it does (wrapped) on the left, `control`
/// on the right (laid out right to left, centered on the text), then a
/// divider.
pub fn setting<R>(ui: &mut Ui, title: &str, description: Option<&str>, control: impl FnOnce(&mut Ui) -> R) -> R {
    let out = setting_row(ui, title, description, control);
    divider(ui);
    out
}

/// A [`setting`] without the divider under it: something more of the
/// setting follows (call [`divider`] after it).
pub fn setting_row<R>(ui: &mut Ui, title: &str, description: Option<&str>, control: impl FnOnce(&mut Ui) -> R) -> R {
    row(ui, title, description, ROW_PAD_Y, control)
}

/// A row of a long list (the commands and their keys): a [`setting`] with
/// less space around it.
pub fn list_row<R>(ui: &mut Ui, title: &str, control: impl FnOnce(&mut Ui) -> R) -> R {
    let out = row(ui, title, None, 4.0, control);
    divider(ui);
    out
}

fn row<R>(ui: &mut Ui, title: &str, description: Option<&str>, pad: f32, control: impl FnOnce(&mut Ui) -> R) -> R {
    let id = ui.id().with(("setting", title));
    let full = ui.available_width();
    // The control's width in the last frame leaves the rest to the text.
    let reserved = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(CONTROL_GUESS).min(full * 0.62);
    let text_width = (full - reserved - GAP).max(60.0);
    let painter = ui.painter();
    let name = painter.layout(title.to_string(), FontId::proportional(size(13.5)), PALETTE.text, text_width);
    let what = description
        .filter(|d| !d.is_empty())
        .map(|d| painter.layout(d.to_string(), FontId::proportional(size(12.0)), PALETTE.text_muted, text_width));
    let text_height = name.size().y + what.as_ref().map_or(0.0, |g| 3.0 + g.size().y);

    let top = ui.cursor().min;
    let band = text_height.max(28.0);
    let control_rect = Rect::from_min_size(pos2(top.x + full - reserved.max(1.0), top.y + pad), vec2(reserved, band));
    let right = Rect::from_min_max(pos2(top.x, control_rect.top()), pos2(top.x + full, control_rect.bottom()));
    let mut child = ui.new_child(UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)));
    let out = control(&mut child);
    let used = child.min_rect();
    let width = used.width().ceil();
    if (width - reserved).abs() > 1.0 && width.is_finite() {
        ui.data_mut(|d| d.insert_temp(id, width));
        ui.ctx().request_discard("setting control width");
    }
    let height = band.max(used.height()) + 2.0 * pad;
    let rect = Rect::from_min_size(top, vec2(full, height));
    let text_top = rect.center().y - text_height / 2.0;
    let mut y = text_top;
    let name_height = name.size().y;
    ui.painter().galley(pos2(top.x, y), name, PALETTE.text);
    y += name_height + 3.0;
    if let Some(what) = what {
        ui.painter().galley(pos2(top.x, y), what, PALETTE.text_muted);
    }
    ui.allocate_rect(rect, Sense::hover());
    out
}

/// A text under the settings, in small muted type (what a group does).
pub fn note(ui: &mut Ui, text: &str) {
    ui.add_space(8.0);
    ui.add(egui::Label::new(RichText::new(text).size(size(12.0)).color(PALETTE.text_muted)).wrap());
    ui.add_space(8.0);
}

/// An on/off switch for a setting named `label` (reported as a checkbox).
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> egui::Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(r.id, *on);
    let enabled = ui.is_enabled();
    let off_fill = if r.hovered() { PALETTE.border_strong } else { PALETTE.pressed };
    let mut fill = lerp_color(off_fill, PALETTE.accent, t);
    if !enabled {
        fill = fill.gamma_multiply(0.5);
    }
    let painter = ui.painter();
    painter.rect_filled(rect, rect.height() / 2.0, fill);
    let knob_x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, t);
    let knob = pos2(knob_x, rect.center().y);
    painter.circle_filled(knob + vec2(0.0, 0.5), 8.0, Color32::from_black_alpha(30));
    painter.circle_filled(knob, 8.0, if enabled { Color32::WHITE } else { Color32::from_gray(235) });
    if r.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            rect.height() / 2.0 + 2.0,
            Stroke::new(1.5, PALETTE.accent),
            egui::StrokeKind::Outside,
        );
    }
    let value = *on;
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, value, label));
    r
}

/// A [`setting`] turned on and off by a [`switch`].
pub fn switch_setting(ui: &mut Ui, title: &str, description: Option<&str>, on: &mut bool) -> egui::Response {
    setting(ui, title, description, |ui| switch(ui, on, title))
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Make the combo boxes, buttons and text fields drawn in `ui` framed and
/// filled like fields, as they stand on the page and not in a table.
pub fn framed(ui: &mut Ui) {
    let field = ui.visuals().extreme_bg_color;
    let w = &mut ui.visuals_mut().widgets;
    w.inactive.bg_stroke = Stroke::new(1.0, PALETTE.border_strong);
    w.inactive.weak_bg_fill = field;
    w.hovered.bg_stroke = Stroke::new(1.0, PALETTE.accent);
    w.hovered.weak_bg_fill = field;
    w.open.bg_stroke = Stroke::new(1.0, PALETTE.accent);
    w.open.weak_bg_fill = field;
}

/// A framed combo box `width` wide choosing among `options`.
pub fn combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    width: f32,
    value: &mut T,
    options: impl IntoIterator<Item = T>,
    label: impl Fn(T) -> String,
) -> bool {
    let before = *value;
    ui.scope(|ui| {
        framed(ui);
        ui.spacing_mut().interact_size.y = 28.0;
        egui::ComboBox::from_id_salt(("settings-combo", id)).width(width).selected_text(label(*value)).show_ui(
            ui,
            |ui| {
                for option in options {
                    ui.selectable_value(value, option, label(option));
                }
            },
        );
    });
    *value != before
}

/// A framed button.
pub fn button(ui: &mut Ui, enabled: bool, text: &str) -> egui::Response {
    ui.scope(|ui| {
        framed(ui);
        let w = &mut ui.visuals_mut().widgets;
        w.hovered.weak_bg_fill = PALETTE.hover;
        w.hovered.bg_stroke = Stroke::new(1.0, PALETTE.border_strong);
        ui.add_enabled(enabled, egui::Button::new(RichText::new(text).size(size(12.5))).min_size(vec2(0.0, 28.0)))
    })
    .inner
}

/// One option of a [`segmented`] control: what it shows, at what size, and
/// its name (its tooltip and accessible label when it shows something else).
pub struct Segment<T> {
    pub value: T,
    pub text: String,
    pub text_size: f32,
    pub name: String,
}

/// Options side by side in one pill, the chosen one raised.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: Vec<Segment<T>>) -> bool {
    let before = *value;
    let galleys: Vec<_> = options
        .iter()
        .map(|o| ui.painter().layout_no_wrap(o.text.clone(), FontId::proportional(o.text_size), PALETTE.text))
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| (g.size().x + 24.0).max(40.0)).collect();
    let total = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 32.0), Sense::hover());
    ui.painter().rect(rect, 7.0, PALETTE.surface, Stroke::new(1.0, PALETTE.border), egui::StrokeKind::Inside);
    let mut x = rect.left() + 3.0;
    for ((option, galley), width) in options.iter().zip(galleys).zip(widths) {
        let cell = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(width, rect.height() - 6.0));
        x += width;
        let r = ui.interact(cell, ui.id().with(("segment", &option.name)), Sense::click());
        let selected = *value == option.value;
        if r.clicked() {
            *value = option.value;
        }
        let painter = ui.painter();
        if selected {
            painter.rect_filled(cell.translate(vec2(0.0, 1.0)), 5.0, Color32::from_black_alpha(18));
            painter.rect_filled(cell, 5.0, if PALETTE.dark { PALETTE.pressed } else { Color32::WHITE });
        } else if r.hovered() {
            painter.rect_filled(cell, 5.0, PALETTE.hover);
        }
        let color = if selected { PALETTE.accent_text } else { PALETTE.text_muted };
        painter.galley(cell.center() - galley.size() / 2.0, galley, color);
        let name = option.name.clone();
        let r = if option.text != option.name { r.on_hover_text(&name) } else { r };
        r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name));
    }
    *value != before
}

/// The theme as three picture cards (light, dark, the system's), each a
/// small window drawn in that theme's colors, the chosen one outlined.
pub fn theme_cards(ui: &mut Ui, value: &mut ThemeChoice) {
    let order = [ThemeChoice::Light, ThemeChoice::Dark, ThemeChoice::System];
    let gap = 16.0;
    let width = ((ui.available_width() - 2.0 * gap) / 3.0).floor();
    let height = (width * 0.56).round();
    let label_height = size(13.0) + 12.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height + label_height + 4.0), Sense::hover());
    for (i, choice) in order.into_iter().enumerate() {
        let card =
            Rect::from_min_size(pos2(rect.left() + i as f32 * (width + gap), rect.top() + 2.0), vec2(width, height));
        let hit = Rect::from_min_max(card.min, pos2(card.right(), card.bottom() + label_height));
        let r = ui.interact(hit, ui.id().with(("theme-card", choice.code())), Sense::click());
        if r.clicked() {
            *value = choice;
        }
        let selected = *value == choice;
        let painter = ui.painter();
        match choice {
            ThemeChoice::Light => mini_window(painter, card, &theme::LIGHT),
            ThemeChoice::Dark => mini_window(painter, card, &theme::DARK),
            ThemeChoice::System => {
                mini_window(painter, card, &theme::LIGHT);
                let right = Rect::from_min_max(pos2(card.center().x, card.top()), card.max);
                mini_window(&painter.with_clip_rect(right.intersect(painter.clip_rect())), card, &theme::DARK);
            }
        }
        let outline = if selected {
            Stroke::new(2.0, PALETTE.accent)
        } else if r.hovered() {
            Stroke::new(1.0, PALETTE.border_strong)
        } else {
            Stroke::new(1.0, PALETTE.border)
        };
        painter.rect_stroke(card.expand(if selected { 2.0 } else { 0.0 }), 9.0, outline, egui::StrokeKind::Outside);
        if selected {
            let center = card.right_bottom() - vec2(16.0, 16.0);
            painter.circle_filled(center, 10.0, PALETTE.accent);
            let tick = Stroke::new(2.0, Color32::WHITE);
            painter.line_segment([center + vec2(-4.5, 0.0), center + vec2(-1.2, 3.3)], tick);
            painter.line_segment([center + vec2(-1.2, 3.3), center + vec2(4.8, -3.2)], tick);
        }
        let font = if selected { FontId::new(size(13.0), theme::semibold()) } else { FontId::proportional(size(13.0)) };
        let label = choice.label();
        painter.text(pos2(card.left() + 2.0, card.bottom() + 10.0), Align2::LEFT_TOP, &label, font, PALETTE.text);
        r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, &label));
    }
}

/// A small window in `p`'s colors: a title strip, a sidebar and lines of text.
fn mini_window(painter: &egui::Painter, rect: Rect, p: &Palette) {
    painter.rect_filled(rect, 8.0, p.app_bg);
    let window = rect.shrink2(vec2(rect.width() * 0.07, rect.height() * 0.1));
    painter.rect_filled(window, 5.0, p.canvas);
    let side = Rect::from_min_max(window.min, pos2(window.left() + window.width() * 0.3, window.bottom()));
    painter.rect_filled(side, CornerRadius { nw: 5, sw: 5, ..CornerRadius::ZERO }, p.panel);
    painter.vline(side.right(), side.y_range(), Stroke::new(1.0, p.border));
    for (k, color) in [p.text_faint, p.border_strong, p.border_strong].into_iter().enumerate() {
        painter.circle_filled(pos2(window.left() + 8.0 + k as f32 * 6.0, window.top() + 8.0), 2.0, color);
    }
    let bar = |painter: &egui::Painter, x: f32, y: f32, w: f32, color: Color32| {
        painter.rect_filled(Rect::from_min_size(pos2(x, y), vec2(w, 3.0)), 1.5, color);
    };
    let step = (window.height() - 22.0) / 6.0;
    for k in 0..5 {
        let y = window.top() + 20.0 + k as f32 * step;
        let w = side.width() - 14.0 - if k % 2 == 1 { 8.0 } else { 0.0 };
        bar(painter, side.left() + 7.0, y, w.max(4.0), if k == 1 { p.accent } else { p.border_strong });
    }
    let main = Rect::from_min_max(pos2(side.right() + 10.0, window.top() + 12.0), window.max - vec2(10.0, 8.0));
    painter.circle_filled(pos2(main.left() + 4.0, main.top() + 4.0), 4.0, p.border_strong);
    bar(painter, main.left() + 12.0, main.top() + 2.5, main.width() * 0.35, p.text_faint);
    for k in 0..4 {
        let y = main.top() + 16.0 + k as f32 * step;
        if y + 3.0 > main.bottom() {
            break;
        }
        let w = main.width() * [0.9, 0.75, 0.82, 0.6][k];
        bar(painter, main.left(), y, w, p.border);
    }
}

/// A folder as a field-like box: its icon and path (shortened to fit,
/// whole on hover), or `empty`; a folder that is gone is marked `missing`.
pub fn path_box(ui: &mut Ui, path: Option<&Path>, empty: &str, missing_text: &str) {
    let width = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(vec2(width, 30.0), Sense::hover());
    let missing = path.is_some_and(|p| !p.is_dir());
    let painter = ui.painter();
    painter.rect(
        rect,
        theme::RADIUS as f32,
        PALETTE.surface,
        Stroke::new(1.0, PALETTE.border),
        egui::StrokeKind::Inside,
    );
    let icon = Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(16.0, 16.0));
    let tone = if missing {
        PALETTE.warning
    } else if path.is_some() {
        PALETTE.text_muted
    } else {
        PALETTE.text_faint
    };
    icons::paint(painter, icon, Icon::Open, tone);
    let mut right = rect.right() - 10.0;
    if missing {
        let font = FontId::new(size(11.5), theme::semibold());
        let badge = painter.layout_no_wrap(missing_text.to_string(), font, PALETTE.warning);
        let at = pos2(right - badge.size().x, rect.center().y - badge.size().y / 2.0);
        right = at.x - 10.0;
        painter.galley(at, badge, PALETTE.warning);
    }
    let room = (right - rect.left() - 32.0).max(20.0);
    let (text, color) = match path {
        Some(p) => (p.display().to_string(), if missing { PALETTE.warning } else { PALETTE.text }),
        None => (empty.to_string(), PALETTE.text_faint),
    };
    let font = FontId::proportional(size(12.5));
    let shown = fit_middle(ui, &text, &font, room);
    painter.text(pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, shown, font, color);
    if path.is_some() {
        r.on_hover_text(text);
    }
}

/// `text` shortened in the middle (with an ellipsis) to fit `room` points in `font`.
fn fit_middle(ui: &Ui, text: &str, font: &FontId, room: f32) -> String {
    let fits = |s: &str| ui.painter().layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x <= room;
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    // Shorter and shorter until it fits: keep more of the end (the folder's own name).
    let mut keep = chars.len().saturating_sub(1);
    while keep > 4 {
        let tail = keep * 3 / 5;
        let head = keep - tail;
        let shortened: String =
            chars[..head].iter().chain(['…'].iter()).chain(chars[chars.len() - tail..].iter()).collect();
        if fits(&shortened) {
            return shortened;
        }
        keep -= (keep / 12).max(1);
    }
    format!("…{}", chars.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<String>())
}
