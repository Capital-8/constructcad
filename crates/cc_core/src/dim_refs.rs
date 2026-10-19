//! Associative dimensions: each end of a dimension may reference a point
//! of an element, stored in that element's own frame. Every edit ends by
//! recomputing referenced ends ([`crate::EditCtx`]), inside the same
//! transaction, so dimensions follow moved, rotated or stretched elements
//! and undo restores both together.

use crate::{ElementId, ElementKind, Project};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A point of an element that a dimension end follows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimRef {
    pub element: ElementId,
    pub anchor: RefAnchor,
}

/// Where the point sits in the element's frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RefAnchor {
    /// On a wall, beam or grid: `along` meters from its start (or from its
    /// end when `from_end`, measured back toward the start), `across`
    /// meters to the left of its line.
    Line { from_end: bool, along: f64, across: f64 },
    /// In the frame of a column or component (its position and rotation).
    Local { x: f64, y: f64 },
    /// On a door or window: `along` its wall from the opening center,
    /// `across` to the left of the wall's line.
    Hosted { along: f64, across: f64 },
}

impl DimRef {
    pub(crate) fn anchor_is_finite(&self) -> bool {
        self.anchor.is_finite()
    }
}

impl RefAnchor {
    fn is_finite(&self) -> bool {
        match *self {
            RefAnchor::Line { along, across, .. } | RefAnchor::Hosted { along, across } => {
                along.is_finite() && across.is_finite()
            }
            RefAnchor::Local { x, y } => x.is_finite() && y.is_finite(),
        }
    }
}

/// Unit direction and its left normal of the segment `a`-`b`.
fn frame(a: DVec2, b: DVec2) -> Option<(DVec2, DVec2)> {
    let u = (b - a).try_normalize()?;
    Some((u, u.perp()))
}

impl Project {
    /// The reference of point `p` (plan coordinates) on element `id`, when
    /// the element kind can carry one.
    pub fn dim_ref_at(&self, id: ElementId, p: DVec2) -> Option<DimRef> {
        let e = self.element(id)?;
        let anchor = match &e.kind {
            ElementKind::Wall(_) | ElementKind::Beam(_) | ElementKind::Grid(_) => {
                let (a, b) = crate::wall_ops::line_of(e)?;
                let (u, n) = frame(a, b)?;
                let from_end = p.distance(b) < p.distance(a);
                let along = if from_end { (p - b).dot(-u) } else { (p - a).dot(u) };
                RefAnchor::Line { from_end, along, across: (p - a).dot(n) }
            }
            ElementKind::Column(c) => local(c.position, c.rotation_deg, p),
            ElementKind::Component(c) => local(c.position, c.rotation_deg, p),
            ElementKind::Door(h) | ElementKind::Window(h) => {
                let w = self.element(h.host)?.as_wall()?;
                let (u, n) = frame(w.start, w.end)?;
                let center = w.start + u * h.offset;
                RefAnchor::Hosted { along: (p - center).dot(u), across: (p - center).dot(n) }
            }
            _ => return None,
        };
        anchor.is_finite().then_some(DimRef { element: id, anchor })
    }

    /// The current plan point of a reference (`None` when the element is
    /// gone or no longer fits the anchor).
    pub fn resolve_dim_ref(&self, r: &DimRef) -> Option<DVec2> {
        let e = self.element(r.element)?;
        let p = match (r.anchor, &e.kind) {
            (RefAnchor::Line { from_end, along, across }, _) => {
                let (a, b) = crate::wall_ops::line_of(e)?;
                let (u, n) = frame(a, b)?;
                let base = if from_end { b - u * along } else { a + u * along };
                base + n * across
            }
            (RefAnchor::Local { x, y }, ElementKind::Column(c)) => from_local(c.position, c.rotation_deg, x, y),
            (RefAnchor::Local { x, y }, ElementKind::Component(c)) => from_local(c.position, c.rotation_deg, x, y),
            (RefAnchor::Hosted { along, across }, ElementKind::Door(h) | ElementKind::Window(h)) => {
                let w = self.element(h.host)?.as_wall()?;
                let (u, n) = frame(w.start, w.end)?;
                w.start + u * (h.offset + along) + n * across
            }
            _ => return None,
        };
        p.is_finite().then_some(p)
    }
}

fn local(position: DVec2, rotation_deg: f64, p: DVec2) -> RefAnchor {
    let d = DVec2::from_angle(-rotation_deg.to_radians()).rotate(p - position);
    RefAnchor::Local { x: d.x, y: d.y }
}

fn from_local(position: DVec2, rotation_deg: f64, x: f64, y: f64) -> DVec2 {
    position + DVec2::from_angle(rotation_deg.to_radians()).rotate(DVec2::new(x, y))
}

impl crate::EditCtx<'_> {
    /// Move the referenced ends of every dimension to their elements'
    /// current points; references to deleted elements are dropped (the end
    /// stays where it was). Only changed dimensions are touched.
    pub(crate) fn sync_dimensions(&mut self) {
        let updates: Vec<(ElementId, [Option<DimRef>; 2], DVec2, DVec2)> = self
            .project()
            .elements
            .values()
            .filter_map(|e| {
                let ElementKind::Dimension(d) = &e.kind else { return None };
                if d.refs.iter().all(Option::is_none) {
                    return None;
                }
                let mut refs = d.refs;
                let mut ends = [d.a, d.b];
                for (r, end) in refs.iter_mut().zip(ends.iter_mut()) {
                    let Some(reference) = r else { continue };
                    match self.project().resolve_dim_ref(reference) {
                        Some(p) => *end = p,
                        None => *r = None,
                    }
                }
                let changed = refs != d.refs || ends[0].distance(d.a) > 1e-12 || ends[1].distance(d.b) > 1e-12;
                changed.then_some((e.id, refs, ends[0], ends[1]))
            })
            .collect();
        for (id, refs, a, b) in updates {
            if let Some(ElementKind::Dimension(d)) = self.element_mut(id).map(|e| &mut e.kind) {
                // A linear dimension keeps its line where it was placed.
                if let Some(dir) = d.direction() {
                    d.offset += (d.a - a).dot(dir.perp());
                }
                d.refs = refs;
                d.a = a;
                d.b = b;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Dimension, DimensionKind, Document, ElementKind, PlanTransform};
    use glam::DVec2;

    fn sample() -> Document {
        Document::new(crate::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap())
    }

    /// A dimension from the inside face of one wall to a free point.
    fn dimension_to_wall(doc: &mut Document) -> (crate::ElementId, crate::ElementId) {
        let (wall, w) =
            doc.project.walls().map(|(e, w)| (e.id, w.clone())).find(|(_, w)| w.start.x == w.end.x).unwrap();
        let p = (w.start + w.end) / 2.0 + DVec2::new(0.075, 0.0);
        let r = doc.project.dim_ref_at(wall, p).unwrap();
        let view = *doc.project.views.keys().next().unwrap();
        let level = doc.project.levels[0].id;
        let ty = doc.project.default_type(crate::Category::Dimension).unwrap();
        let dim = Dimension {
            view,
            a: p,
            b: p + DVec2::new(3.0, 0.0),
            offset: 1.0,
            kind: DimensionKind::Linear { dir: DVec2::X },
            refs: [Some(r), None],
        };
        let id = doc.edit("dim", |e| {
            let el = crate::Element {
                id: crate::ElementId(0),
                level,
                type_id: ty,
                mark: String::new(),
                comments: String::new(),
                kind: ElementKind::Dimension(dim),
                uid: Default::default(),
            };
            e.add_element(el)
        });
        (wall, id)
    }

    fn dim(doc: &Document, id: crate::ElementId) -> Dimension {
        match &doc.project.element(id).unwrap().kind {
            ElementKind::Dimension(d) => d.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn dimensions_follow_moved_walls_and_undo_with_them() {
        let mut doc = sample();
        let (wall, id) = dimension_to_wall(&mut doc);
        let before = dim(&doc, id);
        assert!((before.value() - 3.0).abs() < 1e-9);
        doc.edit("move", |e| e.transform_elements(&[wall], &PlanTransform::translate(DVec2::new(0.5, 0.0))));
        let after = dim(&doc, id);
        assert!((after.a - before.a - DVec2::new(0.5, 0.0)).length() < 1e-9);
        assert!((after.value() - 2.5).abs() < 1e-9);
        // The dimension line stays where it was placed.
        let line = |d: &Dimension| d.a.y + d.offset;
        assert!((line(&after) - line(&before)).abs() < 1e-9);
        doc.undo();
        assert_eq!(dim(&doc, id), before);
    }

    #[test]
    fn deleting_the_element_frees_the_dimension_end() {
        let mut doc = sample();
        let (wall, id) = dimension_to_wall(&mut doc);
        let before = dim(&doc, id);
        doc.edit("delete", |e| e.remove_element(wall));
        let after = dim(&doc, id);
        assert_eq!(after.refs, [None, None]);
        assert_eq!((after.a, after.b), (before.a, before.b));
    }

    #[test]
    fn references_follow_rotated_columns_and_stretched_walls() {
        let doc = sample();
        let p = &doc.project;
        let (wall, w) = p.walls().map(|(e, w)| (e.id, w.clone())).next().unwrap();
        // A point near the end stays at the same distance from that end when the wall stretches.
        let near_end = w.end + (w.start - w.end).normalize() * 0.2;
        let r = p.dim_ref_at(wall, near_end).unwrap();
        let mut stretched = p.clone();
        if let Some(ElementKind::Wall(x)) = stretched.elements.get_mut(&wall).map(|e| &mut e.kind) {
            x.start -= (x.end - x.start).normalize();
        }
        assert!(stretched.resolve_dim_ref(&r).unwrap().distance(near_end) < 1e-9);
    }
}
