//! Detail lines: view-specific 2D curves (lines, arcs, circles and whole or
//! partial ellipses, the [`SketchCurve`] geometry) drawn in one view with a
//! line style. Like text notes they live in their view's drawing
//! coordinates (meters), are owned by it (`Element::owner_view`) and go
//! with it; they belong to no phase-dependent geometry, take no part in the
//! model's 3D or IFC, and are drawn in drafting views and on top of plans,
//! ceiling plans, elevations, sections and legends.
//!
//! A polyline is drawn as one element per segment, so each segment is an
//! ordinary curve that the sketch edits ([`crate::sketch_edit`]: trim,
//! extend, split, offset) and grips change on its own.
//!
//! The line style is a [`LineStyleRef`]: for now one of the built-in
//! [`DetailStyle`]s. Project line styles (named, with weight, color and
//! dash pattern, kept as project settings) will be another variant of it,
//! so files written now keep loading.

use crate::sketch::SketchCurve;
use crate::{LinePattern, ViewId};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A detail line of one view.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DetailLine {
    /// The view that owns (and alone shows) the line.
    pub view: ViewId,
    /// The curve, in the view's drawing coordinates (meters).
    pub curve: SketchCurve,
    /// How the line is drawn.
    #[serde(default)]
    pub style: LineStyleRef,
}

impl DetailLine {
    /// Finite numbers and a size (see [`SketchCurve::is_valid`]), within bounds.
    pub fn is_valid(&self) -> bool {
        self.curve.is_valid() && within(&self.curve)
    }

    /// What drawing tools snap to on the line (in the shape of an imported
    /// drawing's snap features): ends, middles, centers, quadrant and axis
    /// points, and the lines and arcs themselves (ellipses by their chords).
    pub fn snap_features(&self, out: &mut crate::cad::CadSnap) {
        use crate::arc::Arc;
        use std::f64::consts::TAU;
        if !self.is_valid() {
            return;
        }
        match self.curve {
            SketchCurve::Line { a, b } => {
                out.ends.extend([a, b]);
                out.mids.push((a + b) * 0.5);
                out.lines.push((a, b));
            }
            SketchCurve::Arc { a, b, bulge } => {
                out.ends.extend([a, b]);
                match Arc::from_bulge(a, b, bulge) {
                    Some(arc) => {
                        out.mids.push(arc.point(arc.length() * 0.5));
                        out.centers.push(arc.center);
                        out.arcs.push(arc);
                    }
                    None => out.lines.push((a, b)),
                }
            }
            SketchCurve::Circle { center, radius } => {
                out.centers.push(center);
                out.ends.extend((0..4).map(|k| center + DVec2::from_angle(k as f64 * TAU / 4.0) * radius));
                out.arcs.push(Arc { center, radius, start_angle: 0.0, sweep: TAU });
            }
            SketchCurve::Ellipse { center, radii, rotation_deg, .. } => {
                out.centers.push(center);
                let turn = DVec2::from_angle(rotation_deg.to_radians());
                if let Some((a, b)) = self.curve.ends() {
                    out.ends.extend([a, b]);
                } else {
                    out.ends.extend(
                        [DVec2::X * radii.x, DVec2::Y * radii.y, -DVec2::X * radii.x, -DVec2::Y * radii.y]
                            .map(|v| center + turn.rotate(v)),
                    );
                }
                out.lines.extend(self.curve.points().windows(2).map(|w| (w[0], w[1])));
            }
        }
    }
}

/// Coordinates and sizes past this (meters) are refused (a crafted file).
pub const MAX_COORD: f64 = 1.0e7;

fn within(c: &SketchCurve) -> bool {
    let ok = |v: f64| v.abs() <= MAX_COORD;
    match *c {
        SketchCurve::Line { a, b } | SketchCurve::Arc { a, b, .. } => ok(a.x) && ok(a.y) && ok(b.x) && ok(b.y),
        SketchCurve::Circle { center, radius } => ok(center.x) && ok(center.y) && ok(radius),
        SketchCurve::Ellipse { center, radii, rotation_deg, start_deg, sweep_deg, .. } => {
            [center.x, center.y, radii.x, radii.y, rotation_deg, start_deg, sweep_deg].into_iter().all(ok)
        }
    }
}

/// The style a detail line is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineStyleRef {
    /// One of the built-in styles.
    BuiltIn(DetailStyle),
}

impl Default for LineStyleRef {
    fn default() -> Self {
        LineStyleRef::BuiltIn(DetailStyle::default())
    }
}

impl LineStyleRef {
    /// Width on paper (millimeters), color and dash pattern.
    pub fn pen(self) -> (f64, [u8; 3], LinePattern) {
        match self {
            LineStyleRef::BuiltIn(s) => (s.weight_mm(), [0, 0, 0], s.pattern()),
        }
    }

    /// The style's name as a code (DXF layers, commands).
    pub fn code(self) -> &'static str {
        match self {
            LineStyleRef::BuiltIn(s) => s.code(),
        }
    }

    /// Translation key of the style's name.
    pub fn key(self) -> &'static str {
        match self {
            LineStyleRef::BuiltIn(s) => s.key(),
        }
    }
}

/// The built-in line styles of detail lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DetailStyle {
    /// Fine continuous line.
    #[default]
    Thin,
    Medium,
    Wide,
    /// Short dashes.
    Hidden,
    /// Long dashes.
    Dashed,
    /// Dash-dot (center lines, axes).
    Center,
    Dotted,
}

impl DetailStyle {
    pub const ALL: [DetailStyle; 7] = [
        DetailStyle::Thin,
        DetailStyle::Medium,
        DetailStyle::Wide,
        DetailStyle::Hidden,
        DetailStyle::Dashed,
        DetailStyle::Center,
        DetailStyle::Dotted,
    ];

    /// Width on paper, millimeters.
    pub fn weight_mm(self) -> f64 {
        match self {
            DetailStyle::Thin => 0.13,
            DetailStyle::Medium => 0.25,
            DetailStyle::Wide => 0.5,
            DetailStyle::Hidden | DetailStyle::Dashed | DetailStyle::Center | DetailStyle::Dotted => 0.18,
        }
    }

    pub fn pattern(self) -> LinePattern {
        match self {
            DetailStyle::Thin | DetailStyle::Medium | DetailStyle::Wide => LinePattern::Solid,
            DetailStyle::Hidden => LinePattern::ShortDash,
            DetailStyle::Dashed => LinePattern::Dash,
            DetailStyle::Center => LinePattern::DashDot,
            DetailStyle::Dotted => LinePattern::Dot,
        }
    }

    /// The name used by commands and DXF layers.
    pub fn code(self) -> &'static str {
        match self {
            DetailStyle::Thin => "thin",
            DetailStyle::Medium => "medium",
            DetailStyle::Wide => "wide",
            DetailStyle::Hidden => "hidden",
            DetailStyle::Dashed => "dashed",
            DetailStyle::Center => "center",
            DetailStyle::Dotted => "dotted",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        DetailStyle::ALL.into_iter().find(|s| s.code().eq_ignore_ascii_case(code.trim()))
    }

    /// Translation key of the name.
    pub fn key(self) -> &'static str {
        match self {
            DetailStyle::Thin => "line-style-thin",
            DetailStyle::Medium => "line-style-medium",
            DetailStyle::Wide => "line-style-wide",
            DetailStyle::Hidden => "line-style-hidden",
            DetailStyle::Dashed => "line-style-dashed",
            DetailStyle::Center => "line-style-center",
            DetailStyle::Dotted => "line-style-dotted",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_bad_curves_are_invalid() {
        for s in DetailStyle::ALL {
            assert_eq!(DetailStyle::from_code(s.code()), Some(s));
        }
        let line = |b: DVec2| DetailLine {
            view: ViewId(1),
            curve: SketchCurve::Line { a: DVec2::ZERO, b },
            style: LineStyleRef::default(),
        };
        assert!(line(DVec2::X).is_valid());
        assert!(!line(DVec2::ZERO).is_valid());
        assert!(!line(DVec2::new(f64::NAN, 0.0)).is_valid());
        assert!(!line(DVec2::new(1e9, 0.0)).is_valid());
    }

    #[test]
    fn style_reads_from_older_text_without_it() {
        let l: DetailLine = ron::from_str("(view: (3), curve: Line(a: (0.0, 0.0), b: (1.0, 0.0)))").unwrap();
        assert_eq!(l.style, LineStyleRef::BuiltIn(DetailStyle::Thin));
    }
}
