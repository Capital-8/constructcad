//! Import OBJ and glTF (*Insert → Import OBJ…*, *Import glTF…*): after the
//! file is picked, a dialog asks what the format does not say (for OBJ the
//! units and which axis is up; for both one element or one per object),
//! then `obj_import` or `gltf_import` runs as a background job.

use crate::dialog::Dialog;
use crate::icons::Icon;
use crate::settings;
use crate::state::AppState;
use crate::t;
use cc_engine::commands::io::{GltfImport, ObjImport, ObjUnits};
use egui::Context;
use std::path::PathBuf;

/// The choices of the import dialog, kept for the next import.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObjOptions {
    pub units: ObjUnits,
    pub z_up: bool,
    pub merge: bool,
}

const UNITS: [(ObjUnits, &str); 5] = [
    (ObjUnits::M, "unit-meter"),
    (ObjUnits::Cm, "unit-centimeter"),
    (ObjUnits::Mm, "unit-millimeter"),
    (ObjUnits::In, "unit-inch"),
    (ObjUnits::Ft, "unit-foot"),
];

/// Pick an .obj file and open the options dialog for it.
pub fn import_obj(state: &mut AppState) {
    let mut dialog = rfd::FileDialog::new().add_filter(t!("file-filter-obj"), &["obj"]);
    if let Some(dir) = state.prefs.projects_dir() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(path) = dialog.pick_file() {
        state.dialogs.obj_import = Some(path);
    }
}

/// Pick a .glb or .gltf file and open the options dialog for it.
pub fn import_gltf(state: &mut AppState) {
    let mut dialog = rfd::FileDialog::new().add_filter(t!("file-filter-gltf"), &["glb", "gltf"]);
    if let Some(dir) = state.prefs.projects_dir() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(path) = dialog.pick_file() {
        state.dialogs.obj_import = Some(path);
    }
}

/// Whether `path` is a glTF model (else OBJ).
fn is_gltf(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("glb") || e.eq_ignore_ascii_case("gltf"))
}

/// The options dialog of a picked file; OK starts the import.
pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(path) = state.dialogs.obj_import.clone() else { return };
    let mut options = state.dialogs.obj_options;
    let mut ok = None;
    let gltf = is_gltf(&path);
    let title = if gltf { t!("dlg-import-gltf") } else { t!("dlg-import-obj") };
    let response = Dialog::new("obj-import", title).width(460.0).show(
        ctx,
        |ui| {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            settings::file_box(ui, Icon::Component, &name);
            // glTF is in meters with Y up.
            if !gltf {
                settings::setting(ui, &t!("label-obj-units"), None, |ui| {
                    let units: Vec<(ObjUnits, String)> = UNITS.iter().map(|(u, k)| (*u, crate::i18n::tr(k))).collect();
                    settings::combo_items(ui, "obj-units", 160.0, &mut options.units, &units);
                });
                settings::setting(ui, &t!("label-obj-up"), None, |ui| {
                    let axis = |value, text: &str| settings::Segment {
                        value,
                        text: text.to_string(),
                        text_size: 13.0,
                        name: text.to_string(),
                    };
                    settings::segmented(ui, &mut options.z_up, vec![axis(false, "Y"), axis(true, "Z")]);
                });
            }
            let label = t!("label-obj-merge");
            settings::setting_row(ui, &label, None, |ui| settings::switch(ui, &mut options.merge, &label));
        },
        |f| {
            ok = Some(f.primary(t!("action-ok")));
            f.secondary(t!("action-cancel"));
        },
    );
    state.dialogs.obj_options = options;
    if response.clicked.is_some() && response.clicked == ok {
        start(state, path, options);
    }
    if response.clicked.is_some() || response.dismissed {
        state.dialogs.obj_import = None;
    }
}

/// Run `obj_import` or `gltf_import` on the active view's level in the background.
fn start(state: &mut AppState, path: PathBuf, options: ObjOptions) {
    let level = state.active_level().map(|l| cc_engine::Ref::Id(l.0));
    let gltf = is_gltf(&path);
    let command = ObjImport {
        path: path.clone(),
        level: level.clone(),
        position: None,
        rotation: None,
        units: options.units,
        z_up: options.z_up,
        merge: options.merge,
    };
    let gltf_command = GltfImport { path: path.clone(), level, position: None, rotation: None, merge: options.merge };
    let shown = path.clone();
    let finish: crate::jobs::Finish = Box::new(move |state, result, _| match result {
        Ok(out) => {
            let count = out["elements"].as_array().map_or(0, Vec::len);
            state.message = if gltf {
                t!("msg-gltf-imported", "path" => path.display(), "count" => count)
            } else {
                t!("msg-obj-imported", "path" => path.display(), "count" => count)
            };
        }
        Err(e) => state.dialogs.error = Some(t!("msg-import-failed", "error" => e.message)),
    });
    let job = if gltf { t!("job-importing-gltf") } else { t!("job-importing-obj") };
    let run = move |s: &mut cc_engine::Session| if gltf { s.run(gltf_command) } else { s.run(command) };
    crate::jobs::start(state, job, shown, crate::jobs::Base::Current, run, finish);
}
