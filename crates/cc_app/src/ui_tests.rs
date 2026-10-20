//! Interface tests: the ribbon run headless with egui_kittest, queried
//! through its accessibility tree and rendered for review.

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

fn ribbon_harness(state: AppState, width: f32) -> Harness<'static, AppState> {
    ribbon_harness_sized(state, egui::vec2(width, 150.0))
}

fn ribbon_harness_sized(state: AppState, size: egui::Vec2) -> Harness<'static, AppState> {
    // The theme's fonts load on the frame after they are set, so the first
    // frame (run by the builder) only installs the theme.
    let mut themed = false;
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        move |ui, state: &mut AppState| {
            if !themed {
                crate::theme::apply(ui.ctx());
                themed = true;
                return;
            }
            crate::ribbon::ribbon(ui, state)
        },
        state,
    );
    harness.run();
    harness
}

/// `UI_SHOTS=dir cargo test -p cc_app ui_shots` writes every ribbon tab as a PNG for review.
#[test]
fn ui_shots() {
    let Ok(dir) = std::env::var("UI_SHOTS") else { return };
    let dir = std::path::Path::new(&dir);
    let tabs = [
        RibbonTab::File,
        RibbonTab::Architecture,
        RibbonTab::Structure,
        RibbonTab::Modify,
        RibbonTab::Annotate,
        RibbonTab::View,
        RibbonTab::Manage,
    ];
    for tab in tabs {
        let mut state = sample_state();
        state.ribbon_tab = tab;
        let mut harness = ribbon_harness(state, 1100.0);
        harness.render().unwrap().save(dir.join(format!("ribbon-{tab:?}.png"))).unwrap();
    }
    let mut state = sample_state();
    state.ribbon_tab = RibbonTab::Architecture;
    let mut harness = ribbon_harness(state, 520.0);
    harness.render().unwrap().save(dir.join("ribbon-narrow.png")).unwrap();
    let mut state = sample_state();
    state.ribbon_collapsed = true;
    state.ribbon_peek = true;
    state.ribbon_tab = RibbonTab::Annotate;
    let mut harness = ribbon_harness_sized(state, egui::vec2(900.0, 220.0));
    harness.render().unwrap().save(dir.join("ribbon-peek.png")).unwrap();
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
