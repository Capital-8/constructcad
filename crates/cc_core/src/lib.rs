//! Core BIM data model for ConstructCAD.
//!
//! The [`Project`] is the single source of truth. Every view, schedule and
//! sheet is a *query* over it; all geometry is derived. Edits go through
//! [`Document::edit`], which records a [`Transaction`] for undo/redo.

pub mod arc;
pub mod component;
pub mod component_file;
pub mod dim_refs;
mod document;
mod element;
pub mod expr;
pub mod graphics;
mod ids;
pub mod material;
pub mod mep;
mod persist;
pub mod phase;
pub mod profile;
mod project;
pub mod properties;
mod repair;
mod schedule;
pub mod transform;
mod types;
pub mod units;
mod view;
pub mod wall_ops;

pub use component::{ComponentDef, ComponentItem, ComponentKind, ComponentParam, ComponentShape, ParamKind};
pub use dim_refs::{DimRef, Projection, RefAnchor, RefHeight, RefTarget};
pub use document::{Document, EditCtx, Transaction};
pub use element::{
    BarFace, Beam, BeamLateral, BeamVertical, Ceiling, Column, Component, Dimension, DimensionKind, Element,
    ElementKind, Floor, Foundation, FoundationShape, Grid, Hosted, Leader, LeaderSymbol, LocationLine, Railing, Rebar,
    RebarLayout, Roof, RoofShape, Room, Stair, TextNote, Wall, WallTop,
};
pub use graphics::{
    CategoryGraphics, FillGraphics, FilterOp, FilterParam, FilterRule, FilterUse, GraphicOverride, LineGraphics,
    LinePattern, TemplateIncludes, TemporaryVisibility, ViewFilter, ViewGraphics, ViewTemplate,
};
pub use ids::{
    ElementId, ElementTypeId, FilterId, GroupId, LevelId, PhaseFilterId, PhaseId, SystemId, TemplateId, Uid, ViewId,
};
pub use material::{Material, MaterialClass, Pattern, PatternFill};
pub use mep::{DeviceKind, MepData, MepSegment, MepSystem, SystemKind};
pub use persist::{
    FORMAT_VERSION, PersistError, from_ron, from_ron_with_repairs, load, load_with_repairs, save, to_ron,
};
pub use phase::{Phase, PhaseDisplay, PhaseFilter, PhaseGraphics, PhaseState, PhaseStatus};
pub use profile::{ProfilePoint, SketchPlane};
pub use project::{Group, Level, Project, ProjectInfo, can_join_geometry, join_priority};
pub use schedule::{ScheduleDef, ScheduleField};
pub use transform::PlanTransform;
pub use types::{
    Category, ColumnSection, ElementType, FoundationKind, Layer, LayerFunction, SectionShape, TypeData, TypeInfo,
    WallFunction, stair_layout,
};
pub use units::{AreaUnit, LengthUnit, NumberStyle, SlopeUnit, Units, VolumeUnit};
pub use view::{CameraState, Direction, MepDiscipline, SheetSize, View, ViewCategory, ViewKind, Viewport};

pub use glam::{DVec2, DVec3};
