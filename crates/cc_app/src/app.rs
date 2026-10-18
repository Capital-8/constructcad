//! Top-level eframe application: layout, dock and keyboard shortcuts.

use crate::i18n::Language;
use crate::state::{AppState, PendingAction};
use crate::theme::PALETTE;
use crate::tools::Tool;
use crate::{actions, browser, canvas, dialogs, properties, ribbon, schedule_ui, sheet, t, theme, view3d};
use cc_core::{Category, Document, Project, ViewCategory, ViewId};
use egui::{Event, Key, Modifiers, Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};

/// Command-line options.
#[derive(Debug, Default)]
pub struct Args {
    pub path: Option<std::path::PathBuf>,
    /// Views (by name or tab title) to open at startup.
    pub views: Vec<String>,
    pub language: Option<Language>,
}

/// eframe storage key of the UI language preference.
const LANGUAGE_KEY: &str = "language";

pub struct ConstructApp {
    state: AppState,
    dock: DockState<ViewId>,
    /// Document generation the dock layout belongs to.
    dock_generation: u64,
}

impl ConstructApp {
    /// Create the app; see [`Args`] for the startup options.
    pub fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        let Args { path, views, language } = args;
        crate::theme::apply(&cc.egui_ctx);
        // Command line, then saved preference, then the operating system language.
        let saved = cc.storage.and_then(|s| s.get_string(LANGUAGE_KEY)).and_then(|c| Language::from_code(&c));
        crate::i18n::set_language(language.or(saved).unwrap_or_else(Language::from_system));
        let mut state = AppState::new(Document::new(actions::new_project()));
        if let Some(path) = path {
            actions::open_path(&mut state, path);
        }
        let dock = initial_dock(state.project());
        let dock_generation = state.generation;
        for name in views {
            let by_title = state.project().views.keys().copied().find(|id| state.view_title(*id) == name);
            let found = by_title.or_else(|| state.project().views.values().find(|v| v.name == name).map(|v| v.id));
            match found {
                Some(id) => state.open_view(id),
                None => log::warn!("view not found: {name}"),
            }
        }
        Self { state, dock, dock_generation }
    }
}

/// First floor plan on the left, the 3D view on the right.
fn initial_dock(project: &Project) -> DockState<ViewId> {
    let plan = project.views_of(ViewCategory::FloorPlan).next().map(|v| v.id);
    let threed = project.views_of(ViewCategory::ThreeD).next().map(|v| v.id);
    let mut dock = DockState::new(plan.into_iter().collect());
    if let Some(t3) = threed {
        dock.main_surface_mut().split_right(NodeIndex::root(), 0.55, vec![t3]);
    }
    // Focus the plan so the properties palette shows it from the start.
    if let Some(path) = plan.and_then(|p| dock.find_tab(&p)) {
        dock.set_focused_node_and_surface(path.node_path());
    }
    dock
}

/// Zoom the active view to fit its contents.
pub fn zoom_fit(state: &mut AppState) {
    let Some(view) = state.active_view else { return };
    if let Some(v) = state.view2d.get_mut(&view) {
        v.fitted = false;
    }
    if let Some(v) = state.view3d.get_mut(&view) {
        v.fit_requested = true;
    }
}

struct Tabs<'a> {
    state: &'a mut AppState,
    render_state: Option<&'a egui_wgpu::RenderState>,
}

impl TabViewer for Tabs<'_> {
    type Tab = ViewId;

    fn id(&mut self, tab: &mut ViewId) -> egui::Id {
        egui::Id::new(("view-tab", tab.0))
    }

    fn title(&mut self, tab: &mut ViewId) -> WidgetText {
        self.state.view_title(*tab).into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut ViewId) {
        let Some(category) = self.state.project().view(*tab).map(|v| v.category()) else { return };
        match category {
            ViewCategory::ThreeD => view3d::show(ui, self.state, *tab, self.render_state),
            ViewCategory::Schedule => schedule_ui::show(ui, self.state, *tab),
            ViewCategory::Sheet => sheet::show(ui, self.state, *tab),
            _ => canvas::show(ui, self.state, *tab),
        }
    }

    fn on_close(&mut self, tab: &mut ViewId) -> OnCloseResponse {
        if let Some(v) = self.state.view3d.remove(tab) {
            self.state.textures_to_free.extend(v.texture);
        }
        OnCloseResponse::Close
    }

    fn scroll_bars(&self, _tab: &ViewId) -> [bool; 2] {
        [false, false]
    }
}

impl ConstructApp {
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let state = &mut self.state;
        let cmd = |i: &mut egui::InputState, k| i.consume_key(Modifiers::COMMAND, k);
        let cmd_shift = |i: &mut egui::InputState, k| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let (undo, redo, save, save_as, open, new) = ctx.input_mut(|i| {
            let redo = cmd(i, Key::Y) || cmd_shift(i, Key::Z);
            (cmd(i, Key::Z), redo, cmd(i, Key::S), cmd_shift(i, Key::S), cmd(i, Key::O), cmd(i, Key::N))
        });
        if undo && state.interaction.is_idle() {
            state.doc.undo();
        }
        if redo {
            state.doc.redo();
        }
        if save_as {
            actions::save(state, true);
        } else if save {
            actions::save(state, false);
        }
        if open {
            actions::guarded(state, PendingAction::Open);
        }
        if new {
            actions::guarded(state, PendingAction::New);
        }

        // Plain keys only when no text field has focus.
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let (escape, delete, backspace, events) = ctx.input(|i| {
            (i.key_pressed(Key::Escape), i.key_pressed(Key::Delete), i.key_pressed(Key::Backspace), i.events.clone())
        });
        if escape {
            if !state.length_input.is_empty() {
                state.length_input.clear();
            } else if !state.interaction.is_idle() {
                state.interaction = Default::default();
            } else if state.tool != Tool::Select {
                state.set_tool(Tool::Select);
            } else {
                state.selection.clear();
                state.sheet_selection = None;
            }
            state.key_buffer.clear();
        }
        if delete {
            match state.sheet_selection {
                Some((sheet, index)) if state.active_view == Some(sheet) => {
                    actions::remove_viewport(state, sheet, index)
                }
                _ => state.delete_selection(),
            }
        }
        let typing_length = matches!(state.tool, Tool::Wall { .. }) && state.interaction.anchor.is_some();
        if backspace && typing_length {
            state.length_input.pop();
        }
        for event in events {
            let Event::Text(text) = event else { continue };
            for ch in text.chars() {
                if typing_length && (ch.is_ascii_digit() || ch == ',' || ch == '.') {
                    state.length_input.push(ch);
                    continue;
                }
                if !ch.is_ascii_alphabetic() {
                    continue;
                }
                state.key_buffer.push(ch.to_ascii_uppercase());
                if state.key_buffer.len() > 2 {
                    state.key_buffer.remove(0);
                }
                let matched = match state.key_buffer.as_str() {
                    "WA" => Some(Some(Category::Wall)),
                    "DR" => Some(Some(Category::Door)),
                    "WN" => Some(Some(Category::Window)),
                    "SB" => Some(Some(Category::Floor)),
                    "FO" => Some(Some(Category::Ceiling)),
                    "TE" => Some(Some(Category::Roof)),
                    "MD" => {
                        state.set_tool(Tool::Select);
                        Some(None)
                    }
                    "CT" => {
                        state.set_tool(Tool::Section);
                        state.message = t!("msg-section-hint");
                        Some(None)
                    }
                    "ZE" => {
                        zoom_fit(state);
                        Some(None)
                    }
                    _ => None,
                };
                if let Some(category) = matched {
                    if let Some(c) = category {
                        actions::activate_category_tool(state, c);
                    }
                    state.key_buffer.clear();
                }
            }
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        let state = &mut self.state;
        if state.quit_confirmed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) && state.doc.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            state.dialogs.confirm = Some(PendingAction::Quit);
        }
    }

    fn sync_dock(&mut self) {
        let state = &mut self.state;
        // Drop tabs whose view no longer exists (deleted or undone).
        let project = &state.doc.project;
        let mut removed = Vec::new();
        self.dock.retain_tabs(|tab| {
            let keep = project.views.contains_key(tab);
            if !keep {
                removed.push(*tab);
            }
            keep
        });
        for tab in removed {
            if let Some(v) = state.view3d.remove(&tab) {
                state.textures_to_free.extend(v.texture);
            }
        }
        for id in std::mem::take(&mut state.open_requests) {
            if state.project().view(id).is_none() {
                continue;
            }
            match self.dock.find_tab(&id) {
                Some(path) => {
                    let _ = self.dock.set_active_tab(path);
                    self.dock.set_focused_node_and_surface(path.node_path());
                }
                None => self.dock.push_to_focused_leaf(id),
            }
        }
        state.active_view = self.dock.find_active_focused().map(|(_, tab)| *tab);
    }
}

impl eframe::App for ConstructApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(LANGUAGE_KEY, crate::i18n::language().code().to_string());
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.state.sync();
        if !self.state.textures_to_free.is_empty()
            && let Some(rs) = frame.wgpu_render_state()
        {
            let mut r = rs.renderer.write();
            for tex in self.state.textures_to_free.drain(..) {
                r.free_texture(&tex);
            }
        }
        self.shortcuts(&ctx);
        if self.dock_generation != self.state.generation {
            self.dock = initial_dock(self.state.project());
            self.dock_generation = self.state.generation;
        }
        self.handle_close(&ctx);
        self.sync_dock();

        let title = format!(
            "ConstructCAD — {}{}",
            self.state.project().info.name,
            if self.state.doc.is_dirty() { " *" } else { "" }
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        let state = &mut self.state;
        let bar = |fill, y| egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(10, y));
        egui::Panel::top("ribbon")
            .frame(bar(PALETTE.panel, 6).stroke(egui::Stroke::new(1.0, PALETTE.border)))
            .show(ui, |ui| ribbon::ribbon(ui, state));
        egui::Panel::top("options")
            .frame(bar(PALETTE.surface, 5).stroke(egui::Stroke::new(1.0, PALETTE.border)))
            .show(ui, |ui| ribbon::options_bar(ui, state));
        egui::Panel::bottom("status")
            .frame(bar(PALETTE.surface, 3).stroke(egui::Stroke::new(1.0, PALETTE.border)))
            .show(ui, |ui| ribbon::status_bar(ui, state));
        egui::Panel::left("left")
            .frame(egui::Frame::new().fill(PALETTE.panel).stroke(egui::Stroke::new(1.0, PALETTE.border)))
            .resizable(true)
            .default_size(290.0)
            .size_range(220.0..=520.0)
            .show(ui, |ui| {
                let h = ui.available_height();
                egui::Panel::top("browser")
                    .frame(egui::Frame::new().fill(PALETTE.panel))
                    .resizable(true)
                    .default_size(h * 0.5)
                    .size_range(120.0..=h - 120.0)
                    .show(ui, |ui| browser::show(ui, state));
                properties::show(ui, state);
            });
        let render_state = frame.wgpu_render_state();
        let central = egui::Frame::new().fill(PALETTE.app_bg).inner_margin(egui::Margin::same(4));
        egui::CentralPanel::no_frame().frame(central).show(ui, |ui| {
            if self.dock.iter_all_tabs().next().is_none() {
                ui.centered_and_justified(|ui| ui.label(t!("msg-no-views-open")));
            }
            DockArea::new(&mut self.dock)
                .style(theme::dock_style(ui.style()))
                .show_leaf_collapse_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_inside(ui, &mut Tabs { state: &mut self.state, render_state });
        });
        dialogs::show(&ctx, &mut self.state);
    }
}

