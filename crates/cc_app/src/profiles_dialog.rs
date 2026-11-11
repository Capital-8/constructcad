//! The Profile Manager (*Structure → Profiles*, also *Manage → Profiles*):
//! the project's structural profiles grouped by material, with a search;
//! the selected one's outline drawn to fit, its name, material and
//! dimensions (millimeters on screen, meters to the engine) and its section
//! properties. *New* makes a profile of a standard shape, *Duplicate* copies
//! one, *Delete* removes one (with the profile that takes its place in the
//! types using it) and *Import from catalog* copies rows of the built-in
//! series in. Every change is a `profile_*` command; OK keeps them, Cancel
//! undoes them all (`dialog::session`, through [`crate::graphics_dialog::Edits`]).
//! What is shown is read back with `profiles_list` and `profile_get`.

use crate::dialog::Dialog;
use crate::graphics_dialog::{Edits, key};
use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use crate::table;
use crate::theme::{self, PALETTE};
use cc_core::structural_profile::MAX_NAME;
use cc_core::{ProfileId, ProfileMaterial, ProfileShape};
use cc_engine::Ref;
use cc_engine::commands::profiles as cmd;
use egui::{Context, Pos2, RichText, Sense, Shape, Stroke, Ui, vec2};
use glam::DVec2;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Side of the outline preview, points.
const PREVIEW: f32 = 250.0;

/// The i18n key of a shape code's name (`profile-shape-cold-lipped-u`).
pub fn shape_key(code: &str) -> String {
    format!("profile-shape-{}", code.replace('_', "-"))
}

/// A shape's name in the interface language.
fn shape_label(code: &str) -> String {
    tr(&shape_key(code))
}

/// The material a new profile of a shape starts with.
fn material_for(code: &str) -> ProfileMaterial {
    if code.starts_with("cold_") {
        ProfileMaterial::ColdFormed
    } else if code.starts_with("concrete_") || code == "rect" || code == "round" {
        ProfileMaterial::Concrete
    } else {
        ProfileMaterial::Steel
    }
}

/// One profile as `profiles_list` gives it.
struct Row {
    id: ProfileId,
    name: String,
    material: ProfileMaterial,
    users: usize,
}

/// What the dialog shows, read back while the document revision is unchanged.
#[derive(Default)]
struct Cache {
    revision: Option<u64>,
    list: Vec<Row>,
    /// `profile_get` of the selected profile.
    detail: Option<(ProfileId, Value)>,
}

/// The open Profile Manager.
pub struct ProfilesDialog {
    pub(crate) selected: Option<ProfileId>,
    search: String,
    /// The profile that takes a deleted one's place in its types.
    replace_with: Option<ProfileId>,
    /// The last refused change, shown under the parameters.
    error: Option<String>,
    pub(crate) catalog: Option<Catalog>,
    cache: Cache,
    edits: Edits,
}

/// *Import from catalog*: a series, a filter and the designations picked.
pub struct Catalog {
    /// The series' code.
    pub(crate) series: String,
    filter: String,
    picked: BTreeSet<String>,
    /// Every series: code and name.
    series_list: Vec<(String, String)>,
    /// The designations shown, for (series, filter).
    rows: Option<(String, String, Vec<String>)>,
}

pub fn open(state: &mut AppState) {
    let edits = Edits::begin(state);
    state.dialogs.profiles = Some(ProfilesDialog {
        selected: None,
        search: String::new(),
        replace_with: None,
        error: None,
        catalog: None,
        cache: Cache::default(),
        edits,
    });
}

/// Run an edit; a refusal goes to the dialog's error line.
fn run<C: cc_engine::Command + serde::Serialize>(
    state: &mut AppState,
    error: &mut Option<String>,
    merge: Option<u64>,
    command: C,
) -> Option<Value> {
    let result = match merge {
        Some(k) => state.session.run_merged(k, command),
        None => state.session.run(command),
    };
    match result {
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

fn profile_id(v: &Value) -> Option<ProfileId> {
    v["id"].as_u64().map(ProfileId)
}

/// Read the list and the selected profile again if the model changed.
fn refresh(state: &mut AppState, d: &mut ProfilesDialog) {
    let revision = state.session.doc.revision();
    if d.cache.revision != Some(revision) {
        let out = state.session.run(cmd::ProfilesList::default()).unwrap_or_default();
        let mut list: Vec<Row> = out
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some(Row {
                    id: profile_id(r)?,
                    name: r["name"].as_str()?.to_string(),
                    material: ProfileMaterial::from_code(r["material"].as_str()?)?,
                    users: r["users"].as_u64().unwrap_or(0) as usize,
                })
            })
            .collect();
        list.sort_by_key(|r| r.name.to_lowercase());
        d.cache = Cache { revision: Some(revision), list, detail: None };
    }
    if d.selected.is_none_or(|id| !d.cache.list.iter().any(|r| r.id == id)) {
        d.selected = d.cache.list.first().map(|r| r.id);
    }
    if d.cache.detail.as_ref().map(|(id, _)| *id) != d.selected {
        d.cache.detail = d
            .selected
            .and_then(|id| state.session.run(cmd::ProfileGet { profile: Ref::Id(id.0) }).ok().map(|v| (id, v)));
    }
    // The replacement is another profile, of the same material when there is one.
    let selected = d.selected.and_then(|id| d.cache.list.iter().find(|r| r.id == id));
    let valid = d.replace_with.is_some_and(|w| Some(w) != d.selected && d.cache.list.iter().any(|r| r.id == w));
    if !valid {
        let others = || d.cache.list.iter().filter(|r| Some(r.id) != d.selected);
        d.replace_with = others()
            .find(|r| selected.is_some_and(|s| s.material == r.material))
            .or_else(|| others().next())
            .map(|r| r.id);
    }
}

pub fn show(ctx: &Context, state: &mut AppState) {
    let Some(mut d) = state.dialogs.profiles.take() else { return };
    refresh(state, &mut d);
    let mut ok = 0;
    let response = Dialog::new("profiles", t!("profiles-title")).width(920.0).show(
        ctx,
        |ui| {
            crate::dialog::two_columns(ui, [250.0, 660.0], |ui, column| {
                if column == 0 {
                    list(ui, state, &mut d);
                } else {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(PREVIEW);
                            if let Some((_, detail)) = &d.cache.detail {
                                preview(ui, detail);
                            }
                        });
                        ui.add_space(8.0);
                        ui.vertical(|ui| details(ui, state, &mut d));
                    });
                }
            });
        },
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    catalog_dialog(ctx, state, &mut d);
    let close = match response.clicked {
        Some(i) => Some(i == ok),
        None if response.dismissed && d.catalog.is_none() => Some(false),
        None => None,
    };
    match close {
        Some(keep) => d.edits.finish(state, keep),
        None => state.dialogs.profiles = Some(d),
    }
}

// ---------------------------------------------------------------- the list

fn list(ui: &mut Ui, state: &mut AppState, d: &mut ProfilesDialog) {
    ui.add(egui::TextEdit::singleline(&mut d.search).hint_text(t!("profiles-search")).desired_width(f32::INFINITY));
    let search = d.search.trim().to_lowercase();
    egui::Frame::new().stroke(Stroke::new(1.0, PALETTE.border)).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("profiles-list").max_height(400.0).auto_shrink([false, false]).show(
            ui,
            |ui| {
                for m in ProfileMaterial::ALL {
                    let rows: Vec<(ProfileId, String)> = d
                        .cache
                        .list
                        .iter()
                        .filter(|r| r.material == m && r.name.to_lowercase().contains(&search))
                        .map(|r| (r.id, r.name.clone()))
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    table::section(ui, &format!("profiles-{}", m.code()), tr(m.key()), |ui| {
                        for (id, name) in rows {
                            if ui.selectable_label(d.selected == Some(id), name).clicked() {
                                d.selected = Some(id);
                                d.replace_with = None;
                                d.error = None;
                            }
                        }
                    });
                }
            },
        );
    });
    ui.add_space(6.0);
    buttons(ui, state, d);
}

fn buttons(ui: &mut Ui, state: &mut AppState, d: &mut ProfilesDialog) {
    let selected = d.selected;
    let users = selected.and_then(|id| d.cache.list.iter().find(|r| r.id == id)).map_or(0, |r| r.users);
    ui.horizontal_wrapped(|ui| {
        ui.menu_button(t!("profiles-new"), |ui| {
            for code in ProfileShape::CODES {
                if ui.button(shape_label(code)).clicked() {
                    let command = cmd::ProfileCreate {
                        material: material_for(code).code().into(),
                        shape: Some(code.to_string()),
                        ..Default::default()
                    };
                    if let Some(out) = run(state, &mut d.error, None, command) {
                        d.selected = profile_id(&out);
                    }
                    ui.close();
                }
            }
        });
        if ui.add_enabled(selected.is_some(), egui::Button::new(t!("profiles-duplicate"))).clicked()
            && let Some(id) = selected
            && let Some(out) =
                run(state, &mut d.error, None, cmd::ProfileDuplicate { profile: Ref::Id(id.0), name: None })
        {
            d.selected = profile_id(&out);
        }
        let can_delete = selected.is_some() && (users == 0 || d.replace_with.is_some());
        if ui.add_enabled(can_delete, egui::Button::new(t!("profiles-delete"))).clicked()
            && let Some(id) = selected
        {
            let replace_with = if users > 0 { d.replace_with.map(|w| Ref::Id(w.0)) } else { None };
            if run(state, &mut d.error, None, cmd::ProfileDelete { profile: Ref::Id(id.0), replace_with }).is_some() {
                d.selected = None;
            }
        }
        if ui.button(t!("profiles-import")).clicked() {
            open_catalog(state, d);
        }
    });
    if users > 0 {
        ui.horizontal(|ui| {
            ui.label(theme::muted(t!("profiles-replace-with")));
            let name = |id: Option<ProfileId>| {
                id.and_then(|id| d.cache.list.iter().find(|r| r.id == id)).map_or(String::new(), |r| r.name.clone())
            };
            let mut choice = d.replace_with;
            egui::ComboBox::from_id_salt("profiles-replace").selected_text(name(choice)).width(140.0).show_ui(
                ui,
                |ui| {
                    for r in d.cache.list.iter().filter(|r| Some(r.id) != selected) {
                        ui.selectable_value(&mut choice, Some(r.id), &r.name);
                    }
                },
            );
            d.replace_with = choice;
        });
    }
}

// ---------------------------------------------------------------- the selected profile

/// The parameters in the order the shape declares them (a sketch's as listed).
fn ordered_params(detail: &Value) -> Vec<(String, f64)> {
    let params = detail["params"].as_object();
    let values = |name: &str| params.and_then(|p| p.get(name)).and_then(Value::as_f64);
    match detail["shape"].as_str().and_then(ProfileShape::default_for) {
        Some(shape) if !matches!(shape, ProfileShape::Sketch(_)) => {
            shape.params().into_iter().filter_map(|(n, _)| Some((n.clone(), values(&n)?))).collect()
        }
        _ => params.into_iter().flatten().filter_map(|(n, v)| Some((n.clone(), v.as_f64()?))).collect(),
    }
}

/// A value with about four significant digits.
fn number(v: f64) -> String {
    let a = v.abs();
    if !v.is_finite() {
        "—".into()
    } else if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 100.0 {
        format!("{v:.1}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.4}")
    }
}

fn details(ui: &mut Ui, state: &mut AppState, d: &mut ProfilesDialog) {
    let Some((id, detail)) = d.cache.detail.clone() else { return };
    let profile = || Ref::Id(id.0);
    let mut failed = false;
    table::section(ui, "profiles-params", t!("profiles-params"), |ui| {
        let name = detail["name"].as_str().unwrap_or_default().to_string();
        let typed =
            table::row(ui, &t!("prop-name"), |ui| table::commit_text(ui, ("profile-name", id), &name, 200.0, ""));
        if let Some(text) = typed {
            let command = cmd::ProfileSet { profile: profile(), name: Some(text), ..Default::default() };
            failed |= run(state, &mut d.error, None, command).is_none();
        }
        let material = detail["material"].as_str().and_then(ProfileMaterial::from_code).unwrap_or_default();
        let mut choice = material;
        table::row(ui, &t!("prop-material"), |ui| {
            egui::ComboBox::from_id_salt(("profile-material", id)).selected_text(tr(material.key())).show_ui(
                ui,
                |ui| {
                    for m in ProfileMaterial::ALL {
                        ui.selectable_value(&mut choice, m, tr(m.key()));
                    }
                },
            );
        });
        if choice != material {
            let command =
                cmd::ProfileSet { profile: profile(), material: Some(choice.code().into()), ..Default::default() };
            failed |= run(state, &mut d.error, None, command).is_none();
        }
        let shape = detail["shape"].as_str().unwrap_or_default();
        table::readonly_row(ui, &t!("prop-shape"), shape_label(shape));
        for (param, value) in ordered_params(&detail) {
            // Lengths in millimeters; the Z section's lip angle in degrees.
            let degrees = param == "angle";
            let mut shown = if degrees { value } else { value * 1000.0 };
            let r = table::row(ui, &param, |ui| {
                let suffix = if degrees { "°" } else { " mm" };
                ui.add(egui::DragValue::new(&mut shown).speed(0.5).range(0.0..=f64::MAX).max_decimals(2).suffix(suffix))
            });
            if r.changed() {
                let v = if degrees { shown } else { shown / 1000.0 };
                let command = cmd::ProfileSet {
                    profile: profile(),
                    params: Some(BTreeMap::from([(param.clone(), v)])),
                    ..Default::default()
                };
                failed |= run(state, &mut d.error, Some(key(("profile-param", id, &param))), command).is_none();
            }
            if r.drag_stopped() || r.lost_focus() {
                state.session.doc.break_merge();
            }
        }
    });
    if let Some(e) = &d.error {
        ui.label(RichText::new(e).color(PALETTE.danger));
    }
    ui.add_space(6.0);
    let p = &detail["properties"];
    let get = |k: &str| p[k].as_f64().unwrap_or(f64::NAN);
    table::section(ui, "profiles-properties", t!("profiles-properties"), |ui| {
        let rows = [
            ("profile-prop-area", get("area") * 1e4, "cm²"),
            ("profile-prop-mass", get("linear_mass"), "kg/m"),
            ("profile-prop-ix", get("ix") * 1e8, "cm⁴"),
            ("profile-prop-iy", get("iy") * 1e8, "cm⁴"),
            ("profile-prop-wx", get("wx") * 1e6, "cm³"),
            ("profile-prop-wy", get("wy") * 1e6, "cm³"),
            ("profile-prop-rx", get("rx") * 1e2, "cm"),
            ("profile-prop-ry", get("ry") * 1e2, "cm"),
            ("profile-prop-j", get("j") * 1e8, "cm⁴"),
        ];
        for (label, value, unit) in rows {
            table::readonly_row(ui, &tr(label), format!("{} {unit}", number(value)));
        }
        let users: Vec<&str> = detail["used_by"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let users = if users.is_empty() { "—".to_string() } else { users.join(", ") };
        table::readonly_row(ui, &t!("profiles-used-by"), users);
    });
    if detail["shape"] == "sketch" {
        ui.add_space(6.0);
        // Drawing the sketch comes with the profile sketch editor.
        ui.add_enabled(false, egui::Button::new(t!("profiles-edit-sketch")))
            .on_disabled_hover_text(t!("profiles-edit-sketch-later"));
    }
    if failed {
        // Read the profile back, so a refused value shows what it is.
        d.cache.revision = None;
    }
}

/// The outline from `profile_get`, scaled to fit with a 10% margin: filled
/// with the soft tint, holes left open, the centroid's cross and axes, and
/// the overall depth and width.
fn preview(ui: &mut Ui, detail: &Value) {
    let palette = theme::palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(PREVIEW, PREVIEW), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, palette.canvas);
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, palette.border), egui::StrokeKind::Inside);
    let points = |v: &Value| -> Vec<DVec2> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some(DVec2::new(p[0].as_f64()?, p[1].as_f64()?)))
            .filter(|p| p.is_finite())
            .collect()
    };
    let outer = points(&detail["outline"]["outer"]);
    let holes: Vec<Vec<DVec2>> =
        detail["outline"]["holes"].as_array().into_iter().flatten().map(points).filter(|h| h.len() >= 3).collect();
    if outer.len() < 3 {
        return;
    }
    let (min, max) = outer.iter().fold((outer[0], outer[0]), |(a, b), p| (a.min(*p), b.max(*p)));
    let size = max - min;
    let area = rect.shrink(rect.width() * 0.1);
    let scale = (area.width() as f64 / size.x).min(area.height() as f64 / size.y);
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }
    let mid = (min + max) * 0.5;
    let to = |p: DVec2| {
        let c = area.center();
        Pos2::new(c.x + ((p.x - mid.x) * scale) as f32, c.y - ((p.y - mid.y) * scale) as f32)
    };
    let mut mesh = egui::Mesh::default();
    for tri in cc_geom::polygon::triangulate_with_holes(&outer, &holes) {
        let base = mesh.vertices.len() as u32;
        for p in tri {
            mesh.colored_vertex(to(p), palette.accent_soft);
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    painter.add(Shape::mesh(mesh));
    let edge = Stroke::new(1.5, palette.text);
    for ring in std::iter::once(&outer).chain(&holes) {
        painter.add(Shape::closed_line(ring.iter().map(|p| to(*p)).collect(), edge));
    }
    // The centroid: dashed axes across the drawing and a cross.
    let c = &detail["properties"]["centroid"];
    if let (Some(x), Some(y)) = (c[0].as_f64(), c[1].as_f64()) {
        let at = to(DVec2::new(x, y));
        let axis = Stroke::new(1.0, palette.text_faint);
        for line in [
            [Pos2::new(rect.left() + 6.0, at.y), Pos2::new(rect.right() - 6.0, at.y)],
            [Pos2::new(at.x, rect.top() + 6.0), Pos2::new(at.x, rect.bottom() - 6.0)],
        ] {
            painter.extend(Shape::dashed_line(&line, axis, 5.0, 3.0));
        }
        let cross = Stroke::new(1.5, palette.accent);
        painter.line_segment([at - vec2(5.0, 5.0), at + vec2(5.0, 5.0)], cross);
        painter.line_segment([at - vec2(5.0, -5.0), at + vec2(5.0, -5.0)], cross);
    }
    let font = egui::FontId::proportional(11.0);
    let mm = |v: f64| format!("{} mm", number(v * 1000.0));
    let bottom = Pos2::new(rect.center().x, rect.bottom() - 4.0);
    painter.text(bottom, egui::Align2::CENTER_BOTTOM, format!("bf = {}", mm(size.x)), font.clone(), palette.text_muted);
    let left = Pos2::new(rect.left() + 6.0, rect.top() + 4.0);
    painter.text(left, egui::Align2::LEFT_TOP, format!("d = {}", mm(size.y)), font, palette.text_muted);
}

// ---------------------------------------------------------------- the catalog

fn open_catalog(state: &mut AppState, d: &mut ProfilesDialog) {
    let out = state.session.run(cmd::ProfileCatalog::default()).unwrap_or_default();
    let series_list: Vec<(String, String)> = out
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let code = s["code"].as_str()?.to_string();
            let name = s["name"].as_str().map_or_else(|| code.clone(), str::to_string);
            Some((code, name))
        })
        .collect();
    let Some((first, _)) = series_list.first().cloned() else { return };
    d.catalog =
        Some(Catalog { series: first, filter: String::new(), picked: BTreeSet::new(), series_list, rows: None });
}

/// The designations of the catalog's series that pass its filter, read again
/// when either changes (a new series also clears what was picked).
fn catalog_rows(state: &mut AppState, c: &mut Catalog) {
    let current = c.rows.as_ref().map(|(s, f, _)| (s.as_str(), f.as_str()));
    if current == Some((c.series.as_str(), c.filter.as_str())) {
        return;
    }
    if c.rows.as_ref().is_some_and(|(s, _, _)| *s != c.series) {
        c.picked.clear();
    }
    let search: String = c.filter.chars().take(MAX_NAME).collect();
    let command = cmd::ProfileCatalog { series: Some(c.series.clone()), search: Some(search) };
    let out = state.session.run(command).unwrap_or_default();
    let rows = out["rows"].as_array().into_iter().flatten().filter_map(|r| Some(r["designation"].as_str()?.into()));
    c.rows = Some((c.series.clone(), c.filter.clone(), rows.collect()));
}

fn catalog_dialog(ctx: &Context, state: &mut AppState, d: &mut ProfilesDialog) {
    let Some(mut c) = d.catalog.take() else { return };
    catalog_rows(state, &mut c);
    let mut add = 0;
    let response = Dialog::new("profiles-catalog", t!("profiles-catalog-title")).width(440.0).show(
        ctx,
        |ui| {
            egui::Grid::new("profiles-catalog-options").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(theme::muted(t!("profiles-catalog-series")));
                let name = c.series_list.iter().find(|(code, _)| *code == c.series).map(|(_, n)| n.clone());
                let mut series = c.series.clone();
                egui::ComboBox::from_id_salt("profiles-catalog-series")
                    .selected_text(name.unwrap_or_default())
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for (code, name) in &c.series_list {
                            ui.selectable_value(&mut series, code.clone(), name);
                        }
                    });
                c.series = series;
                ui.end_row();
                ui.label(theme::muted(t!("profiles-catalog-filter")));
                ui.add(egui::TextEdit::singleline(&mut c.filter).char_limit(MAX_NAME).desired_width(300.0));
                ui.end_row();
            });
            ui.add_space(4.0);
            egui::Frame::new().stroke(Stroke::new(1.0, PALETTE.border)).inner_margin(4).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("profiles-catalog-rows").max_height(320.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let rows = c.rows.as_ref().map(|(_, _, r)| r.as_slice()).unwrap_or_default();
                    for designation in rows {
                        let mut on = c.picked.contains(designation);
                        if ui.checkbox(&mut on, designation).changed() {
                            if on {
                                c.picked.insert(designation.clone());
                            } else {
                                c.picked.remove(designation);
                            }
                        }
                    }
                });
            });
        },
        |f| {
            add = f.primary(t!("profiles-catalog-add"));
            f.secondary(t!("action-cancel"));
        },
    );
    let done = match response.clicked {
        Some(i) if i == add => {
            if c.picked.is_empty() {
                false
            } else {
                let command = cmd::ProfileImport {
                    series: c.series.clone(),
                    designations: c.picked.iter().cloned().collect(),
                    on_clash: None,
                };
                match run(state, &mut d.error, None, command) {
                    Some(out) => {
                        if let Some(id) = profile_id(&out["imported"][0]) {
                            d.selected = Some(id);
                        }
                        true
                    }
                    None => false,
                }
            }
        }
        Some(_) => true,
        None => response.dismissed,
    };
    if !done {
        d.catalog = Some(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_keep_the_shape_order() {
        let detail =
            serde_json::json!({"shape": "i", "params": {"bf": 0.1, "d": 0.2, "r": 0.01, "tf": 0.008, "tw": 0.005}});
        let names: Vec<String> = ordered_params(&detail).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["d", "bf", "tw", "tf", "r"]);
    }

    #[test]
    fn shape_keys_use_hyphens() {
        assert_eq!(shape_key("cold_lipped_u"), "profile-shape-cold-lipped-u");
        assert_eq!(material_for("cold_z"), ProfileMaterial::ColdFormed);
    }

    #[test]
    fn number_keeps_four_digits() {
        assert_eq!(number(28.4812), "28.48");
        assert_eq!(number(1943.2), "1943");
        assert_eq!(number(f64::NAN), "—");
    }
}
