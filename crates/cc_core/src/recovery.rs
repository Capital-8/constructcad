//! Crash recovery: while a project has unsaved changes, a copy of it is
//! written now and then to a recovery folder, so a crash (or a power cut)
//! loses at most the last few minutes of work. Each running session writes
//! its own entry: a plain `.ccad` file (it opens like any project) and a
//! small `.meta` file naming the project and where it was saved. Both are
//! written to a temporary file first and renamed, so an entry is never half
//! written. A clean save or close removes the entry; at start-up, the
//! entries left by earlier sessions are offered for restoring.

use crate::{PersistError, Project};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What a recovery entry says about the project it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RecoveryMeta {
    /// The project's name.
    #[serde(default)]
    pub name: String,
    /// Where the project was saved, if it ever was.
    #[serde(default)]
    pub original: Option<PathBuf>,
    /// When the copy was written, in seconds since the Unix epoch.
    #[serde(default)]
    pub saved_at: u64,
}

/// A recovery entry found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryEntry {
    /// The entry's id (the session that wrote it).
    pub id: String,
    /// The copy of the project.
    pub project: PathBuf,
    pub meta: RecoveryMeta,
}

fn project_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.ccad"))
}

fn meta_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.meta"))
}

/// Write `text` to `path` through a temporary file and a rename.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Seconds since the Unix epoch (0 if the clock is before it).
pub fn now_seconds() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Write the recovery entry `id` in `dir` (created if needed): the project
/// first, then its meta file, each atomically. Returns the project's path.
pub fn write(dir: &Path, id: &str, project: &Project, meta: &RecoveryMeta) -> Result<PathBuf, PersistError> {
    std::fs::create_dir_all(dir)?;
    let path = project_path(dir, id);
    write_atomic(&path, &crate::to_ron(project)?)?;
    let config = ron::ser::PrettyConfig::new();
    write_atomic(&meta_path(dir, id), &ron::ser::to_string_pretty(meta, config)?)?;
    Ok(path)
}

/// Remove the recovery entry `id` from `dir` (nothing if there is none).
pub fn remove(dir: &Path, id: &str) {
    for path in [meta_path(dir, id), project_path(dir, id)] {
        // A file that is already gone is what we want.
        let _ = std::fs::remove_file(path);
    }
}

/// The recovery entries in `dir`, the newest first: those whose project
/// file exists (a meta file that cannot be read still offers its project).
pub fn list(dir: &Path) -> Vec<RecoveryEntry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<RecoveryEntry> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ccad"))
        .filter_map(|project| {
            let id = project.file_stem()?.to_str()?.to_string();
            let meta = std::fs::read_to_string(meta_path(dir, &id))
                .ok()
                .and_then(|text| ron::from_str::<RecoveryMeta>(&text).ok())
                .unwrap_or_default();
            Some(RecoveryEntry { id, project, meta })
        })
        .collect();
    entries.sort_by(|a, b| b.meta.saved_at.cmp(&a.meta.saved_at).then(a.id.cmp(&b.id)));
    entries
}

/// Load the project of a recovery entry (repaired like any opened file).
pub fn load(entry: &RecoveryEntry) -> Result<(Project, usize), PersistError> {
    crate::load_with_repairs(&entry.project)
}

/// What [`Autosave::poll`] asks the caller to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosaveAction {
    Nothing,
    /// Write the document (its revision is the argument of [`Autosave::written`]).
    Write,
    /// The document has no unsaved changes: remove the entry.
    Remove,
}

/// When to write a recovery copy: once the document has had unsaved changes
/// for `interval`, and only if its revision changed since the last copy.
#[derive(Debug, Clone)]
pub struct Autosave {
    pub interval: Duration,
    /// Revision of the last copy written, if one is on disk.
    written: Option<u64>,
    /// When the changes not yet copied started.
    pending_since: Option<Instant>,
}

impl Autosave {
    pub fn new(interval: Duration) -> Self {
        Self { interval, written: None, pending_since: None }
    }

    /// Called often (once a frame) with the document's `revision` and
    /// whether it has unsaved changes.
    pub fn poll(&mut self, revision: u64, dirty: bool, now: Instant) -> AutosaveAction {
        if !dirty {
            self.pending_since = None;
            return match self.written.take() {
                Some(_) => AutosaveAction::Remove,
                None => AutosaveAction::Nothing,
            };
        }
        if self.written == Some(revision) {
            self.pending_since = None;
            return AutosaveAction::Nothing;
        }
        let since = *self.pending_since.get_or_insert(now);
        if now.saturating_duration_since(since) >= self.interval {
            AutosaveAction::Write
        } else {
            AutosaveAction::Nothing
        }
    }

    /// A copy of `revision` was written.
    pub fn written(&mut self, revision: u64) {
        self.written = Some(revision);
        self.pending_since = None;
    }

    /// Writing failed: try again an interval after `now`.
    pub fn retry_later(&mut self, now: Instant) {
        self.pending_since = Some(now);
    }

    /// Whether a copy is on disk.
    pub fn has_copy(&self) -> bool {
        self.written.is_some()
    }

    /// The entry was removed (the document was saved, closed or discarded).
    pub fn removed(&mut self) {
        self.written = None;
        self.pending_since = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ccad-recovery-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn entries_round_trip_and_are_removed() {
        let dir = scratch("round-trip");
        assert!(list(&dir).is_empty(), "a missing folder has no entries");
        let mut project = Project::from_template();
        project.info.name = "Casa".into();
        let meta = RecoveryMeta { name: "Casa".into(), original: Some(PathBuf::from("casa.ccad")), saved_at: 10 };
        write(&dir, "older", &Project::from_template(), &RecoveryMeta { saved_at: 5, ..meta.clone() }).unwrap();
        let path = write(&dir, "newer", &project, &meta).unwrap();
        let entries = list(&dir);
        assert_eq!(entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["newer", "older"]);
        assert_eq!((&entries[0].project, &entries[0].meta), (&path, &meta));
        let (back, repairs) = load(&entries[0]).unwrap();
        assert_eq!((back, repairs), (project.clone(), 0));
        // Rewriting replaces the entry; no temporary files stay behind.
        write(&dir, "newer", &project, &RecoveryMeta { saved_at: 20, ..meta }).unwrap();
        assert_eq!(list(&dir)[0].meta.saved_at, 20);
        let names: Vec<String> =
            std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
        remove(&dir, "newer");
        remove(&dir, "never-written");
        assert_eq!(list(&dir).iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["older"]);
        // A lost meta file still offers the project.
        std::fs::remove_file(meta_path(&dir, "older")).unwrap();
        assert_eq!(list(&dir)[0].meta, RecoveryMeta::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_writes_only_changed_revisions_after_the_interval() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut a = Autosave::new(Duration::from_secs(60));
        // Clean documents are never written.
        assert_eq!(a.poll(1, false, at(0)), AutosaveAction::Nothing);
        assert_eq!(a.poll(1, false, at(500)), AutosaveAction::Nothing);
        // A change waits for the interval.
        assert_eq!(a.poll(2, true, at(500)), AutosaveAction::Nothing);
        assert_eq!(a.poll(3, true, at(530)), AutosaveAction::Nothing);
        assert_eq!(a.poll(3, true, at(560)), AutosaveAction::Write);
        a.written(3);
        // The same revision is not written again, however long it waits.
        assert_eq!(a.poll(3, true, at(700)), AutosaveAction::Nothing);
        assert_eq!(a.poll(3, true, at(9_000)), AutosaveAction::Nothing);
        // A new change starts a new wait.
        assert_eq!(a.poll(4, true, at(9_001)), AutosaveAction::Nothing);
        assert_eq!(a.poll(4, true, at(9_061)), AutosaveAction::Write);
        // A failed write waits another interval instead of retrying every frame.
        a.retry_later(at(9_061));
        assert_eq!(a.poll(4, true, at(9_062)), AutosaveAction::Nothing);
        assert_eq!(a.poll(4, true, at(9_121)), AutosaveAction::Write);
        a.written(4);
        // Saving removes the copy, once.
        assert_eq!(a.poll(4, false, at(9_100)), AutosaveAction::Remove);
        a.removed();
        assert_eq!(a.poll(4, false, at(9_200)), AutosaveAction::Nothing);
        assert!(!a.has_copy());
    }
}
