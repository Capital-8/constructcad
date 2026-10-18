//! Properties palette: instance parameters of the selection, or of the active view.

use crate::actions;
use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use cc_core::{Category, Element, ElementId, ElementKind, RoofShape, SheetSize, ViewId, ViewKind};
use cc_views::format;
use egui::{DragValue, Response, RichText, Ui};
use std::hash::{Hash, Hasher};

const SCALES: [u32; 9] = [10, 20, 25, 50, 75, 100, 200, 500, 1000];

fn key(parts: impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

/// End coalescing when a drag or text edit finishes.
fn finish(state: &mut AppState, r: &Response) {
    if r.drag_stopped() || r.lost_focus() {
        state.doc.break_merge();
    }
}

fn modify(state: &mut AppState, id: ElementId, field: &str, f: impl FnOnce(&mut Element)) {
    state.doc.edit_merged(t!("tx-modify"), key((id, field)), |e| {
        if let Some(el) = e.element_mut(id) {
            f(el);
        }
    });
}

fn length_row(
    ui: &mut Ui,
    state: &mut AppState,
    label: &str,
    id: ElementId,
    field: &str,
    value: f64,
    set: impl FnOnce(&mut Element, f64),
) {
    ui.label(label);
    let mut v = value;
    let r = ui.add(DragValue::new(&mut v).speed(0.01).fixed_decimals(3).suffix(" m"));
    if r.changed() {
        modify(state, id, field, |el| set(el, v));
    }
    finish(state, &r);
    ui.end_row();
}

fn readonly_row(ui: &mut Ui, label: &str, value: String) {
    ui.label(label);
    ui.label(RichText::new(value).weak());
    ui.end_row();
}

fn level_row(ui: &mut Ui, state: &mut AppState, el: &Element) {
    ui.label(t!("prop-level"));
    let mut level = el.level;
    let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
    egui::ComboBox::from_id_salt(("level", el.id.0))
        .selected_text(state.project().level_name(el.level).to_string())
        .show_ui(ui, |ui| {
            for (id, name) in levels {
                ui.selectable_value(&mut level, id, name);
            }
        });
    if level != el.level {
        let hosted: Vec<_> = state.project().hosted_by(el.id).map(|h| h.id).collect();
        state.doc.edit(t!("tx-modify"), |e| {
            for id in hosted.into_iter().chain([el.id]) {
                if let Some(x) = e.element_mut(id) {
                    x.level = level;
                }
            }
        });
    }
    ui.end_row();
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new(t!("props-title")).strong());
    ui.separator();
    egui::ScrollArea::vertical().id_salt("props").auto_shrink([false, false]).show(ui, |ui| {
        if let Some((sheet, index)) = state.sheet_selection
            && state.active_view == Some(sheet)
        {
            viewport_props(ui, state, sheet, index);
            return;
        }
        match state.selection.len() {
            0 => match state.active_view {
                Some(v) => view_props(ui, state, v),
                None => {
                    ui.label(RichText::new(t!("props-nothing")).weak());
                }
            },
            1 => {
                let id = *state.selection.iter().next().unwrap();
                element_props(ui, state, id);
            }
            n => multi_props(ui, state, n),
        }
    });
}

fn multi_props(ui: &mut Ui, state: &mut AppState, n: usize) {
    ui.label(t!("props-multi", "count" => n));
    let mut counts = std::collections::BTreeMap::new();
    for id in &state.selection {
        if let Some(e) = state.project().element(*id) {
            *counts.entry(e.category()).or_insert(0) += 1;
        }
    }
    for (c, k) in counts {
        ui.label(format!("• {}: {k}", actions::t_category(c)));
    }
    ui.add_space(8.0);
    if ui.button(t!("action-delete")).clicked() {
        state.delete_selection();
    }
}

fn element_props(ui: &mut Ui, state: &mut AppState, id: ElementId) {
    let Some(el) = state.project().element(id).cloned() else { return };
    let category = el.category();
    ui.label(RichText::new(format!("{} ({})", actions::t_category(category), el.id)).heading());

    // Type selector.
    let types: Vec<_> = state.project().types_of(category).map(|t| (t.id, t.name.clone())).collect();
    let mut type_id = el.type_id;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(("type", id.0))
            .width(150.0)
            .selected_text(state.project().type_name(el.type_id).to_string())
            .show_ui(ui, |ui| {
                for (tid, name) in &types {
                    ui.selectable_value(&mut type_id, *tid, name);
                }
            });
        if ui.button(t!("props-edit-type")).clicked() {
            state.dialogs.type_editor = Some(el.type_id);
        }
    });
    if type_id != el.type_id {
        state.doc.edit(t!("tx-change-type"), |e| {
            if let Some(x) = e.element_mut(id) {
                x.type_id = type_id;
            }
        });
    }
    ui.add_space(4.0);

    egui::Grid::new(("props", id.0)).num_columns(2).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
        match &el.kind {
            ElementKind::Wall(w) => {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-base-offset"), id, "base", w.base_offset, |e, v| {
                    if let ElementKind::Wall(w) = &mut e.kind {
                        w.base_offset = v;
                    }
                });
                length_row(ui, state, &t!("prop-height"), id, "height", w.height, |e, v| {
                    if let ElementKind::Wall(w) = &mut e.kind {
                        w.height = v.max(0.1);
                    }
                });
                for (label, field, value) in [
                    (t!("prop-start-x"), "sx", w.start.x),
                    (t!("prop-start-y"), "sy", w.start.y),
                    (t!("prop-end-x"), "ex", w.end.x),
                    (t!("prop-end-y"), "ey", w.end.y),
                ] {
                    length_row(ui, state, &label, id, field, value, |e, v| {
                        if let ElementKind::Wall(w) = &mut e.kind {
                            match field {
                                "sx" => w.start.x = v,
                                "sy" => w.start.y = v,
                                "ex" => w.end.x = v,
                                _ => w.end.y = v,
                            }
                        }
                    });
                }
                readonly_row(ui, &t!("prop-length"), format!("{} m", format::meters(w.length())));
                readonly_row(
                    ui,
                    &t!("prop-thickness"),
                    format!("{} m", format::meters(state.project().thickness_of(&el))),
                );
                readonly_row(ui, &t!("prop-volume"), format!("{} m³", format::number(state.geom.volume(id), 3)));
            }
            ElementKind::Door(h) | ElementKind::Window(h) => {
                let host = state.project().element(h.host).map(|w| w.id.to_string()).unwrap_or_default();
                readonly_row(ui, &t!("prop-level"), state.project().level_name(el.level).to_string());
                readonly_row(ui, &t!("prop-host"), host);
                length_row(ui, state, &t!("prop-offset"), id, "offset", h.offset, |e, v| {
                    if let Some(h) = e.as_hosted_mut() {
                        h.offset = v;
                    }
                });
                if category == Category::Window {
                    length_row(ui, state, &t!("prop-sill"), id, "sill", h.sill_height, |e, v| {
                        if let Some(h) = e.as_hosted_mut() {
                            h.sill_height = v.max(0.0);
                        }
                    });
                }
                for (label, field, value) in
                    [(t!("prop-flip-facing"), "ff", h.flip_facing), (t!("prop-flip-hand"), "fh", h.flip_hand)]
                {
                    ui.label(label);
                    let mut v = value;
                    if ui.checkbox(&mut v, "").changed() {
                        state.doc.edit(t!("tx-modify"), |e| {
                            if let Some(h) = e.element_mut(id).and_then(|x| x.as_hosted_mut()) {
                                if field == "ff" {
                                    h.flip_facing = v;
                                } else {
                                    h.flip_hand = v;
                                }
                            }
                        });
                    }
                    ui.end_row();
                }
                if let Some((w, hh)) = state.project().element_type(el.type_id).and_then(|t| t.opening_size()) {
                    readonly_row(ui, &t!("col-width"), format!("{} m", format::meters(w)));
                    readonly_row(ui, &t!("col-height"), format!("{} m", format::meters(hh)));
                }
            }
            ElementKind::Floor(f) => {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-height-offset"), id, "hoff", f.height_offset, |e, v| {
                    if let ElementKind::Floor(f) = &mut e.kind {
                        f.height_offset = v;
                    }
                });
                readonly_row(
                    ui,
                    &t!("col-area"),
                    format!("{} m²", format::number(cc_geom::polygon::area(&f.boundary), 2)),
                );
            }
            ElementKind::Ceiling(c) => {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-height-offset"), id, "hoff", c.height_offset, |e, v| {
                    if let ElementKind::Ceiling(c) = &mut e.kind {
                        c.height_offset = v;
                    }
                });
                readonly_row(
                    ui,
                    &t!("col-area"),
                    format!("{} m²", format::number(cc_geom::polygon::area(&c.boundary), 2)),
                );
            }
            ElementKind::Roof(r) => {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-base-offset"), id, "base", r.base_offset, |e, v| {
                    if let ElementKind::Roof(r) = &mut e.kind {
                        r.base_offset = v;
                    }
                });
                length_row(ui, state, &t!("prop-overhang"), id, "overhang", r.overhang, |e, v| {
                    if let ElementKind::Roof(r) = &mut e.kind {
                        r.overhang = v.max(0.0);
                    }
                });
                ui.label(t!("prop-roof-shape"));
                let mut shed = matches!(r.shape, RoofShape::Shed { .. });
                ui.horizontal(|ui| {
                    ui.radio_value(&mut shed, false, t!("roof-flat"));
                    ui.radio_value(&mut shed, true, t!("roof-shed"));
                });
                ui.end_row();
                if shed != matches!(r.shape, RoofShape::Shed { .. }) {
                    state.doc.edit(t!("tx-modify"), |e| {
                        if let Some(ElementKind::Roof(r)) = e.element_mut(id).map(|x| &mut x.kind) {
                            r.shape = if shed {
                                RoofShape::Shed { slope: 0.15, direction_deg: 90.0 }
                            } else {
                                RoofShape::Flat
                            };
                        }
                    });
                }
                if let RoofShape::Shed { slope, direction_deg } = r.shape {
                    ui.label(t!("prop-slope"));
                    let mut pct = slope * 100.0;
                    let rs = ui.add(DragValue::new(&mut pct).speed(0.5).range(0.0..=200.0).suffix(" %"));
                    ui.end_row();
                    ui.label(t!("prop-direction"));
                    let mut dir = direction_deg;
                    let rd = ui.add(DragValue::new(&mut dir).speed(1.0).suffix("°"));
                    ui.end_row();
                    if rs.changed() || rd.changed() {
                        modify(state, id, "shape", |e| {
                            if let ElementKind::Roof(r) = &mut e.kind {
                                r.shape = RoofShape::Shed { slope: pct / 100.0, direction_deg: dir.rem_euclid(360.0) };
                            }
                        });
                    }
                    finish(state, &rs);
                    finish(state, &rd);
                }
            }
        }
        for (label, field, value) in
            [(t!("col-mark"), "mark", el.mark.clone()), (t!("col-comments"), "comments", el.comments.clone())]
        {
            ui.label(label);
            let mut text = value;
            let r = ui.text_edit_singleline(&mut text);
            if r.changed() {
                modify(state, id, field, |e| {
                    if field == "mark" {
                        e.mark = text;
                    } else {
                        e.comments = text;
                    }
                });
            }
            finish(state, &r);
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    if ui.button(t!("action-delete")).clicked() {
        state.delete_selection();
    }
}

fn view_text_row(
    ui: &mut Ui,
    state: &mut AppState,
    view: ViewId,
    label: &str,
    field: &str,
    value: String,
    set: impl FnOnce(&mut cc_core::View, String),
) {
    ui.label(label);
    let mut text = value;
    let r = ui.text_edit_singleline(&mut text);
    if r.changed() {
        state.doc.edit_merged(t!("tx-modify-view"), key((view, field)), |e| {
            if let Some(v) = e.view_mut(view) {
                set(v, text);
            }
        });
    }
    finish(state, &r);
    ui.end_row();
}

fn view_props(ui: &mut Ui, state: &mut AppState, id: ViewId) {
    let Some(view) = state.project().view(id).cloned() else { return };
    ui.label(RichText::new(tr(view.category().key())).heading());
    egui::Grid::new(("vprops", id.0)).num_columns(2).striped(true).spacing([10.0, 6.0]).show(ui, |ui| {
        view_text_row(ui, state, id, &t!("prop-name"), "name", view.name.clone(), |v, s| v.name = s);
        let graphical = !matches!(view.kind, ViewKind::Schedule { .. } | ViewKind::Sheet { .. });
        if graphical {
            ui.label(t!("prop-scale"));
            let mut scale = view.scale;
            egui::ComboBox::from_id_salt(("scale", id.0)).selected_text(format!("1:{scale}")).show_ui(ui, |ui| {
                for s in SCALES {
                    ui.selectable_value(&mut scale, s, format!("1:{s}"));
                }
            });
            if scale != view.scale {
                state.doc.edit(t!("tx-modify-view"), |e| {
                    if let Some(v) = e.view_mut(id) {
                        v.scale = scale;
                    }
                });
            }
            ui.end_row();
        }
        match &view.kind {
            ViewKind::FloorPlan { level, cut_height } | ViewKind::CeilingPlan { level, cut_height } => {
                readonly_row(ui, &t!("prop-level"), state.project().level_name(*level).to_string());
                ui.label(t!("prop-cut-height"));
                let mut h = *cut_height;
                let r = ui.add(DragValue::new(&mut h).speed(0.01).range(0.0..=20.0).fixed_decimals(2).suffix(" m"));
                if r.changed() {
                    state.doc.edit_merged(t!("tx-modify-view"), key((id, "cut")), |e| {
                        if let Some(ViewKind::FloorPlan { cut_height, .. } | ViewKind::CeilingPlan { cut_height, .. }) =
                            e.view_mut(id).map(|v| &mut v.kind)
                        {
                            *cut_height = h;
                        }
                    });
                }
                finish(state, &r);
                ui.end_row();
            }
            ViewKind::Section { depth, .. } => {
                ui.label(t!("prop-depth"));
                let mut d = *depth;
                let r = ui.add(DragValue::new(&mut d).speed(0.05).range(0.1..=500.0).fixed_decimals(2).suffix(" m"));
                if r.changed() {
                    state.doc.edit_merged(t!("tx-modify-view"), key((id, "depth")), |e| {
                        if let Some(ViewKind::Section { depth, .. }) = e.view_mut(id).map(|v| &mut v.kind) {
                            *depth = d;
                        }
                    });
                }
                finish(state, &r);
                ui.end_row();
                ui.label("");
                if ui.button(t!("prop-flip-section")).clicked() {
                    state.doc.edit(t!("tx-modify-view"), |e| {
                        if let Some(ViewKind::Section { start, end, .. }) = e.view_mut(id).map(|v| &mut v.kind) {
                            std::mem::swap(start, end);
                        }
                    });
                }
                ui.end_row();
            }
            ViewKind::ThreeD { .. } => {
                ui.label("");
                if ui.button(t!("action-zoom-fit")).clicked()
                    && let Some(v3) = state.view3d.get_mut(&id)
                {
                    v3.fit_requested = true;
                }
                ui.end_row();
            }
            ViewKind::Schedule { category } => readonly_row(ui, &t!("prop-category"), actions::t_category(*category)),
            ViewKind::Sheet { number, size, .. } => {
                view_text_row(ui, state, id, &t!("prop-sheet-number"), "number", number.clone(), |v, s| {
                    if let ViewKind::Sheet { number, .. } = &mut v.kind {
                        *number = s;
                    }
                });
                ui.label(t!("prop-sheet-size"));
                let mut sz = *size;
                egui::ComboBox::from_id_salt(("size", id.0)).selected_text(sz.label()).show_ui(ui, |ui| {
                    for s in SheetSize::ALL {
                        ui.selectable_value(&mut sz, s, s.label());
                    }
                });
                if sz != *size {
                    state.doc.edit(t!("tx-modify-view"), |e| {
                        if let Some(ViewKind::Sheet { size, .. }) = e.view_mut(id).map(|v| &mut v.kind) {
                            *size = sz;
                        }
                    });
                    if let Some(st) = state.view2d.get_mut(&id) {
                        st.fitted = false;
                    }
                }
                ui.end_row();
            }
            ViewKind::Legend { .. } | ViewKind::Elevation { .. } => {}
        }
    });

    match &view.kind {
        ViewKind::Legend { types } => legend_types(ui, state, id, types),
        ViewKind::Sheet { viewports, .. } => sheet_views(ui, state, id, viewports),
        _ => {}
    }
}

fn legend_types(ui: &mut Ui, state: &mut AppState, id: ViewId, current: &[cc_core::ElementTypeId]) {
    ui.add_space(6.0);
    ui.label(RichText::new(t!("prop-legend-types")).strong());
    let all: Vec<_> = state.project().types.values().map(|t| (t.id, t.name.clone())).collect();
    for (tid, name) in all {
        let mut on = current.contains(&tid);
        if ui.checkbox(&mut on, name).changed() {
            state.doc.edit(t!("tx-modify-view"), |e| {
                if let Some(ViewKind::Legend { types }) = e.view_mut(id).map(|v| &mut v.kind) {
                    if on {
                        types.push(tid);
                    } else {
                        types.retain(|t| *t != tid);
                    }
                }
            });
        }
    }
}

fn sheet_views(ui: &mut Ui, state: &mut AppState, sheet: ViewId, viewports: &[cc_core::Viewport]) {
    ui.add_space(6.0);
    ui.label(RichText::new(t!("prop-sheet-views")).strong());
    for (i, vp) in viewports.iter().enumerate() {
        let name = state.project().view(vp.view).map(|v| v.name.clone()).unwrap_or_default();
        if ui.selectable_label(state.sheet_selection == Some((sheet, i)), name).clicked() {
            state.sheet_selection = Some((sheet, i));
        }
    }
    let candidates: Vec<_> = state
        .project()
        .views
        .values()
        .filter(|v| v.placeable() && actions::sheet_of(state.project(), v.id).is_none())
        .map(|v| (v.id, state.view_title(v.id)))
        .collect();
    ui.menu_button(t!("prop-add-view"), |ui| {
        if candidates.is_empty() {
            ui.label(RichText::new(t!("prop-no-views")).weak());
        }
        for (vid, title) in candidates {
            if ui.button(title).clicked() {
                actions::place_on_sheet(state, sheet, vid);
                ui.close();
            }
        }
    });
}

fn viewport_props(ui: &mut Ui, state: &mut AppState, sheet: ViewId, index: usize) {
    let Some(ViewKind::Sheet { viewports, .. }) = state.project().view(sheet).map(|v| v.kind.clone()) else { return };
    let Some(vp) = viewports.get(index).cloned() else {
        state.sheet_selection = None;
        return;
    };
    ui.label(RichText::new(t!("props-viewport")).heading());
    ui.label(state.view_title(vp.view));
    ui.add_space(6.0);
    egui::Grid::new("vp").num_columns(2).show(ui, |ui| {
        for (label, axis) in [(t!("prop-center-x"), 0), (t!("prop-center-y"), 1)] {
            ui.label(label);
            let mut v = vp.center[axis];
            let r = ui.add(DragValue::new(&mut v).speed(1.0).fixed_decimals(1).suffix(" mm"));
            if r.changed() {
                state.doc.edit_merged(t!("tx-move-viewport"), key((sheet, index, axis)), |e| {
                    if let Some(ViewKind::Sheet { viewports, .. }) = e.view_mut(sheet).map(|v| &mut v.kind)
                        && let Some(vp) = viewports.get_mut(index)
                    {
                        vp.center[axis] = v;
                    }
                });
            }
            finish(state, &r);
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(t!("browser-open")).clicked() {
            state.open_view(vp.view);
        }
        if ui.button(t!("prop-remove-from-sheet")).clicked() {
            actions::remove_viewport(state, sheet, index);
        }
    });
}
