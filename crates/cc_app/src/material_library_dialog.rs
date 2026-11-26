//! Material Libraries (*Libraries…* in the Material Browser): the built-in
//! libraries of materials by region or standard, filtered by region, and
//! the selected one's materials with their main values. *Load* copies the
//! materials checked (or all of them) into the project, *Import file…*
//! reads a library file and *Export materials…* writes the project's
//! materials as one, for another project or another user. Every change is a
//! `material_library_*` command; OK keeps them, Cancel undoes them all
//! (through [`crate::graphics_dialog::Edits`]), as the Pipe Catalog does.

use crate::dialog::Dialog;
use crate::graphics_dialog::Edits;
use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use crate::table;
use crate::theme::{self, PALETTE};
use cc_core::pipe_catalog::Region;
use cc_engine::commands::materials as cmd;
use egui::{Context, RichText, Stroke, Ui};
use serde_json::Value;
use std::collections::BTreeSet;

/// One library of the list.
#[derive(Debug, Clone)]
struct ListRow {
    code: String,
    name: String,
    region: Option<Region>,
    materials: u64,
}

#[derive(Default)]
struct Cache {
    revision: Option<u64>,
    libraries: Vec<ListRow>,
    /// The selected library as `material_catalog` gives it.
    detail: Option<(String, Value)>,
}

/// The open Material Libraries dialog.
pub struct MaterialLibraryDialog {
    pub(crate) selected: Option<String>,
    region: Option<Region>,
    /// The materials checked for *Load*, by name.
    pub(crate) picked: BTreeSet<String>,
    error: Option<String>,
    /// What the last load did.
    note: Option<String>,
    cache: Cache,
    edits: Edits,
}

/// Open the Material Libraries dialog.
pub fn open(state: &mut AppState) {
    if state.dialogs.material_library.is_some() {
        return;
    }
    let edits = Edits::begin(state);
    state.dialogs.material_library = Some(MaterialLibraryDialog {
        selected: None,
        region: None,
        picked: BTreeSet::new(),
        error: None,
        note: None,
        cache: Cache::default(),
        edits,
    });
}

/// Run a command; a refusal goes to the dialog's error line.
fn run<C: cc_engine::Command + serde::Serialize>(
    state: &mut AppState,
    error: &mut Option<String>,
    command: C,
) -> Option<Value> {
    match state.session.run(command) {
        Ok(out) => {
            *error = None;
            Some(out)
        }
        Err(e) => {
            *error = Some(e.message);
            None
        }
    }
}

/// Read the libraries once, and the selected one again when the model changed.
fn refresh(state: &mut AppState, d: &mut MaterialLibraryDialog) {
    if d.cache.libraries.is_empty() {
        let out = state.session.run(cmd::MaterialCatalog::default()).unwrap_or_default();
        d.cache.libraries = out["libraries"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| {
                let code = l["code"].as_str()?.to_string();
                Some(ListRow {
                    name: l["name"].as_str().unwrap_or(&code).to_string(),
                    region: Region::ALL.into_iter().find(|r| Some(r.code()) == l["region"].as_str()),
                    materials: l["materials"].as_u64().unwrap_or(0),
                    code,
                })
            })
            .collect();
    }
    if d.selected.as_ref().is_none_or(|s| !d.cache.libraries.iter().any(|r| r.code == *s)) {
        d.selected = d.cache.libraries.first().map(|r| r.code.clone());
    }
    let revision = state.session.doc.revision();
    let stale = d.cache.revision != Some(revision) || d.cache.detail.as_ref().map(|(c, _)| c) != d.selected.as_ref();
    if stale {
        if d.cache.detail.as_ref().map(|(c, _)| c) != d.selected.as_ref() {
            d.picked.clear();
        }
        d.cache.detail = d.selected.clone().and_then(|code| {
            let out = state.session.run(cmd::MaterialCatalog { library: Some(code.clone()) }).ok()?;
            Some((code, out["libraries"][0].clone()))
        });
        d.cache.revision = Some(revision);
    }
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(mut d) = state.dialogs.material_library.take() else { return };
    refresh(state, &mut d);
    let mut ok = 0;
    let response = Dialog::new("material-library", t!("mat-lib-title")).width(900.0).show(
        ctx,
        |ui| {
            crate::dialog::two_columns(ui, [260.0, 630.0], |ui, column| {
                if column == 0 {
                    list(ui, state, &mut d);
                } else {
                    details(ui, state, &mut d);
                }
            });
        },
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    let close = match response.clicked {
        Some(i) => Some(i == ok),
        None if response.dismissed => Some(false),
        None => None,
    };
    match close {
        Some(keep) => d.edits.finish(state, keep),
        None => state.dialogs.material_library = Some(d),
    }
}

/// The region filter, the libraries, and the file buttons.
fn list(ui: &mut Ui, state: &mut AppState, d: &mut MaterialLibraryDialog) {
    ui.horizontal(|ui| {
        ui.label(theme::muted(t!("pipe-catalog-region")));
        let shown = d.region.map_or_else(|| t!("pipe-catalog-all"), |r| tr(r.key()));
        egui::ComboBox::from_id_salt("mat-lib-region").selected_text(shown).width(150.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut d.region, None, t!("pipe-catalog-all"));
            for r in Region::ALL {
                ui.selectable_value(&mut d.region, Some(r), tr(r.key()));
            }
        });
    });
    ui.add_space(4.0);
    let rows: Vec<ListRow> = d
        .cache
        .libraries
        .iter()
        .filter(|r| d.region.is_none_or(|g| r.region.is_none_or(|x| x == g)))
        .cloned()
        .collect();
    egui::Frame::new().stroke(Stroke::new(1.0, PALETTE.border)).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("mat-lib-list").max_height(380.0).auto_shrink([false, false]).show(
            ui,
            |ui| {
                if rows.is_empty() {
                    ui.label(theme::muted(t!("mat-lib-none")));
                }
                for r in rows {
                    let on = d.selected.as_ref() == Some(&r.code);
                    let label = format!("{} ({})", r.name, r.materials);
                    if ui.selectable_label(on, label).on_hover_text(&r.code).clicked() {
                        d.selected = Some(r.code.clone());
                        d.error = None;
                        d.note = None;
                    }
                }
            },
        );
    });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        if ui.button(t!("mat-lib-import-file")).clicked() {
            import_file(state, d);
        }
        let any = !state.project().materials.is_empty();
        if ui.add_enabled(any, egui::Button::new(t!("mat-lib-export"))).clicked() {
            export_file(state, d);
        }
    });
}

/// What a load did, for the note under the table.
fn loaded_note(out: &Value) -> String {
    let count = |k: &str| out[k].as_array().map_or(0, Vec::len);
    t!("mat-lib-loaded", "count" => count("imported"), "kept" => count("kept"))
}

fn import_file(state: &mut AppState, d: &mut MaterialLibraryDialog) {
    let dialog = rfd::FileDialog::new().add_filter(t!("file-filter-material-library"), &["json"]);
    let Some(path) = dialog.pick_file() else { return };
    let command = cmd::MaterialLibraryImport { path: Some(path), ..Default::default() };
    if let Some(out) = run(state, &mut d.error, command) {
        d.note = Some(loaded_note(&out));
    }
}

fn export_file(state: &mut AppState, d: &mut MaterialLibraryDialog) {
    let dialog = rfd::FileDialog::new()
        .add_filter(t!("file-filter-material-library"), &["json"])
        .set_file_name(format!("{}.json", t!("mat-lib-file-name")));
    let Some(path) = dialog.save_file() else { return };
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "materials".into());
    let command = cmd::MaterialLibraryExport {
        code: stem.to_uppercase(),
        name: stem,
        region: String::new(),
        standard: String::new(),
        sources: Vec::new(),
        materials: None,
        path: Some(path.clone()),
    };
    if run(state, &mut d.error, command).is_some() {
        state.message = t!("msg-exported", "path" => path.display());
    }
}

/// A value of the catalog's JSON, rounded for the table, or a dash.
/// The columns of the table: a library value and its header.
const COLUMNS: [(&str, &str); 7] = [
    ("density", "ρ (kg/m³)"),
    ("thermal_conductivity", "λ (W/(m·K))"),
    ("specific_heat", "c (J/(kg·K))"),
    ("youngs_modulus", "E (MPa)"),
    ("compressive_strength", "fck (MPa)"),
    ("yield_strength", "fy (MPa)"),
    ("tensile_strength", "fu (MPa)"),
];

/// A small square of a material's color (`[r, g, b]`).
fn swatch(ui: &mut Ui, color: &Value) {
    let c = |i: usize| color[i].as_u64().map_or(180, |v| v.min(255) as u8);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(c(0), c(1), c(2)));
    ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, PALETTE.border), egui::StrokeKind::Inside);
}

fn value(v: &Value) -> String {
    match v.as_f64() {
        Some(x) if x.abs() >= 100.0 => format!("{}", x.round()),
        Some(x) => format!("{}", (x * 1000.0).round() / 1000.0),
        None => "—".into(),
    }
}

/// The selected library: its name, standard and sources, its materials with
/// their density, modulus and strengths, and *Load*.
fn details(ui: &mut Ui, state: &mut AppState, d: &mut MaterialLibraryDialog) {
    let Some((code, detail)) = d.cache.detail.clone() else {
        ui.label(theme::muted(t!("mat-lib-none")));
        return;
    };
    ui.label(RichText::new(detail["name"].as_str().unwrap_or_default()).strong().size(15.0));
    let facts: Vec<&str> =
        [detail["standard"].as_str(), Some(code.as_str())].into_iter().flatten().filter(|s| !s.is_empty()).collect();
    ui.label(theme::muted(facts.join(" · ")));
    ui.add_space(4.0);
    let materials: Vec<Value> = detail["materials"].as_array().cloned().unwrap_or_default();
    table::section(ui, "mat-lib-materials", t!("mat-lib-materials"), |ui| {
        egui::ScrollArea::both().id_salt("mat-lib-rows").max_height(300.0).show(ui, |ui| {
            // The values some material of the library has, with the
            // symbols of the standards, as in their tables.
            let columns: Vec<(&str, &str)> =
                COLUMNS.iter().copied().filter(|(k, _)| materials.iter().any(|m| m[*k].is_number())).collect();
            egui::Grid::new("mat-lib-grid").num_columns(columns.len() + 4).striped(true).spacing([12.0, 3.0]).show(
                ui,
                |ui| {
                    let name_header = t!("mat-lib-name");
                    let headers = ["", "", name_header.as_str()].into_iter().chain(columns.iter().map(|(_, h)| *h));
                    for h in headers.chain([""]) {
                        ui.label(theme::muted(h));
                    }
                    ui.end_row();
                    for m in &materials {
                        let name = m["name"].as_str().unwrap_or_default().to_string();
                        let have = m["in_project"].as_bool().unwrap_or(false);
                        let mut on = d.picked.contains(&name);
                        let r = ui.add_enabled(!have, egui::Checkbox::without_text(&mut on));
                        if r.changed() {
                            if on {
                                d.picked.insert(name.clone());
                            } else {
                                d.picked.remove(&name);
                            }
                        }
                        swatch(ui, &m["color"]);
                        let description = m["description"].as_str().unwrap_or_default();
                        ui.label(&name).on_hover_text(description);
                        for (k, _) in &columns {
                            ui.label(value(&m[*k]));
                        }
                        ui.label(if have { t!("mat-lib-in-project") } else { String::new() });
                        ui.end_row();
                    }
                },
            );
        });
        table::button_row(ui, |ui| {
            let missing = materials.iter().filter(|m| m["in_project"].as_bool() != Some(true)).count();
            let (label, names) = if d.picked.is_empty() {
                (t!("mat-lib-load-all"), None)
            } else {
                (t!("mat-lib-load-picked", "count" => d.picked.len()), Some(d.picked.iter().cloned().collect()))
            };
            let load =
                ui.add_enabled(missing > 0, egui::Button::new(label)).on_disabled_hover_text(t!("mat-lib-all-in"));
            if load.clicked() {
                let command =
                    cmd::MaterialLibraryImport { library: Some(code.clone()), materials: names, ..Default::default() };
                if let Some(out) = run(state, &mut d.error, command) {
                    d.note = Some(loaded_note(&out));
                    d.picked.clear();
                }
            }
        });
    });
    let sources: Vec<String> =
        detail["sources"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect();
    if !sources.is_empty() {
        table::section_closed(ui, "mat-lib-sources", t!("mat-lib-sources"), |ui| {
            for s in sources {
                ui.label(RichText::new(format!("• {s}")).size(11.5).color(PALETTE.text_muted));
            }
        });
    }
    if let Some(note) = &d.note {
        ui.label(theme::muted(note));
    }
    if let Some(e) = &d.error {
        ui.label(RichText::new(e).color(PALETTE.warning));
    }
}
