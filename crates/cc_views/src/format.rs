//! Number formatting for drawings, schedules and the interface.
//!
//! Quantities follow the project units ([`cc_core::Units`]), set by the
//! application with [`set_units`]; with the default number style the
//! decimal symbol follows the interface language ([`set_decimal_comma`]).
//! The `*_with` functions take the settings explicitly (used by tests and
//! previews); the others read the current ones.

use cc_core::Units;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

static DECIMAL_COMMA: AtomicBool = AtomicBool::new(true);

thread_local! {
    /// The units quantities are shown in, per thread: the app draws on one
    /// thread, and sessions on other threads (tests, scripts) each keep
    /// their own project's units instead of overwriting one another's.
    static UNITS: RefCell<Units> = RefCell::new(Units::default());
}

/// Use `,` (true) or `.` (false) as the decimal separator of the language.
pub fn set_decimal_comma(comma: bool) {
    DECIMAL_COMMA.store(comma, Ordering::Relaxed);
}

/// Display quantities with `units` from now on.
pub fn set_units(units: &Units) {
    UNITS.with_borrow_mut(|current| {
        if *current != *units {
            *current = units.clone();
        }
    });
}

/// The current units and language decimal symbol.
pub fn current() -> (Units, bool) {
    let units = UNITS.with_borrow(Units::clone);
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

/// Inches rounded to 1/`den` (a power of two), as whole inches and the
/// reduced fraction left (numerator, denominator; 0 when none).
fn fraction(inches: f64, den: u32) -> (f64, u32, u32) {
    let den = den.max(1);
    let ticks = (inches * f64::from(den)).round();
    let whole = (ticks / f64::from(den)).floor();
    let (mut num, mut den) = ((ticks - whole * f64::from(den)) as u32, den);
    while num > 0 && num % 2 == 0 && den % 2 == 0 {
        (num, den) = (num / 2, den / 2);
    }
    (whole, num, den)
}

/// The denominator of the fractional inches of a precision setting.
fn denominator(decimals: u8) -> u32 {
    1 << decimals.min(cc_core::units::MAX_DECIMALS)
}

/// Inches with a fraction: `6`, `6 1/2`, `1/2` (`0 1/2` with `zero`; no
/// inch mark).
fn mixed_inches(units: &Units, comma: bool, inches: f64, den: u32, zero: bool) -> String {
    let (whole, num, den) = fraction(inches, den);
    let whole_text = number_with(units, comma, whole, 0);
    match (num, whole == 0.0 && !zero) {
        (0, _) => whole_text,
        (_, true) => format!("{num}/{den}"),
        _ => format!("{whole_text} {num}/{den}"),
    }
}

/// A length in feet and fractional inches (`12'-6 1/2"`) or fractional
/// inches (`150 1/2"`), to 1/2^decimals of an inch.
fn fractional_with(units: &Units, comma: bool, meters: f64) -> String {
    let f = units.length;
    let meters = if meters.is_finite() { meters } else { 0.0 };
    let den = denominator(f.decimals);
    let inches = meters.abs() / cc_core::units::INCH;
    // The sign goes only on lengths that do not round to zero.
    let (whole, num, _) = fraction(inches, den);
    let sign = if meters < 0.0 && (whole > 0.0 || num > 0) { "-" } else { "" };
    if f.unit == cc_core::LengthUnit::FeetInches {
        // Round first, so 11 31/32" at 1/16" becomes 1'-0".
        let rounded = (inches * f64::from(den)).round() / f64::from(den);
        let feet = (rounded / 12.0 + 1e-9).floor();
        let rest = (rounded - feet * 12.0).max(0.0);
        format!("{sign}{}'-{}\"", number_with(units, comma, feet, 0), mixed_inches(units, comma, rest, den, true))
    } else {
        format!("{sign}{}\"", mixed_inches(units, comma, inches, den, false))
    }
}

/// A length's number in the length unit, without the unit symbol; feet
/// and inches and fractional inches carry their own marks.
pub fn length_number_with(units: &Units, comma: bool, meters: f64) -> String {
    let f = units.length;
    if f.unit.is_fractional() {
        fractional_with(units, comma, meters)
    } else {
        number_with(units, comma, meters * f.unit.per_meter(), f.decimals as usize)
    }
}

/// Length in meters, in the length unit (symbol when the settings ask for it).
pub fn length_with(units: &Units, comma: bool, meters: f64) -> String {
    let f = units.length;
    let text = length_number_with(units, comma, meters);
    if f.unit.is_fractional() { text } else { with_symbol(text, f.unit.symbol(), f.symbol) }
}

/// Length always with its unit symbol (property fields, labels).
pub fn length_unit_with(units: &Units, comma: bool, meters: f64) -> String {
    let f = units.length;
    let text = length_number_with(units, comma, meters);
    if f.unit.is_fractional() { text } else { format!("{text} {}", f.unit.symbol()) }
}

/// A small size (sections, diameters) in inches with a fraction to 1/8
/// (`7 1/2`), no mark.
pub fn inches_with(units: &Units, comma: bool, meters: f64) -> String {
    let meters = if meters.is_finite() { meters.abs() } else { 0.0 };
    mixed_inches(units, comma, meters / cc_core::units::INCH, 8, false)
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

/// A number that may be mixed with a fraction: `6`, `6,5`, `1/2`, `6 1/2`
/// or `6-1/2`.
fn parse_mixed(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    // Spaces separate a whole number from its fraction, never digit groups.
    if !t.contains('/') {
        return if t.contains(char::is_whitespace) { None } else { parse_number(t) };
    }
    let t = t.replace('-', " ");
    let mut parts = t.split_whitespace();
    let (first, second) = (parts.next()?, parts.next());
    if parts.next().is_some() {
        return None;
    }
    let ratio = |s: &str| -> Option<f64> {
        let (n, d) = s.split_once('/')?;
        let (n, d) = (parse_number(n)?, parse_number(d)?);
        (d != 0.0).then(|| n / d).filter(|v| v.is_finite())
    };
    match second {
        Some(frac) => Some(parse_number(first)? + ratio(frac)?),
        None => ratio(first),
    }
}

/// Feet and inches: `12'6"`, `12'-6 1/2"`, `12' 6.5"`, `12'`, `6 1/2"`,
/// `12ft 6in`, and, read in feet and inches, `12 6` or `12 6 1/2`. The
/// outer `None` when the text is not written in feet or inches; the inner
/// one when it is, but cannot be read.
fn parse_imperial(units: &Units, text: &str) -> Option<Option<f64>> {
    let t = text.trim();
    let (negative, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, t),
    };
    let t = t.replace("feet", "'").replace("foot", "'").replace("ft", "'");
    let t = t.replace("inches", "\"").replace("inch", "\"").replace("in", "\"").replace("''", "\"");
    let unit = units.length.unit;
    let inches = if let Some((feet, rest)) = t.split_once('\'') {
        let rest = rest.trim().trim_start_matches('-').trim();
        let rest = rest.strip_suffix('"').unwrap_or(rest);
        let inches = if rest.trim().is_empty() { Some(0.0) } else { parse_mixed(rest) };
        parse_mixed(feet).zip(inches).map(|(f, i)| f * 12.0 + i)
    } else if let Some(inches) = t.trim().strip_suffix('"') {
        parse_mixed(inches)
    } else if unit == cc_core::LengthUnit::FeetInches {
        // Bare numbers: feet, then inches (with a fraction).
        let mut parts = t.split_whitespace();
        let feet = parts.next().and_then(parse_mixed);
        let rest: Vec<&str> = parts.collect();
        let inches = if rest.is_empty() { Some(0.0) } else { parse_mixed(&rest.join(" ")) };
        feet.zip(inches).map(|(f, i)| f * 12.0 + i)
    } else if unit == cc_core::LengthUnit::FractionalInch || t.contains('/') {
        parse_mixed(&t).map(|v| v / unit.per_meter() / cc_core::units::INCH)
    } else {
        return None;
    };
    let m = inches.map(|i| i * cc_core::units::INCH).filter(|m| m.is_finite());
    Some(m.map(|m| if negative { -m } else { m }))
}

/// Parse a length typed in the length unit (a trailing unit symbol, or
/// feet and inch marks, override it).
pub fn parse_length_with(units: &Units, text: &str) -> Option<f64> {
    let t = text.trim();
    for unit in [cc_core::LengthUnit::Millimeter, cc_core::LengthUnit::Centimeter, cc_core::LengthUnit::Meter] {
        if let Some(n) = t.strip_suffix(unit.symbol()) {
            return parse_number(n).map(|v| v / unit.per_meter());
        }
    }
    if let Some(m) = parse_imperial(units, t) {
        return m;
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

/// Whether lengths are shown in US customary units.
pub fn imperial() -> bool {
    current().0.length.unit.is_imperial()
}

/// A small size in inches with a fraction (see [`inches_with`]).
pub fn inches(meters: f64) -> String {
    let (u, c) = current();
    inches_with(&u, c, meters)
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

    #[test]
    fn feet_and_fractional_inches() {
        let mut u = Units::imperial();
        let ft = |f: f64, i: f64| f * 0.3048 + i * 0.0254;
        assert_eq!(length_with(&u, false, ft(12.0, 6.5)), "12'-6 1/2\"");
        assert_eq!(length_with(&u, false, ft(0.0, 0.0)), "0'-0\"");
        assert_eq!(length_with(&u, false, ft(3.0, 0.0)), "3'-0\"");
        assert_eq!(length_with(&u, false, ft(0.0, 0.25)), "0'-0 1/4\"");
        assert_eq!(length_with(&u, false, -ft(1.0, 2.0)), "-1'-2\"");
        // Rounding carries into the feet: 11 31/32" to the nearest 1/16" is 1'-0".
        assert_eq!(length_with(&u, false, ft(0.0, 11.0 + 31.0 / 32.0)), "1'-0\"");
        assert_eq!(length_with(&u, false, ft(1234.0, 0.0)), "1,234'-0\"");
        // The precision: 1/2^decimals.
        u.length.decimals = 1;
        assert_eq!(length_with(&u, false, ft(0.0, 6.3)), "0'-6 1/2\"");
        u.length.decimals = 6;
        assert_eq!(length_with(&u, false, ft(0.0, 1.0 / 64.0)), "0'-0 1/64\"");
        u.length.unit = cc_core::LengthUnit::FractionalInch;
        u.length.decimals = 3;
        assert_eq!(length_with(&u, false, ft(12.0, 6.125)), "150 1/8\"");
        assert_eq!(length_unit_with(&u, false, ft(0.0, 0.5)), "1/2\"");
        u.length.unit = cc_core::LengthUnit::Foot;
        u.length.decimals = 2;
        u.length.symbol = true;
        assert_eq!(length_with(&u, false, ft(12.0, 6.0)), "12.50 ft");
        u.length.unit = cc_core::LengthUnit::Inch;
        assert_eq!(length_unit_with(&u, false, ft(1.0, 0.5)), "12.50 in");
        u.area.unit = cc_core::AreaUnit::SquareFoot;
        assert_eq!(area_with(&u, false, ft(10.0, 0.0) * ft(10.0, 0.0)), "100.00 ft²");
        u.volume.unit = cc_core::VolumeUnit::CubicYard;
        u.volume.decimals = 2;
        assert_eq!(volume_with(&u, false, 27.0 * ft(1.0, 0.0).powi(3)), "1.00 yd³");
        assert_eq!(inches_with(&u, false, ft(0.0, 7.5)), "7 1/2");
        assert_eq!(inches_with(&u, false, ft(0.0, 8.0)), "8");
    }

    #[test]
    fn parsing_feet_and_inches() {
        let u = Units::imperial();
        let ft = |f: f64, i: f64| Some(f * 0.3048 + i * 0.0254);
        let close = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            _ => false,
        };
        for (text, want) in [
            ("12'6\"", ft(12.0, 6.0)),
            ("12'-6 1/2\"", ft(12.0, 6.5)),
            ("12' 6.5\"", ft(12.0, 6.5)),
            ("12' 6,5\"", ft(12.0, 6.5)),
            ("12'", ft(12.0, 0.0)),
            ("6 1/2\"", ft(0.0, 6.5)),
            ("6-1/2\"", ft(0.0, 6.5)),
            ("3/4\"", ft(0.0, 0.75)),
            ("12ft 6in", ft(12.0, 6.0)),
            ("12 ft", ft(12.0, 0.0)),
            ("6 in", ft(0.0, 6.0)),
            ("-1'-2\"", ft(-1.0, -2.0)),
            // Bare numbers in feet and inches: feet, then inches.
            ("10", ft(10.0, 0.0)),
            ("10 6", ft(10.0, 6.0)),
            ("10 6 1/2", ft(10.0, 6.5)),
            ("2.5", ft(2.5, 0.0)),
            // Metric still works.
            ("2,5 m", Some(2.5)),
            ("30 cm", Some(0.3)),
        ] {
            assert!(close(parse_length_with(&u, text), want), "{text}: {:?}", parse_length_with(&u, text));
        }
        for bad in ["", "'", "1/0\"", "abc", "1 2 3 4", "12'abc"] {
            assert_eq!(parse_length_with(&u, bad), None, "{bad}");
        }
        let mut u = u;
        u.length.unit = cc_core::LengthUnit::FractionalInch;
        assert!(close(parse_length_with(&u, "6 1/2"), ft(0.0, 6.5)));
        u.length.unit = cc_core::LengthUnit::Inch;
        assert!(close(parse_length_with(&u, "6.5"), ft(0.0, 6.5)));
        u.length.unit = cc_core::LengthUnit::Foot;
        assert!(close(parse_length_with(&u, "1 1/2"), ft(1.5, 0.0)));
    }
}
