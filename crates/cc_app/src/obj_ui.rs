//! Import OBJ (*Insert → Import OBJ…*): after the file is picked, a dialog
//! asks what the format does not say (units, which axis is up, one element
//! or one per object), then `obj_import` runs as a background job.

use crate::dialog::Dialog;
use crate::state::AppState;
use crate::t;
use crate::theme::PALETTE;
use cc_engine::commands::io::{ObjImport, ObjUnits};
use egui::{Context, RichText};
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

fn unit_label(units: ObjUnits) -> String {
    let key = UNITS.iter().find(|(u, _)| *u == units);
    key.map(|(_, k)| crate::i18n::tr(k)).unwrap_or_default()
}

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

/// The options dialog of a picked file; OK starts the import.
pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(path) = state.dialogs.obj_import.clone() else { return };
    let mut options = state.dialogs.obj_options;
    let mut ok = None;
    let response = Dialog::new("obj-import", t!("dlg-import-obj")).width(380.0).show(
        ctx,
        |ui| {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            ui.label(RichText::new(name).strong());
            ui.add_space(8.0);
            egui::Grid::new("obj-options").num_columns(2).spacing([14.0, 8.0]).min_col_width(110.0).show(ui, |ui| {
                ui.label(RichText::new(t!("label-obj-units")).color(PALETTE.text_muted));
                egui::ComboBox::from_id_salt("obj-units").selected_text(unit_label(options.units)).show_ui(ui, |ui| {
                    for (units, key) in UNITS {
                        ui.selectable_value(&mut options.units, units, crate::i18n::tr(key));
                    }
                });
                ui.end_row();
                ui.label(RichText::new(t!("label-obj-up")).color(PALETTE.text_muted));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut options.z_up, false, "Y");
                    ui.radio_value(&mut options.z_up, true, "Z");
                });
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.checkbox(&mut options.merge, t!("label-obj-merge"));
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

/// Run `obj_import` on the active view's level in the background.
fn start(state: &mut AppState, path: PathBuf, options: ObjOptions) {
    let level = state.active_level().map(|l| cc_engine::Ref::Id(l.0));
    let command = ObjImport {
        path: path.clone(),
        level,
        position: None,
        rotation: None,
        units: options.units,
        z_up: options.z_up,
        merge: options.merge,
    };
    let shown = path.clone();
    let finish: crate::jobs::Finish = Box::new(move |state, result, _| match result {
        Ok(out) => {
            let count = out["elements"].as_array().map_or(0, Vec::len);
            state.message = t!("msg-obj-imported", "path" => path.display(), "count" => count);
        }
        Err(e) => state.dialogs.error = Some(t!("msg-import-failed", "error" => e.message)),
    });
    let run = move |s: &mut cc_engine::Session| s.run(command);
    crate::jobs::start(state, t!("job-importing-obj"), shown, crate::jobs::Base::Current, run, finish);
}
