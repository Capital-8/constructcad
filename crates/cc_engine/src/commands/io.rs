//! Files: IFC import/export, sheet PDF/SVG, view PNG/SVG/PDF, schedule CSV, component files.

use super::edit;
use crate::error::{EngineError, Result};
use crate::refs::{self, Ref};
use crate::registry::{Command, CommandKind};
use crate::session::{Session, guard, new_project};
use crate::t;
use cc_core::{Document, ViewKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    Ok(std::fs::write(path, bytes)?)
}

/// Import an IFC file (IFC2X3 or IFC4), replacing the session's project. The
/// result has no file yet; save it with `project_save` and a path.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IfcImport {
    /// Path of the .ifc file.
    pub path: PathBuf,
}

impl Command for IfcImport {
    const NAME: &'static str = "ifc_import";
    const KIND: CommandKind = CommandKind::File;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (project, report) = guard(|| cc_ifc::import_file(&self.path, new_project()))??;
        s.replace(Document::new(project));
        let imported: serde_json::Map<String, Value> =
            report.imported.iter().map(|(c, n)| (refs::category_name(*c), json!(n))).collect();
        Ok(json!({
            "schema": report.schema,
            "levels": report.levels,
            "imported": imported,
            "generic": report.generic,
            "skipped": report.skipped,
        }))
    }
}

/// Export the project as IFC4.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IfcExport {
    /// Path of the .ifc file to write.
    pub path: PathBuf,
}

impl Command for IfcExport {
    const NAME: &'static str = "ifc_export";
    const KIND: CommandKind = CommandKind::File;
    fn run(self, s: &mut Session) -> Result<Value> {
        guard(|| cc_ifc::export_to_file(s.project(), &self.path))??;
        Ok(json!({ "path": self.path }))
    }
}

/// Export a sheet as vector PDF or SVG (by `format`, else by the file extension).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SheetExport {
    /// Sheet id or name.
    pub sheet: Ref,
    /// Path of the file to write (.pdf or .svg).
    pub path: PathBuf,
    /// `pdf` or `svg`.
    #[serde(default)]
    pub format: Option<String>,
}

impl Command for SheetExport {
    const NAME: &'static str = "sheet_export";
    const KIND: CommandKind = CommandKind::File;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::view(s.project(), &self.sheet)?;
        if !matches!(s.project().view(id).map(|v| &v.kind), Some(ViewKind::Sheet { .. })) {
            return Err(EngineError::invalid("not a sheet"));
        }
        let ext = self.path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        let format = self.format.map(|f| f.to_ascii_lowercase()).unwrap_or(ext);
        let sheet = s.sheet(id).ok_or_else(|| EngineError::invalid("the sheet cannot be composed"))?;
        let bytes = match format.as_str() {
            "pdf" => guard(|| cc_views::export::to_pdf(&sheet.drawing, sheet.paper))?,
            "svg" => guard(|| cc_views::export::to_svg(&sheet.drawing, sheet.paper))?.into_bytes(),
            other => return Err(EngineError::invalid(format!("unknown format '{other}': use pdf or svg"))),
        };
        write(&self.path, &bytes)?;
        Ok(json!({ "path": self.path, "format": format }))
    }
}

/// Export any view with a drawing (plan, elevation, section, 3D view as a
/// hidden-line axonometric, legend, drafting view, diagram or sheet) as a
/// PNG image, or as vector SVG or PDF (by `format`, else by the file
/// extension). The view is drawn at its scale as printed, without temporary
/// hiding, on a page that fits it; sheets keep their paper size.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewExport {
    /// View id or name.
    pub view: Ref,
    /// Path of the file to write (.png, .svg or .pdf).
    pub path: PathBuf,
    /// `png`, `svg` or `pdf`.
    #[serde(default)]
    pub format: Option<String>,
    /// Resolution of a PNG, in dots per inch (default 150).
    #[serde(default)]
    pub dpi: Option<f64>,
}

/// Margin around a view's drawing on its page, millimeters.
const VIEW_MARGIN: f64 = 5.0;

impl ViewExport {
    /// The view drawn on paper (millimeters) and the paper size.
    pub fn page(s: &mut Session, view: Ref) -> Result<(cc_views::Drawing2D, glam::DVec2)> {
        let id = refs::view(s.project(), &view)?;
        let kind = s.project().view(id).map(|v| (v.kind.clone(), v.scale));
        match kind {
            Some((ViewKind::Sheet { .. }, _)) => {
                let sheet = s.sheet(id).ok_or_else(|| EngineError::invalid("the sheet cannot be composed"))?;
                Ok((sheet.drawing.clone(), sheet.paper))
            }
            Some((_, scale)) => {
                let drawing =
                    s.printed_drawing(id).ok_or_else(|| EngineError::invalid("the view has no drawing to export"))?;
                Ok(cc_views::export::view_on_paper(&drawing, scale, VIEW_MARGIN))
            }
            None => Err(EngineError::not_found("view")),
        }
    }
}

impl Command for ViewExport {
    const NAME: &'static str = "view_export";
    const KIND: CommandKind = CommandKind::File;
    fn run(self, s: &mut Session) -> Result<Value> {
        let ext = self.path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        let format = self.format.map(|f| f.to_ascii_lowercase()).unwrap_or(ext);
        let dpi = refs::finite(self.dpi.unwrap_or(150.0), "dpi")?;
        let (drawing, paper) = Self::page(s, self.view)?;
        let mut out = json!({ "path": self.path, "format": format, "paper_mm": [paper.x, paper.y] });
        let bytes = match format.as_str() {
            "png" => {
                if dpi <= 0.0 {
                    return Err(EngineError::invalid("dpi must be positive"));
                }
                let png = guard(|| cc_views::export::to_png(&drawing, paper, dpi))?.map_err(EngineError::invalid)?;
                let px = (paper / 25.4 * dpi).round();
                out["pixels"] = json!([px.x, px.y]);
                png
            }
            "pdf" => guard(|| cc_views::export::to_pdf(&drawing, paper))?,
            "svg" => guard(|| cc_views::export::to_svg(&drawing, paper))?.into_bytes(),
            other => return Err(EngineError::invalid(format!("unknown format '{other}': use png, svg or pdf"))),
        };
        write(&self.path, &bytes)?;
        Ok(out)
    }
}

/// Export a schedule view as CSV (UTF-8; semicolons when the decimal comma is used).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleExportCsv {
    /// Schedule view id or name.
    pub view: Ref,
    /// Path of the .csv file to write.
    pub path: PathBuf,
}

impl Command for ScheduleExportCsv {
    const NAME: &'static str = "schedule_export_csv";
    const KIND: CommandKind = CommandKind::File;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::view(s.project(), &self.view)?;
        let table = s.schedule(id).ok_or_else(|| EngineError::invalid("not a schedule view"))?;
        let cell = |c: &cc_views::Cell| match c {
            cc_views::Cell::Text(t) => t.clone(),
            cc_views::Cell::Key(k) => crate::i18n::tr(k),
        };
        // A BOM so spreadsheets read UTF-8 (accents) correctly.
        let csv = format!("\u{feff}{}", cc_views::to_csv(&table, crate::i18n::tr, cell));
        write(&self.path, csv.as_bytes())?;
        Ok(json!({ "path": self.path, "rows": table.rows.len() }))
    }
}

/// Load a component file (.ccomp): its types and missing materials join the project.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentLoad {
    /// Path of the .ccomp file.
    pub path: PathBuf,
}

impl Command for ComponentLoad {
    const NAME: &'static str = "component_load";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let file = guard(|| cc_core::component_file::load(&self.path))??;
        let ids = edit(s, t!("tx-load-component"), |e| {
            let mut ids: Vec<u64> = (0..file.types.len()).map(|_| e.alloc_id()).collect();
            ids.reverse();
            let (types, materials) = cc_core::component_file::types_for(e.project(), &file, || ids.pop().unwrap_or(0));
            e.materials_mut().extend(materials);
            Ok(types.into_iter().map(|t| e.add_type(t).0).collect::<Vec<_>>())
        })?;
        Ok(json!({ "component": file.component, "types": ids }))
    }
}

/// Save a component (all its types and the materials they use) to a .ccomp file.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentSave {
    /// Component name (as in a component type's `params.component`).
    pub component: String,
    /// Path of the .ccomp file to write.
    pub path: PathBuf,
}

impl Command for ComponentSave {
    const NAME: &'static str = "component_save";
    const KIND: CommandKind = CommandKind::File;
    fn run(self, s: &mut Session) -> Result<Value> {
        let file = cc_core::component_file::extract(s.project(), &self.component)?;
        guard(|| cc_core::component_file::save(&file, &self.path))??;
        Ok(json!({ "path": self.path, "types": file.types.len() }))
    }
}
