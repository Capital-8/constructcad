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

/// The wall tool switches between straight and arc walls in the options bar.
#[test]
fn wall_tool_has_an_arc_mode() {
    let mut state = sample_state();
    let wall_type = state.project().default_type(cc_core::Category::Wall).unwrap();
    state.tool = crate::tools::Tool::Wall { type_id: wall_type };
    let mut harness = themed_harness(state, egui::vec2(1600.0, 60.0), egui::Theme::Light, |ui, state| {
        crate::ribbon::options_bar(ui, state)
    });
    assert!(!harness.state().wall_arc);
    harness.get_by_label(&t!("wall-mode-arc")).click();
    harness.run();
    assert!(harness.state().wall_arc);
    harness.get_by_label(&t!("wall-mode-line")).click();
    harness.run();
    assert!(!harness.state().wall_arc);
}

/// A roof gets an opening (a courtyard) sketched inside it; one outside is refused.
#[test]
fn roof_openings_are_added_inside_the_roof() {
    let mut state = sample_state();
    let (roof, footprint) = state
        .project()
        .elements
        .values()
        .find_map(|e| match &e.kind {
            cc_core::ElementKind::Roof(r) => Some((e.id, r.footprint.clone())),
            _ => None,
        })
        .unwrap();
    let lo = footprint.iter().fold(glam::DVec2::splat(f64::MAX), |a, p| a.min(*p));
    let hi = footprint.iter().fold(glam::DVec2::splat(f64::MIN), |a, p| a.max(*p));
    let c = (lo + hi) * 0.5;
    let square = |c: glam::DVec2| {
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|&(x, y)| c + glam::DVec2::new(x, y)).collect()
    };
    let holes = |state: &AppState| match &state.project().element(roof).unwrap().kind {
        cc_core::ElementKind::Roof(r) => r.holes.len(),
        _ => 0,
    };
    state.set_tool(crate::tools::Tool::RoofOpening { roof });
    crate::tools::add_roof_opening(&mut state, roof, square(hi + glam::DVec2::splat(5.0)));
    assert_eq!(holes(&state), 0, "outside the roof");
    crate::tools::add_roof_opening(&mut state, roof, square(c));
    assert_eq!(holes(&state), 1);
    assert_eq!(state.tool, crate::tools::Tool::Select);
    state.sync();
    assert!(state.session.geom().roof_ridges.contains_key(&roof));
}

/// Visibility/Graphics of the ground plan, with a filter applied.
fn graphics_state() -> (AppState, cc_core::ViewId) {
    let mut state = sample_state();
    let plan = state.project().views_of(cc_core::ViewCategory::FloorPlan).next().unwrap().id;
    state.active_view = Some(plan);
    let rules = vec![cc_engine::commands::graphics::RuleArg {
        param: "length".into(),
        op: "greater".into(),
        value: Some(serde_json::json!(5.0)),
    }];
    let filter = cc_engine::commands::graphics::FilterCreate {
        name: "Long walls".into(),
        categories: vec!["wall".into()],
        rules,
        any: false,
    };
    state.session.run(filter).unwrap();
    let add = cc_engine::commands::graphics::ViewFilterAdd {
        view: Some(cc_engine::Ref::Id(plan.0)),
        filter: cc_engine::Ref::Name("Long walls".into()),
        overrides: Some(serde_json::json!({"cut_lines": {"color": [200, 0, 0], "weight": 0.7}})),
        ..Default::default()
    };
    state.session.run(add).unwrap();
    (state, plan)
}

#[test]
fn graphics_dialog_hides_categories_and_cancel_undoes() {
    let (mut state, plan) = graphics_state();
    let depth = state.session.doc.undo_depth();
    crate::graphics_dialog::open(&mut state, crate::graphics_dialog::Target::View(plan));
    let mut harness = themed_harness(state, egui::vec2(980.0, 680.0), egui::Theme::Light, |ui, state| {
        crate::graphics_dialog::show(ui.ctx(), state)
    });
    let walls = format!("{}: {}", t!("vg-visible"), crate::actions::t_category(cc_core::Category::Wall));
    harness.get_by_label(&walls).click_accesskit();
    harness.run();
    let hidden = |s: &AppState| !s.project().view(plan).unwrap().graphics.category(cc_core::Category::Wall).visible;
    assert!(hidden(harness.state()));
    harness.get_by_label(&t!("action-cancel")).click();
    harness.run();
    assert!(!hidden(harness.state()));
    assert_eq!(harness.state().session.doc.undo_depth(), depth);
    assert!(harness.state().dialogs.graphics.is_none());
}

/// The filters and view templates dialogs fit a tall window: their header
/// and buttons stay on screen (a divider once stretched them past it).
#[test]
fn list_dialogs_fit_a_tall_window() {
    let size = egui::vec2(1600.0, 1400.0);
    let (mut state, _) = graphics_state();
    crate::graphics_dialog::open_filters(&mut state);
    let mut filters =
        themed_harness(state, size, egui::Theme::Light, |ui, state| crate::graphics_dialog::show(ui.ctx(), state));
    let (mut state, plan) = graphics_state();
    let create = cc_engine::commands::graphics::TemplateCreate {
        name: "Working plan".into(),
        view: Some(cc_engine::Ref::Id(plan.0)),
    };
    state.session.run(create).unwrap();
    crate::graphics_dialog::open_templates(&mut state);
    let mut templates =
        themed_harness(state, size, egui::Theme::Light, |ui, state| crate::graphics_dialog::show(ui.ctx(), state));
    // Empty lists: nothing selected.
    let mut state = sample_state();
    crate::graphics_dialog::open_filters(&mut state);
    let mut no_filters =
        themed_harness(state, size, egui::Theme::Light, |ui, state| crate::graphics_dialog::show(ui.ctx(), state));
    let mut state = sample_state();
    crate::graphics_dialog::open_templates(&mut state);
    let mut no_templates =
        themed_harness(state, size, egui::Theme::Light, |ui, state| crate::graphics_dialog::show(ui.ctx(), state));
    // The other dialogs with lists and tabs.
    let (mut state, plan) = graphics_state();
    crate::graphics_dialog::open(&mut state, crate::graphics_dialog::Target::View(plan));
    let mut graphics =
        themed_harness(state, size, egui::Theme::Light, |ui, state| crate::graphics_dialog::show(ui.ctx(), state));
    let mut state = sample_state();
    crate::phases_dialog::open(&mut state);
    let mut phases =
        themed_harness(state, size, egui::Theme::Light, |ui, state| crate::phases_dialog::show(ui.ctx(), state));
    for harness in [&mut filters, &mut templates, &mut no_filters, &mut no_templates, &mut graphics, &mut phases] {
        // The dialog once grew a little every frame, until it left the screen.
        for _ in 0..40 {
            harness.step();
        }
        let ok = harness.get_by_label(&t!("action-ok")).rect();
        let close = harness.get_by_label(&t!("action-close")).rect();
        assert!(close.top() > 0.0 && ok.bottom() < size.y, "{close:?} {ok:?}");
        assert!(ok.bottom() - close.top() < 900.0, "the dialog grew to {}", ok.bottom() - close.top());
    }
}

#[test]
fn graphics_dialog_snapshots() {
    if !snapshots_requested() {
        return;
    }
    let mut results = egui_kittest::SnapshotResults::new();
    let (mut state, plan) = graphics_state();
    crate::graphics_dialog::open(&mut state, crate::graphics_dialog::Target::View(plan));
    let mut harness = themed_harness(state, egui::vec2(980.0, 680.0), egui::Theme::Light, |ui, state| {
        crate::graphics_dialog::show(ui.ctx(), state)
    });
    results.add(harness.try_snapshot("visibility_graphics"));
    harness.get_by_label(&t!("vg-filters")).click_accesskit();
    for _ in 0..4 {
        harness.run();
    }
    results.add(harness.try_snapshot("visibility_graphics_filters"));
    let (mut state, _) = graphics_state();
    crate::graphics_dialog::open_filters(&mut state);
    let mut harness = themed_harness(state, egui::vec2(980.0, 760.0), egui::Theme::Light, |ui, state| {
        crate::graphics_dialog::show(ui.ctx(), state)
    });
    results.add(harness.try_snapshot("filters"));
    let (mut state, plan) = graphics_state();
    let create = cc_engine::commands::graphics::TemplateCreate {
        name: "Working plan".into(),
        view: Some(cc_engine::Ref::Id(plan.0)),
    };
    state.session.run(create).unwrap();
    crate::graphics_dialog::open_templates(&mut state);
    let mut harness = themed_harness(state, egui::vec2(980.0, 680.0), egui::Theme::Light, |ui, state| {
        crate::graphics_dialog::show(ui.ctx(), state)
    });
    results.add(harness.try_snapshot("view_templates"));
    results.unwrap();
}

fn phases_harness(state: AppState) -> Harness<'static, AppState> {
    themed_harness(state, egui::vec2(1000.0, 640.0), egui::Theme::Light, |ui, state| {
        crate::phases_dialog::show(ui.ctx(), state)
    })
}

#[test]
fn phases_dialog_adds_a_phase_and_cancel_undoes() {
    let mut state = sample_state();
    let depth = state.session.doc.undo_depth();
    let phases = state.project().phases.len();
    crate::phases_dialog::open(&mut state);
    let mut harness = phases_harness(state);
    harness.get_by_label(&t!("ph-add")).click();
    harness.run();
    assert_eq!(harness.state().project().phases.len(), phases + 1);
    harness.get_by_label(&t!("action-cancel")).click();
    harness.run();
    assert_eq!(harness.state().project().phases.len(), phases);
    assert_eq!(harness.state().session.doc.undo_depth(), depth);
    assert!(harness.state().dialogs.phases.is_none());
}

#[test]
fn phases_dialog_ok_keeps_one_undo_step() {
    let mut state = sample_state();
    let depth = state.session.doc.undo_depth();
    let filters = state.project().phase_filters.len();
    crate::phases_dialog::open(&mut state);
    let mut harness = phases_harness(state);
    harness.get_by_label(&t!("ph-add")).click();
    harness.run();
    harness.get_by_label(&t!("ph-tab-filters")).click();
    harness.run();
    harness.get_by_label(&t!("ph-new-filter")).click();
    harness.run();
    assert_eq!(harness.state().project().phase_filters.len(), filters + 1);
    harness.get_by_label(&t!("action-ok")).click();
    harness.run();
    assert!(harness.state().dialogs.phases.is_none());
    assert_eq!(harness.state().project().phase_filters.len(), filters + 1);
    // OK keeps both edits.
    assert_eq!(harness.state().session.doc.undo_depth(), depth + 2);
}

#[test]
fn phases_dialog_snapshots() {
    if !snapshots_requested() {
        return;
    }
    let mut results = egui_kittest::SnapshotResults::new();
    // The dialog opens on the phases tab.
    for (tab, name) in
        [(None, "phases"), (Some("ph-tab-filters"), "phase_filters"), (Some("ph-tab-graphics"), "phase_graphics")]
    {
        let mut state = sample_state();
        crate::phases_dialog::open(&mut state);
        let mut harness = phases_harness(state);
        if let Some(tab) = tab {
            harness.get_by_label(&crate::i18n::tr(tab)).click();
        }
        for _ in 0..3 {
            harness.run();
        }
        results.add(harness.try_snapshot(name));
    }
    results.unwrap();
}
