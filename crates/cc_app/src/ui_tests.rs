//! Interface tests: the ribbon, the command palette and the icons run
//! headless with egui_kittest, queried through the accessibility tree that
//! the widgets describe (`widget_info`), and compared with screenshots.

use crate::state::{AppState, RibbonTab};
use crate::t;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// The sample house, open in the editor.
fn sample_state() -> AppState {
    let project = crate::home::sample_project("casa_simples").unwrap();
    let mut state = AppState::new(cc_core::Document::new(project));
    state.project_open = true;
    state.show_home = false;
    state
}

/// A harness drawing `draw` with the app theme. The theme's fonts load on
/// the frame after they are set, so the first frame (run by the builder)
/// only installs it.
fn themed_harness(
    state: AppState,
    size: egui::Vec2,
    theme: egui::Theme,
    draw: fn(&mut egui::Ui, &mut AppState),
) -> Harness<'static, AppState> {
    let mut themed = false;
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        move |ui, state: &mut AppState| {
            if !themed {
                crate::theme::apply(ui.ctx());
                ui.ctx().set_theme(theme);
                themed = true;
                return;
            }
            draw(ui, state)
        },
        state,
    );
    harness.run();
    harness
}

fn ribbon_harness(state: AppState, width: f32) -> Harness<'static, AppState> {
    ribbon_harness_sized(state, egui::vec2(width, 150.0))
}

fn ribbon_harness_sized(state: AppState, size: egui::Vec2) -> Harness<'static, AppState> {
    themed_harness(state, size, egui::Theme::Light, crate::ribbon::ribbon)
}

/// The command palette over an empty window.
fn palette_harness(state: AppState) -> Harness<'static, AppState> {
    themed_harness(state, egui::vec2(800.0, 480.0), egui::Theme::Light, |ui, state| {
        crate::palette::show(ui.ctx(), state)
    })
}

/// Screenshot tests compare renders with the images in `tests/snapshots`.
/// GPUs differ slightly, so they run on request: `CC_UI_SNAPSHOTS=1 cargo
/// test -p cc_app snapshots` compares, `UPDATE_SNAPSHOTS=1` rewrites them
/// after an intended change (review the new images before committing).
fn snapshots_requested() -> bool {
    std::env::var_os("CC_UI_SNAPSHOTS").is_some() || std::env::var_os("UPDATE_SNAPSHOTS").is_some()
}

#[test]
fn ribbon_snapshots() {
    if !snapshots_requested() {
        return;
    }
    let mut results = egui_kittest::SnapshotResults::new();
    let tabs = [
        RibbonTab::File,
        RibbonTab::Architecture,
        RibbonTab::Structure,
        RibbonTab::Mep,
        RibbonTab::Modify,
        RibbonTab::Annotate,
        RibbonTab::View,
        RibbonTab::Manage,
    ];
    for tab in tabs {
        let mut state = sample_state();
        state.ribbon_tab = tab;
        let mut harness = ribbon_harness(state, 1100.0);
        results.add(harness.try_snapshot(format!("ribbon_{tab:?}").to_lowercase()));
    }
    // Narrow window: the groups scroll.
    let mut state = sample_state();
    state.ribbon_tab = RibbonTab::Architecture;
    results.add(ribbon_harness(state, 520.0).try_snapshot("ribbon_narrow"));
    // Collapsed, with a tab's panel shown over the views.
    let mut state = sample_state();
    state.ribbon_collapsed = true;
    state.ribbon_peek = true;
    state.ribbon_tab = RibbonTab::Annotate;
    results.add(ribbon_harness_sized(state, egui::vec2(900.0, 220.0)).try_snapshot("ribbon_peek"));
    // The rich tooltip of an unavailable command.
    let mut state = sample_state();
    state.ribbon_tab = RibbonTab::Modify;
    let mut harness = ribbon_harness_sized(state, egui::vec2(900.0, 300.0));
    harness.get_by_label(&t!("tool-move")).hover();
    harness.run();
    results.add(harness.try_snapshot("ribbon_tooltip"));
    // The palette searching.
    let mut state = sample_state();
    crate::palette::toggle(&mut state);
    let mut harness = palette_harness(state);
    harness.get_by_role(Role::TextInput).type_text("planta");
    harness.run();
    results.add(harness.try_snapshot("palette"));
    results.unwrap();
}

/// Every icon in both tones at the ribbon sizes, the badges, and the
/// enabled, selected and disabled states, in the light and dark themes.
fn icon_gallery(ui: &mut egui::Ui, _: &mut AppState) {
    use crate::icons::{self, Badge, Icon, Tones};
    let palette = crate::theme::palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, palette.panel);
    let per_row = 18;
    for (i, icon) in Icon::ALL.iter().enumerate() {
        let origin = rect.min + egui::vec2(8.0 + (i % per_row) as f32 * 52.0, 8.0 + (i / per_row) as f32 * 30.0);
        for (k, side) in [16.0, 22.0].into_iter().enumerate() {
            let r = egui::Rect::from_min_size(origin + egui::vec2(k as f32 * 22.0, 0.0), egui::vec2(side, side));
            icons::paint_toned(ui.painter(), r, *icon, Tones::of(palette, None, true, false), None);
        }
    }
    let top = rect.min.y + 8.0 + Icon::ALL.len().div_ceil(per_row) as f32 * 30.0 + 8.0;
    let badges = [None, Some(Badge::New), Some(Badge::Import), Some(Badge::Export), Some(Badge::Remove)];
    for (row, (enabled, selected)) in [(true, false), (true, true), (false, false)].into_iter().enumerate() {
        for (col, badge) in badges.into_iter().enumerate() {
            let r = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 8.0 + col as f32 * 36.0, top + row as f32 * 32.0),
                egui::vec2(24.0, 24.0),
            );
            if selected {
                ui.painter().rect_filled(r.expand(4.0), 4.0, palette.accent_soft);
            }
            let tones = Tones::of(palette, badge, enabled, selected);
            icons::paint_toned(ui.painter(), r, Icon::Component, tones, badge);
        }
    }
}

#[test]
fn icon_gallery_snapshots() {
    if !snapshots_requested() {
        return;
    }
    let mut results = egui_kittest::SnapshotResults::new();
    for (theme, name) in [(egui::Theme::Light, "icons_light"), (egui::Theme::Dark, "icons_dark")] {
        let mut harness = themed_harness(sample_state(), egui::vec2(950.0, 260.0), theme, icon_gallery);
        results.add(harness.try_snapshot(name));
    }
    results.unwrap();
}

/// A ribbon wider than the window scrolls with arrows at its ends.
#[test]
fn narrow_ribbon_scrolls() {
    let mut state = sample_state();
    state.ribbon_tab = RibbonTab::Architecture;
    let mut harness = ribbon_harness(state, 520.0);
    assert!(harness.get_by_label(&t!("ribbon-scroll-left")).accesskit_node().is_disabled(), "at the start");
    harness.get_by_label(&t!("ribbon-scroll-right")).click();
    harness.run();
    let left = t!("ribbon-scroll-left");
    assert!(!harness.get_by_label(&left).accesskit_node().is_disabled(), "scrolled away from the start");
    // Wide enough, no arrows.
    let mut state = sample_state();
    state.ribbon_tab = RibbonTab::Structure;
    let harness = ribbon_harness(state, 1100.0);
    assert!(harness.query_by_label(&t!("ribbon-scroll-right")).is_none());
}

/// Double-clicking a tab collapses the ribbon to its tabs; a click shows a
/// tab's panel over the views, and running a command closes it.
#[test]
fn ribbon_collapses_and_peeks() {
    let mut harness = ribbon_harness_sized(sample_state(), egui::vec2(1100.0, 220.0));
    let tab = t!("ribbon-annotate");
    harness.get_by_label(&tab).click();
    harness.run();
    assert_eq!(harness.state().ribbon_tab, RibbonTab::Annotate);
    // The harness clock runs faster than a hand, so allow a slow double click.
    harness.ctx.options_mut(|o| o.input_options.max_double_click_delay = 60.0);
    harness.get_by_label(&tab).click();
    harness.run();
    assert!(harness.state().ribbon_collapsed, "double click collapses");
    assert!(harness.query_by_role_and_label(Role::Button, &t!("tool-text")).is_none(), "no panel while collapsed");
    harness.get_by_label(&tab).click();
    harness.run();
    assert!(harness.state().ribbon_peek);
    harness.get_by_role_and_label(Role::Button, &t!("tool-text")).click();
    harness.run();
    assert!(matches!(harness.state().tool, crate::tools::Tool::Text { .. }));
    assert!(!harness.state().ribbon_peek, "the panel closes after a command");
    assert!(harness.state().ribbon_collapsed);
}

/// Typing in the palette finds commands by name, accents aside, and Enter runs the first.
#[test]
fn palette_runs_commands() {
    let mut state = sample_state();
    crate::palette::toggle(&mut state);
    let mut harness = palette_harness(state);
    harness.get_by_role(Role::TextInput).type_text("parede");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(matches!(harness.state().tool, crate::tools::Tool::Wall { .. }));
    assert!(harness.state().palette.is_none(), "the palette closes after running");
    // A two-letter shortcut finds its command too; unavailable ones explain why.
    crate::palette::toggle(harness.state_mut());
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("MV");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().message, t!("msg-select-first"));
}

/// The palette button opens the palette (its own click used to close it in
/// the same frame); a click outside closes it.
#[test]
fn palette_button_opens_the_palette() {
    let mut harness = themed_harness(sample_state(), egui::vec2(1400.0, 480.0), egui::Theme::Light, |ui, state| {
        crate::ribbon::ribbon(ui, state);
        crate::palette::show(ui.ctx(), state);
    });
    harness.get_by_role_and_label(Role::Button, &t!("action-command-palette")).click();
    harness.run();
    assert!(harness.state().palette.is_some(), "the button opens the palette");
    assert!(harness.query_by_role(Role::TextInput).is_some());
    harness.get_by_role_and_label(Role::Button, &t!("action-command-palette")).click();
    harness.run();
    assert!(harness.state().palette.is_none(), "a click outside closes it");
}

/// Engine commands open a form with their arguments and run in the session.
#[test]
fn palette_runs_engine_commands() {
    let mut state = sample_state();
    let levels = state.project().levels.len();
    crate::palette::toggle(&mut state);
    let mut harness = palette_harness(state);
    harness.get_by_role(Role::TextInput).type_text("level_create");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    let editor = harness.get_by_role(Role::MultilineTextInput);
    editor.focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_role(Role::MultilineTextInput).type_text(r#"{"elevation": 9.0, "name": "Roof deck"}"#);
    harness.run();
    harness.get_by_role_and_label(Role::Button, &t!("palette-run")).click();
    harness.run();
    assert_eq!(harness.state().project().levels.len(), levels + 1);
    assert!(harness.state().project().levels.iter().any(|l| l.name == "Roof deck"));
}

/// Every ribbon button tells assistive technology its name, and every
/// interface command shown in a tab can be found by its label.
#[test]
fn ribbon_buttons_are_accessible() {
    use crate::commands::UiCommand;
    let tabs = [
        RibbonTab::File,
        RibbonTab::Architecture,
        RibbonTab::Structure,
        RibbonTab::Mep,
        RibbonTab::Modify,
        RibbonTab::Annotate,
        RibbonTab::View,
        RibbonTab::Manage,
    ];
    let mut found = std::collections::HashSet::new();
    for tab in tabs {
        let mut state = sample_state();
        state.ribbon_tab = tab;
        let harness = ribbon_harness(state, 1600.0);
        for node in harness.query_all_by_role(Role::Button) {
            let label = node.accesskit_node().label().unwrap_or_default();
            assert!(!label.trim().is_empty(), "{tab:?}: a button without a name");
            found.insert(label);
        }
    }
    let state = sample_state();
    let hidden = [UiCommand::CommandPalette, UiCommand::ToggleRibbon];
    for command in UiCommand::all().into_iter().filter(|c| !hidden.contains(c)) {
        assert!(found.contains(&command.label(&state)), "{command:?} is not in the ribbon");
    }
}
