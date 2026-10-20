//! Geometry edits of linear elements (walls, beams, grids): trim or extend
//! to a corner or to a reference line, split, offset and align.
//!
//! Picks are plan points on the element: the picked part is the part kept.

use crate::{EditCtx, Element, ElementId, ElementKind, Project};
use glam::DVec2;

/// Location line of a linear element, if it has one.
pub fn line_of(e: &Element) -> Option<(DVec2, DVec2)> {
    match &e.kind {
        ElementKind::Wall(w) => Some((w.start, w.end)),
        ElementKind::Beam(b) => Some((b.start, b.end)),
        ElementKind::Grid(g) => Some((g.start, g.end)),
        _ => None,
    }
}

fn set_line(e: &mut Element, start: DVec2, end: DVec2) {
    match &mut e.kind {
        ElementKind::Wall(w) => (w.start, w.end) = (start, end),
        ElementKind::Beam(b) => (b.start, b.end) = (start, end),
        ElementKind::Grid(g) => (g.start, g.end) = (start, end),
        _ => {}
    }
}

/// Parameter on `a`-`b` where the infinite lines `a`-`b` and `c`-`d` cross.
fn crossing(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<f64> {
    let (u, v) = (b - a, d - c);
    let den = u.perp_dot(v);
    if den.abs() < 1e-9 * u.length() * v.length() {
        return None;
    }
    Some((c - a).perp_dot(v) / den)
}

/// The line `a`-`b` cut or extended to the line `c`-`d`, keeping the side of `pick`.
fn trimmed(a: DVec2, b: DVec2, pick: DVec2, c: DVec2, d: DVec2) -> Option<(DVec2, DVec2)> {
    let t = crossing(a, b, c, d)?;
    let x = a + (b - a) * t;
    let tp = (pick - a).dot(b - a) / (b - a).length_squared().max(1e-12);
    let (s, e) = if tp < t { (a, x) } else { (x, b) };
    (s.distance(e) > 1e-4).then_some((s, e))
}

/// Ends of other walls on the same level that meet `point` (within 1 cm),
/// as `(wall, is_start)`.
pub fn connected_ends(project: &Project, id: ElementId, point: DVec2) -> Vec<(ElementId, bool)> {
    let Some(me) = project.element(id).filter(|e| e.as_wall().is_some()) else { return Vec::new() };
    project
        .walls()
        .filter(|(e, _)| e.id != id && e.level == me.level)
        .filter_map(|(e, w)| {
            if w.start.distance(point) < 0.01 {
                Some((e.id, true))
            } else if w.end.distance(point) < 0.01 {
                Some((e.id, false))
            } else {
                None
            }
        })
        .collect()
}

/// Why an edit could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    /// The element has no location line (or does not exist).
    NotLinear,
    /// The lines are parallel.
    Parallel,
    /// The result would have no length.
    Degenerate,
}

fn line(project: &Project, id: ElementId) -> Result<(DVec2, DVec2), EditError> {
    project.element(id).and_then(line_of).ok_or(EditError::NotLinear)
}

/// Hosted openings beyond the new length move inward; offsets follow a moved start.
fn keep_openings(ctx: &mut EditCtx, wall: ElementId, old_start: DVec2, new_start: DVec2, dir: DVec2, length: f64) {
    let shift = (new_start - old_start).dot(dir);
    let hosted: Vec<ElementId> = ctx.project().hosted_by(wall).map(|h| h.id).collect();
    for h in hosted {
        if let Some(x) = ctx.element_mut(h).and_then(|e| e.as_hosted_mut()) {
            x.offset = (x.offset - shift).clamp(0.0, length.max(0.0));
        }
    }
}

impl EditCtx<'_> {
    fn apply_line(&mut self, id: ElementId, (s, e): (DVec2, DVec2)) {
        let old = self.project().element(id).and_then(line_of);
        if let Some(el) = self.element_mut(id) {
            set_line(el, s, e);
        }
        if let Some((old_s, _)) = old {
            let dir = (e - s).normalize_or_zero();
            keep_openings(self, id, old_s, s, dir, s.distance(e));
        }
    }

    /// Trim or extend two elements so they meet at a corner, keeping the
    /// picked parts.
    pub fn trim_corner(&mut self, a: ElementId, pick_a: DVec2, b: ElementId, pick_b: DVec2) -> Result<(), EditError> {
        let (la, lb) = (line(self.project(), a)?, line(self.project(), b)?);
        crossing(la.0, la.1, lb.0, lb.1).ok_or(EditError::Parallel)?;
        let na = trimmed(la.0, la.1, pick_a, lb.0, lb.1).ok_or(EditError::Degenerate)?;
        let nb = trimmed(lb.0, lb.1, pick_b, la.0, la.1).ok_or(EditError::Degenerate)?;
        self.apply_line(a, na);
        self.apply_line(b, nb);
        Ok(())
    }

    /// Trim or extend `id` to the reference line `r0`-`r1`, keeping the picked part.
    pub fn trim_to(&mut self, id: ElementId, pick: DVec2, r0: DVec2, r1: DVec2) -> Result<(), EditError> {
        let l = line(self.project(), id)?;
        crossing(l.0, l.1, r0, r1).ok_or(EditError::Parallel)?;
        let n = trimmed(l.0, l.1, pick, r0, r1).ok_or(EditError::Degenerate)?;
        self.apply_line(id, n);
        Ok(())
    }

    /// Split a wall, beam or grid at the projection of `at`; returns the
    /// new element (the part after the split). Openings past the split go
    /// to the new wall.
    pub fn split(&mut self, id: ElementId, at: DVec2) -> Result<ElementId, EditError> {
        let (a, b) = line(self.project(), id)?;
        let dir = (b - a).normalize_or_zero();
        let len = a.distance(b);
        let t = (at - a).dot(dir);
        if t < 1e-3 || t > len - 1e-3 {
            return Err(EditError::Degenerate);
        }
        let p = a + dir * t;
        let Some(original) = self.project().element(id).cloned() else { return Err(EditError::NotLinear) };
        let mut second = original.clone();
        second.id = ElementId(self.alloc_id());
        second.uid = Default::default();
        if let ElementKind::Grid(g) = &mut second.kind {
            g.name = format!("{}'", g.name);
        }
        set_line(&mut second, p, b);
        // A sloped beam keeps its slope: the split point's elevation joins the parts.
        let split_z = match &original.kind {
            ElementKind::Beam(x) if x.end_offset.is_some() => Some(x.top_offset + x.rise() * t / len),
            _ => None,
        };
        if let (Some(z), ElementKind::Beam(x)) = (split_z, &mut second.kind) {
            x.top_offset = z;
        }
        if let Some(el) = self.element_mut(id) {
            set_line(el, a, p);
            if let (Some(z), ElementKind::Beam(x)) = (split_z, &mut el.kind) {
                x.end_offset = Some(z);
            }
        }
        let new = self.add_element(second);
        let hosted: Vec<(ElementId, f64)> =
            self.project().hosted_by(id).filter_map(|h| h.as_hosted().map(|x| (h.id, x.offset))).collect();
        for (h, offset) in hosted {
            if offset > t
                && let Some(x) = self.element_mut(h).and_then(|e| e.as_hosted_mut())
            {
                x.host = new;
                x.offset = offset - t;
            }
        }
        Ok(new)
    }

    /// Move one end of a wall, beam or grid to `to` (lengthening or
    /// shortening it). Walls whose ends met that end move with it, so
    /// corners stay closed; openings keep their place.
    pub fn move_endpoint(&mut self, id: ElementId, at_start: bool, to: DVec2) -> Result<(), EditError> {
        let (a, b) = line(self.project(), id)?;
        let from = if at_start { a } else { b };
        let new = if at_start { (to, b) } else { (a, to) };
        if new.0.distance(new.1) < 0.01 {
            return Err(EditError::Degenerate);
        }
        let connected = connected_ends(self.project(), id, from);
        self.apply_line(id, new);
        for (other, other_start) in connected {
            let Ok((s, e)) = line(self.project(), other) else { continue };
            let moved = if other_start { (to, e) } else { (s, to) };
            if moved.0.distance(moved.1) >= 0.01 {
                self.apply_line(other, moved);
            }
        }
        Ok(())
    }

    /// A copy of `id` moved `distance` perpendicular to its line (positive
    /// to the left of start→end). Openings are not copied.
    pub fn offset_copy(&mut self, id: ElementId, distance: f64) -> Result<ElementId, EditError> {
        let (a, b) = line(self.project(), id)?;
        let n = (b - a).normalize_or_zero().perp();
        if n == DVec2::ZERO || distance.abs() < 1e-6 {
            return Err(EditError::Degenerate);
        }
        let Some(mut copy) = self.project().element(id).cloned() else { return Err(EditError::NotLinear) };
        copy.id = ElementId(self.alloc_id());
        copy.uid = Default::default();
        copy.translate(n * distance);
        Ok(self.add_element(copy))
    }

    /// Move `id` (with its openings) so its line lies on the parallel
    /// reference line `r0`-`r1`.
    pub fn align_to(&mut self, id: ElementId, r0: DVec2, r1: DVec2) -> Result<(), EditError> {
        let (a, b) = line(self.project(), id)?;
        let (u, v) = ((b - a).normalize_or_zero(), (r1 - r0).normalize_or_zero());
        if u.perp_dot(v).abs() > 0.02 {
            return Err(EditError::Parallel);
        }
        let n = v.perp();
        let shift = n * (r0 - a).dot(n);
        self.transform_elements(&[id], &crate::PlanTransform::translate(shift));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Category, Document, Hosted, Wall};

    fn doc_with_walls(lines: &[(DVec2, DVec2)]) -> (Document, Vec<ElementId>) {
        let mut doc = Document::new(Project::from_template());
        let level = doc.project.levels[0].id;
        let ty = doc.project.default_type(Category::Wall).unwrap();
        let ids = doc.edit("walls", |e| {
            lines
                .iter()
                .map(|(s, t)| {
                    let id = ElementId(e.alloc_id());
                    let kind = ElementKind::Wall(Wall { start: *s, end: *t, ..Default::default() });
                    e.add_element(Element {
                        id,
                        level,
                        type_id: ty,
                        mark: String::new(),
                        comments: String::new(),
                        kind,
                        uid: Default::default(),
                    })
                })
                .collect()
        });
        (doc, ids)
    }

    fn wall(doc: &Document, id: ElementId) -> (DVec2, DVec2) {
        doc.project.element(id).and_then(line_of).unwrap()
    }

    fn v(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    #[test]
    fn corner_extends_and_trims_keeping_the_picked_parts() {
        // A horizontal wall short of a vertical one that overshoots.
        let (mut doc, ids) = doc_with_walls(&[(v(0.0, 0.0), v(4.0, 0.0)), (v(5.0, -2.0), v(5.0, 3.0))]);
        doc.edit("corner", |e| e.trim_corner(ids[0], v(1.0, 0.0), ids[1], v(5.0, 2.0))).unwrap();
        assert_eq!(wall(&doc, ids[0]), (v(0.0, 0.0), v(5.0, 0.0)));
        assert_eq!(wall(&doc, ids[1]), (v(5.0, 0.0), v(5.0, 3.0)));
        // Parallel walls cannot make a corner.
        let (mut doc, ids) = doc_with_walls(&[(v(0.0, 0.0), v(4.0, 0.0)), (v(0.0, 1.0), v(4.0, 1.0))]);
        assert_eq!(
            doc.edit("x", |e| e.trim_corner(ids[0], v(1.0, 0.0), ids[1], v(1.0, 1.0))),
            Err(EditError::Parallel)
        );
    }

    #[test]
    fn trim_to_reference_and_split_keep_openings() {
        let (mut doc, ids) = doc_with_walls(&[(v(0.0, 0.0), v(10.0, 0.0))]);
        let level = doc.project.levels[0].id;
        let door_t = doc.project.default_type(Category::Door).unwrap();
        let door = doc.edit("door", |e| {
            let id = ElementId(e.alloc_id());
            let hosted = Hosted { host: ids[0], offset: 8.0, sill_height: 0.0, flip_facing: false, flip_hand: false };
            e.add_element(Element {
                id,
                level,
                type_id: door_t,
                mark: "1".into(),
                comments: String::new(),
                kind: ElementKind::Door(hosted),
                uid: Default::default(),
            })
        });
        // Split at x = 6: the door (at 8) moves to the second wall at 2.
        let second = doc.edit("split", |e| e.split(ids[0], v(6.0, 0.3))).unwrap();
        assert_eq!(wall(&doc, ids[0]), (v(0.0, 0.0), v(6.0, 0.0)));
        assert_eq!(wall(&doc, second), (v(6.0, 0.0), v(10.0, 0.0)));
        let h = doc.project.element(door).and_then(|e| e.as_hosted()).cloned().unwrap();
        assert_eq!((h.host, h.offset), (second, 2.0));
        // Trim the second wall's start back to x = 7 (keeping the end): the door stays put.
        doc.edit("trim", |e| e.trim_to(second, v(9.0, 0.0), v(7.0, -1.0), v(7.0, 1.0))).unwrap();
        assert_eq!(wall(&doc, second), (v(7.0, 0.0), v(10.0, 0.0)));
        let h = doc.project.element(door).and_then(|e| e.as_hosted()).cloned().unwrap();
        assert!((h.offset - 1.0).abs() < 1e-12);
        assert_eq!(doc.edit("split", |e| e.split(second, v(7.0, 0.0))), Err(EditError::Degenerate));
        assert_eq!(doc.project.repair(), 0);
    }

    #[test]
    fn offset_and_align() {
        let (mut doc, ids) = doc_with_walls(&[(v(0.0, 0.0), v(4.0, 0.0)), (v(0.0, 3.0), v(4.0, 3.2))]);
        let copy = doc.edit("offset", |e| e.offset_copy(ids[0], 1.5)).unwrap();
        assert_eq!(wall(&doc, copy), (v(0.0, 1.5), v(4.0, 1.5)));
        doc.edit("align", |e| e.align_to(copy, v(-1.0, 2.0), v(9.0, 2.0))).unwrap();
        assert_eq!(wall(&doc, copy), (v(0.0, 2.0), v(4.0, 2.0)));
        // Not parallel enough.
        assert_eq!(doc.edit("align", |e| e.align_to(ids[1], v(0.0, 0.0), v(0.0, 5.0))), Err(EditError::Parallel));
    }

    #[test]
    fn dragging_an_end_keeps_the_corner_closed() {
        let (mut doc, ids) =
            doc_with_walls(&[(v(0.0, 0.0), v(4.0, 0.0)), (v(4.0, 0.0), v(4.0, 3.0)), (v(9.0, 9.0), v(9.0, 12.0))]);
        doc.edit("grip", |e| e.move_endpoint(ids[0], false, v(5.0, 0.0))).unwrap();
        assert_eq!(wall(&doc, ids[0]), (v(0.0, 0.0), v(5.0, 0.0)));
        // The wall that met that end follows; unrelated walls do not.
        assert_eq!(wall(&doc, ids[1]), (v(5.0, 0.0), v(4.0, 3.0)));
        assert_eq!(wall(&doc, ids[2]), (v(9.0, 9.0), v(9.0, 12.0)));
        assert_eq!(doc.edit("grip", |e| e.move_endpoint(ids[2], true, v(9.0, 12.0))), Err(EditError::Degenerate));
    }
}
