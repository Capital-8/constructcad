//! Conceptual masses: volumes for massing studies. A mass is a base outline
//! in plan extruded `height` up from its level (plus `base_offset`), or
//! lofted to a different top outline. Mass floors cut it at the levels it
//! lists: each gives the floor outline at that level, with its area and
//! perimeter, for gross floor areas before any wall or floor is drawn.

use crate::LevelId;
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Largest outline a mass keeps (points).
pub const MAX_OUTLINE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mass {
    /// Base outline in plan (closed, either winding).
    pub outline: Vec<DVec2>,
    /// Offset of the base from the level elevation.
    #[serde(default)]
    pub base_offset: f64,
    /// Height of the mass above its base.
    pub height: f64,
    /// Top outline of a loft; `None` extrudes the base outline.
    #[serde(default)]
    pub top: Option<Vec<DVec2>>,
    /// Levels where mass floors are taken (those the mass spans).
    #[serde(default)]
    pub floors: Vec<LevelId>,
}

impl Mass {
    /// Outlines of at least three finite points (bounded), a finite positive
    /// height and a finite base offset.
    pub fn is_valid(&self) -> bool {
        let outline_ok = |o: &[DVec2]| o.len() >= 3 && o.len() <= MAX_OUTLINE && o.iter().all(|p| p.is_finite());
        outline_ok(&self.outline)
            && self.top.as_deref().is_none_or(outline_ok)
            && self.height.is_finite()
            && self.height > 1e-6
            && self.base_offset.is_finite()
    }

    /// The outline at `t` of the height (0 at the base, 1 at the top),
    /// counter-clockwise: the base outline of an extrusion, or the loft's
    /// section between its outlines.
    pub fn section(&self, t: f64) -> Vec<DVec2> {
        let base = crate::sweep::counter_clockwise(&self.outline);
        match &self.top {
            None => base,
            Some(top) => crate::sweep::blend_section(&base, &crate::sweep::counter_clockwise(top), t.clamp(0.0, 1.0)),
        }
    }
}

/// A mass floor: the outline of a mass at a level, its area and perimeter.
#[derive(Debug, Clone, PartialEq)]
pub struct MassFloor {
    pub level: LevelId,
    pub outline: Vec<DVec2>,
    pub area: f64,
    pub perimeter: f64,
}

/// Area of a closed polygon (either winding).
pub fn area(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].perp_dot(poly[(i + 1) % n])).sum::<f64>().abs() * 0.5
}

/// Perimeter of a closed polygon.
pub fn perimeter(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].distance(poly[(i + 1) % n])).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, d: f64) -> Vec<DVec2> {
        vec![DVec2::ZERO, DVec2::new(w, 0.0), DVec2::new(w, d), DVec2::new(0.0, d)]
    }

    #[test]
    fn sections_of_extrusions_and_lofts() {
        let mut m = Mass { outline: rect(10.0, 6.0), base_offset: 0.0, height: 9.0, top: None, floors: Vec::new() };
        assert!(m.is_valid());
        assert!((area(&m.section(0.5)) - 60.0).abs() < 1e-9);
        // A loft from 10 x 6 to 4 x 6: halfway the section is 7 x 6.
        m.top = Some(rect(4.0, 6.0).into_iter().map(|p| p + DVec2::new(3.0, 0.0)).collect());
        assert!((area(&m.section(0.5)) - 42.0).abs() < 1e-6, "{}", area(&m.section(0.5)));
        assert!((area(&m.section(1.0)) - 24.0).abs() < 1e-6);
        assert!((perimeter(&rect(10.0, 6.0)) - 32.0).abs() < 1e-12);
        // Bad data is invalid.
        m.height = 0.0;
        assert!(!m.is_valid());
        m.height = 3.0;
        m.outline = vec![DVec2::ZERO, DVec2::X];
        assert!(!m.is_valid());
        m.outline = rect(1.0, 1.0);
        m.top = Some(vec![DVec2::new(f64::NAN, 0.0); 3]);
        assert!(!m.is_valid());
    }
}
