//! IFC export for ConstructCAD.
//!
//! [`export`] maps a [`cc_core::Project`] to an IFC4 STEP physical file
//! (ISO 10303-21): spatial structure (project, site, building, storeys),
//! walls with openings, doors, windows, slabs, ceilings and roofs as swept
//! solids, element types, material layer sets, surface styles and base
//! quantities. The STEP writer and the mapping are implemented here without
//! third-party IFC libraries.

mod export;
mod guid;
pub mod step;

pub use export::{ExportOptions, export, export_to_file};
pub use guid::guid;
