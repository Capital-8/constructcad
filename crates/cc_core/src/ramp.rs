//! Ramps: the plan shapes of a ramp element and the accessibility rules of
//! its type (steepest slope, highest rise of one flight, shortest landing).

use crate::Turn;
use serde::{Deserialize, Serialize};

/// Plan shape of a ramp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum RampShape {
    /// Flights in line, with landings between them as the rules need.
    #[default]
    Straight,
    /// Two flights at a right angle around a corner landing.
    LShaped,
    /// Flights going back and forth (switchback), joined by landings.
    UShaped,
    /// One helical flight around a center.
    Spiral,
}

impl RampShape {
    pub const ALL: [RampShape; 4] = [RampShape::Straight, RampShape::LShaped, RampShape::UShaped, RampShape::Spiral];

    pub fn key(self) -> &'static str {
        match self {
            RampShape::Straight => "ramp-shape-straight",
            RampShape::LShaped => "ramp-shape-l",
            RampShape::UShaped => "ramp-shape-u",
            RampShape::Spiral => "ramp-shape-spiral",
        }
    }

    pub fn turns(self) -> bool {
        self != RampShape::Straight
    }
}

/// Plan layout of a ramp element.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RampLayout {
    pub shape: RampShape,
    pub turn: Turn,
    /// Slope of the flights, rise over run (0: the steepest the type allows).
    pub slope: f64,
    /// Landing length along the way (0: the type's shortest landing).
    pub landing: f64,
    /// Clear gap between the flights of a U-shaped ramp.
    pub gap: f64,
    /// Radius of the inner edge of a spiral ramp.
    pub inner_radius: f64,
}

impl Default for RampLayout {
    fn default() -> Self {
        Self { shape: RampShape::Straight, turn: Turn::Left, slope: 0.0, landing: 0.0, gap: 0.1, inner_radius: 1.5 }
    }
}

impl RampLayout {
    /// Replace invalid values by defaults; returns how many changed.
    pub fn repair(&mut self) -> usize {
        let d = RampLayout::default();
        let mut n = 0;
        for (v, default, max) in [
            (&mut self.slope, d.slope, 1.0),
            (&mut self.landing, d.landing, 100.0),
            (&mut self.gap, d.gap, 100.0),
            (&mut self.inner_radius, d.inner_radius, 100.0),
        ] {
            if !v.is_finite() || *v < 0.0 || *v > max {
                *v = default;
                n += 1;
            }
        }
        n
    }
}

/// Accessibility rules of a ramp type (defaults after common accessibility
/// codes: 8.33 % slope, 0.80 m rise per flight, 1.20 m landings).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RampRules {
    /// Steepest slope, rise over run.
    pub max_slope: f64,
    /// Highest rise of one flight between landings.
    pub max_flight_rise: f64,
    /// Shortest landing along the way.
    pub min_landing: f64,
}

impl Default for RampRules {
    fn default() -> Self {
        Self { max_slope: 0.0833, max_flight_rise: 0.8, min_landing: 1.2 }
    }
}

impl RampRules {
    /// Replace invalid limits by defaults; returns how many changed.
    pub fn repair(&mut self) -> usize {
        let d = RampRules::default();
        let mut n = 0;
        for (v, default, lo, hi) in [
            (&mut self.max_slope, d.max_slope, 0.005, 1.0),
            (&mut self.max_flight_rise, d.max_flight_rise, 0.05, 100.0),
            (&mut self.min_landing, d.min_landing, 0.0, 100.0),
        ] {
            if !v.is_finite() || *v < lo || *v > hi {
                *v = default;
                n += 1;
            }
        }
        n
    }
}

/// A broken rule of a ramp.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "issue", rename_all = "snake_case")]
pub enum RampIssue {
    TooSteep { slope: f64, max: f64 },
    FlightTooHigh { rise: f64, max: f64 },
    LandingTooShort { landing: f64, min: f64 },
}

impl RampIssue {
    pub fn key(&self) -> &'static str {
        match self {
            RampIssue::TooSteep { .. } => "ramp-issue-steep",
            RampIssue::FlightTooHigh { .. } => "ramp-issue-flight-rise",
            RampIssue::LandingTooShort { .. } => "ramp-issue-landing-short",
        }
    }
}

/// Slope, flights and the rules a ramp breaks.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RampCheck {
    pub slope: f64,
    pub flights: usize,
    /// Rise of each flight.
    pub flight_rise: f64,
    /// Plan length of each flight.
    pub flight_run: f64,
    pub issues: Vec<RampIssue>,
}

impl RampCheck {
    pub fn ok(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Most flights of one ramp.
pub const MAX_FLIGHTS: usize = 50;

/// Slope, flight count, rise and run of each flight of a ramp rising
/// `height` laid out as `layout`, and what it breaks of `rules`. Straight
/// and U-shaped ramps get as many flights as the highest flight rise
/// needs; L-shaped ones two; spiral ones one.
pub fn ramp_flights(height: f64, rules: &RampRules, layout: &RampLayout) -> RampCheck {
    let height = if height.is_finite() { height.max(0.0) } else { 0.0 };
    let max = if rules.max_slope.is_finite() && rules.max_slope > 1e-3 { rules.max_slope } else { 0.0833 };
    let slope = if layout.slope > 1e-3 && layout.slope.is_finite() { layout.slope } else { max };
    let per =
        if rules.max_flight_rise.is_finite() && rules.max_flight_rise > 0.01 { rules.max_flight_rise } else { 0.8 };
    let flights = match layout.shape {
        RampShape::Straight | RampShape::UShaped => ((height / per - 1e-9).ceil().max(1.0) as usize).min(MAX_FLIGHTS),
        RampShape::LShaped => 2,
        RampShape::Spiral => 1,
    };
    let flight_rise = height / flights as f64;
    let mut issues = Vec::new();
    if slope > max + 1e-6 {
        issues.push(RampIssue::TooSteep { slope, max });
    }
    if flight_rise > per + 1e-6 {
        issues.push(RampIssue::FlightTooHigh { rise: flight_rise, max: per });
    }
    let landing = if layout.landing > 1e-3 { layout.landing } else { rules.min_landing };
    if flights > 1 && layout.shape != RampShape::LShaped && landing < rules.min_landing - 1e-6 {
        issues.push(RampIssue::LandingTooShort { landing, min: rules.min_landing });
    }
    RampCheck { slope, flights, flight_rise, flight_run: flight_rise / slope, issues }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flights_follow_the_rules() {
        let rules = RampRules::default();
        // 1.50 m at 8.33 %: two flights of 0.75 m, 9 m long each.
        let c = ramp_flights(1.5, &rules, &RampLayout::default());
        assert_eq!(c.flights, 2);
        assert!((c.flight_rise - 0.75).abs() < 1e-12 && (c.flight_run - 0.75 / 0.0833).abs() < 1e-9);
        assert!(c.ok(), "{:?}", c.issues);
        // Exactly 0.80 m is one flight.
        assert_eq!(ramp_flights(0.8, &rules, &RampLayout::default()).flights, 1);
        // Steeper than allowed, an L-shaped one too high per flight, a short landing.
        let steep = RampLayout { slope: 0.12, landing: 1.0, ..Default::default() };
        let keys: Vec<_> = ramp_flights(1.5, &rules, &steep).issues.iter().map(|i| i.key()).collect();
        assert_eq!(keys, ["ramp-issue-steep", "ramp-issue-landing-short"]);
        let l = RampLayout { shape: RampShape::LShaped, ..Default::default() };
        assert_eq!(ramp_flights(3.0, &rules, &l).issues[0].key(), "ramp-issue-flight-rise");
    }

    #[test]
    fn repair_resets_bad_values() {
        let mut l = RampLayout { slope: f64::NAN, gap: -1.0, ..Default::default() };
        assert_eq!(l.repair(), 2);
        let mut r = RampRules { max_slope: 0.0, ..Default::default() };
        assert_eq!(r.repair(), 1);
        // Degenerate rules still give a finite layout.
        let c = ramp_flights(f64::INFINITY, &RampRules { max_flight_rise: 0.0, ..Default::default() }, &l);
        assert!(c.flights >= 1 && c.flight_run.is_finite());
    }
}
