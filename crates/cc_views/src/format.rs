//! Number formatting for drawings, schedules and the interface.
//!
//! Quantities follow the project units ([`cc_core::Units`]), set by the
//! application with [`set_units`]; with the default number style the
//! decimal symbol follows the interface language ([`set_decimal_comma`]).
//! The `*_with` functions take the settings explicitly (used by tests and
//! previews); the others read the current ones.

use cc_core::Units;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, RwLock};

static DECIMAL_COMMA: AtomicBool = AtomicBool::new(true);
static UNITS: LazyLock<RwLock<Units>> = LazyLock::new(|| RwLock::new(Units::default()));

/// Use `,` (true) or `.` (false) as the decimal separator of the language.
pub fn set_decimal_comma(comma: bool) {
    DECIMAL_COMMA.store(comma, Ordering::Relaxed);
}

/// Display quantities with `units` from now on.
pub fn set_units(units: &Units) {
    let mut guard = UNITS.write().unwrap_or_else(|e| e.into_inner());
    if *guard != *units {
        *guard = units.clone();
    }
}

/// The current units and language decimal symbol.
pub fn current() -> (Units, bool) {
    let units = UNITS.read().unwrap_or_else(|e| e.into_inner()).clone();
    (units, DECIMAL_COMMA.load(Ordering::Relaxed))
}

/// `value` with `decimals` places, the style's decimal symbol and grouping.
pub fn number_with(units: &Units, comma: bool, value: f64, decimals: usize) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let s = format!("{:.*}", decimals.min(12), value);
    let (decimal, group) = units.style.separators(comma);
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", s.as_str()),
    };
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let mut grouped = String::with_capacity(int.len() + int.len() / 3);
    for (i, c) in int.chars().enumerate() {
        if i > 0
            && (int.len() - i) % 3 == 0
            && let Some(g) = group
        {
            grouped.push(g);
        }
        grouped.push(c);
    }
    // "-0,00" reads oddly: drop the sign when everything rounds to zero.
    let zero = int.chars().all(|c| c == '0') && frac.chars().all(|c| c == '0');
    let sign = if zero { "" } else { sign };
    if frac.is_empty() { format!("{sign}{grouped}") } else { format!("{sign}{grouped}{decimal}{frac}") }
}

fn with_symbol(text: String, symbol: &str, show: bool) -> String {
    if show { format!("{text} {symbol}") } else { text }
}

/// Length in meters, in the length unit (symbol when the settings ask for it).
pub fn length_with(units: &Units, comma: bool, meters: f64) -> String {
    let f = units.length;
    with_symbol(number_with(units, comma, meters * f.unit.per_meter(), f.decimals as usize), f.unit.symbol(), f.symbol)
}

/// Length always with its unit symbol (property fields, labels).
pub fn length_unit_with(units: &Units, comma: bool, meters: f64) -> String {
    let f = units.length;
    format!("{} {}", number_with(units, comma, meters * f.unit.per_meter(), f.decimals as usize), f.unit.symbol())
}

pub fn area_with(units: &Units, comma: bool, square_meters: f64) -> String {
    let f = units.area;
    let v = square_meters * f.unit.per_square_meter();
    with_symbol(number_with(units, comma, v, f.decimals as usize), f.unit.symbol(), f.symbol)
}

pub fn volume_with(units: &Units, comma: bool, cubic_meters: f64) -> String {
    let f = units.volume;
    let v = cubic_meters * f.unit.per_cubic_meter();
    with_symbol(number_with(units, comma, v, f.decimals as usize), f.unit.symbol(), f.symbol)
}

pub fn angle_with(units: &Units, comma: bool, degrees: f64) -> String {
    let text = number_with(units, comma, degrees, units.angle.decimals as usize);
    if units.angle.symbol { format!("{text}°") } else { text }
}

/// Slope given as rise over run.
pub fn slope_with(units: &Units, comma: bool, slope: f64) -> String {
    let f = units.slope;
    let (v, symbol) = match f.unit {
        cc_core::SlopeUnit::Percent => (slope * 100.0, " %"),
        cc_core::SlopeUnit::Degrees => (slope.atan().to_degrees(), "°"),
    };
    let text = number_with(units, comma, v, f.decimals as usize);
    if f.symbol { format!("{text}{symbol}") } else { text }
}

pub fn money_with(units: &Units, comma: bool, amount: f64) -> String {
    let text = number_with(units, comma, amount, units.currency_decimals as usize);
    if units.currency.is_empty() { text } else { format!("{} {text}", units.currency) }
}

/// Parse a number typed with either decimal symbol and optional grouping.
pub fn parse_number(text: &str) -> Option<f64> {
    let t: String = text.trim().chars().filter(|c| !c.is_whitespace() && *c != '\u{202f}').collect();
    let t = match (t.rfind(','), t.rfind('.')) {
        // Both present: the last one is the decimal symbol.
        (Some(c), Some(p)) if c > p => t.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => t.replace(',', ""),
        (Some(_), None) => t.replace(',', "."),
        _ => t,
    };
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parse a length typed in the length unit (a trailing unit symbol overrides it).
pub fn parse_length_with(units: &Units, text: &str) -> Option<f64> {
    let t = text.trim();
    for unit in [cc_core::LengthUnit::Millimeter, cc_core::LengthUnit::Centimeter, cc_core::LengthUnit::Meter] {
        if let Some(n) = t.strip_suffix(unit.symbol()) {
            return parse_number(n).map(|v| v / unit.per_meter());
        }
    }
    parse_number(t).map(|v| v / units.length.unit.per_meter())
}

// --- Current settings ---

/// Plain number with the current decimal symbol and grouping.
pub fn number(value: f64, decimals: usize) -> String {
    let (u, c) = current();
    number_with(&u, c, value, decimals)
}

/// Length in the project's length unit (dimensions, tags, schedules).
pub fn meters(value: f64) -> String {
    let (u, c) = current();
    length_with(&u, c, value)
}

/// Length with its unit symbol.
pub fn length(value: f64) -> String {
    let (u, c) = current();
    length_unit_with(&u, c, value)
}

pub fn area(value: f64) -> String {
    let (u, c) = current();
    area_with(&u, c, value)
}

pub fn volume(value: f64) -> String {
    let (u, c) = current();
    volume_with(&u, c, value)
}

pub fn angle(degrees: f64) -> String {
    let (u, c) = current();
    angle_with(&u, c, degrees)
}

pub fn slope(value: f64) -> String {
    let (u, c) = current();
    slope_with(&u, c, value)
}

pub fn money(value: f64) -> String {
    let (u, c) = current();
    money_with(&u, c, value)
}

pub fn parse_length(text: &str) -> Option<f64> {
    parse_length_with(&current().0, text)
}

/// Level elevation with an explicit sign, e.g. `+3,00`.
pub fn elevation(value: f64) -> String {
    let sign = if value >= 0.0 { "+" } else { "-" };
    format!("{sign}{}", meters(value.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::{LengthUnit, NumberStyle, SlopeUnit};

    #[test]
    fn defaults_match_the_language() {
        let u = Units::default();
        assert_eq!(length_with(&u, true, 0.8), "0,80");
        assert_eq!(length_with(&u, false, 0.8), "0.80");
        assert_eq!(area_with(&u, true, 28.37), "28,37 m²");
        assert_eq!(volume_with(&u, true, 1.5), "1,500 m³");
        assert_eq!(number_with(&u, true, -0.001, 2), "0,00");
        assert_eq!(number_with(&u, true, 1234567.891, 2), "1234567,89");
    }

    #[test]
    fn units_symbols_and_grouping() {
        let mut u = Units::default();
        u.length.unit = LengthUnit::Millimeter;
        u.length.decimals = 0;
        u.style = NumberStyle::CommaDecimalPointGroups;
        assert_eq!(length_with(&u, false, 5.6), "5.600");
        assert_eq!(length_unit_with(&u, false, 10.9), "10.900 mm");
        u.length.symbol = true;
        assert_eq!(length_with(&u, false, 0.15), "150 mm");
        u.style = NumberStyle::PointDecimalCommaGroups;
        assert_eq!(number_with(&u, true, -1234567.5, 1), "-1,234,567.5");
        u.style = NumberStyle::CommaDecimalSpaceGroups;
        assert_eq!(number_with(&u, true, 1234.5, 1), "1\u{202f}234,5");
        u.slope.unit = SlopeUnit::Degrees;
        u.slope.decimals = 0;
        assert_eq!(slope_with(&u, true, 1.0), "45°");
        u.currency = "R$".into();
        assert_eq!(money_with(&u, true, 1500.0), "R$ 1\u{202f}500,00");
        assert_eq!(angle_with(&u, true, 90.0), "90,00°");
    }

    #[test]
    fn parsing_accepts_both_symbols_and_units() {
        assert_eq!(parse_number("3,5"), Some(3.5));
        assert_eq!(parse_number("1.234,5"), Some(1234.5));
        assert_eq!(parse_number("1,234.5"), Some(1234.5));
        assert_eq!(parse_number(" 1 234 "), Some(1234.0));
        assert_eq!(parse_number("abc"), None);
        let mut u = Units::default();
        u.length.unit = LengthUnit::Millimeter;
        assert_eq!(parse_length_with(&u, "150"), Some(0.15));
        assert_eq!(parse_length_with(&u, "2,5 m"), Some(2.5));
        assert_eq!(parse_length_with(&u, "30cm"), Some(0.3));
    }
}
