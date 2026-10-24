//! View graphics applied to drawings: hidden categories, filters and
//! elements leave the drawing, and the resolved overrides change the pens
//! of lines and the colors and patterns of fills (see [`cc_core::graphics`]).

use crate::{Drawing2D, LineStyle, Pen, Prim, format};
use cc_core::{
    Element, ElementId, FilterParam, GraphicOverride, LineGraphics, Pattern, Project, TemporaryVisibility, View,
    ViewFilter, ViewGraphics,
};
use cc_geom::ModelGeometry;
use std::collections::HashMap;

/// Whether `filter` matches the element `e`.
pub fn filter_matches(project: &Project, geom: &ModelGeometry, filter: &ViewFilter, e: &Element) -> bool {
    filter.matches(e.category(), &mut |rule| {
        let actual = match rule.param {
            FilterParam::Field(field) => crate::field_value(project, geom, e, field),
            FilterParam::Elevation => {
                project.level(e.level).map(|l| (format::number(l.elevation, 3), Some(l.elevation)))
            }
        };
        rule.op.test(actual.as_ref().map(|(t, n)| (t.as_str(), *n)), &rule.value)
    })
}

/// The elements `filter` matches, in id order.
pub fn filter_elements(project: &Project, geom: &ModelGeometry, filter: &ViewFilter) -> Vec<ElementId> {
    project.elements.values().filter(|e| filter_matches(project, geom, filter, e)).map(|e| e.id).collect()
}

/// Resolves the graphics of elements in one view (cached per element).
pub struct Resolver<'a> {
    project: &'a Project,
    geom: &'a ModelGeometry,
    graphics: &'a ViewGraphics,
    temporary: Option<&'a TemporaryVisibility>,
    cache: HashMap<ElementId, Option<GraphicOverride>>,
}

impl<'a> Resolver<'a> {
    pub fn new(
        project: &'a Project,
        geom: &'a ModelGeometry,
        view: &'a View,
        temporary: Option<&'a TemporaryVisibility>,
    ) -> Self {
        Self { project, geom, graphics: &view.graphics, temporary, cache: HashMap::new() }
    }

    /// Whether the view draws everything as generated.
    pub fn is_trivial(&self) -> bool {
        self.graphics.is_empty() && self.temporary.is_none_or(|t| t.is_empty())
    }

    /// The overrides of an element, or `None` when it is hidden. Ids that
    /// are not elements are drawn as they are.
    pub fn style(&mut self, id: ElementId) -> Option<GraphicOverride> {
        if let Some(s) = self.cache.get(&id) {
            return *s;
        }
        let s = match self.project.element(id) {
            None => Some(GraphicOverride::default()),
            Some(e) if self.temporary.is_some_and(|t| !t.shows(id, e.category())) => None,
            Some(e) => {
                let (project, geom) = (self.project, self.geom);
                self.graphics.resolve(id, e.category(), &mut |f| {
                    project.filter(f).is_some_and(|filter| filter_matches(project, geom, filter, e))
                })
            }
        };
        self.cache.insert(id, s);
        s
    }
}

fn pen(lines: &LineGraphics, halftone: bool) -> Option<Pen> {
    (!lines.is_empty() || halftone).then_some(Pen {
        color: lines.color,
        weight: lines.weight,
        pattern: lines.pattern,
        halftone,
    })
}

fn rgb(c: [u8; 4]) -> [u8; 3] {
    [c[0], c[1], c[2]]
}

fn rgba(c: [u8; 3], a: u8) -> [u8; 4] {
    [c[0], c[1], c[2], a]
}

/// Apply the view's graphics to a generated drawing at `1:scale`.
pub fn apply(d: &mut Drawing2D, resolver: &mut Resolver, scale: u32) {
    if resolver.is_trivial() {
        return;
    }
    let spacing = crate::cut::spacing(scale);
    let prims = std::mem::take(&mut d.prims);
    for prim in prims {
        let Some(id) = prim.element() else {
            d.prims.push(prim);
            continue;
        };
        let Some(g) = resolver.style(id) else { continue };
        if g.is_empty() {
            d.prims.push(prim);
            continue;
        }
        let halftone = g.halftone == Some(true);
        match prim {
            Prim::Line { a, b, style, element, pen: _ } => {
                let lines = match style {
                    LineStyle::Hatch if g.cut_fill.pattern.is_some() => continue,
                    LineStyle::Hatch => LineGraphics { color: g.cut_fill.color, ..Default::default() },
                    LineStyle::Cut => g.cut_lines,
                    _ => g.projection_lines,
                };
                d.prims.push(Prim::Line { a, b, style, element, pen: pen(&lines, halftone) });
            }
            Prim::Fill { pts, color, element, cut } => {
                // Invisible fills (pick areas, highlight-only regions) stay as they are.
                if color[3] <= 1 {
                    d.prims.push(Prim::Fill { pts, color, element, cut });
                    continue;
                }
                let fill = if cut { g.cut_fill } else { g.surface_fill };
                let mut alpha = color[3];
                if !cut && let Some(t) = g.transparency {
                    alpha = ((1.0 - t.clamp(0.0, 1.0)) * 255.0).round().clamp(1.0, 255.0) as u8;
                }
                let white = [255, 255, 255];
                let mut hatch = None;
                let mut c = match fill.pattern {
                    None => match fill.color {
                        // A white background (under pattern lines) stays white.
                        Some(over) if rgb(color) != white => over,
                        _ => rgb(color),
                    },
                    Some(Pattern::None) => white,
                    Some(Pattern::Solid) => fill.color.unwrap_or(rgb(color)),
                    Some(p) => {
                        hatch = Some(p);
                        white
                    }
                };
                if halftone {
                    c = crate::drawing::halftone(c);
                }
                let lines = hatch.map(|p| crate::hatch::hatch(p, &pts, spacing)).unwrap_or_default();
                d.prims.push(Prim::Fill { pts, color: rgba(c, alpha), element, cut });
                let hatch_pen = pen(&LineGraphics { color: fill.color, ..Default::default() }, halftone);
                for (a, b) in lines {
                    d.prims.push(Prim::Line { a, b, style: LineStyle::Hatch, element, pen: hatch_pen });
                }
            }
            text @ Prim::Text { .. } => d.prims.push(text),
        }
    }
}
