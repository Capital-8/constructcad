use cc_core::ElementId;
use glam::DVec2;

/// Graphic weight of a line, resolved to pen widths by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineStyle {
    /// Element cut by the view plane.
    Cut,
    /// Visible element beyond the cut plane.
    Projection,
    /// Fine detail (layers, glass, door swings).
    Thin,
    /// Dashed: element above the cut plane or hidden.
    Hidden,
    /// Levels, section marks, text leaders.
    Annotation,
    /// Dash-dot: grid lines and center lines.
    Center,
    /// Background linework (the architecture under building services).
    Faint,
    /// Lines of a cut pattern (drawn like `Thin`; view graphics replace them
    /// when they override the cut pattern).
    Hatch,
}

/// Graphic overrides of one line (view graphics); `None` keeps the style's pen.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pen {
    pub color: Option<[u8; 3]>,
    /// Width on paper, millimeters.
    pub weight: Option<f64>,
    pub pattern: Option<cc_core::LinePattern>,
    /// Mixed with white.
    pub halftone: bool,
}

/// A line's pen resolved: width in paper millimeters, color and dash lengths
/// in paper millimeters (empty: continuous).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPen {
    pub width: f64,
    pub color: [u8; 3],
    pub dashes: &'static [f64],
}

impl LineStyle {
    /// The default pen of the style: width (paper mm) and color.
    pub fn pen(self) -> (f64, [u8; 3]) {
        match self {
            LineStyle::Cut => (0.5, [10, 10, 10]),
            LineStyle::Projection => (0.25, [45, 45, 45]),
            LineStyle::Thin | LineStyle::Hatch => (0.13, [95, 95, 95]),
            LineStyle::Hidden => (0.18, [120, 120, 120]),
            LineStyle::Annotation => (0.18, [30, 90, 170]),
            LineStyle::Center => (0.18, [30, 90, 170]),
            LineStyle::Faint => (0.13, [175, 175, 175]),
        }
    }

    /// Dash pattern of the style, paper millimeters.
    pub fn dashes(self) -> &'static [f64] {
        match self {
            LineStyle::Hidden => cc_core::LinePattern::ShortDash.dashes_mm(),
            LineStyle::Center => cc_core::LinePattern::DashDot.dashes_mm(),
            _ => &[],
        }
    }

    /// The pen of a line of this style with its overrides.
    pub fn resolve(self, pen: Option<&Pen>) -> ResolvedPen {
        let (width, color) = self.pen();
        let mut out = ResolvedPen { width, color, dashes: self.dashes() };
        if let Some(p) = pen {
            if let Some(c) = p.color {
                out.color = c;
            }
            if let Some(w) = p.weight.filter(|w| w.is_finite() && *w > 0.0) {
                out.width = w.min(10.0);
            }
            if let Some(pattern) = p.pattern {
                out.dashes = pattern.dashes_mm();
            }
            if p.halftone {
                out.color = halftone(out.color);
            }
        }
        out
    }
}

/// A color mixed two thirds with white (faint drawing).
pub fn halftone(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| (f64::from(v) + (255.0 - f64::from(v)) * 0.66).round() as u8)
}

/// Which point of a text box sits at its position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    LeftBottom,
    CenterBottom,
    Center,
}

/// Drawing primitive, in view coordinates (meters on the view plane).
#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Line {
        a: DVec2,
        b: DVec2,
        style: LineStyle,
        element: Option<ElementId>,
        /// Overrides from the view's graphics.
        pen: Option<Pen>,
    },
    /// Simple polygon fill (any winding, may be concave).
    Fill {
        pts: Vec<DVec2>,
        color: [u8; 4],
        element: Option<ElementId>,
        /// A cut region (plans, sections), not a surface seen beyond the cut.
        cut: bool,
    },
    /// Text placed by `anchor` at `pos`, rotated by `angle` radians
    /// (counter-clockwise); `size` is paper height in millimeters.
    Text {
        pos: DVec2,
        text: String,
        size: f64,
        anchor: Anchor,
        angle: f64,
        /// The element the text annotates (hidden with it).
        element: Option<ElementId>,
    },
}

impl Prim {
    /// The element a primitive belongs to, if any.
    pub fn element(&self) -> Option<ElementId> {
        match self {
            Prim::Line { element, .. } | Prim::Fill { element, .. } | Prim::Text { element, .. } => *element,
        }
    }
}

/// Ordered list of primitives, painted first to last.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawing2D {
    pub prims: Vec<Prim>,
}

impl Drawing2D {
    pub fn line(&mut self, a: DVec2, b: DVec2, style: LineStyle, element: Option<ElementId>) {
        if a.distance_squared(b) > 1e-14 {
            self.prims.push(Prim::Line { a, b, style, element, pen: None });
        }
    }

    pub fn polyline(&mut self, pts: &[DVec2], closed: bool, style: LineStyle, element: Option<ElementId>) {
        for w in pts.windows(2) {
            self.line(w[0], w[1], style, element);
        }
        if closed && pts.len() > 2 {
            self.line(pts[pts.len() - 1], pts[0], style, element);
        }
    }

    pub fn fill(&mut self, pts: Vec<DVec2>, color: [u8; 4], element: Option<ElementId>) {
        if pts.len() >= 3 {
            self.prims.push(Prim::Fill { pts, color, element, cut: false });
        }
    }

    /// A fill of a cut region.
    pub fn cut_fill(&mut self, pts: Vec<DVec2>, color: [u8; 4], element: Option<ElementId>) {
        if pts.len() >= 3 {
            self.prims.push(Prim::Fill { pts, color, element, cut: true });
        }
    }

    pub fn text(&mut self, pos: DVec2, text: impl Into<String>, size: f64) {
        self.text_at(pos, text, size, Anchor::LeftBottom, 0.0);
    }

    pub fn text_at(&mut self, pos: DVec2, text: impl Into<String>, size: f64, anchor: Anchor, angle: f64) {
        self.prims.push(Prim::Text { pos, text: text.into(), size, anchor, angle, element: None });
    }

    /// Text that annotates `element` (a tag, a dimension value…).
    pub fn tag_text(
        &mut self,
        pos: DVec2,
        text: impl Into<String>,
        size: f64,
        anchor: Anchor,
        angle: f64,
        element: Option<ElementId>,
    ) {
        self.prims.push(Prim::Text { pos, text: text.into(), size, anchor, angle, element });
    }

    /// Circular arc approximated with line segments.
    pub fn arc(&mut self, center: DVec2, radius: f64, a0: f64, a1: f64, style: LineStyle, element: Option<ElementId>) {
        let steps = 16;
        let pts: Vec<DVec2> = (0..=steps)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f64 / steps as f64;
                center + DVec2::new(a.cos(), a.sin()) * radius
            })
            .collect();
        self.polyline(&pts, false, style, element);
    }

    /// Bounding box of geometry (text anchors included).
    pub fn bounds(&self) -> Option<(DVec2, DVec2)> {
        let mut it = self.prims.iter().flat_map(|p| match p {
            Prim::Line { a, b, .. } => vec![*a, *b],
            Prim::Fill { pts, .. } => pts.clone(),
            Prim::Text { pos, .. } => vec![*pos],
        });
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

    /// Topmost element under `p` within `tol` (view units).
    pub fn hit_test(&self, p: DVec2, tol: f64) -> Option<ElementId> {
        // Lines first: they are thin and drawn over fills.
        let mut best: Option<(f64, ElementId)> = None;
        for prim in &self.prims {
            if let Prim::Line { a, b, element: Some(id), .. } = prim {
                let (q, _) = cc_geom::polygon::closest_on_segment(*a, *b, p);
                let d = q.distance(p);
                if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, *id));
                }
            }
        }
        if let Some((_, id)) = best {
            return Some(id);
        }
        self.prims.iter().rev().find_map(|prim| match prim {
            // Fully transparent fills only show selection (room areas); they are not picked.
            Prim::Fill { pts, element: Some(id), color, .. } if color[3] > 0 && cc_geom::polygon::contains(pts, p) => {
                Some(*id)
            }
            _ => None,
        })
    }

    /// Elements with any geometry inside the rectangle `lo`..`hi`.
    pub fn elements_in(&self, lo: DVec2, hi: DVec2, fully: bool) -> Vec<ElementId> {
        use std::collections::BTreeMap;
        let inside = |p: &DVec2| p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y;
        // Track (any point inside, all points inside) per element.
        let mut state: BTreeMap<ElementId, (bool, bool)> = BTreeMap::new();
        for prim in &self.prims {
            let (pts, id): (Vec<DVec2>, _) = match prim {
                Prim::Line { a, b, element: Some(id), .. } => (vec![*a, *b], *id),
                Prim::Fill { pts, element: Some(id), .. } => (pts.clone(), *id),
                _ => continue,
            };
            let s = state.entry(id).or_insert((false, true));
            s.0 |= pts.iter().any(inside);
            s.1 &= pts.iter().all(inside);
        }
        state.into_iter().filter(|(_, (any, all))| if fully { *all } else { *any }).map(|(id, _)| id).collect()
    }
}
