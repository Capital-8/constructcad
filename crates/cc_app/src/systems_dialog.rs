//! Plumbing systems and electrical circuits: names, codes, colors, the
//! board feeding each circuit and its voltage, with the circuits' loads.
//! Every change is an engine command (`system_*`); Cancel undoes them all.

use crate::dialog::{Dialog, session};
use crate::state::AppState;
use crate::t;
use crate::theme::PALETTE;
use cc_core::{DeviceKind, ElementId, SystemId, SystemKind};
use cc_engine::Ref;
use cc_engine::commands::mep;
use egui::{Context, RichText, Ui};

fn key(parts: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

fn done(state: &mut AppState, r: &egui::Response) {
    if r.drag_stopped() || r.lost_focus() {
        state.session.doc.break_merge();
    }
}

/// The distribution boards of the project: (element, label).
pub fn panels(state: &AppState) -> Vec<(ElementId, String)> {
    let p = state.project();
    p.elements
        .values()
        .filter(|e| p.mep_data(e).and_then(|m| m.device()) == Some(DeviceKind::Panel))
        .map(|e| {
            (e.id, if e.mark.is_empty() { format!("{} {}", p.type_name(e.type_id), e.id.0) } else { e.mark.clone() })
        })
        .collect()
}

/// A choice of the systems that take `electrical` elements (or any), with "none".
pub fn system_combo(
    ui: &mut Ui,
    state: &AppState,
    id: impl std::hash::Hash + std::fmt::Debug,
    electrical: Option<bool>,
    current: &mut Option<SystemId>,
) -> bool {
    let p = state.project();
    let label = |id: SystemId| p.system(id).map_or(String::new(), |s| format!("{} – {}", s.label(), s.name));
    let before = *current;
    let text = current.map_or(t!("value-none"), label);
    egui::ComboBox::from_id_salt(id).selected_text(text).width(170.0).show_ui(ui, |ui| {
        ui.selectable_value(current, None, t!("value-none"));
        for (sid, s) in &p.systems {
            if electrical.is_none_or(|e| e == s.kind.is_electrical()) {
                ui.selectable_value(current, Some(*sid), label(*sid));
            }
        }
    });
    *current != before
}

pub fn show(ctx: &Context, state: &mut AppState) {
    if !state.dialogs.systems {
        return;
    }
    session::begin(state);
    let mut ok = 0;
    let response = Dialog::new("systems", t!("dlg-systems")).width(720.0).show(
        ctx,
        |ui| body(ui, state),
        |f| {
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    if crate::dialogs::ok_cancel(state, &response, ok) {
        state.dialogs.systems = false;
    }
}

fn body(ui: &mut Ui, state: &mut AppState) {
    let systems: Vec<(SystemId, cc_core::MepSystem)> =
        state.project().systems.iter().map(|(id, s)| (*id, s.clone())).collect();
    let panels = panels(state);
    let header = |ui: &mut Ui, text: String| {
        ui.label(RichText::new(text).color(PALETTE.text_muted));
    };
    for electrical in [false, true] {
        ui.label(RichText::new(if electrical { t!("systems-circuits") } else { t!("systems-plumbing") }).strong());
        egui::Grid::new(("systems", electrical)).num_columns(7).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
            header(ui, String::new());
            header(ui, t!("info-number"));
            header(ui, t!("prop-name"));
            header(ui, t!("prop-system-kind"));
            if electrical {
                header(ui, t!("device-panel"));
                header(ui, t!("prop-voltage"));
            } else {
                ui.label("");
                ui.label("");
            }
            ui.label("");
            ui.end_row();
            for (id, s) in systems.iter().filter(|(_, s)| s.kind.is_electrical() == electrical) {
                row(ui, state, *id, s, &panels);
                ui.end_row();
            }
        });
        let label = if electrical { t!("systems-add-circuit") } else { t!("systems-add-system") };
        if ui.button(label).clicked() {
            let kind = if electrical { SystemKind::Power } else { SystemKind::ColdWater };
            let panel = if electrical { panels.first().map(|(id, _)| id.0) } else { None };
            let command = mep::SystemCreate { kind: kind.name().into(), panel, ..Default::default() };
            crate::actions::run_command(state, command);
        }
        ui.add_space(8.0);
    }
    loads(ui, state);
    fittings(ui, state);
}

/// The fittings takeoff: how many of each kind, angle and diameter.
fn fittings(ui: &mut Ui, state: &mut AppState) {
    let Ok(out) = state.session.run(mep::MepFittings { level: None }) else { return };
    let rows = out["fittings"].as_array().cloned().unwrap_or_default();
    if rows.is_empty() {
        return;
    }
    ui.add_space(8.0);
    ui.label(RichText::new(t!("systems-fittings")).strong());
    egui::Grid::new("fittings").num_columns(4).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
        for h in [t!("col-system"), t!("col-fitting"), t!("col-diameter"), t!("col-quantity")] {
            ui.label(RichText::new(h).color(PALETTE.text_muted));
        }
        ui.end_row();
        for r in &rows {
            let kind = crate::i18n::tr(&format!("fitting-{}", r["kind"].as_str().unwrap_or("")));
            let angle = r["angle"].as_f64().map_or(String::new(), |a| format!(" {a:.0}°"));
            let category = if r["category"] == "conduit" { t!("category-conduit") } else { String::new() };
            let system = r["system"].as_str().map_or(category, str::to_string);
            let sizes: Vec<String> =
                r["diameters_mm"].as_array().into_iter().flatten().map(|d| format!("Ø{d}")).collect();
            ui.label(system);
            ui.label(format!("{kind}{angle}"));
            ui.label(sizes.join(" × "));
            ui.label(r["count"].to_string());
            ui.end_row();
        }
    });
}

fn row(ui: &mut Ui, state: &mut AppState, id: SystemId, s: &cc_core::MepSystem, panels: &[(ElementId, String)]) {
    let mut color = s.color();
    if ui.color_edit_button_srgb(&mut color).changed() {
        crate::engine_edit::system(state, id, Some(key((id, "color"))), |x| x.color = Some(color));
    }
    let mut code = s.code.clone();
    let r = ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut code));
    if r.changed() {
        crate::engine_edit::system(state, id, Some(key((id, "code"))), |x| x.code = code);
    }
    done(state, &r);
    let mut name = s.name.clone();
    let r = ui.add_sized([180.0, 22.0], egui::TextEdit::singleline(&mut name));
    if r.changed() && !name.trim().is_empty() {
        crate::engine_edit::system(state, id, Some(key((id, "name"))), |x| x.name = name);
    }
    done(state, &r);
    let mut kind = s.kind;
    egui::ComboBox::from_id_salt(("system-kind", id)).selected_text(crate::i18n::tr(kind.key())).width(150.0).show_ui(
        ui,
        |ui| {
            for k in SystemKind::ALL.into_iter().filter(|k| k.is_electrical() == s.kind.is_electrical()) {
                ui.selectable_value(&mut kind, k, crate::i18n::tr(k.key()));
            }
        },
    );
    if kind != s.kind {
        crate::engine_edit::system(state, id, None, |x| x.kind = kind);
    }
    if s.kind.is_electrical() {
        let mut panel = s.panel;
        let text =
            panel.and_then(|p| panels.iter().find(|(e, _)| *e == p)).map_or(t!("value-none"), |(_, l)| l.clone());
        egui::ComboBox::from_id_salt(("system-panel", id)).selected_text(text).width(140.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut panel, None, t!("value-none"));
            for (e, label) in panels {
                ui.selectable_value(&mut panel, Some(*e), label);
            }
        });
        if panel != s.panel {
            crate::engine_edit::system(state, id, None, |x| x.panel = panel);
        }
        let mut volts = s.voltage;
        let r = ui.add(egui::DragValue::new(&mut volts).range(1.0..=1000.0).suffix(" V"));
        if r.changed() {
            crate::engine_edit::system(state, id, Some(key((id, "volts"))), |x| x.voltage = volts);
        }
        done(state, &r);
    } else {
        ui.label("");
        ui.label("");
    }
    if ui.button(t!("action-delete")).clicked() {
        crate::actions::run_command(state, mep::SystemDelete { system: Ref::Id(id.0) });
    }
}

/// The circuits' installed loads and currents, per board.
fn loads(ui: &mut Ui, state: &mut AppState) {
    let Ok(out) = state.session.run(mep::CircuitLoads { panel: None }) else { return };
    let rows = out["circuits"].as_array().cloned().unwrap_or_default();
    if rows.is_empty() {
        return;
    }
    ui.label(RichText::new(t!("systems-loads")).strong());
    let number = |v: &serde_json::Value, d: usize| cc_views::format::number(v.as_f64().unwrap_or(0.0), d);
    egui::Grid::new("loads").num_columns(5).striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
        for h in [t!("info-number"), t!("prop-name"), t!("col-count"), t!("col-power"), t!("col-current")] {
            ui.label(RichText::new(h).color(PALETTE.text_muted));
        }
        ui.end_row();
        for r in &rows {
            ui.label(r["code"].as_str().unwrap_or(""));
            ui.label(r["name"].as_str().unwrap_or(""));
            ui.label(r["devices"].to_string());
            ui.label(format!("{} VA", number(&r["power_va"], 0)));
            ui.label(format!("{} A", number(&r["current_a"], 1)));
            ui.end_row();
        }
    });
    let total: f64 = rows.iter().filter_map(|r| r["power_va"].as_f64()).sum();
    ui.label(
        RichText::new(t!("systems-total-load", "power" => cc_views::format::number(total, 0)))
            .color(PALETTE.text_muted),
    );
}
