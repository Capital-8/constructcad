//! `.ccad` project files: a versioned RON document.

use crate::Project;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current file format version. Bump when the schema changes incompatibly.
pub const FORMAT_VERSION: u32 = 26;

/// Largest project file read, bytes (pictures and textures are kept in it).
pub const MAX_FILE_BYTES: u64 = 1 << 30;

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
    #[error("the file is larger than {} MB", .0 >> 20)]
    TooLarge(u64),
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
        generator: format!("ConstructCraft {}", env!("CARGO_PKG_VERSION")),
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
    from_ron_with_repairs_localized(text, &|_| None)
}

/// Like [`from_ron_with_repairs`], naming what migration adds with
/// `translate` (template keys; Portuguese defaults when it gives none).
pub fn from_ron_with_repairs_localized(
    text: &str,
    translate: &dyn Fn(&str) -> Option<String>,
) -> Result<(Project, usize), PersistError> {
    let file: ProjectFile = ron::from_str(text)?;
    if file.format_version > FORMAT_VERSION {
        return Err(PersistError::Version(file.format_version));
    }
    // Version 1 files have no persistent ids; versions 2 and 3 only add fields
    // with defaults; version 4 adds dimension kinds (older files: aligned);
    // version 9 adds arc walls (older files: straight); version 10 adds view
    // graphics, filters and templates (older files: none); version 11 adds
    // construction phases (older files: the template's, everything in the last);
    // version 12 makes door and window types components (older files: the
    // built-in door and window components with the same sizes); version 18
    // adds script pieces and text, boolean, choice and material parameters
    // (older files have none); version 22 makes title blocks components
    // (older files: the standard title block on every sheet); version 23 adds
    // design options (older files have none, everything in the main model);
    // version 24 adds material textures and painted faces (older files have none);
    // version 25 adds custom fill patterns (older files have none).
    let mut project = file.project;
    if file.format_version < 11 {
        project.ensure_phases(&|key| crate::project::template_default(key).to_string());
    }
    project.convert_opening_types(&|key| crate::project::template_default(key).to_string());
    let mut repairs = project.repair();
    if file.format_version < 22 && project.title_blocks().is_empty() {
        let n = |k: &str| translate(k).unwrap_or_else(|| crate::project::template_default(k).to_string());
        let id = project.add_standard_title_block(&n);
        repairs += 1 + project.give_sheets_title_block(id);
    }
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

/// The text of the file at `path`, refused past `max` bytes before it is
/// read (an I/O error of kind `FileTooLarge`): files are hostile input, and
/// one far larger than any real file would only exhaust memory.
pub fn read_to_string_limited(path: &Path, max: u64) -> std::io::Result<String> {
    if std::fs::metadata(path)?.len() > max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            format!("the file is larger than {} MB", max >> 20),
        ));
    }
    std::fs::read_to_string(path)
}

/// The text of the file at `path`, refused past `max` bytes before it is read.
fn read_text(path: &Path, max: u64) -> Result<String, PersistError> {
    if std::fs::metadata(path)?.len() > max {
        return Err(PersistError::TooLarge(max));
    }
    Ok(std::fs::read_to_string(path)?)
}

pub fn load(path: &Path) -> Result<Project, PersistError> {
    load_with_repairs(path).map(|(project, _)| project)
}

pub fn load_with_repairs(path: &Path) -> Result<(Project, usize), PersistError> {
    from_ron_with_repairs(&read_text(path, MAX_FILE_BYTES)?)
}

/// Like [`load_with_repairs`], naming what migration adds with `translate`
/// (see [`from_ron_with_repairs_localized`]).
pub fn load_with_repairs_localized(
    path: &Path,
    translate: &dyn Fn(&str) -> Option<String>,
) -> Result<(Project, usize), PersistError> {
    from_ron_with_repairs_localized(&read_text(path, MAX_FILE_BYTES)?, translate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_files_are_refused_before_reading() {
        let path = std::env::temp_dir().join(format!("ccad-persist-{}-large.ccad", std::process::id()));
        std::fs::write(&path, "x".repeat(2048)).unwrap();
        assert!(matches!(read_text(&path, 1024), Err(PersistError::TooLarge(1024))));
        assert_eq!(read_text(&path, 4096).unwrap().len(), 2048);
        // The same for the other files read whole (components, graphs, macros).
        let err = read_to_string_limited(&path, 1024).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::FileTooLarge);
        assert_eq!(read_to_string_limited(&path, 4096).unwrap().len(), 2048);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn template_round_trips() {
        let project = Project::from_template();
        let text = to_ron(&project).unwrap();
        let back = from_ron(&text).unwrap();
        assert_eq!(project, back);
    }

    #[test]
    fn format_20_sheets_get_the_standard_title_block() {
        let mut p = Project::from_template();
        // As a format-20 file: no title block types, sheets without one.
        p.types.retain(|_, t| t.category() != crate::Category::TitleBlock);
        for v in p.views.values_mut() {
            if let crate::ViewKind::Sheet { title_block, .. } = &mut v.kind {
                *title_block = None;
            }
        }
        let text = to_ron(&p).unwrap().replace(&format!("format_version: {FORMAT_VERSION}"), "format_version: 20");
        let english = |k: &str| (k == "tpl-tb-project").then(|| "PROJECT".to_string());
        let (q, repairs) = from_ron_with_repairs_localized(&text, &english).unwrap();
        assert!(repairs >= 1);
        // Format 21 (graph bindings) had no title blocks either.
        let (q21, _) = from_ron_with_repairs(&text.replace("format_version: 20", "format_version: 21")).unwrap();
        assert_eq!(q21.title_blocks().len(), 1);
        let blocks = q.title_blocks();
        assert_eq!(blocks.len(), 1);
        assert!(q.views.values().all(|v| match &v.kind {
            crate::ViewKind::Sheet { title_block, .. } => *title_block == Some(blocks[0]),
            _ => true,
        }));
        let Some(crate::TypeData::Component { def, .. }) = q.element_type(blocks[0]).map(|t| &t.data) else { panic!() };
        assert!(
            def.sheet
                .iter()
                .any(|i| matches!(&i.shape, crate::title_block::SheetShape::Text { text, .. } if text == "PROJECT"))
        );
        assert!(!q.element_type(blocks[0]).unwrap().uid.is_nil());
        // A current file is left alone.
        let (r, _) = from_ron_with_repairs(&to_ron(&q).unwrap()).unwrap();
        assert_eq!(r.title_blocks().len(), 1);
    }

    #[test]
    fn a_wrong_id_counter_never_reuses_an_id() {
        // A format-20 file whose counter points at a wall type in use: the
        // standard title block it gets must not replace that type.
        let mut p = Project::from_template();
        p.types.retain(|_, t| t.category() != crate::Category::TitleBlock);
        for v in p.views.values_mut() {
            if let crate::ViewKind::Sheet { title_block, .. } = &mut v.kind {
                *title_block = None;
            }
        }
        let wall = p.types.values().find(|t| t.category() == crate::Category::Wall).map(|t| t.id).unwrap();
        let counter = format!("next_id: {}", p.next_id());
        for (n, id) in [(1, wall.0), (2, u64::MAX)] {
            let text = to_ron(&p)
                .unwrap()
                .replace(&format!("format_version: {FORMAT_VERSION}"), "format_version: 20")
                .replace(&counter, &format!("next_id: {id}"));
            let (q, _) = from_ron_with_repairs(&text).unwrap();
            assert_eq!(q.element_type(wall).map(|t| t.category()), Some(crate::Category::Wall), "case {n}");
            assert_eq!(q.title_blocks().len(), 1, "case {n}");
            assert!(q.next_id() > q.max_id(), "case {n}");
        }
    }

    #[test]
    fn design_options_round_trip_and_format_22_files_load() {
        let mut p = Project::from_template();
        let set = crate::OptionSetId(p.alloc_id());
        let (a, b) = (crate::OptionId(p.alloc_id()), crate::OptionId(p.alloc_id()));
        p.option_sets.push(crate::OptionSet {
            id: set,
            name: "Entrance".into(),
            options: vec![
                crate::DesignOption { id: a, name: "Porch".into() },
                crate::DesignOption { id: b, name: "Canopy".into() },
            ],
            primary: b,
        });
        let view = *p.views.keys().next().unwrap();
        p.views.get_mut(&view).unwrap().design_options.insert(set, a);
        p.active_option = Some(a);
        let back = from_ron(&to_ron(&p).unwrap()).unwrap();
        assert_eq!(back.option_sets, p.option_sets);
        assert_eq!(back.view(view).unwrap().design_options.get(&set), Some(&a));
        // The option being edited is session state.
        assert_eq!(back.active_option, None);
        // A format-22 file (before design options) loads with none.
        let old = to_ron(&Project::from_template())
            .unwrap()
            .replace(&format!("format_version: {FORMAT_VERSION}"), "format_version: 22");
        let q = from_ron(&old).unwrap();
        assert!(q.option_sets.is_empty() && q.elements.values().all(|e| e.design_option.is_none()));
    }

    #[test]
    fn custom_fill_patterns_round_trip_and_format_24_files_load() {
        use crate::fill_pattern::{FillPattern, FillPatternKind, PatternLine};
        let mut p = Project::from_template();
        let id = crate::FillPatternId(p.max_id() + 1);
        p.fill_patterns.insert(
            id,
            FillPattern {
                id,
                name: "Tiles 300".into(),
                description: String::new(),
                kind: FillPatternKind::Model,
                scale: 1.0,
                lines: vec![PatternLine::solid(0.0, 300.0), PatternLine::solid(90.0, 300.0)],
            },
        );
        p.materials[0].graphics.cut.foreground = crate::Pattern::Custom(id);
        let back = from_ron(&to_ron(&p).unwrap()).unwrap();
        assert_eq!(back.fill_patterns, p.fill_patterns);
        assert_eq!(back.materials[0].graphics.cut.foreground, crate::Pattern::Custom(id));
        // A format-24 file (before custom patterns) loads with none.
        let old = to_ron(&Project::from_template())
            .unwrap()
            .replace(&format!("format_version: {FORMAT_VERSION}"), "format_version: 24");
        let (q, repairs) = from_ron_with_repairs(&old).unwrap();
        assert!(q.fill_patterns.is_empty());
        assert_eq!(repairs, 0);
    }

    #[test]
    fn arc_strips_round_trip_and_format_25_strips_load_straight() {
        use crate::{Element, ElementId, ElementKind, Foundation, FoundationShape};
        use glam::DVec2;
        let mut p = Project::from_template();
        let level = p.levels[0].id;
        let type_id = p.default_type(crate::Category::Foundation).unwrap();
        let strip = |bulge: f64| Foundation {
            shape: FoundationShape::Strip { start: DVec2::ZERO, end: DVec2::new(4.0, 0.0), bulge },
            top_offset: 0.0,
        };
        let id = ElementId(p.alloc_id());
        let e = Element {
            id,
            level,
            type_id,
            mark: String::new(),
            comments: String::new(),
            kind: ElementKind::Foundation(strip(0.5)),
            uid: Default::default(),
            phase_created: Default::default(),
            phase_demolished: None,
            design_option: None,
            paints: Vec::new(),
        };
        p.elements.insert(id, e);
        let text = to_ron(&p).unwrap();
        assert_eq!(from_ron(&text).unwrap().element(id).unwrap().kind, ElementKind::Foundation(strip(0.5)));
        // A format-25 file has no bulge: its strips are straight.
        let old = text
            .replace(", bulge: 0.5", "")
            .replace("bulge: 0.5,", "")
            .replace(&format!("format_version: {FORMAT_VERSION}"), "format_version: 25");
        assert!(!old.contains("bulge: 0.5"), "{old}");
        let (q, repairs) = from_ron_with_repairs(&old).unwrap();
        assert_eq!(q.element(id).unwrap().kind, ElementKind::Foundation(strip(0.0)));
        assert_eq!(repairs, 0);
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
