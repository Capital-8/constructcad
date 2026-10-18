//! View generation for ConstructCAD.
//!
//! Each [`cc_core::View`] is a query over the model. [`generate`] produces a
//! resolution-independent [`Drawing2D`] for graphical views; [`schedule`]
//! computes tables.

mod drawing;
pub mod format;
mod legend;
mod plan;
mod projected;
mod schedule;

pub use drawing::{Drawing2D, LineStyle, Prim};
pub use legend::legend;
pub use plan::plan;
pub use projected::{Frame, axonometric, elevation, section};
pub use schedule::{Row, Table, schedule};

use cc_core::{Project, View, ViewKind};
use cc_geom::ModelGeometry;

/// Drawing for any graphical view (`None` for schedules and sheets).
pub fn generate(project: &Project, geom: &ModelGeometry, view: &View) -> Option<Drawing2D> {
    Some(match &view.kind {
        ViewKind::FloorPlan { level, cut_height } => plan(project, geom, *level, *cut_height, false),
        ViewKind::CeilingPlan { level, cut_height } => plan(project, geom, *level, *cut_height, true),
        ViewKind::Elevation { direction } => elevation(project, geom, *direction),
        ViewKind::Section { start, end, depth } => section(project, geom, *start, *end, *depth),
        ViewKind::ThreeD { camera } => axonometric(project, geom, camera),
        ViewKind::Legend { types } => legend(project, types, view.scale),
        ViewKind::Schedule { .. } | ViewKind::Sheet { .. } => return None,
    })
}
