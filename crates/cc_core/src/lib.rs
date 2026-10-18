//! Core BIM data model for ConstructCAD.
//!
//! The [`Project`] is the single source of truth. Every view, schedule and
//! sheet is a *query* over it; all geometry is derived. Edits go through
//! [`Document::edit`], which records a [`Transaction`] for undo/redo.

mod document;
mod element;
mod ids;
mod persist;
mod project;
mod repair;
mod types;
mod view;

pub use document::{Document, EditCtx, Transaction};
pub use element::{
    Ceiling, Dimension, Element, ElementKind, Floor, Grid, Hosted, LocationLine, Roof, RoofShape, Room, Wall, WallTop,
};
pub use ids::{ElementId, ElementTypeId, LevelId, Uid, ViewId};
pub use persist::{
    FORMAT_VERSION, PersistError, from_ron, from_ron_with_repairs, load, load_with_repairs, save, to_ron,
};
pub use project::{Level, Material, Project, ProjectInfo};
pub use types::{Category, ElementType, Layer, LayerFunction, TypeData, TypeInfo, WallFunction};
pub use view::{CameraState, Direction, SheetSize, View, ViewCategory, ViewKind, Viewport};

pub use glam::{DVec2, DVec3};
