//! Interface tests: the ribbon run headless with egui_kittest, queried
//! through its accessibility tree and rendered for review.

use crate::state::{AppState, RibbonTab};
use egui_kittest::Harness;

/// The sample house, open in the editor.
fn sample_state() -> AppState {
    let project = crate::home::sample_project("casa_simples").unwrap();
    let mut state = AppState::new(cc_core::Document::new(project));
    state.project_open = true;
    state.show_home = false;
    state
}

fn ribbon_harness(state: AppState, width: f32) -> Harness<'static, AppState> {
    // The theme's fonts load on the frame after they are set, so the first
    // frame (run by the builder) only installs the theme.
    let mut themed = false;
    let mut harness = Harness::builder().with_size(egui::vec2(width, 150.0)).build_ui_state(
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
        let image = harness.render().unwrap();
        image.save(std::path::Path::new(&dir).join(format!("ribbon-{tab:?}.png"))).unwrap();
    }
}
