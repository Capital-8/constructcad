//! Structural profiles in the project: lookups and repair of loaded files.

use crate::structural_profile::{MAX_NAME, MAX_PROFILES, ProfileDef};
use crate::{ElementTypeId, ProfileId, Project, Uid};
use std::collections::BTreeSet;

/// `base`, or `base 2`, `base 3`… the first not in `taken` (compared
/// ignoring case), the base cut so the result fits `MAX_NAME`.
fn free_name(base: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let base: String = base.trim().chars().take(MAX_NAME - 12).collect();
    if !taken(&base) {
        return base;
    }
    let mut n = 2u64;
    loop {
        let candidate = format!("{base} {n}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

impl ProfileDef {
    /// Fix what a hostile file can break without dropping the profile: the
    /// id (it must be the map key), the name and a missing uid. Returns the
    /// number of fixes.
    pub fn repair(&mut self, id: ProfileId) -> usize {
        let mut repairs = 0;
        if self.id != id {
            self.id = id;
            repairs += 1;
        }
        let trimmed: String = self.name.trim().chars().take(MAX_NAME).collect();
        let name = if trimmed.trim().is_empty() { format!("Profile {}", id.0) } else { trimmed };
        if name != self.name {
            self.name = name;
            repairs += 1;
        }
        if self.uid.is_nil() {
            self.uid = Uid::generate();
            repairs += 1;
        }
        repairs
    }
}

impl Project {
    /// A profile by id.
    pub fn profile(&self, id: ProfileId) -> Option<&ProfileDef> {
        self.profiles.get(&id)
    }

    /// A profile by name, ignoring case and surrounding spaces.
    pub fn profile_named(&self, name: &str) -> Option<&ProfileDef> {
        let wanted = name.trim().to_lowercase();
        self.profiles.values().find(|p| p.name.trim().to_lowercase() == wanted)
    }

    /// The types that use a profile. Types do not carry a profile yet (that
    /// comes with the column and beam type fields), so this is empty until then.
    pub fn profile_users(&self, _id: ProfileId) -> Vec<ElementTypeId> {
        Vec::new()
    }

    /// `base`, or `base 2`, `base 3`… the first name no profile has.
    pub fn free_profile_name(&self, base: &str) -> String {
        free_name(base, &|n| self.profile_named(n).is_some())
    }

    /// Fix the project's profiles after loading; returns the number of fixes.
    pub(crate) fn repair_profiles(&mut self) -> usize {
        let mut repairs = 0;
        if self.profiles.len() > MAX_PROFILES {
            repairs += self.profiles.len() - MAX_PROFILES;
            let keep: BTreeSet<ProfileId> = self.profiles.keys().take(MAX_PROFILES).copied().collect();
            self.profiles.retain(|k, _| keep.contains(k));
        }
        let before = self.profiles.len();
        self.profiles.retain(|_, p| p.outline().is_ok());
        repairs += before - self.profiles.len();

        let mut names: BTreeSet<String> = BTreeSet::new();
        let mut uids: BTreeSet<Uid> = BTreeSet::new();
        for (id, p) in self.profiles.iter_mut() {
            repairs += p.repair(*id);
            if !names.insert(p.name.to_lowercase()) {
                p.name = free_name(&p.name, &|n| names.contains(&n.to_lowercase()));
                names.insert(p.name.to_lowercase());
                repairs += 1;
            }
            if !uids.insert(p.uid) {
                p.uid = Uid::generate();
                uids.insert(p.uid);
                repairs += 1;
            }
        }
        repairs
    }
}

#[cfg(test)]
mod tests {
    use crate::structural_profile::*;
    use crate::{Document, ProfileId, Project};

    fn def(id: u64, name: &str) -> ProfileDef {
        ProfileDef {
            id: ProfileId(id),
            uid: crate::Uid::generate(),
            name: name.into(),
            material: ProfileMaterial::Steel,
            shape: ProfileShape::default_for("i").unwrap(),
            catalog: None,
            description: String::new(),
        }
    }

    #[test]
    fn profiles_are_undone_as_settings() {
        let mut doc = Document::new(Project::from_template());
        doc.edit("add", |ctx| {
            let id = ProfileId(ctx.alloc_id());
            ctx.profiles_mut().insert(id, def(id.0, "IPE 200"));
        });
        assert!(doc.project.profile_named("ipe 200").is_some());
        doc.undo();
        assert!(doc.project.profile_named("IPE 200").is_none());
    }

    #[test]
    fn repair_fixes_ids_names_and_bad_shapes() {
        let mut p = Project::from_template();
        let base = p.max_id() + 1;
        p.profiles.insert(ProfileId(base), def(999, "A"));
        p.profiles.insert(ProfileId(base + 1), def(base + 1, "a"));
        let mut bad = def(base + 2, "Bad");
        bad.shape = ProfileShape::Rect { b: f64::NAN, h: 1.0, chamfer: 0.0 };
        p.profiles.insert(ProfileId(base + 2), bad);
        assert!(p.repair_profiles() >= 3);
        assert_eq!(p.profiles.len(), 2);
        assert_eq!(p.profiles[&ProfileId(base)].id, ProfileId(base));
        assert_ne!(
            p.profiles[&ProfileId(base)].name.to_lowercase(),
            p.profiles[&ProfileId(base + 1)].name.to_lowercase()
        );
        assert_eq!(p.repair_profiles(), 0);
    }

    #[test]
    fn repair_fixes_nil_and_duplicate_uids_and_empty_names() {
        let mut p = Project::empty();
        let uid = crate::Uid::generate();
        let mut a = def(1, "  ");
        a.uid = uid;
        let mut b = def(2, "B");
        b.uid = uid;
        let mut c = def(3, "C");
        c.uid = crate::Uid::default();
        for d in [a, b, c] {
            p.profiles.insert(d.id, d);
        }
        assert!(p.repair_profiles() >= 3);
        assert_eq!(p.profiles[&ProfileId(1)].name, "Profile 1");
        let uids: std::collections::BTreeSet<_> = p.profiles.values().map(|d| d.uid).collect();
        assert_eq!(uids.len(), 3);
        assert!(p.profiles.values().all(|d| !d.uid.is_nil()));
        assert_eq!(p.repair_profiles(), 0);
    }

    #[test]
    fn free_names_count_up() {
        let mut p = Project::empty();
        p.profiles.insert(ProfileId(1), def(1, "W 150 x 13"));
        assert_eq!(p.free_profile_name("W 150 x 13"), "W 150 x 13 2");
        assert_eq!(p.free_profile_name("HEB 200"), "HEB 200");
    }

    #[test]
    fn oversized_sketch_is_refused_before_expansion() {
        use crate::profile::ProfilePoint;
        let mut sk = ParamSketch::square(0.1, 0.1);
        sk.outline = (0..MAX_POINTS + 1).map(|_| ProfilePoint::new("0", "0")).collect();
        assert_eq!(sk.resolve().unwrap_err(), ProfileError::TooBig);
        let mut sk = ParamSketch::square(0.1, 0.1);
        sk.openings = vec![(0..MAX_POINTS).map(|_| ProfilePoint::new("0", "0")).collect()];
        assert_eq!(sk.resolve().unwrap_err(), ProfileError::TooBig);
    }

    #[test]
    fn sketch_errors_read_as_sentences() {
        use crate::profile::ProfilePoint;
        let mut sk = ParamSketch::square(0.1, 0.1);
        sk.outline[0] = ProfilePoint::new("nope", "0");
        let msg = sk.resolve().unwrap_err().to_string();
        assert!(!msg.contains('{') && !msg.contains('}'), "{msg}");
        assert!(msg.contains("nope"), "{msg}");
    }
}
