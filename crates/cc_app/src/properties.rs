//! Properties palette: instance parameters of the selection, or of the active view.
//!
//! A header block names what is shown (and its type, with "Edit Type"); below
//! it, parameters are listed in collapsible sections of a two-column table.

use crate::actions;
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::state::AppState;
use crate::t;
use crate::theme::{self, PALETTE};
use cc_core::{Category, Element, ElementId, ElementKind, RoofShape, SheetSize, ViewId, ViewKind};
use cc_views::format;
use egui::{Align, DragValue, Layout, Rect, Response, RichText, Sense, Stroke, Ui, UiBuilder, vec2};
use std::hash::{Hash, Hasher};

const SCALES: [u32; 9] = [10, 20, 25, 50, 75, 100, 200, 500, 1000];

/// Height of a table row and of a section header.
const ROW_H: f32 = 22.0;
/// Share of the table width taken by the label column.
const LABEL_SHARE: f32 = 0.46;
const FONT_SIZE: f32 = 12.5;

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

// --- Table primitives ---

/// Collapsible section header; `add` draws the rows while it is open.
fn section(ui: &mut Ui, id: &str, title: String, add: impl FnOnce(&mut Ui)) {
    let state_id = ui.make_persistent_id(("prop-section", id));
    let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(state_id, true));
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let painter = ui.painter();
    let fill = if response.hovered() { PALETTE.hover } else { PALETTE.surface };
    painter.rect_filled(rect, 0.0, fill);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, PALETTE.border));
    let chevron = Rect::from_center_size(egui::pos2(rect.left() + 11.0, rect.center().y), vec2(9.0, 9.0));
    icons::paint(painter, chevron, if open { Icon::ChevronDown } else { Icon::ChevronRight }, PALETTE.text_muted);
    let font = egui::FontId::new(FONT_SIZE, theme::semibold());
    painter.text(egui::pos2(rect.left() + 22.0, rect.center().y), egui::Align2::LEFT_CENTER, title, font, PALETTE.text);
    if response.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(state_id, open));
    }
    if open {
        add(ui);
    }
}

/// One table row: a label cell and a value cell filled by `add`.
fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::hover());
    let split = (rect.left() + width * LABEL_SHARE).round();
    let painter = ui.painter();
    let grid = Stroke::new(1.0, PALETTE.border);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, grid);
    painter.vline(split + 0.5, rect.y_range(), grid);
    let label_rect = Rect::from_min_max(rect.min, egui::pos2(split, rect.bottom()));
    let font = egui::FontId::proportional(FONT_SIZE);
    ui.painter_at(label_rect.shrink2(vec2(6.0, 0.0))).text(
        egui::pos2(label_rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        PALETTE.text_muted,
    );
    let label_hover = ui.interact(label_rect, ui.id().with(("prop-label", label)), Sense::hover());
    label_hover.on_hover_text(label);

    let value_rect = Rect::from_min_max(egui::pos2(split + 2.0, rect.top() + 1.0), rect.max - vec2(2.0, 2.0));
    let builder = UiBuilder::new().max_rect(value_rect).layout(Layout::left_to_right(Align::Center));
    ui.scope_builder(builder, |ui| {
        let w = value_rect.width();
        let s = ui.spacing_mut();
        s.interact_size = vec2(w, ROW_H - 4.0);
        s.combo_width = w - 8.0;
        s.text_edit_width = w - 4.0;
        s.button_padding = vec2(6.0, 1.0);
        s.item_spacing = vec2(4.0, 0.0);
        // Flat cells: borders appear only on hover and focus.
        let v = ui.visuals_mut();
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
        v.extreme_bg_color = egui::Color32::TRANSPARENT;
        ui.style_mut().override_font_id = Some(egui::FontId::proportional(FONT_SIZE));
        add(ui)
    })
    .inner
}

/// A row whose value cannot be edited (shown dimmed).
fn readonly_row(ui: &mut Ui, label: &str, value: String) {
    row(ui, label, |ui| {
        ui.add_space(4.0);
        ui.label(RichText::new(value).color(PALETTE.text_faint));
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
    let mut v = value;
    let r = row(ui, label, |ui| ui.add(DragValue::new(&mut v).speed(0.01).fixed_decimals(3).suffix(" m")));
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
}

fn text_row(ui: &mut Ui, label: &str, value: String) -> (Response, String) {
    let mut text = value;
    let r = row(ui, label, |ui| ui.text_edit_singleline(&mut text));
    (r, text)
}

// --- Header block ---

/// Header with a large icon tile, a title and a subtitle.
fn header_block(ui: &mut Ui, icon: Icon, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        let (tile, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
        ui.painter().rect_filled(tile, theme::RADIUS as f32, egui::Color32::WHITE);
        ui.painter().rect_stroke(
            tile,
            theme::RADIUS as f32,
            Stroke::new(1.0, PALETTE.border),
            egui::StrokeKind::Inside,
        );
        icons::paint(ui.painter(), tile.shrink(10.0), icon, PALETTE.accent_text);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(3.0);
            ui.label(RichText::new(title).family(theme::semibold()).size(13.5).color(PALETTE.text));
            ui.label(RichText::new(subtitle).size(12.0).color(PALETTE.text_faint));
        });
    });
    ui.add_space(6.0);
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
                let id = *state.selection.iter().next().unwrap();
                element_props(ui, state, id);
            }
            n => multi_props(ui, state, n),
        }
    });
}

/// Content with the panel's side padding (headers, buttons, lists).
fn padded<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            add(ui)
        })
        .inner
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
                .width(ui.available_width() - button_w - 8.0)
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
        state.doc.edit(t!("tx-change-type"), |e| {
            if let Some(x) = e.element_mut(id) {
                x.type_id = type_id;
            }
        });
    }

    match &el.kind {
        ElementKind::Wall(w) => {
            section(ui, "constraints", t!("section-constraints"), |ui| {
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
                readonly_row(ui, &t!("prop-length"), format!("{} m", format::meters(w.length())));
                let thickness = state.project().thickness_of(&el);
                readonly_row(ui, &t!("prop-thickness"), format!("{} m", format::meters(thickness)));
                readonly_row(ui, &t!("prop-volume"), format!("{} m³", format::number(state.geom.volume(id), 3)));
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
                }
            });
            if let Some((w, hh)) = state.project().element_type(el.type_id).and_then(|t| t.opening_size()) {
                section(ui, "dimensions", t!("section-dimensions"), |ui| {
                    readonly_row(ui, &t!("col-width"), format!("{} m", format::meters(w)));
                    readonly_row(ui, &t!("col-height"), format!("{} m", format::meters(hh)));
                });
            }
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
                readonly_row(ui, &t!("col-area"), format!("{} m²", format::number(area, 2)));
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
                readonly_row(ui, &t!("col-area"), format!("{} m²", format::number(area, 2)));
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

    section(ui, "identity", t!("section-identity"), |ui| {
        for (label, field, value) in
            [(t!("col-mark"), "mark", el.mark.clone()), (t!("col-comments"), "comments", el.comments.clone())]
        {
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
    });
    padded(ui, |ui| {
        if ui.button(t!("action-delete")).clicked() {
            state.delete_selection();
        }
    });
}

fn roof_rows(ui: &mut Ui, state: &mut AppState, id: ElementId, r: &cc_core::Roof) {
    length_row(ui, state, &t!("prop-overhang"), id, "overhang", r.overhang, |e, v| {
        if let ElementKind::Roof(r) = &mut e.kind {
            r.overhang = v.max(0.0);
        }
    });
    let mut shed = matches!(r.shape, RoofShape::Shed { .. });
    row(ui, &t!("prop-roof-shape"), |ui| {
        ui.radio_value(&mut shed, false, t!("roof-flat"));
        ui.radio_value(&mut shed, true, t!("roof-shed"));
    });
    if shed != matches!(r.shape, RoofShape::Shed { .. }) {
        state.doc.edit(t!("tx-modify"), |e| {
            if let Some(ElementKind::Roof(r)) = e.element_mut(id).map(|x| &mut x.kind) {
                r.shape = if shed { RoofShape::Shed { slope: 0.15, direction_deg: 90.0 } } else { RoofShape::Flat };
            }
        });
    }
    if let RoofShape::Shed { slope, direction_deg } = r.shape {
        let mut pct = slope * 100.0;
        let rs = row(ui, &t!("prop-slope"), |ui| {
            ui.add(DragValue::new(&mut pct).speed(0.5).range(0.0..=200.0).suffix(" %"))
        });
        let mut dir = direction_deg;
        let rd = row(ui, &t!("prop-direction"), |ui| ui.add(DragValue::new(&mut dir).speed(1.0).suffix("°")));
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
        state.doc.edit_merged(t!("tx-modify-view"), key((view, field)), |e| {
            if let Some(v) = e.view_mut(view) {
                set(v, text);
            }
        });
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
                state.doc.edit(t!("tx-modify-view"), |e| {
                    if let Some(v) = e.view_mut(id) {
                        v.scale = scale;
                    }
                });
            }
            if let ViewKind::ThreeD { .. } = view.kind {
                let clicked = row(ui, "", |ui| ui.button(t!("action-zoom-fit")).clicked());
                if clicked && let Some(v3) = state.view3d.get_mut(&id) {
                    v3.fit_requested = true;
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
                    ui.add(DragValue::new(&mut h).speed(0.01).range(0.0..=20.0).fixed_decimals(2).suffix(" m"))
                });
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
            });
        }
        ViewKind::Section { depth, .. } => {
            section(ui, "extents", t!("section-extents"), |ui| {
                let mut d = *depth;
                let r = row(ui, &t!("prop-depth"), |ui| {
                    ui.add(DragValue::new(&mut d).speed(0.05).range(0.1..=500.0).fixed_decimals(2).suffix(" m"))
                });
                if r.changed() {
                    state.doc.edit_merged(t!("tx-modify-view"), key((id, "depth")), |e| {
                        if let Some(ViewKind::Section { depth, .. }) = e.view_mut(id).map(|v| &mut v.kind) {
                            *depth = d;
                        }
                    });
                }
                finish(state, &r);
                if row(ui, "", |ui| ui.button(t!("prop-flip-section")).clicked()) {
                    state.doc.edit(t!("tx-modify-view"), |e| {
                        if let Some(ViewKind::Section { start, end, .. }) = e.view_mut(id).map(|v| &mut v.kind) {
                            std::mem::swap(start, end);
                        }
                    });
                }
            });
        }
        ViewKind::Schedule { category } => {
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
                    state.doc.edit(t!("tx-modify-view"), |e| {
                        if let Some(ViewKind::Sheet { size, .. }) = e.view_mut(id).map(|v| &mut v.kind) {
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
                state.doc.edit_merged(t!("tx-move-viewport"), key((sheet, index, axis)), |e| {
                    if let Some(ViewKind::Sheet { viewports, .. }) = e.view_mut(sheet).map(|v| &mut v.kind)
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
