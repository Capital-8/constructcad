//! The File tab as a page over the window (a backstage): the project
//! commands down the left, and the recent projects or the export formats on
//! the right. It closes with Esc, its back arrow, another tab or a command run.

use egui::{Layout, RichText, Sense, Stroke, Ui, vec2};

use crate::actions;
use crate::commands::{self, UiCommand};
use crate::icons;
use crate::state::{AppState, PendingAction, RibbonTab};
use crate::t;
use crate::theme::{self, PALETTE};

/// The page shown on the right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Recent,
    Export,
}

/// Temp-data key of the page shown.
fn page_id() -> egui::Id {
    egui::Id::new("backstage-page")
}

/// Temp-data key of the tab shown before File, shown again when it closes.
fn before_id() -> egui::Id {
    egui::Id::new("backstage-before")
}

/// File was picked while `before` was the tab: remember it, and start on the recent projects.
pub fn opened(ctx: &egui::Context, before: RibbonTab) {
    ctx.data_mut(|d| {
        d.insert_temp(before_id(), before);
        d.insert_temp(page_id(), Page::Recent);
    });
}

/// Leave the page for the tab shown before it.
fn close(ctx: &egui::Context, state: &mut AppState) {
    let before = ctx.data(|d| d.get_temp::<RibbonTab>(before_id())).unwrap_or_default();
    state.ribbon_tab = if before == RibbonTab::File { RibbonTab::default() } else { before };
}

/// What a click on the page asks for, done once it is drawn.
enum Action {
    Run(UiCommand),
    Page(Page),
    Open(std::path::PathBuf),
    Close,
}

/// The page, from `top` (under the tab titles) to the bottom of the window.
pub fn show(ctx: &egui::Context, state: &mut AppState, top: f32) {
    let screen = ctx.content_rect();
    let rect = egui::Rect::from_min_max(egui::pos2(screen.left(), top), screen.max);
    let page = ctx.data(|d| d.get_temp::<Page>(page_id())).unwrap_or_default();
    let mut action = None;
    egui::Area::new(egui::Id::new("backstage")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        ui.set_min_size(rect.size());
        ui.painter().rect_filled(rect, 0.0, PALETTE.surface);
        let side = egui::Rect::from_min_size(rect.min, vec2(SIDE_WIDTH, rect.height()));
        ui.painter().rect_filled(side, 0.0, PALETTE.panel);
        ui.painter().vline(side.right(), side.y_range(), Stroke::new(1.0, PALETTE.border));
        ui.scope_builder(egui::UiBuilder::new().max_rect(side.shrink2(vec2(12.0, 16.0))), |ui| {
            action = side_column(ui, state, page).or(action.take());
        });
        let body = egui::Rect::from_min_max(egui::pos2(side.right() + 40.0, rect.top() + 28.0), rect.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(body.shrink2(vec2(0.0, 0.0))), |ui| {
            let picked = match page {
                Page::Recent => recent_page(ui, state),
                Page::Export => export_page(ui, state),
            };
            action = picked.or(action.take());
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = Some(Action::Close);
    }
    match action {
        Some(Action::Page(page)) => ctx.data_mut(|d| {
            d.insert_temp(page_id(), page);
        }),
        Some(Action::Close) => close(ctx, state),
        Some(Action::Run(command)) => {
            close(ctx, state);
            commands::run(state, ctx, command);
        }
        Some(Action::Open(path)) => {
            close(ctx, state);
            actions::guarded(state, PendingAction::OpenPath(path));
        }
        None => {}
    }
}

/// Width of the commands' column.
const SIDE_WIDTH: f32 = 240.0;

/// The back arrow, the project commands, the Export page, settings and help.
fn side_column(ui: &mut Ui, state: &AppState, page: Page) -> Option<Action> {
    use UiCommand::*;
    let mut action = None;
    ui.spacing_mut().item_spacing.y = 2.0;
    let back = t!("backstage-back");
    let r = row(ui, icons::Icon::ArrowLeft, &back, true, false);
    if r.on_hover_text(&back).clicked() {
        action = Some(Action::Close);
    }
    ui.add_space(12.0);
    let mut commands = |ui: &mut Ui, list: &[UiCommand]| {
        for command in list {
            let why = command.unavailable(state);
            let r = row(ui, command.icon(), &command.label(state), why.is_none(), false);
            if crate::ribbon::command_tooltip(r, state, *command, why.clone()).clicked() && why.is_none() {
                action = Some(Action::Run(*command));
            }
        }
    };
    commands(ui, &[New, Open, Save, SaveAs]);
    let recent = t!("home-recent");
    if row(ui, icons::Icon::Open, &recent, true, page == Page::Recent).clicked() {
        action = Some(Action::Page(Page::Recent));
    }
    let export = t!("group-export");
    if row(ui, icons::Icon::Export, &export, true, page == Page::Export).clicked() {
        action = Some(Action::Page(Page::Export));
    }
    let mut commands = |ui: &mut Ui, list: &[UiCommand]| {
        for command in list {
            let why = command.unavailable(state);
            let r = row(ui, command.icon(), &command.label(state), why.is_none(), false);
            if crate::ribbon::command_tooltip(r, state, *command, why.clone()).clicked() && why.is_none() {
                action = Some(Action::Run(*command));
            }
        }
    };
    commands(ui, &[CloseProject]);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    commands(ui, &[Preferences, About, CheckUpdates]);
    action
}

/// One row of the side column: an icon and a label; `current` marks the page shown.
fn row(ui: &mut Ui, icon: icons::Icon, label: &str, enabled: bool, current: bool) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), sense);
    let painter = ui.painter();
    if current {
        painter.rect_filled(rect, theme::RADIUS as f32, PALETTE.accent_soft);
    } else if enabled && r.hovered() {
        painter.rect_filled(rect, theme::RADIUS as f32, PALETTE.hover);
    }
    let color = match (enabled, current) {
        (false, _) => PALETTE.text_faint,
        (true, true) => PALETTE.accent_text,
        (true, false) => PALETTE.text,
    };
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), vec2(18.0, 18.0));
    icons::paint(painter, icon_rect, icon, color);
    let font = egui::FontId::proportional(14.0);
    painter.text(egui::pos2(rect.left() + 40.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font, color);
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, current, label));
    if enabled { r.on_hover_cursor(egui::CursorIcon::PointingHand) } else { r }
}

/// The page's title.
fn title(ui: &mut Ui, text: String) {
    ui.label(RichText::new(text).family(theme::semibold()).size(20.0).color(PALETTE.text));
    ui.add_space(16.0);
}

/// The recent projects, newest first: name and folder; a click opens one.
fn recent_page(ui: &mut Ui, state: &AppState) -> Option<Action> {
    title(ui, t!("home-recent"));
    if state.recent.is_empty() {
        ui.label(RichText::new(t!("home-empty")).color(PALETTE.text_faint));
        return None;
    }
    let mut action = None;
    egui::ScrollArea::vertical().id_salt("backstage-recent").show(ui, |ui| {
        ui.set_max_width(640.0);
        for path in &state.recent {
            let exists = path.exists();
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let folder = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
            let sense = if exists { Sense::click() } else { Sense::hover() };
            let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), sense);
            if exists && r.hovered() {
                ui.painter().rect_filled(rect, theme::RADIUS as f32, PALETTE.hover);
            }
            let color = if exists { PALETTE.text } else { PALETTE.text_faint };
            let font = egui::FontId::proportional(14.0);
            let left = rect.left() + 10.0;
            ui.painter().text(egui::pos2(left, rect.top() + 13.0), egui::Align2::LEFT_CENTER, &name, font, color);
            let small = egui::FontId::proportional(11.5);
            let at = egui::pos2(left, rect.bottom() - 12.0);
            ui.painter().text(at, egui::Align2::LEFT_CENTER, &folder, small, PALETTE.text_faint);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, exists, &name));
            let tip = if exists { path.display().to_string() } else { t!("home-missing") };
            if r.on_hover_text(tip).clicked() {
                action = Some(Action::Open(path.clone()));
            }
        }
    });
    action
}

/// The export formats: icon, name and what each writes (greyed with the
/// reason when the active view cannot be written so), then the plugins' formats.
fn export_page(ui: &mut Ui, state: &mut AppState) -> Option<Action> {
    use UiCommand::*;
    title(ui, t!("group-export"));
    let mut action = None;
    ui.set_max_width(640.0);
    for command in [ExportIfc, ExportGltf, ExportPdf, ExportSvg, ExportImage, ExportDxf] {
        let why = command.unavailable(state);
        let enabled = why.is_none();
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), sense);
        if enabled && r.hovered() {
            ui.painter().rect_filled(rect, theme::RADIUS as f32, PALETTE.hover);
        }
        let color = if enabled { PALETTE.text } else { PALETTE.text_faint };
        let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 22.0, rect.center().y), vec2(24.0, 24.0));
        let tones = icons::Tones::of(theme::palette(ui.ctx()), command.badge(), enabled, false);
        icons::paint_toned(ui.painter(), icon_rect, command.icon(), tones, command.badge());
        let label = command.label(state);
        let font = egui::FontId::proportional(14.0);
        let left = rect.left() + 48.0;
        ui.painter().text(egui::pos2(left, rect.top() + 16.0), egui::Align2::LEFT_CENTER, &label, font, color);
        let detail = why.clone().unwrap_or_else(|| command.description());
        let small = egui::FontId::proportional(11.5);
        let detail_color = if enabled { PALETTE.text_muted } else { PALETTE.warning };
        let galley = ui.painter().layout(detail, small, detail_color, rect.width() - 56.0);
        ui.painter().galley(egui::pos2(left, rect.top() + 26.0), galley, detail_color);
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        if r.clicked() && enabled {
            action = Some(Action::Run(command));
        }
    }
    // A plugin's export formats, as its buttons.
    ui.add_space(12.0);
    ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| crate::ribbon::plugin_file_entries(ui, state, true));
    action
}
