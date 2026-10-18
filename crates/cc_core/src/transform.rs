//! Rigid transforms in plan (move, rotate, mirror) applied to elements.

use crate::{Element, ElementId, ElementKind, RoofShape};
use glam::DVec2;

/// `p ↦ R·M·p + t`: an optional reflection across the X axis (`M`), a
/// rotation by `angle` radians (`R`) and a translation `t`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanTransform {
    pub angle: f64,
    pub mirror: bool,
    pub translation: DVec2,
}

impl Default for PlanTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl PlanTransform {
    pub const IDENTITY: Self = Self { angle: 0.0, mirror: false, translation: DVec2::ZERO };

    pub fn translate(delta: DVec2) -> Self {
        Self { translation: delta, ..Self::IDENTITY }
    }

    /// Rotation by `angle` radians about `center`.
    pub fn rotate_about(center: DVec2, angle: f64) -> Self {
        let r = DVec2::from_angle(angle);
        Self { angle, mirror: false, translation: center - r.rotate(center) }
    }

    /// Reflection across the line through `a` with direction `dir`.
    pub fn mirror_across(a: DVec2, dir: DVec2) -> Self {
        let d = dir.try_normalize().unwrap_or(DVec2::X);
        // Reflection across a line at angle φ through the origin is R(2φ)·M.
        let angle = 2.0 * d.y.atan2(d.x);
        let t = Self { angle, mirror: true, translation: DVec2::ZERO };
        Self { translation: a - t.apply_vector(a), ..t }
    }

    /// `self` after `first` (apply `first`, then `self`).
    pub fn then(self, next: Self) -> Self {
        // next(self(p)) = R2 M2 (R1 M1 p + t1) + t2.
        let m1 = if next.mirror { -self.angle } else { self.angle };
        Self {
            angle: next.angle + m1,
            mirror: self.mirror != next.mirror,
            translation: next.apply_vector(self.translation) + next.translation,
        }
    }

    pub fn apply_vector(&self, v: DVec2) -> DVec2 {
        let v = if self.mirror { DVec2::new(v.x, -v.y) } else { v };
        DVec2::from_angle(self.angle).rotate(v)
    }

    pub fn apply(&self, p: DVec2) -> DVec2 {
        self.apply_vector(p) + self.translation
    }

    /// A plan direction given in degrees, transformed.
    pub fn apply_angle_deg(&self, deg: f64) -> f64 {
        let d = if self.mirror { -deg } else { deg };
        (d + self.angle.to_degrees()).rem_euclid(360.0)
    }

    pub fn is_identity(&self) -> bool {
        !self.mirror && self.angle.abs() < 1e-12 && self.translation.length() < 1e-12
    }
}

impl Element {
    /// Move, rotate or mirror the element in plan. Hosted elements are
    /// placed along their wall and are handled with it (see
    /// [`crate::EditCtx::transform_elements`]); here only their hand flips
    /// under a mirror.
    pub fn transform(&mut self, t: &PlanTransform) {
        let p = |v: &mut DVec2| *v = t.apply(*v);
        match &mut self.kind {
            ElementKind::Wall(w) => {
                p(&mut w.start);
                p(&mut w.end);
                // A mirror would put the exterior (left) face inside: walk the wall the other way.
                if t.mirror {
                    std::mem::swap(&mut w.start, &mut w.end);
                    std::mem::swap(&mut w.join_start, &mut w.join_end);
                }
            }
            ElementKind::Door(h) | ElementKind::Window(h) => {
                if t.mirror {
                    h.flip_hand = !h.flip_hand;
                }
            }
            ElementKind::Floor(f) => f.boundary.iter_mut().for_each(p),
            ElementKind::Ceiling(c) => c.boundary.iter_mut().for_each(p),
            ElementKind::Roof(r) => {
                r.footprint.iter_mut().for_each(p);
                match &mut r.shape {
                    RoofShape::Flat | RoofShape::Hip { .. } => {}
                    RoofShape::Shed { direction_deg, .. } => *direction_deg = t.apply_angle_deg(*direction_deg),
                    RoofShape::Gable { ridge_deg, .. } => *ridge_deg = t.apply_angle_deg(*ridge_deg),
                }
            }
            ElementKind::Room(r) => p(&mut r.position),
            ElementKind::Grid(g) => {
                p(&mut g.start);
                p(&mut g.end);
            }
            ElementKind::Dimension(d) => {
                p(&mut d.a);
                p(&mut d.b);
                if t.mirror {
                    d.offset = -d.offset;
                }
            }
            ElementKind::Column(c) => {
                p(&mut c.position);
                c.rotation_deg = t.apply_angle_deg(c.rotation_deg);
            }
            ElementKind::Beam(b) => {
                p(&mut b.start);
                p(&mut b.end);
            }
            ElementKind::Stair(s) => {
                p(&mut s.start);
                s.direction_deg = t.apply_angle_deg(s.direction_deg);
            }
            ElementKind::Railing(r) => r.path.iter_mut().for_each(p),
            ElementKind::Component(c) => {
                p(&mut c.position);
                c.rotation_deg = t.apply_angle_deg(c.rotation_deg);
            }
        }
    }
}

impl crate::EditCtx<'_> {
    /// Transform `ids` in one step. Doors and windows of transformed walls
    /// go with them (their offset follows a mirrored wall's new direction);
    /// hosted elements whose wall is not transformed slide along it by the
    /// translation's component on the wall. Returns the ids changed.
    pub fn transform_elements(&mut self, ids: &[ElementId], t: &PlanTransform) -> Vec<ElementId> {
        let project = self.project();
        let set: std::collections::BTreeSet<ElementId> = ids.iter().copied().collect();
        let walls: Vec<ElementId> =
            ids.iter().copied().filter(|id| project.element(*id).is_some_and(|e| e.as_wall().is_some())).collect();
        // Hosted elements of moved walls, plus selected hosted elements on fixed walls.
        let mut hosted: Vec<(ElementId, bool)> = Vec::new();
        for e in project.elements.values() {
            let Some(h) = e.as_hosted() else { continue };
            if walls.contains(&h.host) {
                hosted.push((e.id, true));
            } else if set.contains(&e.id) {
                hosted.push((e.id, false));
            }
        }
        let lengths: Vec<(ElementId, f64, DVec2)> = walls
            .iter()
            .filter_map(|id| project.element(*id).and_then(|e| e.as_wall()).map(|w| (*id, w.length(), w.direction())))
            .collect();
        let mut changed = Vec::new();
        for id in ids {
            let Some(el) = self.element_mut(*id) else { continue };
            if el.as_hosted().is_some() {
                continue;
            }
            el.transform(t);
            changed.push(*id);
        }
        for (id, with_wall) in hosted {
            let Some(host) = self.project().element(id).and_then(|e| e.as_hosted()).map(|h| h.host) else { continue };
            let wall = lengths.iter().find(|(w, _, _)| *w == host).copied();
            let host_dir = self.project().element(host).and_then(|e| e.as_wall()).map(|w| w.direction());
            let Some(el) = self.element_mut(id) else { continue };
            if with_wall {
                el.transform(t);
                if let (Some((_, len, _)), Some(h)) = (wall, el.as_hosted_mut())
                    && t.mirror
                {
                    h.offset = len - h.offset;
                }
            } else if let (Some(dir), Some(h)) = (host_dir, el.as_hosted_mut()) {
                h.offset += t.translation.dot(dir);
            }
            changed.push(id);
        }
        changed
    }
}

impl crate::Project {
    /// The elements a copy of `ids` takes: the non-hosted ones, plus the
    /// doors and windows of copied walls and any selected hosted element.
    /// The snapshot stays valid after the originals are deleted (cut).
    pub fn copy_set(&self, ids: &[ElementId]) -> Vec<Element> {
        let set: std::collections::BTreeSet<ElementId> = ids.iter().copied().collect();
        let mut out: Vec<Element> =
            set.iter().filter_map(|id| self.element(*id)).filter(|e| e.as_hosted().is_none()).cloned().collect();
        out.extend(
            self.elements
                .values()
                .filter(|e| e.as_hosted().is_some_and(|h| set.contains(&h.host) || set.contains(&e.id)))
                .cloned(),
        );
        out
    }

    /// Whether a copied element still fits this project: its type, level,
    /// view (dimensions) and wall (hosted elements, when not copied along)
    /// may have been deleted since it was copied.
    fn accepts(&self, e: &Element, set: &[Element]) -> bool {
        let type_ok = self.element_type(e.type_id).is_some_and(|t| t.category() == e.category());
        let other_ok = match &e.kind {
            ElementKind::Door(h) | ElementKind::Window(h) => set
                .iter()
                .find(|w| w.id == h.host)
                .or_else(|| self.element(h.host))
                .is_some_and(|w| w.as_wall().is_some()),
            ElementKind::Dimension(d) => self.view(d.view).is_some(),
            _ => true,
        };
        type_ok && other_ok && self.level(e.level).is_some()
    }
}

impl crate::EditCtx<'_> {
    /// Copy `ids` once per transform (arrays pass several). Copied walls take
    /// copies of their doors and windows along; a hosted element copied
    /// without its wall stays on that wall, shifted along it. Returns the
    /// new ids.
    pub fn copy_elements(&mut self, ids: &[ElementId], transforms: &[PlanTransform]) -> Vec<ElementId> {
        let set = self.project().copy_set(ids);
        self.paste_elements(&set, transforms, None)
    }

    /// Add copies of `set` (from [`crate::Project::copy_set`]) once per
    /// transform, on `level` when given (hosted elements whose wall was not
    /// copied stay on that wall's level). Elements whose type, level, view
    /// or wall no longer exist are skipped, and walls attached to a missing
    /// level become unconnected. Returns the new ids.
    pub fn paste_elements(
        &mut self,
        set: &[Element],
        transforms: &[PlanTransform],
        level: Option<crate::LevelId>,
    ) -> Vec<ElementId> {
        let accepted: Vec<&Element> = set.iter().filter(|e| self.project().accepts(e, set)).collect();
        let mut created = Vec::new();
        for t in transforms {
            let mut remap = std::collections::BTreeMap::new();
            for el in accepted.iter().filter(|el| el.as_hosted().is_none()) {
                let mut c = (*el).clone();
                c.id = ElementId(self.alloc_id());
                c.uid = Default::default();
                c.transform(t);
                if let Some(level) = level {
                    c.level = level;
                }
                self.detach_missing_top(&mut c);
                remap.insert(el.id, c.id);
                created.push(self.add_element(c));
            }
            for el in accepted.iter().filter(|el| el.as_hosted().is_some()) {
                let Some(h) = el.as_hosted() else { continue };
                let mut c = (*el).clone();
                c.id = ElementId(self.alloc_id());
                c.uid = Default::default();
                c.mark = self.project().next_mark(c.category());
                let host = set
                    .iter()
                    .find(|w| w.id == h.host)
                    .or_else(|| self.project().element(h.host))
                    .and_then(|w| w.as_wall())
                    .map(|w| (w.length(), w.direction()));
                match (remap.get(&h.host), host) {
                    (Some(new_host), Some((len, _))) => {
                        c.transform(t);
                        if let Some(level) = level {
                            c.level = level;
                        }
                        if let Some(ch) = c.as_hosted_mut() {
                            ch.host = *new_host;
                            if t.mirror {
                                ch.offset = len - ch.offset;
                            }
                        }
                    }
                    // The wall must still be there to host a lone copy.
                    (None, Some((_, dir))) if self.project().element(h.host).is_some() => {
                        if let Some(ch) = c.as_hosted_mut() {
                            ch.offset += t.translation.dot(dir);
                        }
                    }
                    _ => continue,
                }
                created.push(self.add_element(c));
            }
        }
        created
    }

    /// A pasted wall, column or stair attached to a deleted level keeps its
    /// height, unconnected (as [`crate::Project::repair`] does on load).
    fn detach_missing_top(&self, e: &mut Element) {
        let top = match &mut e.kind {
            ElementKind::Wall(w) => &mut w.top,
            ElementKind::Column(c) => &mut c.top,
            ElementKind::Stair(s) => &mut s.top,
            _ => return,
        };
        if let crate::WallTop::Level { level, .. } = *top
            && self.project().level(level).is_none()
        {
            *top = crate::WallTop::Unconnected;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn transforms_compose_and_mirror() {
        let r = PlanTransform::rotate_about(DVec2::new(1.0, 1.0), std::f64::consts::FRAC_PI_2);
        assert!(close(r.apply(DVec2::new(2.0, 1.0)), DVec2::new(1.0, 2.0)));
        let m = PlanTransform::mirror_across(DVec2::new(0.0, 1.0), DVec2::X);
        assert!(close(m.apply(DVec2::new(3.0, 3.0)), DVec2::new(3.0, -1.0)));
        // Mirror across a diagonal swaps coordinates.
        let d = PlanTransform::mirror_across(DVec2::ZERO, DVec2::new(1.0, 1.0));
        assert!(close(d.apply(DVec2::new(2.0, 5.0)), DVec2::new(5.0, 2.0)));
        assert_eq!(d.apply_angle_deg(0.0).round(), 90.0);
        // Mirroring twice is the identity; then() applies in order.
        let twice = m.then(m);
        assert!(!twice.mirror && close(twice.apply(DVec2::new(3.0, 3.0)), DVec2::new(3.0, 3.0)));
        let t = PlanTransform::translate(DVec2::new(1.0, 0.0));
        let rt = r.then(t);
        assert!(close(rt.apply(DVec2::new(2.0, 1.0)), DVec2::new(2.0, 2.0)));
        let mt = m.then(r);
        assert!(close(mt.apply(DVec2::new(3.0, 3.0)), r.apply(m.apply(DVec2::new(3.0, 3.0)))));
    }

    #[test]
    fn mirrored_walls_keep_their_doors_in_place() {
        let mut doc =
            crate::Document::new(crate::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap());
        let (wall, door) = doc
            .project
            .elements
            .values()
            .find_map(|e| e.as_hosted().filter(|_| matches!(e.kind, ElementKind::Door(_))).map(|h| (h.host, e.id)))
            .unwrap();
        let world = |p: &crate::Project| {
            let d = p.element(door).and_then(|e| e.as_hosted()).cloned().unwrap();
            let w = p.element(d.host).and_then(|e| e.as_wall()).cloned().unwrap();
            (w.point_at(d.offset), d.flip_hand)
        };
        let (before, hand) = world(&doc.project);
        let axis = PlanTransform::mirror_across(DVec2::new(20.0, 0.0), DVec2::Y);
        doc.edit("mirror", |e| e.transform_elements(&[wall], &axis));
        let (after, hand2) = world(&doc.project);
        assert!(close(after, axis.apply(before)), "{after} vs {}", axis.apply(before));
        assert_ne!(hand, hand2);
        // Moving a door alone slides it along its fixed wall.
        let dir = doc.project.element(wall).and_then(|e| e.as_wall()).map(|w| w.direction()).unwrap();
        let before = world(&doc.project).0;
        doc.edit("move", |e| e.transform_elements(&[door], &PlanTransform::translate(dir * 0.5)));
        assert!(close(world(&doc.project).0, before + dir * 0.5));
    }

    #[test]
    fn arrays_copy_walls_with_their_openings() {
        let mut doc =
            crate::Document::new(crate::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap());
        let wall = doc.project.elements.values().find_map(|e| e.as_hosted().map(|h| h.host)).unwrap();
        let openings = doc.project.hosted_by(wall).count();
        let before = doc.project.elements.len();
        let steps: Vec<PlanTransform> =
            (1..=3).map(|k| PlanTransform::translate(DVec2::new(0.0, 20.0 * k as f64))).collect();
        let new = doc.edit("array", |e| e.copy_elements(&[wall], &steps));
        assert_eq!(new.len(), 3 * (1 + openings));
        assert_eq!(doc.project.elements.len(), before + new.len());
        // Every copy hosts its own openings, and ids/uids are unique.
        let uids: std::collections::BTreeSet<_> = doc.project.elements.values().map(|e| e.uid).collect();
        assert_eq!(uids.len(), doc.project.elements.len());
        for id in new.iter().filter(|id| doc.project.element(**id).is_some_and(|e| e.as_wall().is_some())) {
            assert_eq!(doc.project.hosted_by(*id).count(), openings);
        }
        assert_eq!(doc.project.repair(), 0);
    }
    #[test]
    fn a_cut_set_pastes_after_its_originals_are_gone() {
        let mut doc =
            crate::Document::new(crate::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap());
        let wall = doc.project.elements.values().find_map(|e| e.as_hosted().map(|h| h.host)).unwrap();
        let openings = doc.project.hosted_by(wall).count();
        let set = doc.project.copy_set(&[wall]);
        assert_eq!(set.len(), 1 + openings);
        doc.edit("cut", |e| e.remove_element(wall));
        assert!(doc.project.element(wall).is_none());
        let new = doc.edit("paste", |e| e.paste_elements(&set, &[PlanTransform::IDENTITY], None));
        assert_eq!(new.len(), 1 + openings);
        let copy = new.iter().copied().find(|id| doc.project.element(*id).is_some_and(|e| e.as_wall().is_some()));
        assert_eq!(doc.project.hosted_by(copy.unwrap()).count(), openings);
        assert_eq!(doc.project.repair(), 0);
    }

    #[test]
    fn pasting_skips_elements_whose_references_are_gone() {
        let mut doc =
            crate::Document::new(crate::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap());
        let wall = doc.project.elements.values().find_map(|e| e.as_hosted().map(|h| h.host)).unwrap();
        let mut set = doc.project.copy_set(&[wall]);
        // The wall's type was deleted since the copy: its openings would have
        // no new wall, and the original is gone too, so nothing is pasted.
        set[0].type_id = crate::ElementTypeId(u64::MAX);
        doc.edit("cut", |e| e.remove_element(wall));
        let new = doc.edit("paste", |e| e.paste_elements(&set, &[PlanTransform::IDENTITY], None));
        assert!(new.is_empty());
        // A missing level skips the element as well.
        let other = doc.project.walls().next().map(|(e, _)| e.id).unwrap();
        let mut set = doc.project.copy_set(&[other]);
        set.retain(|e| e.id == other);
        set[0].level = crate::LevelId(u64::MAX);
        assert!(doc.edit("paste", |e| e.paste_elements(&set, &[PlanTransform::IDENTITY], None)).is_empty());
        assert_eq!(doc.project.repair(), 0);
    }
}
