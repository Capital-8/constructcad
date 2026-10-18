//! Document-level commands shared by the ribbon, browser and shortcuts.

use crate::state::{AppState, PendingAction};
use crate::t;
use crate::tools::Tool;
use cc_core::{
    CameraState, Category, Document, Level, LevelId, Project, SheetSize, View, ViewCategory, ViewId, ViewKind, Viewport,
};
use glam::DVec2;

fn add_view(state: &mut AppState, tx: String, name: String, scale: u32, kind: ViewKind) -> ViewId {
    let id = state.doc.edit(tx, |e| {
        let id = ViewId(e.alloc_id());
        e.add_view(View { id, name, scale, kind })
    });
    state.open_view(id);
    id
}

pub fn new_plan(state: &mut AppState, level: LevelId, ceiling: bool) -> ViewId {
    let name = state.project().level_name(level).to_string();
    let kind = if ceiling {
        ViewKind::CeilingPlan { level, cut_height: 1.2 }
    } else {
        ViewKind::FloorPlan { level, cut_height: 1.2 }
    };
    add_view(state, t!("tx-new-view"), name, 50, kind)
}

pub fn new_3d(state: &mut AppState) -> ViewId {
    let n = state.project().views_of(ViewCategory::ThreeD).count() + 1;
    add_view(
        state,
        t!("tx-new-view"),
        t!("default-3d-name", "n" => n),
        100,
        ViewKind::ThreeD { camera: CameraState::default() },
    )
}

pub fn new_legend(state: &mut AppState) -> ViewId {
    let types = state.project().types.keys().copied().collect();
    let n = state.project().views_of(ViewCategory::Legend).count() + 1;
    add_view(state, t!("tx-new-view"), t!("default-legend-name", "n" => n), 50, ViewKind::Legend { types })
}

pub fn new_schedule(state: &mut AppState, category: Category) -> ViewId {
    let name = t!("default-schedule-name", "category" => t_category(category));
    add_view(state, t!("tx-new-view"), name, 1, ViewKind::Schedule { category })
}

pub fn new_sheet(state: &mut AppState) -> ViewId {
    let n = state.project().views_of(ViewCategory::Sheet).count() + 101;
    let kind = ViewKind::Sheet { number: format!("A{n}"), size: SheetSize::A3, viewports: Vec::new() };
    add_view(state, t!("tx-new-sheet"), t!("default-sheet-name"), 1, kind)
}

pub fn t_category(category: Category) -> String {
    crate::i18n::tr(category.key())
}

pub fn duplicate_view(state: &mut AppState, id: ViewId) {
    let Some(mut v) = state.project().view(id).cloned() else { return };
    v.name = t!("copy-of", "name" => v.name);
    if let ViewKind::Sheet { viewports, number, .. } = &mut v.kind {
        // A view can be placed on only one sheet.
        viewports.clear();
        number.push('a');
    }
    let id = state.doc.edit(t!("tx-duplicate-view"), |e| {
        v.id = ViewId(e.alloc_id());
        e.add_view(v)
    });
    state.open_view(id);
}

pub fn delete_view(state: &mut AppState, id: ViewId) {
    state.doc.edit(t!("tx-delete-view"), |e| e.remove_view(id));
    if state.active_view == Some(id) {
        state.active_view = None;
    }
}

pub fn rename_view(state: &mut AppState, id: ViewId, name: String) {
    let name = name.trim().to_string();
    if name.is_empty() {
        return;
    }
    state.doc.edit(t!("tx-rename-view"), |e| {
        if let Some(v) = e.view_mut(id) {
            v.name = name;
        }
    });
}

/// Sheet that already shows `view`, if any.
pub fn sheet_of(project: &Project, view: ViewId) -> Option<ViewId> {
    project.views.values().find_map(|v| match &v.kind {
        ViewKind::Sheet { viewports, .. } if viewports.iter().any(|vp| vp.view == view) => Some(v.id),
        _ => None,
    })
}

pub fn place_on_sheet(state: &mut AppState, sheet: ViewId, view: ViewId) {
    if sheet_of(state.project(), view).is_some() {
        state.message = t!("msg-view-already-placed");
        return;
    }
    state.doc.edit(t!("tx-place-view"), |e| {
        if let Some(ViewKind::Sheet { viewports, size, .. }) = e.view_mut(sheet).map(|v| &mut v.kind) {
            let center = size.size_mm() * DVec2::new(0.4, 0.55);
            viewports.push(Viewport { view, center });
        }
    });
}

pub fn remove_viewport(state: &mut AppState, sheet: ViewId, index: usize) {
    state.doc.edit(t!("tx-remove-viewport"), |e| {
        if let Some(ViewKind::Sheet { viewports, .. }) = e.view_mut(sheet).map(|v| &mut v.kind)
            && index < viewports.len()
        {
            viewports.remove(index);
        }
    });
    state.sheet_selection = None;
}

/// Add a level above the highest one, with its floor and ceiling plans.
pub fn add_level(state: &mut AppState) {
    let top = state
        .project()
        .levels
        .iter()
        .map(|l| l.elevation)
        .fold(None, |m: Option<f64>, e| Some(m.map_or(e, |m| m.max(e))));
    let elevation = top.map_or(0.0, |t| t + 3.0);
    let n = state.project().levels.len();
    state.doc.edit(t!("tx-add-level"), |e| {
        let id = LevelId(e.alloc_id());
        let name = t!("default-level-name", "n" => n);
        e.levels_mut().push(Level { id, name: name.clone(), elevation });
        for kind in
            [ViewKind::FloorPlan { level: id, cut_height: 1.2 }, ViewKind::CeilingPlan { level: id, cut_height: 1.2 }]
        {
            let vid = ViewId(e.alloc_id());
            e.add_view(View { id: vid, name: name.clone(), scale: 50, kind });
        }
    });
}

/// Delete a level and its plan views; refused while elements use it.
pub fn delete_level(state: &mut AppState, level: LevelId) {
    if state.project().elements.values().any(|e| e.level == level) {
        state.message = t!("msg-level-in-use");
        return;
    }
    if state.project().levels.len() <= 1 {
        return;
    }
    let views: Vec<_> = state.project().views.values().filter(|v| v.level() == Some(level)).map(|v| v.id).collect();
    state.doc.edit(t!("tx-delete-level"), |e| {
        for v in views {
            e.remove_view(v);
        }
        e.levels_mut().retain(|l| l.id != level);
    });
}

pub fn activate_category_tool(state: &mut AppState, category: Category) {
    match Tool::for_category(state.project(), category) {
        Some(tool) => {
            state.set_tool(tool);
            let in_plan = state.active_level().is_some();
            state.message = if in_plan { String::new() } else { t!("msg-open-plan") };
        }
        None => state.message = t!("msg-no-type"),
    }
}

/// A new project from the template, named in the active UI language.
pub fn new_project() -> Project {
    Project::from_template_localized(&crate::i18n::try_tr)
}

/// Switch the UI language and refresh text baked into cached drawings.
pub fn set_language(state: &mut AppState, lang: crate::i18n::Language) {
    if crate::i18n::language() == lang {
        return;
    }
    crate::i18n::set_language(lang);
    state.invalidate_drawings();
    state.message = t!("msg-language-changed");
}

fn persist_error(err: &cc_core::PersistError) -> String {
    use cc_core::PersistError as E;
    match err {
        E::Io(e) => t!("err-io", "detail" => e),
        E::Parse(e) => t!("err-parse", "detail" => e),
        E::Serialize(e) => t!("err-serialize", "detail" => e),
        E::Version(v) => t!("err-version", "version" => v, "max" => cc_core::FORMAT_VERSION),
    }
}

// --- Files ---

fn file_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter(t!("file-filter"), &["ccad"])
}

/// Run `action` now, or ask for confirmation first if there are unsaved changes.
pub fn guarded(state: &mut AppState, action: PendingAction) {
    if state.doc.is_dirty() {
        state.dialogs.confirm = Some(action);
    } else {
        run_pending(state, action);
    }
}

pub fn run_pending(state: &mut AppState, action: PendingAction) {
    match action {
        PendingAction::New => {
            state.replace_document(Document::new(new_project()));
            state.message = t!("msg-new-project");
        }
        PendingAction::Open => open(state),
        PendingAction::Quit => state.quit_confirmed = true,
    }
}

pub fn open(state: &mut AppState) {
    let Some(path) = file_dialog().pick_file() else { return };
    open_path(state, path);
}

pub fn open_path(state: &mut AppState, path: std::path::PathBuf) {
    match cc_core::load(&path) {
        Ok(project) => {
            let mut doc = Document::new(project);
            doc.path = Some(path.clone());
            state.replace_document(doc);
            state.message = t!("msg-opened", "path" => path.display());
        }
        Err(err) => state.dialogs.error = Some(t!("msg-open-failed", "error" => persist_error(&err))),
    }
}

pub fn export_ifc(state: &mut AppState) {
    let name = format!("{}.ifc", state.project().info.name);
    let dialog = rfd::FileDialog::new().add_filter(t!("file-filter-ifc"), &["ifc"]).set_file_name(name);
    let Some(mut path) = dialog.save_file() else { return };
    if path.extension().is_none() {
        path.set_extension("ifc");
    }
    match cc_ifc::export_to_file(state.project(), &path) {
        Ok(()) => state.message = t!("msg-exported", "path" => path.display()),
        Err(err) => {
            let error = t!("err-io", "detail" => err);
            state.dialogs.error = Some(t!("msg-export-failed", "error" => error));
        }
    }
}

pub fn save(state: &mut AppState, save_as: bool) -> bool {
    let path = match (&state.doc.path, save_as) {
        (Some(p), false) => p.clone(),
        _ => {
            let name = format!("{}.ccad", state.project().info.name);
            let Some(mut p) = file_dialog().set_file_name(name).save_file() else { return false };
            if p.extension().is_none() {
                p.set_extension("ccad");
            }
            p
        }
    };
    match cc_core::save(state.project(), &path) {
        Ok(()) => {
            state.doc.path = Some(path.clone());
            state.doc.mark_saved();
            state.message = t!("msg-saved", "path" => path.display());
            true
        }
        Err(err) => {
            state.dialogs.error = Some(t!("msg-save-failed", "error" => persist_error(&err)));
            false
        }
    }
}
