//! Properties palette: instance parameters of the selection, or of the active view.
//!
//! A header block names what is shown (and its type, with "Edit Type"); below
//! it, parameters are listed in collapsible sections of a two-column table.

use crate::actions;
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::state::AppState;
use crate::t;
use crate::table::{header_block, padded, readonly_row, row, section};
use crate::theme::{self, PALETTE};
use cc_core::{Category, Element, ElementId, ElementKind, RoofShape, SheetSize, ViewId, ViewKind};
use cc_views::format;
use egui::{DragValue, Response, RichText, Sense, Ui, vec2};
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
        state.session.doc.break_merge();
    }
}

fn modify(state: &mut AppState, id: ElementId, field: &str, f: impl FnOnce(&mut Element)) {
    crate::engine_edit::element(state, id, Some(key((id, field))), f);
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
    let mut v = value;
    let r = row(ui, label, |ui| ui.add(crate::properties::length_drag(&mut v).speed(0.01)));
    if r.changed() {
        modify(state, id, field, |el| set(el, v));
    }
    finish(state, &r);
}

fn level_row(ui: &mut Ui, state: &mut AppState, el: &Element) {
    let mut level = el.level;
    let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
    let selected = state.project().level_name(el.level).to_string();
    row(ui, &t!("prop-level"), |ui| {
        egui::ComboBox::from_id_salt(("level", el.id.0)).selected_text(selected).show_ui(ui, |ui| {
            for (id, name) in levels {
                ui.selectable_value(&mut level, id, name);
            }
        });
    });
    // The engine moves the doors and windows of a wall along with it.
    if level != el.level {
        crate::engine_edit::element(state, el.id, None, |x| x.level = level);
    }
}

/// The value of a dimension: typing a new one (Enter) moves what it measures.
fn dimension_value_row(ui: &mut Ui, state: &mut AppState, id: ElementId, d: &cc_core::Dimension) {
    let current = match d.kind {
        cc_core::DimensionKind::Angular { .. } => format::angle(d.value().to_degrees()),
        cc_core::DimensionKind::Radial | cc_core::DimensionKind::Diameter => {
            readonly_row(ui, &t!("prop-value"), format::length(d.value()));
            return;
        }
        _ => format::length(d.value()),
    };
    let key = ui.make_persistent_id(("dim-value", id.0));
    let mut text = ui.data(|m| m.get_temp::<String>(key)).unwrap_or(current);
    let r = row(ui, &t!("prop-value"), |ui| ui.add(egui::TextEdit::singleline(&mut text)));
    if r.has_focus() {
        ui.data_mut(|m| m.insert_temp(key, text.clone()));
    } else {
        ui.data_mut(|m| m.remove::<String>(key));
    }
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        let digits = text.trim_end_matches(|c: char| !c.is_ascii_digit()).to_string();
        if let Err(message) = crate::dim_edit::apply_text(state, &crate::dim_edit::Target::Dimension(id), &digits) {
            state.message = message;
        }
    }
}

fn text_row(ui: &mut Ui, label: &str, value: String) -> (Response, String) {
    let mut text = value;
    let r = row(ui, label, |ui| ui.text_edit_singleline(&mut text));
    (r, text)
}

/// Panel entry point.
pub fn show(ui: &mut Ui, state: &mut AppState) {
    theme::panel_header(ui, &t!("props-title"), None);
    egui::ScrollArea::vertical().id_salt("props").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
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
                    padded(ui, |ui| {
                        ui.label(RichText::new(t!("props-nothing")).color(PALETTE.text_faint));
                    });
                }
            },
            1 => {
                if let Some(&id) = state.selection.iter().next() {
                    element_props(ui, state, id);
                }
            }
            n => multi_props(ui, state, n),
        }
    });
}

fn multi_props(ui: &mut Ui, state: &mut AppState, n: usize) {
    padded(ui, |ui| {
        header_block(ui, Icon::Select, &t!("props-multi", "count" => n), "");
        let mut counts = std::collections::BTreeMap::new();
        for id in &state.selection {
            if let Some(e) = state.project().element(*id) {
                *counts.entry(e.category()).or_insert(0) += 1;
            }
        }
        for (c, k) in counts {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                icons::paint(ui.painter(), r, crate::ribbon::category_icon(c), PALETTE.text_muted);
                ui.label(format!("{}: {k}", actions::t_category(c)));
            });
        }
        if ui.button(t!("action-delete")).clicked() {
            state.delete_selection();
        }
    });
}

fn element_props(ui: &mut Ui, state: &mut AppState, id: ElementId) {
    let Some(el) = state.project().element(id).cloned() else { return };
    let category = el.category();

    // Type block: what is selected, its type and "Edit Type".
    let types: Vec<_> = state.project().types_of(category).map(|t| (t.id, t.name.clone())).collect();
    let mut type_id = el.type_id;
    let type_name = state.project().type_name(el.type_id).to_string();
    padded(ui, |ui| {
        let subtitle = format!("{} · {}", actions::t_category(category), el.id);
        header_block(ui, crate::ribbon::category_icon(category), &type_name, &subtitle);
        ui.horizontal(|ui| {
            let button_w = 96.0;
            egui::ComboBox::from_id_salt(("type", id.0))
                .width(ui.available_width() - button_w - 30.0)
                .selected_text(type_name.clone())
                .show_ui(ui, |ui| {
                    for (tid, name) in &types {
                        ui.selectable_value(&mut type_id, *tid, name);
                    }
                });
            if ui.add_sized([button_w, 22.0], egui::Button::new(t!("props-edit-type"))).clicked() {
                state.dialogs.type_editor = Some(el.type_id);
            }
        });
    });
    if type_id != el.type_id {
        crate::engine_edit::element(state, id, None, |x| x.type_id = type_id);
    }

    match &el.kind {
        ElementKind::Component(c) => component_props(ui, state, &el, c),
        ElementKind::Wall(w) => {
            section(ui, "constraints", t!("section-constraints"), |ui| wall_constraints(ui, state, &el, w));
            section(ui, "structural", t!("section-structural"), |ui| {
                let mut on = w.structural;
                if row(ui, &t!("prop-structural"), |ui| ui.checkbox(&mut on, "")).changed() {
                    modify(state, id, "structural", |e| {
                        if let ElementKind::Wall(w) = &mut e.kind {
                            w.structural = on;
                        }
                    });
                }
            });
            section(ui, "geometry", t!("section-geometry"), |ui| {
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
            });
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                readonly_row(ui, &t!("prop-length"), format::length(w.length()));
                let height = state.project().wall_height(&el, w);
                readonly_row(ui, &t!("prop-height"), format::length(height));
                let thickness = state.project().thickness_of(&el);
                readonly_row(ui, &t!("prop-thickness"), format::length(thickness));
                readonly_row(ui, &t!("prop-volume"), format::volume(state.session.geom().volume(id)));
            });
        }
        ElementKind::Door(h) | ElementKind::Window(h) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                readonly_row(ui, &t!("prop-level"), state.project().level_name(el.level).to_string());
                let host = state.project().element(h.host).map(|w| w.id.to_string()).unwrap_or_default();
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
            });
            section(ui, "orientation", t!("section-orientation"), |ui| {
                for (label, field, value) in
                    [(t!("prop-flip-facing"), "ff", h.flip_facing), (t!("prop-flip-hand"), "fh", h.flip_hand)]
                {
                    let mut v = value;
                    if row(ui, &label, |ui| ui.checkbox(&mut v, "")).changed() {
                        crate::engine_edit::element(state, id, None, |x| {
                            if let Some(h) = x.as_hosted_mut() {
                                if field == "ff" {
                                    h.flip_facing = v;
                                } else {
                                    h.flip_hand = v;
                                }
                            }
                        });
                    }
                }
            });
            if let Some((w, hh)) = state.project().element_type(el.type_id).and_then(|t| t.opening_size()) {
                section(ui, "dimensions", t!("section-dimensions"), |ui| {
                    readonly_row(ui, &t!("col-width"), format::length(w));
                    readonly_row(ui, &t!("col-height"), format::length(hh));
                });
            }
        }
        ElementKind::Room(r) => room_props(ui, state, &el, r),
        ElementKind::Column(c) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-base-offset"), id, "base", c.base_offset, |e, v| {
                    if let ElementKind::Column(c) = &mut e.kind {
                        c.base_offset = v;
                    }
                });
                top_rows(ui, state, &el, c.top, c.height);
                let mut rot = c.rotation_deg;
                let r = row(ui, &t!("prop-rotation"), |ui| ui.add(DragValue::new(&mut rot).speed(1.0).suffix("°")));
                if r.changed() {
                    modify(state, id, "rotation", |e| {
                        if let ElementKind::Column(c) = &mut e.kind {
                            c.rotation_deg = rot.rem_euclid(360.0);
                        }
                    });
                }
                finish(state, &r);
            });
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                let (base, top) = state.project().column_span(&el, c);
                readonly_row(ui, &t!("prop-height"), format::length(top - base));
                readonly_row(ui, &t!("prop-volume"), format::volume(state.session.geom().volume(id)));
            });
        }
        ElementKind::Beam(b) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-top-offset"), id, "beam-top", b.top_offset, |e, v| {
                    if let ElementKind::Beam(b) = &mut e.kind {
                        b.top_offset = v;
                    }
                });
            });
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                readonly_row(ui, &t!("prop-length"), format::length(b.length()));
                readonly_row(ui, &t!("prop-volume"), format::volume(state.session.geom().volume(id)));
            });
        }
        ElementKind::Stair(st) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-base-offset"), id, "base", st.base_offset, |e, v| {
                    if let ElementKind::Stair(s) = &mut e.kind {
                        s.base_offset = v;
                    }
                });
                top_rows(ui, state, &el, st.top, st.height);
            });
            if let Some(g) = cc_geom::stair_geom(state.project(), &el, st) {
                section(ui, "dimensions", t!("section-dimensions"), |ui| {
                    readonly_row(ui, &t!("col-risers"), g.risers.to_string());
                    readonly_row(ui, &t!("col-riser-height"), format::length(g.riser));
                    readonly_row(ui, &t!("col-tread"), format::length(g.tread));
                    readonly_row(ui, &t!("col-run"), format::length(g.run()));
                });
            }
        }
        ElementKind::Railing(r) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-base-offset"), id, "base", r.base_offset, |e, v| {
                    if let ElementKind::Railing(r) = &mut e.kind {
                        r.base_offset = v;
                    }
                });
            });
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                readonly_row(ui, &t!("prop-length"), format::length(r.length()));
            });
        }
        ElementKind::Grid(g) => {
            section(ui, "identity-grid", t!("section-identity"), |ui| {
                let (resp, text) = text_row(ui, &t!("col-name"), g.name.clone());
                if resp.changed() {
                    modify(state, id, "grid-name", |e| {
                        if let ElementKind::Grid(g) = &mut e.kind {
                            g.name = text;
                        }
                    });
                }
                finish(state, &resp);
            });
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                readonly_row(ui, &t!("prop-length"), format::length(g.start.distance(g.end)));
            });
        }
        ElementKind::Text(note) => text_props(ui, state, id, note),
        ElementKind::Dimension(d) => {
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                dimension_value_row(ui, state, id, d);
                let view = state.project().view(d.view).map(|v| v.name.clone()).unwrap_or_default();
                readonly_row(ui, &t!("prop-view"), view);
                length_row(ui, state, &t!("prop-dimension-offset"), id, "dim-offset", d.offset, |e, v| {
                    if let ElementKind::Dimension(d) = &mut e.kind {
                        d.offset = v;
                    }
                });
            });
        }
        ElementKind::Floor(f) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-height-offset"), id, "hoff", f.height_offset, |e, v| {
                    if let ElementKind::Floor(f) = &mut e.kind {
                        f.height_offset = v;
                    }
                });
            });
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                let area = cc_geom::polygon::area(&f.boundary);
                readonly_row(ui, &t!("col-area"), format::area(area));
            });
        }
        ElementKind::Ceiling(c) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-height-offset"), id, "hoff", c.height_offset, |e, v| {
                    if let ElementKind::Ceiling(c) = &mut e.kind {
                        c.height_offset = v;
                    }
                });
            });
            section(ui, "dimensions", t!("section-dimensions"), |ui| {
                let area = cc_geom::polygon::area(&c.boundary);
                readonly_row(ui, &t!("col-area"), format::area(area));
            });
        }
        ElementKind::Roof(r) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
                level_row(ui, state, &el);
                length_row(ui, state, &t!("prop-base-offset"), id, "base", r.base_offset, |e, v| {
                    if let ElementKind::Roof(r) = &mut e.kind {
                        r.base_offset = v;
                    }
                });
            });
            section(ui, "construction", t!("section-construction"), |ui| roof_rows(ui, state, id, r));
        }
    }

    // Rooms, grids and dimensions have their own identity rows; only model elements carry a mark.
    let mut identity = vec![(t!("col-comments"), "comments", el.comments.clone())];
    if category.is_model() {
        identity.insert(0, (t!("col-mark"), "mark", el.mark.clone()));
    }
    let identity_id = if category.is_model() { "identity" } else { "identity-extra" };
    let identity_title = if category.is_model() { t!("section-identity") } else { t!("section-other") };
    section(ui, identity_id, identity_title, |ui| {
        for (label, field, value) in identity {
            let (r, text) = text_row(ui, &label, value);
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
        }
        let mut pinned = state.project().is_pinned(id);
        if row(ui, &t!("prop-pinned"), |ui| ui.checkbox(&mut pinned, "")).changed() {
            crate::actions::run_command(state, cc_engine::commands::modify::ElementsPin { ids: vec![id.0], pinned });
        }
    });
    section(ui, "ifc", t!("section-ifc"), |ui| {
        readonly_row(ui, &t!("prop-ifc-guid"), cc_ifc::guid::encode(el.uid.0));
    });
    padded(ui, |ui| {
        if ui.button(t!("action-delete")).clicked() {
            state.delete_selection();
        }
    });
}

/// Top constraint rows shared by columns and stairs (walls have their own).
fn top_rows(ui: &mut Ui, state: &mut AppState, el: &Element, top: cc_core::WallTop, height: f64) {
    use cc_core::WallTop;
    let id = el.id;
    let set_top = |state: &mut AppState, field: &str, top: WallTop, height: Option<f64>| {
        modify(state, id, field, |e| match &mut e.kind {
            ElementKind::Column(c) => {
                c.top = top;
                if let Some(h) = height {
                    c.height = h;
                }
            }
            ElementKind::Stair(s) => {
                s.top = top;
                if let Some(h) = height {
                    s.height = h;
                }
            }
            _ => {}
        });
    };
    let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
    let name = |level| levels.iter().find(|l| l.0 == level).map_or("?".to_string(), |l| l.1.clone());
    let current = match top {
        WallTop::Unconnected => t!("top-unconnected"),
        WallTop::Level { level, .. } => t!("top-up-to", "level" => name(level)),
    };
    let mut choice = match top {
        WallTop::Level { level, .. } => Some(level),
        WallTop::Unconnected => None,
    };
    let before = choice;
    row(ui, &t!("prop-top-constraint"), |ui| {
        egui::ComboBox::from_id_salt(("top", id.0)).selected_text(current).show_ui(ui, |ui| {
            ui.selectable_value(&mut choice, None, t!("top-unconnected"));
            for (lid, lname) in &levels {
                ui.selectable_value(&mut choice, Some(*lid), t!("top-up-to", "level" => lname));
            }
        });
    });
    if choice != before {
        let new_top = choice.map_or(WallTop::Unconnected, |level| WallTop::Level { level, offset: 0.0 });
        set_top(state, "top", new_top, None);
        state.session.doc.break_merge();
    }
    match top {
        WallTop::Level { level, offset } => {
            length_row(ui, state, &t!("prop-top-offset"), id, "top-offset", offset, move |e, v| match &mut e.kind {
                ElementKind::Column(c) => c.top = WallTop::Level { level, offset: v },
                ElementKind::Stair(s) => s.top = WallTop::Level { level, offset: v },
                _ => {}
            });
        }
        WallTop::Unconnected => {
            length_row(ui, state, &t!("prop-unconnected-height"), id, "height", height, |e, v| match &mut e.kind {
                ElementKind::Column(c) => c.height = v.max(0.1),
                ElementKind::Stair(s) => s.height = v.max(0.1),
                _ => {}
            });
        }
    }
}

/// Number, name, height and computed area of a room.
/// Drag value for a length stored in meters, shown and typed in the
/// project's length unit (a typed unit symbol overrides it).
pub fn length_drag(value: &mut f64) -> DragValue<'_> {
    let (units, comma) = format::current();
    let unit = units.length.unit;
    let decimals = units.length.decimals as usize;
    let per_meter = unit.per_meter();
    let parse_units = units.clone();
    DragValue::new(value)
        .speed(0.005)
        .custom_formatter(move |v, _| format::number_with(&units, comma, v * per_meter, decimals))
        .custom_parser(move |s| format::parse_length_with(&parse_units, s))
        .suffix(format!(" {}", unit.symbol()))
}

/// Drag value for a component parameter, with the unit of its kind.
pub fn param_value(value: &mut f64, kind: cc_core::ParamKind) -> DragValue<'_> {
    match kind {
        cc_core::ParamKind::Length => crate::properties::length_drag(value).speed(0.01),
        cc_core::ParamKind::Angle => DragValue::new(value).speed(1.0).suffix("°"),
        cc_core::ParamKind::Number => DragValue::new(value).speed(0.1),
    }
}

fn component_props(ui: &mut Ui, state: &mut AppState, el: &Element, c: &cc_core::Component) {
    let id = el.id;
    section(ui, "constraints", t!("section-constraints"), |ui| {
        level_row(ui, state, el);
        length_row(ui, state, &t!("prop-base-offset"), id, "comp-base", c.base_offset, |e, v| {
            if let ElementKind::Component(c) = &mut e.kind {
                c.base_offset = v;
            }
        });
        let mut rot = c.rotation_deg;
        let r = row(ui, &t!("prop-rotation"), |ui| ui.add(DragValue::new(&mut rot).speed(1.0).suffix("°")));
        if r.changed() {
            modify(state, id, "comp-rotation", |e| {
                if let ElementKind::Component(c) = &mut e.kind {
                    c.rotation_deg = rot.rem_euclid(360.0);
                }
            });
        }
        finish(state, &r);
    });
    let Some(cc_core::TypeData::Component { def, .. }) =
        state.project().element_type(el.type_id).map(|t| t.data.clone())
    else {
        return;
    };
    if let Some(ifc) = &def.ifc {
        section(ui, "comp-ifc", t!("section-ifc"), |ui| {
            readonly_row(ui, &t!("prop-ifc-class"), cc_ifc::display_name(&ifc.entity));
            if !ifc.predefined.is_empty() {
                readonly_row(ui, &t!("prop-ifc-predefined"), ifc.predefined.clone());
            }
            if !ifc.object_type.is_empty() {
                readonly_row(ui, &t!("prop-ifc-object-type"), ifc.object_type.clone());
            }
        });
    }
    // Property and quantity sets from the imported file: the object's, then its type's.
    let type_sets = def.ifc.as_ref().map(|i| i.properties.as_slice()).unwrap_or_default();
    let sets = c.properties.iter().map(|s| (s, false)).chain(type_sets.iter().map(|s| (s, true)));
    for (i, (set, of_type)) in sets.enumerate() {
        let title = if of_type { t!("section-type-set", "name" => set.name.as_str()) } else { set.name.clone() };
        section(ui, &format!("comp-set-{i}"), title, |ui| {
            for p in &set.properties {
                property_rows(ui, &p.name, &p.value);
            }
        });
    }
    let instance: Vec<_> = def.params.iter().filter(|p| p.instance).cloned().collect();
    if !instance.is_empty() {
        section(ui, "comp-params", t!("section-instance-params"), |ui| {
            for p in instance {
                let current = c.overrides.get(&p.name).copied();
                let mut v = current.unwrap_or(p.value);
                let (r, reset) = row(ui, &p.name, |ui| {
                    ui.horizontal(|ui| {
                        let r = ui.add(param_value(&mut v, p.kind));
                        let reset = ui
                            .add_enabled(current.is_some(), egui::Button::new("↺").small())
                            .on_hover_text(t!("hint-param-reset"));
                        (r, reset.clicked())
                    })
                    .inner
                });
                if r.changed() {
                    let name = p.name.clone();
                    modify(state, id, &format!("comp-param-{name}"), |e| {
                        if let ElementKind::Component(c) = &mut e.kind {
                            c.overrides.insert(name, v);
                        }
                    });
                }
                if reset {
                    let name = p.name.clone();
                    crate::engine_edit::element(state, id, None, |x| {
                        if let ElementKind::Component(c) = &mut x.kind {
                            c.overrides.remove(&name);
                        }
                    });
                }
                finish(state, &r);
            }
        });
    }
    section(ui, "dimensions", t!("section-dimensions"), |ui| {
        readonly_row(ui, &t!("prop-volume"), format::volume(state.session.geom().volume(id)));
    });
}

/// A text note: its text, single-line or multiline (with a width), rotation and leaders.
fn text_props(ui: &mut Ui, state: &mut AppState, id: ElementId, note: &cc_core::TextNote) {
    let note = note.clone();
    let set = |state: &mut AppState, field: &str, f: &dyn Fn(&mut cc_core::TextNote)| {
        crate::engine_edit::element(state, id, Some(key((id, field))), |e| {
            if let ElementKind::Text(n) = &mut e.kind {
                f(n);
            }
        });
    };
    section(ui, "text", t!("section-text"), |ui| {
        let mut text = note.text.clone();
        let r = row(ui, &t!("prop-text"), |ui| ui.add(egui::TextEdit::multiline(&mut text).desired_rows(2)));
        if r.changed() && !text.trim().is_empty() {
            set(state, "text", &|n| n.text = text.clone());
        }
        finish(state, &r);
        let mut multiline = note.width.is_some();
        if row(ui, &t!("text-multiline"), |ui| ui.checkbox(&mut multiline, "")).changed() {
            let width = multiline.then_some(state.text_options.width);
            set(state, "multiline", &|n| n.width = width);
        }
        if let Some(width) = note.width {
            let mut w = width;
            let r = row(ui, &t!("prop-text-width"), |ui| {
                ui.add(DragValue::new(&mut w).speed(1.0).range(5.0..=1000.0).suffix(" mm"))
            });
            if r.changed() {
                set(state, "width", &|n| n.width = Some(w));
            }
            finish(state, &r);
        }
        let mut rot = note.rotation_deg;
        let r = row(ui, &t!("prop-rotation"), |ui| ui.add(DragValue::new(&mut rot).speed(1.0).suffix("°")));
        if r.changed() {
            set(state, "rotation", &|n| n.rotation_deg = rot.rem_euclid(360.0));
        }
        finish(state, &r);
        let view = state.project().view(note.view).map(|v| v.name.clone()).unwrap_or_default();
        readonly_row(ui, &t!("prop-view"), view);
    });
    section(ui, "leaders", t!("section-leaders"), |ui| {
        let mut remove = None;
        for (i, leader) in note.leaders.iter().enumerate() {
            let mut symbol = leader.symbol;
            let label = format!("{} {}", t!("label-leader"), i + 1);
            let (changed, removed) = row(ui, &label, |ui| {
                ui.horizontal(|ui| {
                    let changed = crate::text_tool::symbol_combo(ui, ("leader-symbol", id.0, i), &mut symbol);
                    (changed, ui.button(t!("action-remove")).clicked())
                })
                .inner
            });
            if changed {
                set(state, "leader-symbol", &|n| {
                    if let Some(l) = n.leaders.get_mut(i) {
                        l.symbol = symbol;
                    }
                });
            }
            if removed {
                remove = Some(i);
            }
        }
        if let Some(index) = remove {
            crate::actions::run_command(state, cc_engine::commands::annotate::TextRemoveLeader { id: id.0, index });
        }
        if row(ui, "", |ui| ui.button(t!("tool-add-leader")).clicked()) {
            state.set_tool(crate::tools::Tool::AddLeader { id });
            state.message = t!("hint-add-leader");
        }
    });
}

/// Read-only rows of an imported property (nested ones as `parent.child`).
fn property_rows(ui: &mut Ui, label: &str, value: &cc_core::properties::PropertyValue) {
    use cc_core::properties::PropertyValue as V;
    let text = match value {
        V::Single(v) => v.as_ref().map(ifc_value_text).unwrap_or_default(),
        V::Enumerated(values) | V::List(values) => values.iter().map(ifc_value_text).collect::<Vec<_>>().join(", "),
        V::Bounded { lower, upper, set_point } => {
            let t = |v: &Option<cc_core::properties::IfcValue>| v.as_ref().map(ifc_value_text).unwrap_or_default();
            let range = format!("{} – {}", t(lower), t(upper));
            if set_point.is_some() { format!("{range} ({})", t(set_point)) } else { range }
        }
        V::Table { defining, defined } => defining
            .iter()
            .zip(defined)
            .map(|(a, b)| format!("{} → {}", ifc_value_text(a), ifc_value_text(b)))
            .collect::<Vec<_>>()
            .join("; "),
        V::Complex { properties, .. } => {
            for p in properties {
                property_rows(ui, &format!("{label}.{}", p.name), &p.value);
            }
            return;
        }
        V::Quantity { kind, value } => match kind.as_str() {
            "IFCQUANTITYLENGTH" => format::meters(*value),
            "IFCQUANTITYAREA" => format::area(*value),
            "IFCQUANTITYVOLUME" => format::volume(*value),
            _ => format::number(*value, 3),
        },
    };
    readonly_row(ui, label, text);
}

/// An imported value as text, in the project's units for measures.
fn ifc_value_text(v: &cc_core::properties::IfcValue) -> String {
    use cc_core::properties::Atom;
    match &v.value {
        Atom::Real(x) => match v.measure.as_str() {
            "IFCLENGTHMEASURE" | "IFCPOSITIVELENGTHMEASURE" | "IFCNONNEGATIVELENGTHMEASURE" => format::meters(*x),
            "IFCAREAMEASURE" => format::area(*x),
            "IFCVOLUMEMEASURE" => format::volume(*x),
            "IFCPLANEANGLEMEASURE" | "IFCPOSITIVEPLANEANGLEMEASURE" => format::angle(x.to_degrees()),
            _ => format::number(*x, 3),
        },
        Atom::Integer(i) => i.to_string(),
        Atom::Bool(true) => t!("value-yes"),
        Atom::Bool(false) => t!("value-no"),
        Atom::Text(s) | Atom::Enum(s) => s.clone(),
    }
}

fn room_props(ui: &mut Ui, state: &mut AppState, el: &Element, r: &cc_core::Room) {
    let id = el.id;
    section(ui, "identity-room", t!("section-identity"), |ui| {
        for (label, field, value) in
            [(t!("col-number"), "number", r.number.clone()), (t!("col-name"), "name", r.name.clone())]
        {
            let (resp, text) = text_row(ui, &label, value);
            if resp.changed() {
                modify(state, id, field, |e| {
                    if let ElementKind::Room(r) = &mut e.kind {
                        if field == "number" {
                            r.number = text;
                        } else {
                            r.name = text;
                        }
                    }
                });
            }
            finish(state, &resp);
        }
    });
    section(ui, "constraints", t!("section-constraints"), |ui| {
        readonly_row(ui, &t!("prop-level"), state.project().level_name(el.level).to_string());
        length_row(ui, state, &t!("prop-room-height"), id, "room-height", r.height, |e, v| {
            if let ElementKind::Room(r) = &mut e.kind {
                r.height = v.max(0.1);
            }
        });
    });
    section(ui, "dimensions", t!("section-dimensions"), |ui| match state.session.geom().rooms.get(&id).cloned() {
        Some(boundary) => {
            let area = cc_geom::polygon::area(&boundary);
            readonly_row(ui, &t!("col-area"), format::area(area));
            readonly_row(ui, &t!("col-perimeter"), format::length(cc_geom::polygon::perimeter(&boundary)));
            readonly_row(ui, &t!("prop-volume"), format::volume(area * r.height));
        }
        None => readonly_row(ui, &t!("col-area"), t!("room-not-enclosed")),
    });
}

/// Base and top constraints, location line and room bounding of a wall.
fn wall_constraints(ui: &mut Ui, state: &mut AppState, el: &Element, w: &cc_core::Wall) {
    use cc_core::{LocationLine, WallTop};
    let id = el.id;
    let set = |state: &mut AppState, field: &str, f: &dyn Fn(&mut cc_core::Wall)| {
        modify(state, id, field, |e| {
            if let ElementKind::Wall(w) = &mut e.kind {
                f(w);
            }
        });
    };

    let mut location = w.location_line;
    row(ui, &t!("prop-location-line"), |ui| {
        egui::ComboBox::from_id_salt(("loc", id.0)).selected_text(tr(location.key())).show_ui(ui, |ui| {
            for l in LocationLine::ALL {
                ui.selectable_value(&mut location, l, tr(l.key()));
            }
        });
    });
    if location != w.location_line {
        set(state, "loc", &|w| w.location_line = location);
        state.session.doc.break_merge();
    }
    for (label, at_start, value) in
        [(t!("prop-join-start"), true, w.join_start), (t!("prop-join-end"), false, w.join_end)]
    {
        let mut on = value;
        if row(ui, &label, |ui| ui.checkbox(&mut on, "")).changed() {
            crate::engine_edit::element(state, id, None, |x| {
                if let ElementKind::Wall(w) = &mut x.kind {
                    if at_start {
                        w.join_start = on;
                    } else {
                        w.join_end = on;
                    }
                }
            });
        }
    }

    level_row(ui, state, el);
    length_row(ui, state, &t!("prop-base-offset"), id, "base", w.base_offset, |e, v| {
        if let ElementKind::Wall(w) = &mut e.kind {
            w.base_offset = v;
        }
    });

    // Top: unconnected (explicit height) or attached to a level.
    let levels: Vec<_> = state.project().levels.iter().map(|l| (l.id, l.name.clone())).collect();
    let top_label = |top: WallTop| match top {
        WallTop::Unconnected => t!("top-unconnected"),
        WallTop::Level { level, .. } => {
            t!("top-up-to", "level" => levels.iter().find(|l| l.0 == level).map_or("?", |l| l.1.as_str()))
        }
    };
    let mut top_level = match w.top {
        WallTop::Level { level, .. } => Some(level),
        WallTop::Unconnected => None,
    };
    let before = top_level;
    row(ui, &t!("prop-top-constraint"), |ui| {
        egui::ComboBox::from_id_salt(("top", id.0)).selected_text(top_label(w.top)).show_ui(ui, |ui| {
            ui.selectable_value(&mut top_level, None, t!("top-unconnected"));
            for (lid, _) in &levels {
                let label = top_label(WallTop::Level { level: *lid, offset: 0.0 });
                ui.selectable_value(&mut top_level, Some(*lid), label);
            }
        });
    });
    if top_level != before {
        // Keep the wall's current top when switching to unconnected.
        let height = state.project().wall_height(el, w);
        set(state, "top", &|w| {
            w.top = match top_level {
                Some(level) => WallTop::Level { level, offset: 0.0 },
                None => WallTop::Unconnected,
            };
            if top_level.is_none() {
                w.height = height;
            }
        });
        state.session.doc.break_merge();
    }
    match w.top {
        WallTop::Level { level, offset } => {
            length_row(ui, state, &t!("prop-top-offset"), id, "top-offset", offset, move |e, v| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    w.top = WallTop::Level { level, offset: v };
                }
            });
            let height = state.project().wall_height(el, w);
            readonly_row(ui, &t!("prop-unconnected-height"), format::length(height));
        }
        WallTop::Unconnected => {
            length_row(ui, state, &t!("prop-unconnected-height"), id, "height", w.height, |e, v| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    w.height = v.max(0.1);
                }
            });
        }
    }

    let mut bounding = w.room_bounding;
    if row(ui, &t!("prop-room-bounding"), |ui| ui.checkbox(&mut bounding, "")).changed() {
        set(state, "bounding", &|w| w.room_bounding = bounding);
    }
}

fn roof_rows(ui: &mut Ui, state: &mut AppState, id: ElementId, r: &cc_core::Roof) {
    length_row(ui, state, &t!("prop-overhang"), id, "overhang", r.overhang, |e, v| {
        if let ElementKind::Roof(r) = &mut e.kind {
            r.overhang = v.max(0.0);
        }
    });
    // Shape: 0 flat, 1 shed, 2 gable, 3 hip.
    let index = |shape: RoofShape| match shape {
        RoofShape::Flat => 0,
        RoofShape::Shed { .. } => 1,
        RoofShape::Gable { .. } => 2,
        RoofShape::Hip { .. } => 3,
    };
    let current = index(r.shape);
    let mut shape = current;
    let labels = [t!("roof-flat"), t!("roof-shed"), t!("roof-gable"), t!("roof-hip")];
    row(ui, &t!("prop-roof-shape"), |ui| {
        egui::ComboBox::from_id_salt(("roof-shape", id.0)).selected_text(labels[current].clone()).show_ui(ui, |ui| {
            for (i, label) in labels.iter().enumerate() {
                ui.selectable_value(&mut shape, i, label);
            }
        });
    });
    if shape != current {
        // Keep the slope when switching between pitched shapes; gables run along the longest edge.
        let slope = match r.shape {
            RoofShape::Shed { slope, .. } | RoofShape::Gable { slope, .. } | RoofShape::Hip { slope } => slope,
            RoofShape::Flat => 0.4,
        };
        let longest = r
            .footprint
            .iter()
            .zip(r.footprint.iter().cycle().skip(1))
            .map(|(a, b)| *b - *a)
            .max_by(|a, b| a.length().total_cmp(&b.length()))
            .map_or(0.0, |d| d.y.atan2(d.x).to_degrees());
        crate::engine_edit::element(state, id, None, |x| {
            if let ElementKind::Roof(r) = &mut x.kind {
                r.shape = match shape {
                    1 => RoofShape::Shed { slope, direction_deg: 90.0 },
                    2 => RoofShape::Gable { slope, ridge_deg: longest },
                    3 => RoofShape::Hip { slope },
                    _ => RoofShape::Flat,
                };
            }
        });
    }
    if let RoofShape::Gable { slope, ridge_deg } | RoofShape::Shed { slope, direction_deg: ridge_deg } = r.shape {
        let is_gable = matches!(r.shape, RoofShape::Gable { .. });
        let mut pct = slope * 100.0;
        let rs = row(ui, &t!("prop-slope"), |ui| {
            ui.add(DragValue::new(&mut pct).speed(0.5).range(0.0..=200.0).suffix(" %"))
        });
        let mut dir = ridge_deg;
        let label = if is_gable { t!("prop-ridge-direction") } else { t!("prop-direction") };
        let rd = row(ui, &label, |ui| ui.add(DragValue::new(&mut dir).speed(1.0).suffix("°")));
        if rs.changed() || rd.changed() {
            modify(state, id, "shape", |e| {
                if let ElementKind::Roof(r) = &mut e.kind {
                    let (slope, angle) = (pct / 100.0, dir.rem_euclid(360.0));
                    r.shape = if is_gable {
                        RoofShape::Gable { slope, ridge_deg: angle }
                    } else {
                        RoofShape::Shed { slope, direction_deg: angle }
                    };
                }
            });
        }
        finish(state, &rs);
        finish(state, &rd);
    }
    if let RoofShape::Hip { slope } = r.shape {
        let mut pct = slope * 100.0;
        let rs = row(ui, &t!("prop-slope"), |ui| {
            ui.add(DragValue::new(&mut pct).speed(0.5).range(0.0..=200.0).suffix(" %"))
        });
        if rs.changed() {
            modify(state, id, "shape", |e| {
                if let ElementKind::Roof(r) = &mut e.kind {
                    r.shape = RoofShape::Hip { slope: pct / 100.0 };
                }
            });
        }
        finish(state, &rs);
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
    let (r, text) = text_row(ui, label, value);
    if r.changed() {
        crate::engine_edit::view(state, view, Some(key((view, field))), |v| set(v, text));
    }
    finish(state, &r);
}

fn view_props(ui: &mut Ui, state: &mut AppState, id: ViewId) {
    let Some(view) = state.project().view(id).cloned() else { return };
    padded(ui, |ui| {
        let category_name = tr(view.category().key());
        header_block(ui, crate::browser::category_icon(view.category()), &view.name, &category_name);
    });

    let graphical = !matches!(view.kind, ViewKind::Schedule { .. } | ViewKind::Sheet { .. });
    if graphical {
        section(ui, "graphics", t!("section-graphics"), |ui| {
            let mut scale = view.scale;
            row(ui, &t!("prop-scale"), |ui| {
                egui::ComboBox::from_id_salt(("scale", id.0)).selected_text(format!("1:{scale}")).show_ui(ui, |ui| {
                    for s in SCALES {
                        ui.selectable_value(&mut scale, s, format!("1:{s}"));
                    }
                });
            });
            if scale != view.scale {
                crate::engine_edit::view(state, id, None, |v| v.scale = scale);
            }
            if let ViewKind::ThreeD { .. } = view.kind {
                let clicked = row(ui, "", |ui| ui.button(t!("action-zoom-fit")).clicked());
                if clicked && let Some(v3) = state.view3d.get_mut(&id) {
                    v3.request_fit();
                }
            }
        });
    }

    match &view.kind {
        ViewKind::FloorPlan { level, cut_height } | ViewKind::CeilingPlan { level, cut_height } => {
            section(ui, "extents", t!("section-extents"), |ui| {
                readonly_row(ui, &t!("prop-level"), state.project().level_name(*level).to_string());
                let mut h = *cut_height;
                let r = row(ui, &t!("prop-cut-height"), |ui| {
                    ui.add(crate::properties::length_drag(&mut h).speed(0.01).range(0.0..=20.0))
                });
                if r.changed() {
                    crate::engine_edit::view(state, id, Some(key((id, "cut"))), |x| {
                        if let Some(ViewKind::FloorPlan { cut_height, .. } | ViewKind::CeilingPlan { cut_height, .. }) =
                            Some(&mut x.kind)
                        {
                            *cut_height = h;
                        }
                    });
                }
                finish(state, &r);
            });
        }
        ViewKind::Section { depth, .. } => {
            section(ui, "extents", t!("section-extents"), |ui| {
                let mut d = *depth;
                let r = row(ui, &t!("prop-depth"), |ui| {
                    ui.add(crate::properties::length_drag(&mut d).speed(0.05).range(0.1..=500.0))
                });
                if r.changed() {
                    crate::engine_edit::view(state, id, Some(key((id, "depth"))), |x| {
                        if let ViewKind::Section { depth, .. } = &mut x.kind {
                            *depth = d;
                        }
                    });
                }
                finish(state, &r);
                if row(ui, "", |ui| ui.button(t!("prop-flip-section")).clicked()) {
                    crate::engine_edit::view(state, id, None, |x| {
                        if let ViewKind::Section { start, end, .. } = &mut x.kind {
                            std::mem::swap(start, end);
                        }
                    });
                }
            });
        }
        ViewKind::Schedule { category, .. } => {
            section(ui, "graphics", t!("section-graphics"), |ui| {
                readonly_row(ui, &t!("prop-category"), actions::t_category(*category));
            });
        }
        ViewKind::Sheet { number, size, .. } => {
            section(ui, "sheet", t!("section-graphics"), |ui| {
                view_text_row(ui, state, id, &t!("prop-sheet-number"), "number", number.clone(), |v, s| {
                    if let ViewKind::Sheet { number, .. } = &mut v.kind {
                        *number = s;
                    }
                });
                let mut sz = *size;
                row(ui, &t!("prop-sheet-size"), |ui| {
                    egui::ComboBox::from_id_salt(("size", id.0)).selected_text(sz.label()).show_ui(ui, |ui| {
                        for s in SheetSize::ALL {
                            ui.selectable_value(&mut sz, s, s.label());
                        }
                    });
                });
                if sz != *size {
                    crate::engine_edit::view(state, id, None, |x| {
                        if let ViewKind::Sheet { size, .. } = &mut x.kind {
                            *size = sz;
                        }
                    });
                    if let Some(st) = state.view2d.get_mut(&id) {
                        st.fitted = false;
                    }
                }
            });
        }
        ViewKind::ThreeD { .. } | ViewKind::Legend { .. } | ViewKind::Elevation { .. } => {}
    }

    section(ui, "identity", t!("section-identity"), |ui| {
        view_text_row(ui, state, id, &t!("prop-name"), "name", view.name.clone(), |v, s| v.name = s);
    });

    match &view.kind {
        ViewKind::Legend { types } => {
            section(ui, "legend-types", t!("prop-legend-types"), |ui| legend_types(ui, state, id, types));
        }
        ViewKind::Sheet { viewports, .. } => {
            section(ui, "sheet-views", t!("prop-sheet-views"), |ui| sheet_views(ui, state, id, viewports));
        }
        _ => {}
    }
}

fn legend_types(ui: &mut Ui, state: &mut AppState, id: ViewId, current: &[cc_core::ElementTypeId]) {
    let all: Vec<_> = state.project().types.values().map(|t| (t.id, t.name.clone())).collect();
    for (tid, name) in all {
        let mut on = current.contains(&tid);
        if row(ui, &name, |ui| ui.checkbox(&mut on, "")).changed() {
            crate::engine_edit::view(state, id, None, |x| {
                if let ViewKind::Legend { types } = &mut x.kind {
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
    padded(ui, |ui| {
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
    });
}

fn viewport_props(ui: &mut Ui, state: &mut AppState, sheet: ViewId, index: usize) {
    let Some(ViewKind::Sheet { viewports, .. }) = state.project().view(sheet).map(|v| v.kind.clone()) else { return };
    let Some(vp) = viewports.get(index).cloned() else {
        state.sheet_selection = None;
        return;
    };
    padded(ui, |ui| header_block(ui, Icon::Sheet, &state.view_title(vp.view), &t!("props-viewport")));
    section(ui, "position", t!("section-position"), |ui| {
        for (label, axis) in [(t!("prop-center-x"), 0), (t!("prop-center-y"), 1)] {
            let mut v = vp.center[axis];
            let r = row(ui, &label, |ui| ui.add(DragValue::new(&mut v).speed(1.0).fixed_decimals(1).suffix(" mm")));
            if r.changed() {
                crate::engine_edit::view(state, sheet, Some(key((sheet, index, axis))), |x| {
                    if let ViewKind::Sheet { viewports, .. } = &mut x.kind
                        && let Some(vp) = viewports.get_mut(index)
                    {
                        vp.center[axis] = v;
                    }
                });
            }
            finish(state, &r);
        }
    });
    padded(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button(t!("browser-open")).clicked() {
                state.open_view(vp.view);
            }
            if ui.button(t!("prop-remove-from-sheet")).clicked() {
                actions::remove_viewport(state, sheet, index);
            }
        });
    });
}
