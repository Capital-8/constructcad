//! Number formatting for drawings and schedules.
//!
//! The decimal separator follows the UI language (comma for Portuguese, point
//! for English) and is set once by the application with [`set_decimal_comma`].

use std::sync::atomic::{AtomicBool, Ordering};

static DECIMAL_COMMA: AtomicBool = AtomicBool::new(true);

/// Use `,` (true) or `.` (false) as the decimal separator.
pub fn set_decimal_comma(comma: bool) {
    DECIMAL_COMMA.store(comma, Ordering::Relaxed);
}

/// Format with `decimals` places, e.g. `0,80` or `0.80`.
pub fn number(value: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, value);
    if DECIMAL_COMMA.load(Ordering::Relaxed) { s.replace('.', ",") } else { s }
}

/// Length in meters, two decimals.
pub fn meters(value: f64) -> String {
    number(value, 2)
}

/// Level elevation with an explicit sign, e.g. `+3,00`.
pub fn elevation(value: f64) -> String {
    let sign = if value >= 0.0 { "+" } else { "-" };
    format!("{sign}{}", number(value.abs(), 2))
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats() {
        // Default separator; tests do not change it to stay order-independent.
        assert_eq!(super::meters(0.8), "0,80");
        assert_eq!(super::elevation(3.0), "+3,00");
        assert_eq!(super::elevation(-0.15), "-0,15");
    }
}
