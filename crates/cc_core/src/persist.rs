//! `.ccad` project files: a versioned RON document.

use crate::Project;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current file format version. Bump when the schema changes incompatibly.
pub const FORMAT_VERSION: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid file: {0}")]
    Parse(#[from] ron::error::SpannedError),
    #[error("serialization error: {0}")]
    Serialize(#[from] ron::Error),
    #[error("unsupported format version {0} (maximum {FORMAT_VERSION})")]
    Version(u32),
}

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format_version: u32,
    generator: String,
    project: Project,
}

pub fn to_ron(project: &Project) -> Result<String, PersistError> {
    let file = ProjectFile {
        format_version: FORMAT_VERSION,
        generator: format!("ConstructCAD {}", env!("CARGO_PKG_VERSION")),
        project: project.clone(),
    };
    let config = ron::ser::PrettyConfig::new().depth_limit(8).indentor("  ".to_string());
    Ok(ron::ser::to_string_pretty(&file, config)?)
}

pub fn from_ron(text: &str) -> Result<Project, PersistError> {
    from_ron_with_repairs(text).map(|(project, _)| project)
}

/// Parse a project and repair invalid data (see `Project::repair`); also
/// returns how many repairs were needed.
pub fn from_ron_with_repairs(text: &str) -> Result<(Project, usize), PersistError> {
    let file: ProjectFile = ron::from_str(text)?;
    if file.format_version > FORMAT_VERSION {
        return Err(PersistError::Version(file.format_version));
    }
    // Version 1 files have no persistent ids; version 2 only adds fields with defaults.
    let mut project = file.project;
    let repairs = project.repair();
    project.ensure_uids();
    project.ensure_annotation_types();
    Ok((project, repairs))
}

pub fn save(project: &Project, path: &Path) -> Result<(), PersistError> {
    let text = to_ron(project)?;
    // Write to a temp file first so a failed save never truncates the original.
    let tmp = path.with_extension("ccad.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load(path: &Path) -> Result<Project, PersistError> {
    load_with_repairs(path).map(|(project, _)| project)
}

pub fn load_with_repairs(path: &Path) -> Result<(Project, usize), PersistError> {
    from_ron_with_repairs(&std::fs::read_to_string(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_round_trips() {
        let project = Project::from_template();
        let text = to_ron(&project).unwrap();
        let back = from_ron(&text).unwrap();
        assert_eq!(project, back);
    }

    #[test]
    fn rejects_future_version() {
        let text = to_ron(&Project::empty()).unwrap().replacen(
            &format!("format_version: {FORMAT_VERSION}"),
            "format_version: 999",
            1,
        );
        assert!(matches!(from_ron(&text), Err(PersistError::Version(999))));
    }
}
