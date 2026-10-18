//! IFC export and import for ConstructCAD.
//!
//! [`export`] maps a [`cc_core::Project`] to an IFC4 STEP physical file
//! (ISO 10303-21): spatial structure (project, site, building, storeys),
//! walls with openings, doors, windows, slabs, ceilings and roofs as swept
//! solids, element types, material layer sets, surface styles and base
//! quantities. The STEP writer and the mapping are implemented here without
//! third-party IFC libraries. [`import`] reads IFC2X3/IFC4 files back into a
//! project (see the `import` module).

mod export;
pub mod guid;
mod import;
pub mod parse;
pub mod step;

pub use export::{ExportOptions, export, export_to_file};
pub use guid::guid;
pub use import::{ImportError, ImportReport, import, import_file};
