//! Visual theme: color tokens, typography and spacing applied to egui.
//!
//! Colors are defined once in [`Palette`] and every panel reads them from
//! [`PALETTE`] instead of hard-coding values, so the look stays consistent.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText, Shadow, Stroke};
use egui::{TextStyle, Visuals};
use std::sync::Arc;

/// Color tokens of the light neutral theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Application background behind panels (ribbon strip, dock gaps).
    pub app_bg: Color32,
    /// Side panels, ribbon and popups.
    pub panel: Color32,
    /// Section headers and subtle fills inside panels.
    pub surface: Color32,
    /// Drawing canvas (paper).
    pub canvas: Color32,
    /// Area around a sheet.
    pub sheet_bg: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_text: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub danger: Color32,
    pub warning: Color32,
}

pub const PALETTE: Palette = Palette {
    app_bg: Color32::from_rgb(0xE9, 0xEB, 0xEF),
    panel: Color32::from_rgb(0xF7, 0xF8, 0xFA),
    surface: Color32::from_rgb(0xEF, 0xF1, 0xF4),
    canvas: Color32::WHITE,
    sheet_bg: Color32::from_rgb(0xC9, 0xCD, 0xD3),
    border: Color32::from_rgb(0xD9, 0xDC, 0xE1),
    border_strong: Color32::from_rgb(0xB8, 0xBE, 0xC6),
    text: Color32::from_rgb(0x1F, 0x23, 0x28),
    text_muted: Color32::from_rgb(0x5B, 0x63, 0x6E),
    text_faint: Color32::from_rgb(0x8A, 0x92, 0x9C),
    accent: Color32::from_rgb(0x1F, 0x6F, 0xEB),
    accent_soft: Color32::from_rgb(0xDC, 0xE9, 0xFD),
    accent_text: Color32::from_rgb(0x12, 0x4F, 0xB8),
    hover: Color32::from_rgb(0xE6, 0xE9, 0xED),
    pressed: Color32::from_rgb(0xD6, 0xDB, 0xE1),
    danger: Color32::from_rgb(0xC9, 0x2A, 0x2A),
    warning: Color32::from_rgb(0xB2, 0x6B, 0x00),
};

/// Corner radius of buttons, fields and popups.
pub const RADIUS: u8 = 5;

/// Font family of the semibold weight, for titles and emphasis.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Section or panel title text.
pub fn title(text: impl Into<String>) -> RichText {
    RichText::new(text).family(semibold()).size(13.0).color(PALETTE.text)
}

/// Full-width header strip at the top of a side panel.
pub fn panel_header(ui: &mut egui::Ui, title_text: &str, subtitle: Option<&str>) {
    let response = egui::Frame::new().fill(PALETTE.surface).inner_margin(Margin::symmetric(12, 7)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(title(title_text));
            if let Some(sub) = subtitle.filter(|s| !s.is_empty()) {
                ui.label(RichText::new(sub).size(12.0).color(PALETTE.text_faint));
            }
        });
    });
    let r = response.response.rect;
    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, PALETTE.border));
}

/// Small uppercase-free caption, used for group labels.
pub fn caption(text: impl Into<String>) -> RichText {
    RichText::new(text).size(11.0).color(PALETTE.text_faint)
}

/// Secondary text.
pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(PALETTE.text_muted)
}

/// Install fonts, text styles, spacing and colors.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.visuals = visuals();
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, FontId::proportional(13.5)),
            (TextStyle::Heading, FontId::new(17.0, semibold())),
            (TextStyle::Monospace, FontId::monospace(12.5)),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(8.0, 6.0);
        s.button_padding = egui::vec2(8.0, 4.0);
        s.interact_size = egui::vec2(36.0, 24.0);
        s.menu_margin = Margin::same(6);
        s.window_margin = Margin::same(12);
        s.indent = 16.0;
        s.combo_height = 320.0;
    });
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".into(),
        Arc::new(FontData::from_static(include_bytes!("../../../assets/fonts/Inter-Regular.ttf"))),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        Arc::new(FontData::from_static(include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"))),
    );
    // Inter first; egui's bundled fonts stay as fallbacks for symbols and emoji.
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "inter".into());
    let fallbacks: Vec<String> = proportional[1..].to_vec();
    let mut semibold_family = vec!["inter-semibold".to_string()];
    semibold_family.extend(fallbacks);
    fonts.families.insert(semibold(), semibold_family);
    fonts
}

fn visuals() -> Visuals {
    let p = PALETTE;
    let r = CornerRadius::same(RADIUS);
    let mut v = Visuals::light();
    v.override_text_color = None;
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(36) };
    v.popup_shadow = Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(30) };
    v.faint_bg_color = p.surface;
    v.extreme_bg_color = Color32::WHITE;
    v.code_bg_color = p.surface;
    v.hyperlink_color = p.accent_text;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent_text);
    v.striped = true;
    v.indent_has_left_vline = false;
    v.collapsing_header_frame = false;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;

    w.inactive.bg_fill = Color32::WHITE;
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;

    w.hovered.bg_fill = p.hover;
    w.hovered.weak_bg_fill = p.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.border_strong);
    w.hovered.fg_stroke = Stroke::new(1.0, p.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.pressed;
    w.active.weak_bg_fill = p.pressed;
    w.active.bg_stroke = Stroke::new(1.0, p.border_strong);
    w.active.fg_stroke = Stroke::new(1.0, p.text);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;

    w.open.bg_fill = p.hover;
    w.open.weak_bg_fill = p.hover;
    w.open.bg_stroke = Stroke::new(1.0, p.border_strong);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = r;
    v
}

/// Dock (view tabs) style matching the theme.
pub fn dock_style(style: &egui::Style) -> egui_dock::Style {
    let p = PALETTE;
    let mut s = egui_dock::Style::from_egui(style);
    s.main_surface_border_stroke = Stroke::NONE;
    s.separator.width = 4.0;
    s.separator.color_idle = p.app_bg;
    s.separator.color_hovered = p.border_strong;
    s.separator.color_dragged = p.accent;
    s.tab_bar.bg_fill = p.surface;
    s.tab_bar.height = 30.0;
    s.tab_bar.hline_color = p.border;
    s.tab_bar.corner_radius = CornerRadius::ZERO;
    s.tab.spacing = 2.0;
    s.tab.minimum_width = Some(90.0);
    s.tab.hline_below_active_tab_name = false;
    let tab_radius = CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 };
    let tab = |bg: Color32, text: Color32, outline: Color32| egui_dock::TabInteractionStyle {
        outline_color: outline,
        corner_radius: tab_radius,
        bg_fill: bg,
        text_color: text,
    };
    s.tab.inactive = tab(p.surface, p.text_muted, Color32::TRANSPARENT);
    s.tab.inactive_with_kb_focus = s.tab.inactive.clone();
    s.tab.hovered = tab(p.hover, p.text, Color32::TRANSPARENT);
    s.tab.active = tab(p.canvas, p.text, p.border);
    s.tab.active_with_kb_focus = s.tab.active.clone();
    s.tab.focused = tab(p.canvas, p.accent_text, p.border);
    s.tab.focused_with_kb_focus = s.tab.focused.clone();
    s.tab.tab_body.bg_fill = p.canvas;
    s.tab.tab_body.stroke = Stroke::new(1.0, p.border);
    s.tab.tab_body.corner_radius = CornerRadius::ZERO;
    s.tab.tab_body.inner_margin = Margin::ZERO;
    s.buttons.close_tab_color = p.text_faint;
    s.buttons.close_tab_active_color = p.text;
    s.buttons.close_tab_bg_fill = p.pressed;
    s.overlay.selection_color = p.accent.gamma_multiply(0.35);
    s
}
