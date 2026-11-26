//! Project Units dialog: unit, decimals and symbol of each quantity, the
//! currency and the number style, with a live sample of each format.

use crate::dialog::{Dialog, session};
use crate::i18n::tr;
use crate::state::AppState;
use crate::t;
use crate::theme::{self, PALETTE};
use cc_core::units::MAX_DECIMALS;
use cc_core::{AreaUnit, LengthUnit, NumberStyle, SlopeUnit, Units, VolumeUnit};
use cc_views::format;
use egui::{Context, DragValue, RichText, Ui};

/// Sample magnitude shown in the "Example" column, in display units.
const SAMPLE: f64 = 1234.5678;

fn set(state: &mut AppState, field: &str, f: impl FnOnce(&mut Units)) {
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        ("units", field).hash(&mut h);
        h.finish()
    };
    crate::engine_edit::units(state, Some(key), f);
}

fn done(state: &mut AppState, r: &egui::Response) {
    if r.drag_stopped() || r.lost_focus() {
        state.session.doc.break_merge();
    }
}

fn decimals(ui: &mut Ui, state: &mut AppState, field: &str, value: u8, apply: fn(&mut Units, u8)) {
    let mut v = value;
    let r = crate::settings::drag_wide(ui, DragValue::new(&mut v).range(0..=MAX_DECIMALS).speed(0.05));
    if r.changed() {
        set(state, field, |u| apply(u, v));
    }
    done(state, &r);
}

/// Precision of fractional inches: 1", 1/2", … 1/64" (stored as decimals d, 1/2^d).
fn precision(ui: &mut Ui, state: &mut AppState, value: u8) {
    let label = |d: u8| if d == 0 { "1\"".to_string() } else { format!("1/{}\"", 1u32 << d) };
    let mut v = value.min(MAX_DECIMALS);
    crate::settings::framed(ui);
    egui::ComboBox::from_id_salt("units-precision")
        .selected_text(label(v))
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            for d in 0..=MAX_DECIMALS {
                ui.selectable_value(&mut v, d, label(d));
            }
        })
        .response
        .on_hover_text(t!("units-precision-tip"));
    if v != value {
        set(state, "length-decimals", |u| u.length.decimals = v);
    }
}

fn symbol(ui: &mut Ui, state: &mut AppState, field: &str, value: bool, apply: fn(&mut Units, bool)) {
    let mut v = value;
    let label = format!("{} {field}", t!("units-symbol"));
    if crate::settings::switch(ui, &mut v, &label).changed() {
        set(state, field, |u| apply(u, v));
    }
}

fn unit_combo<U: Copy + PartialEq>(
    ui: &mut Ui,
    state: &mut AppState,
    id: &str,
    current: U,
    options: &[U],
    label: impl Fn(U) -> String,
    apply: fn(&mut Units, U),
) {
    let mut v = current;
    crate::settings::framed(ui);
    egui::ComboBox::from_id_salt(id)
        .selected_text(label(current))
        .width(if id == "style" { 260.0 } else { ui.available_width() - 8.0 })
        .show_ui(ui, |ui| {
            for o in options {
                ui.selectable_value(&mut v, *o, label(*o));
            }
        });
    if v != current {
        set(state, id, |u| apply(u, v));
    }
}

fn length_name(u: LengthUnit) -> String {
    match u {
        LengthUnit::Millimeter => t!("unit-millimeter"),
        LengthUnit::Centimeter => t!("unit-centimeter"),
        LengthUnit::Meter => t!("unit-meter"),
        LengthUnit::Foot => t!("unit-foot"),
        LengthUnit::Inch => t!("unit-inch"),
        LengthUnit::FeetInches => t!("unit-feet-inches"),
        LengthUnit::FractionalInch => t!("unit-fractional-inch"),
    }
}

fn area_name(u: AreaUnit) -> String {
    match u {
        AreaUnit::SquareMeter => t!("unit-square-meter"),
        AreaUnit::SquareCentimeter => t!("unit-square-centimeter"),
        AreaUnit::SquareFoot => t!("unit-square-foot"),
        AreaUnit::SquareInch => t!("unit-square-inch"),
    }
}

fn volume_name(u: VolumeUnit) -> String {
    match u {
        VolumeUnit::CubicMeter => t!("unit-cubic-meter"),
        VolumeUnit::Liter => t!("unit-liter"),
        VolumeUnit::CubicFoot => t!("unit-cubic-foot"),
        VolumeUnit::CubicYard => t!("unit-cubic-yard"),
        VolumeUnit::Gallon => t!("unit-gallon"),
    }
}

fn slope_name(u: SlopeUnit) -> String {
    match u {
        SlopeUnit::Percent => t!("unit-percent"),
        SlopeUnit::Degrees => t!("unit-degrees"),
        SlopeUnit::RisePer12 => t!("unit-rise-per-12"),
    }
}

/// The number style's label with a sample number.
fn style_name(style: NumberStyle, comma: bool) -> String {
    let units = Units { style, ..Units::default() };
    let sample = format::number_with(&units, comma, 1234567.891, 2);
    match style {
        NumberStyle::Language => format!("{} ({sample})", t!("units-style-language")),
        _ => sample,
    }
}

pub fn show(ctx: &Context, state: &mut AppState) {
    if !state.dialogs.units {
        return;
    }
    session::begin(state);
    let (mut ok, mut reset, mut imperial) = (0, 0, 0);
    let response = Dialog::new("units", t!("dlg-units")).width(760.0).show(
        ctx,
        |ui| body(ui, state),
        |f| {
            reset = f.extra(t!("units-reset"));
            imperial = f.extra(t!("units-imperial"));
            ok = f.primary(t!("action-ok"));
            f.secondary(t!("action-cancel"));
        },
    );
    if response.clicked == Some(reset) {
        crate::engine_edit::units(state, None, |u| *u = Units::default());
        return;
    }
    if response.clicked == Some(imperial) {
        crate::engine_edit::units(state, None, |u| *u = Units::imperial());
        return;
    }
    if crate::dialogs::ok_cancel(state, &response, ok) {
        state.dialogs.units = false;
    }
}

/// Widths of the cells of a quantity row: name, unit, decimals, symbol.
const CELLS: [f32; 4] = [130.0, 200.0, 90.0, 70.0];

/// A quantity row of the card: its name, then `cells` draws the unit,
/// decimals and symbol cells, and its sample sits on the right.
fn quantity(ui: &mut Ui, id: &str, name: &str, last: bool, sample: String, cells: impl FnOnce(&mut Ui)) {
    crate::settings::card_row(ui, id, 48.0, false, last, |ui| {
        crate::settings::cell(ui, CELLS[0], |ui| {
            ui.label(RichText::new(name).family(theme::semibold()).size(13.0));
        });
        cells(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let font = egui::FontId::monospace(13.0);
            let galley = ui.painter().layout_no_wrap(sample, font, PALETTE.accent_text);
            let (rect, _) = ui.allocate_exact_size(galley.size() + egui::vec2(20.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 6.0, PALETTE.accent_soft.gamma_multiply(0.6));
            ui.painter().galley(rect.center() - galley.size() / 2.0, galley, PALETTE.accent_text);
        });
    });
}

fn body(ui: &mut Ui, state: &mut AppState) {
    let u = state.project().units.clone();
    let comma = crate::i18n::language().decimal_comma();
    ui.add(egui::Label::new(RichText::new(t!("units-intro")).size(12.0).color(PALETTE.text_muted)).wrap());
    ui.add_space(14.0);
    crate::settings::card(ui, |ui| {
        let mut x = 0.0;
        let names = ["units-quantity", "units-unit", "units-decimals", "units-symbol"].map(tr);
        let mut columns: Vec<(f32, String)> = names
            .into_iter()
            .zip(CELLS)
            .map(|(n, w)| {
                let at = x;
                x += w + crate::settings::CELL_GAP;
                (at, n)
            })
            .collect();
        let example_x = ui.available_width() - 2.0 * crate::settings::ROW_PAD_X - 120.0;
        columns.push((example_x, tr("units-example")));
        crate::settings::card_header(ui, &columns);

        let sample = format::length_with(&u, comma, SAMPLE / u.length.unit.per_meter());
        let name = crate::properties::field_label(cc_core::ScheduleField::Length);
        quantity(ui, "length", &name, false, sample, |ui| {
            crate::settings::cell(ui, CELLS[1], |ui| {
                unit_combo(ui, state, "length", u.length.unit, &LengthUnit::ALL, length_name, |u, v| u.length.unit = v);
            });
            if u.length.unit.is_fractional() {
                crate::settings::cell(ui, CELLS[2], |ui| precision(ui, state, u.length.decimals));
                // Feet and inches always show their marks.
            } else {
                crate::settings::cell(ui, CELLS[2], |ui| {
                    decimals(ui, state, "length-decimals", u.length.decimals, |u, v| u.length.decimals = v);
                });
                crate::settings::cell(ui, CELLS[3], |ui| {
                    symbol(ui, state, "length-symbol", u.length.symbol, |u, v| u.length.symbol = v)
                });
            }
        });

        let sample = format::area_with(&u, comma, SAMPLE / u.area.unit.per_square_meter());
        let name = crate::properties::field_label(cc_core::ScheduleField::Area);
        quantity(ui, "area", &name, false, sample, |ui| {
            crate::settings::cell(ui, CELLS[1], |ui| {
                unit_combo(ui, state, "area", u.area.unit, &AreaUnit::ALL, area_name, |u, v| u.area.unit = v);
            });
            crate::settings::cell(ui, CELLS[2], |ui| {
                decimals(ui, state, "area-decimals", u.area.decimals, |u, v| u.area.decimals = v)
            });
            crate::settings::cell(ui, CELLS[3], |ui| {
                symbol(ui, state, "area-symbol", u.area.symbol, |u, v| u.area.symbol = v)
            });
        });

        let sample = format::volume_with(&u, comma, SAMPLE / u.volume.unit.per_cubic_meter());
        let name = crate::properties::field_label(cc_core::ScheduleField::Volume);
        quantity(ui, "volume", &name, false, sample, |ui| {
            crate::settings::cell(ui, CELLS[1], |ui| {
                unit_combo(ui, state, "volume", u.volume.unit, &VolumeUnit::ALL, volume_name, |u, v| u.volume.unit = v);
            });
            crate::settings::cell(ui, CELLS[2], |ui| {
                decimals(ui, state, "volume-decimals", u.volume.decimals, |u, v| u.volume.decimals = v);
            });
            crate::settings::cell(ui, CELLS[3], |ui| {
                symbol(ui, state, "volume-symbol", u.volume.symbol, |u, v| u.volume.symbol = v)
            });
        });

        let sample = format::angle_with(&u, comma, 12.3456);
        quantity(ui, "angle", &t!("units-angle"), false, sample, |ui| {
            crate::settings::cell(ui, CELLS[1], |ui| {
                ui.add_space(8.0);
                ui.label(t!("unit-degrees"));
            });
            crate::settings::cell(ui, CELLS[2], |ui| {
                decimals(ui, state, "angle-decimals", u.angle.decimals, |u, v| u.angle.decimals = v)
            });
            crate::settings::cell(ui, CELLS[3], |ui| {
                symbol(ui, state, "angle-symbol", u.angle.symbol, |u, v| u.angle.symbol = v)
            });
        });

        let sample = format::slope_with(&u, comma, 0.2345);
        quantity(ui, "slope", &t!("units-slope"), false, sample, |ui| {
            crate::settings::cell(ui, CELLS[1], |ui| {
                unit_combo(ui, state, "slope", u.slope.unit, &SlopeUnit::ALL, slope_name, |u, v| u.slope.unit = v);
            });
            crate::settings::cell(ui, CELLS[2], |ui| {
                decimals(ui, state, "slope-decimals", u.slope.decimals, |u, v| u.slope.decimals = v)
            });
            crate::settings::cell(ui, CELLS[3], |ui| {
                symbol(ui, state, "slope-symbol", u.slope.symbol, |u, v| u.slope.symbol = v)
            });
        });

        let sample = format::money_with(&u, comma, SAMPLE);
        quantity(ui, "currency", &t!("units-currency"), true, sample, |ui| {
            crate::settings::cell(ui, CELLS[1], |ui| {
                crate::settings::framed(ui);
                let mut currency = u.currency.clone();
                let r = crate::table::live_text(ui, "units-currency", &mut currency, |ui, c| {
                    ui.add(crate::settings::text_edit(c, CELLS[1]).hint_text(t!("units-currency-hint")))
                });
                if r.changed() {
                    let currency: String = currency.chars().take(8).collect();
                    set(state, "currency", |u| u.currency = currency);
                }
                done(state, &r);
            });
            crate::settings::cell(ui, CELLS[2], |ui| {
                decimals(ui, state, "currency-decimals", u.currency_decimals, |u, v| u.currency_decimals = v);
            });
        });
    });
    ui.add_space(8.0);
    crate::settings::setting(ui, &t!("units-style"), None, |ui| {
        unit_combo(ui, state, "style", u.style, &NumberStyle::ALL, |s| style_name(s, comma), |u, v| u.style = v);
    });
    let label = t!("units-diameter-point");
    let mut point = u.diameter_point;
    crate::settings::setting_row(ui, &label, None, |ui| {
        if crate::settings::switch(ui, &mut point, &label).changed() {
            set(state, "diameter-point", |u| u.diameter_point = point);
        }
    });
}
