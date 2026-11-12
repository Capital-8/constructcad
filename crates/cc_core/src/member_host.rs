//! Member hosts: elements with no geometry of their own (steel buildings,
//! trusses) that own ordinary columns and beams laid out from their
//! parameters. Every edit keeps them in step ([`crate::EditCtx`] runs
//! [`crate::EditCtx::sync_member_hosts`]): a changed host rewrites its
//! members in place, keeping their ids while their roles exist; a member
//! the user edited or deleted is detached, and the host leaves its role
//! alone from then on. Each host keeps its own typed roles; the shared code
//! sees them as [`RoleKey`]s, whose order is the typed roles' order.

use crate::{Category, Element, ElementId, ElementKind, ElementTypeId, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

/// A member's role as the shared code sees it: kind, group (frame, copy)
/// and index packed so that the order is the typed role's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoleKey(pub u64);

impl RoleKey {
    pub fn new(kind: u8, group: u16, index: u16) -> Self {
        Self((u64::from(kind) << 32) | (u64::from(group) << 16) | u64::from(index))
    }

    pub fn parts(self) -> (u8, u16, u16) {
        (((self.0 >> 32) & 0xff) as u8, ((self.0 >> 16) & 0xffff) as u16, (self.0 & 0xffff) as u16)
    }
}

/// A host of members: what the shared sync and repair need from it.
pub trait MemberHost {
    /// The member types it uses (an edit of one regenerates it).
    fn member_types(&self) -> Vec<ElementTypeId>;
    /// Its members as its parameters lay them out, in a fixed order.
    fn member_layout(&self, project: &Project) -> Vec<(RoleKey, ElementTypeId, ElementKind)>;
    /// The members it manages, by role.
    fn member_list(&self) -> Vec<(RoleKey, ElementId)>;
    /// Replace the managed members (keys it cannot read are left out).
    fn set_member_list(&mut self, members: &[(RoleKey, ElementId)]);
    /// Roles the user took over: not made again.
    fn detached_roles(&self) -> Vec<RoleKey>;
    fn set_detached_roles(&mut self, roles: &[RoleKey]);
    /// Whether an element of `kind` can fill `role`.
    fn role_fits(&self, role: RoleKey, kind: &ElementKind) -> bool;
    /// Whether its numbers are in range and its types of the right categories.
    fn check_parameters(&self, project: &Project) -> Result<(), String>;
    /// From a file: numbers back to their defaults, types to the category's
    /// default; returns the repairs, `None` when the host must go.
    fn repair_parameters(&mut self, project: &Project) -> Option<usize>;
}

impl ElementKind {
    /// The host behind this element, if it is one.
    pub fn member_host(&self) -> Option<&dyn MemberHost> {
        match self {
            ElementKind::SteelBuilding(b) => Some(b.as_ref() as &dyn MemberHost),
            _ => None,
        }
    }

    pub fn member_host_mut(&mut self) -> Option<&mut dyn MemberHost> {
        match self {
            ElementKind::SteelBuilding(b) => Some(b.as_mut() as &mut dyn MemberHost),
            _ => None,
        }
    }
}

impl Element {
    pub fn member_host(&self) -> Option<&dyn MemberHost> {
        self.kind.member_host()
    }
}

impl Project {
    /// The members a host manages (none for other elements).
    pub fn host_members_of(&self, host: ElementId) -> Vec<ElementId> {
        self.element(host)
            .and_then(|e| e.member_host())
            .map(|h| h.member_list().into_iter().map(|(_, id)| id).collect())
            .unwrap_or_default()
    }
}

/// Whether `v` is a finite number in `range` (else an error naming it).
pub(crate) fn within(name: &str, v: f64, range: &RangeInclusive<f64>) -> Result<(), String> {
    if v.is_finite() && range.contains(&v) {
        Ok(())
    } else {
        Err(format!("{name} must be between {} and {}", range.start(), range.end()))
    }
}

/// Reset `v` to `default` when it is out of `range`; returns 1 when it did.
pub(crate) fn reset(v: &mut f64, range: &RangeInclusive<f64>, default: f64) -> usize {
    if v.is_finite() && range.contains(v) {
        0
    } else {
        *v = default;
        1
    }
}

/// `slot` if it is a type of `category`, else the category's default type
/// (`Some(1)`: one repair); `None` when there is none.
pub(crate) fn fix_type(p: &Project, slot: &mut ElementTypeId, category: Category) -> Option<usize> {
    if p.element_type(*slot).is_some_and(|t| t.category() == category) {
        return Some(0);
    }
    *slot = p.default_type(category)?;
    Some(1)
}

impl crate::EditCtx<'_> {
    /// Keep member hosts and their members in step, from what this edit
    /// itself changed (it runs before the syncs that touch elements):
    /// 1. a member the edit deleted, or whose kind, type or level it changed
    ///    while leaving its host alone, is detached (its role leaves the
    ///    member list for the detached roles; the element stays as the user
    ///    left it);
    /// 2. a host the edit added or changed, or whose member types it
    ///    changed, gets the members of its layout (except detached roles):
    ///    existing ones rewritten in place (same ids), missing ones added,
    ///    those whose role is gone deleted. A host the edit added (a copy, a
    ///    paste) keeps none of the members it lists and gets its own: the
    ///    listed elements may be another host's or, after a paste, unrelated
    ///    elements the user owns;
    /// 3. a host whose phases or design option the edit changed passes them
    ///    on to the members it manages (a member's own phase or option edit
    ///    is not a detach, and stays until the host's next change).
    pub(crate) fn sync_member_hosts(&mut self) {
        let p = self.project();
        if !p.elements.values().any(|e| e.member_host().is_some()) {
            return;
        }
        let types = self.changed_types();
        let mut added: BTreeSet<ElementId> = BTreeSet::new();
        let mut changed: BTreeSet<ElementId> = BTreeSet::new();
        let mut removed: BTreeSet<ElementId> = BTreeSet::new();
        let mut reshaped: BTreeSet<ElementId> = BTreeSet::new();
        let mut moved_phase: BTreeSet<ElementId> = BTreeSet::new();
        for (id, before) in self.element_changes() {
            match (before, p.element(id)) {
                (None, Some(_)) => {
                    added.insert(id);
                    changed.insert(id);
                }
                (Some(_), None) => {
                    removed.insert(id);
                }
                (Some(b), Some(n)) => {
                    changed.insert(id);
                    if b.kind != n.kind || b.type_id != n.type_id || b.level != n.level {
                        reshaped.insert(id);
                    }
                    if (b.phase_created, b.phase_demolished, b.design_option)
                        != (n.phase_created, n.phase_demolished, n.design_option)
                    {
                        moved_phase.insert(id);
                    }
                }
                (None, None) => {}
            }
        }
        let mut detach: Vec<(ElementId, Vec<RoleKey>)> = Vec::new();
        let mut regenerate: Vec<ElementId> = Vec::new();
        for e in p.elements.values() {
            let Some(h) = e.member_host() else { continue };
            let host_changed = changed.contains(&e.id) || h.member_types().iter().any(|t| types.contains(t));
            let taken: Vec<RoleKey> = h
                .member_list()
                .into_iter()
                .filter(|(_, id)| removed.contains(id) || (reshaped.contains(id) && !host_changed))
                .map(|(r, _)| r)
                .collect();
            if !taken.is_empty() {
                detach.push((e.id, taken));
            }
            if host_changed {
                regenerate.push(e.id);
            }
        }
        for (host, roles) in detach {
            if let Some(h) = self.element_mut(host).and_then(|e| e.kind.member_host_mut()) {
                let mut list = h.member_list();
                list.retain(|(r, _)| !roles.contains(r));
                h.set_member_list(&list);
                let mut detached = h.detached_roles();
                for r in roles {
                    if !detached.contains(&r) {
                        detached.push(r);
                    }
                }
                h.set_detached_roles(&detached);
            }
        }
        for host in regenerate {
            self.regenerate_members(host, added.contains(&host), moved_phase.contains(&host));
        }
    }

    /// Give host `host` the members of its layout (see
    /// [`Self::sync_member_hosts`]); `added` when this edit added it,
    /// `follow` when its phases or design option changed.
    fn regenerate_members(&mut self, host: ElementId, added: bool, follow: bool) {
        let p = self.project();
        let Some(e) = p.element(host) else { return };
        let Some(h) = e.member_host() else { return };
        let (level, phase_created, phase_demolished, design_option) =
            (e.level, e.phase_created, e.phase_demolished, e.design_option);
        let phases = (phase_created, phase_demolished, design_option);
        let listed = h.member_list();
        // A copy or a paste keeps none of the members it lists: they are the
        // original's or, pasted elsewhere or after a detach, the user's.
        let members = if added { Vec::new() } else { listed.clone() };
        let detached = h.detached_roles();
        let mut wanted: BTreeMap<RoleKey, (ElementTypeId, ElementKind)> = h
            .member_layout(p)
            .into_iter()
            .filter(|(r, _, _)| !detached.contains(r))
            .map(|(r, t, k)| (r, (t, k)))
            .collect();
        let (mut kept, mut gone, mut rewrite, mut phased) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        // Each element serves one role only (a crafted list may repeat one).
        let mut used = BTreeSet::new();
        for &(role, id) in &members {
            // A role made detached by hand: the element is the user's now.
            if detached.contains(&role) {
                continue;
            }
            let Some(x) = p.element(id).filter(|x| x.id != host && h.role_fits(role, &x.kind) && used.insert(id))
            else {
                // Gone, or no longer of the role's kind: dropped from the list.
                continue;
            };
            match wanted.remove(&role) {
                Some((t, kind)) => {
                    if x.kind != kind || x.type_id != t || x.level != level {
                        rewrite.push((id, t, kind));
                    }
                    if follow && (x.phase_created, x.phase_demolished, x.design_option) != phases {
                        phased.push(id);
                    }
                    kept.push((role, id));
                }
                None => gone.push(id),
            }
        }
        for (id, t, kind) in rewrite {
            if let Some(x) = self.element_mut(id) {
                (x.kind, x.type_id, x.level) = (kind, t, level);
            }
        }
        for id in phased {
            if let Some(x) = self.element_mut(id) {
                (x.phase_created, x.phase_demolished, x.design_option) = phases;
            }
        }
        for id in gone {
            self.remove_element(id);
        }
        for (role, (type_id, kind)) in wanted {
            let id = ElementId(self.alloc_id());
            self.add_element(Element {
                id,
                level,
                type_id,
                mark: String::new(),
                comments: String::new(),
                kind,
                uid: Default::default(),
                phase_created,
                phase_demolished,
                design_option,
                paints: Vec::new(),
            });
            kept.push((role, id));
        }
        kept.sort_by_key(|m| m.0);
        if kept != listed
            && let Some(h) = self.element_mut(host).and_then(|e| e.kind.member_host_mut())
        {
            h.set_member_list(&kept);
        }
    }
}

impl Project {
    /// Member hosts from a file: numbers and types repaired by the host
    /// (it goes when a type has no fallback), members that are gone, do not
    /// fit their role, repeat a role or are another host's dropped, and
    /// repeated detached roles dropped. Returns the repairs.
    pub(crate) fn repair_member_hosts(&mut self) -> usize {
        let ids: Vec<ElementId> = self.elements.values().filter(|e| e.member_host().is_some()).map(|e| e.id).collect();
        let mut repairs = 0;
        let mut claimed = BTreeSet::new();
        let mut dropped = Vec::new();
        for id in ids {
            let Some(mut kind) = self.elements.get(&id).map(|e| e.kind.clone()) else { continue };
            let Some(h) = kind.member_host_mut() else { continue };
            let Some(mut n) = h.repair_parameters(self) else {
                dropped.push(id);
                continue;
            };
            let list = h.member_list();
            let mut roles = BTreeSet::new();
            let kept: Vec<(RoleKey, ElementId)> = {
                let fits = &*h;
                list.iter()
                    .copied()
                    .filter(|&(r, m)| {
                        self.element(m).is_some_and(|e| fits.role_fits(r, &e.kind))
                            && roles.insert(r)
                            && claimed.insert(m)
                    })
                    .collect()
            };
            n += list.len() - kept.len();
            h.set_member_list(&kept);
            let detached = h.detached_roles();
            let mut seen = BTreeSet::new();
            let unique: Vec<RoleKey> = detached.iter().copied().filter(|r| seen.insert(*r)).collect();
            n += detached.len() - unique.len();
            h.set_detached_roles(&unique);
            if n > 0 {
                if let Some(e) = self.elements.get_mut(&id) {
                    e.kind = kind;
                }
                repairs += n;
            }
        }
        for id in dropped {
            self.elements.remove(&id);
            repairs += 1;
        }
        repairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steel_building::{MemberRole, RoleKind};

    #[test]
    fn role_keys_keep_the_typed_order_and_round_trip() {
        let roles = [
            MemberRole { kind: RoleKind::Column, frame: 0, index: 1 },
            MemberRole { kind: RoleKind::Column, frame: 1, index: 0 },
            MemberRole { kind: RoleKind::Rafter, frame: 0, index: 0 },
            MemberRole { kind: RoleKind::Girt, frame: 3, index: 1002 },
        ];
        for w in roles.windows(2) {
            assert!(w[0] < w[1] && w[0].key() < w[1].key());
        }
        for r in roles {
            assert_eq!(MemberRole::from_key(r.key()), Some(r));
        }
        assert_eq!(RoleKey::new(9, 1, 2).parts(), (9, 1, 2));
        assert_eq!(MemberRole::from_key(RoleKey::new(9, 0, 0)), None, "no such kind");
    }

    #[test]
    fn only_hosts_dispatch() {
        let (doc, host) = crate::steel_building::tests::site();
        let p = &doc.project;
        let h = p.element(host).and_then(|e| e.member_host()).expect("a host");
        assert_eq!(h.member_list().len(), 134);
        assert!(h.detached_roles().is_empty());
        let member = h.member_list()[0].1;
        assert!(p.element(member).and_then(|e| e.member_host()).is_none(), "a column is no host");
        assert_eq!(p.host_members_of(host).len(), 134);
        assert!(p.host_members_of(member).is_empty());
    }
}
