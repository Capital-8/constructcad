//! Bending schedules: what a reinforcement schedule shows of a bar mark.
//!
//! The shape of a mark is written as a code (CSV, the app's table):
//! `straight 500`, `L 30-470`, `U 30-470-30`, `Z …`, `bent …`,
//! `stirrup 14x44`, `ring R=50 L=314`, `closed …`, `arc R=220 L=345`, legs
//! of arcs as `(R=… L=…)`; lengths as [`format::bar_length`] writes them.

use crate::format;
use cc_geom::bar_shape::{LegSummary, ShapeSummary};

fn arc_text(l: &LegSummary) -> String {
    format!("R={} L={}", format::bar_length(l.radius.unwrap_or(0.0)), format::bar_length(l.length))
}

/// The code of a shape (see the module's documentation).
pub fn shape_code(shape: &ShapeSummary) -> String {
    // The shorter end leg first.
    let (first, last) = (shape.legs.first().map_or(0.0, |l| l.length), shape.legs.last().map_or(0.0, |l| l.length));
    let s = if !shape.closed && first > last + 1e-9 { shape.reversed() } else { shape.clone() };
    let legs: Vec<&LegSummary> = s.legs.iter().collect();
    let straight = legs.iter().all(|l| l.radius.is_none());
    let lengths = |legs: &[&LegSummary]| {
        legs.iter()
            .map(|l| if l.radius.is_some() { format!("({})", arc_text(l)) } else { format::bar_length(l.length) })
            .collect::<Vec<_>>()
            .join("-")
    };
    let right_angle = |l: &LegSummary| (l.bend.abs() - std::f64::consts::FRAC_PI_2).abs() < 0.02;
    match (s.closed, legs.as_slice()) {
        (true, [ring]) if ring.radius.is_some() => format!("ring {}", arc_text(ring)),
        (true, [a, b, _, _]) if straight && legs.iter().all(|l| right_angle(l)) => {
            let (lo, hi) = if a.length <= b.length { (a.length, b.length) } else { (b.length, a.length) };
            format!("stirrup {}x{}", format::bar_length(lo), format::bar_length(hi))
        }
        (true, legs) => format!("closed {}", lengths(legs)),
        (false, []) => String::new(),
        (false, [one]) if one.radius.is_some() => format!("arc {}", arc_text(one)),
        (false, [one]) => format!("straight {}", format::bar_length(one.length)),
        (false, legs @ [_, _]) if straight => format!("L {}", lengths(legs)),
        (false, legs @ [a, b, _]) if straight => {
            let same_way = a.bend * b.bend > 0.0;
            format!("{} {}", if same_way { "U" } else { "Z" }, lengths(legs))
        }
        (false, legs) => format!("bent {}", lengths(legs)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_geom::bar_shape::summary;
    use cc_geom::rebar::Bar;
    use glam::DVec3;

    fn code(points: &[[f64; 3]], closed: bool) -> String {
        format::set_units(&cc_core::Units::default());
        shape_code(&summary(&Bar::new(points.iter().map(|p| DVec3::from_array(*p)).collect(), closed)))
    }

    #[test]
    fn shapes_have_codes() {
        assert_eq!(code(&[[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]], false), "straight 500");
        // The shorter end leg first, whichever end the bar was drawn from.
        assert_eq!(code(&[[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [4.7, 0.0, 0.0]], false), "L 30-470");
        assert_eq!(code(&[[4.7, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.3]], false), "L 30-470");
        let u = [[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [4.7, 0.0, 0.0], [4.7, 0.0, 0.3]];
        assert_eq!(code(&u, false), "U 30-470-30");
        let z = [[0.0, 0.0, 0.3], [0.0, 0.0, 0.0], [4.7, 0.0, 0.0], [4.7, 0.0, -0.3]];
        assert_eq!(code(&z, false), "Z 30-470-30");
        let stirrup = [[0.0, 0.0, 0.0], [0.14, 0.0, 0.0], [0.14, 0.0, 0.44], [0.0, 0.0, 0.44]];
        assert_eq!(code(&stirrup, true), "stirrup 14x44");
        assert_eq!(code(&[], false), "");
    }
}
