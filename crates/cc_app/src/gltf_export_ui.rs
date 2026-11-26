//! Export glTF (*File → Export → glTF…*): after the file is picked, a
//! dialog asks whether to compress the meshes with Draco, then
//! `gltf_export` writes the active 3D view (or the first).

use crate::dialog::Dialog;
use crate::icons::Icon;
use crate::settings;
use crate::state::AppState;
use crate::t;
use cc_engine::commands::io;
use egui::Context;

/// Pick the .glb to write and open the options dialog for it.
pub fn open(state: &mut AppState) {
    if crate::actions::gltf_view(state).is_none() {
        return;
    }
    let name = format!("{}.glb", state.project().info.name);
    let dialog = rfd::FileDialog::new().add_filter(t!("file-filter-gltf"), &["glb"]).set_file_name(name);
    let Some(mut path) = dialog.save_file() else { return };
    if path.extension().is_none() {
        path.set_extension("glb");
    }
    state.dialogs.gltf_export = Some(path);
}

/// The options dialog of a picked file; OK writes it.
pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(path) = state.dialogs.gltf_export.clone() else { return };
    let mut draco = state.dialogs.gltf_draco;
    let mut ok = None;
    let response = Dialog::new("gltf-export", t!("dlg-export-gltf")).width(460.0).show(
        ctx,
        |ui| {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            settings::file_box(ui, Icon::Export, &name);
            let label = t!("label-gltf-draco");
            settings::setting_row(ui, &label, Some(&t!("hint-gltf-draco")), |ui| {
                settings::switch(ui, &mut draco, &label)
            });
        },
        |f| {
            ok = Some(f.primary(t!("action-ok")));
            f.secondary(t!("action-cancel"));
        },
    );
    state.dialogs.gltf_draco = draco;
    if response.clicked.is_some() && response.clicked == ok {
        write(state, path.clone(), draco);
    }
    if response.clicked.is_some() || response.dismissed {
        state.dialogs.gltf_export = None;
    }
}

/// Run `gltf_export` on the active 3D view (or the first).
fn write(state: &mut AppState, path: std::path::PathBuf, draco: bool) {
    let Some(view) = crate::actions::gltf_view(state) else { return };
    let view = Some(cc_engine::Ref::Id(view.0));
    match state.session.run(io::GltfExport { path: path.clone(), view, draco }) {
        Ok(_) => state.message = t!("msg-exported", "path" => path.display()),
        Err(e) => state.dialogs.error = Some(t!("msg-export-failed", "error" => e.message)),
    }
}
