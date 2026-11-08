//! Detail lines in the view they belong to: each curve as short segments
//! in its style's own pen (width, black, dash pattern), on top of the
//! model's linework. View graphics override them like any line.

use crate::drawing::{Drawing2D, LineStyle, Pen, Prim};
use cc_core::detail::DetailLine;
use cc_core::{ElementId, ElementKind, Project, ViewId};

/// The pen of a detail line's style.
pub fn pen(line: &DetailLine) -> Pen {
    let (weight, color, pattern) = line.style.pen();
    Pen { color: Some(color), weight: Some(weight), pattern: Some(pattern), halftone: false }
}

/// Draw one detail line.
pub fn draw(d: &mut Drawing2D, line: &DetailLine, id: Option<ElementId>) {
    let pen = Some(pen(line));
    for w in line.curve.points().windows(2) {
        d.prims.push(Prim::Line { a: w[0], b: w[1], style: LineStyle::Projection, element: id, pen });
    }
}

/// The detail lines of `view`.
pub fn lines(d: &mut Drawing2D, project: &Project, view: ViewId) {
    for e in project.elements.values() {
        if let ElementKind::DetailLine(l) = &e.kind
            && l.view == view
            && l.is_valid()
        {
            draw(d, l, Some(e.id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::sketch::SketchCurve;
    use glam::DVec2;

    #[test]
    fn arcs_become_segments_in_the_style_pen() {
        let line = DetailLine {
            view: ViewId(1),
            curve: SketchCurve::Circle { center: DVec2::ZERO, radius: 1.0 },
            style: cc_core::LineStyleRef::BuiltIn(cc_core::DetailStyle::Hidden),
        };
        let mut d = Drawing2D::default();
        draw(&mut d, &line, Some(ElementId(7)));
        assert!(d.prims.len() > 60);
        let Prim::Line { pen: Some(pen), element, .. } = &d.prims[0] else { panic!("a line") };
        assert_eq!(pen.pattern, Some(cc_core::LinePattern::ShortDash));
        assert_eq!(*element, Some(ElementId(7)));
    }
}
