//! Derived geometry for ConstructCAD.
//!
//! [`ModelGeometry::build`] turns a [`cc_core::Project`] into boundary
//! representation [`Solid`]s (planar polygon faces) that feed the 3D renderer,
//! elevations and sections, plus [`WallGeom`] data used by plan views.

mod build;
mod cut;
pub mod polygon;
mod rooms;
mod solid;
mod wall;

pub use build::{FRAME_COLOR, HostedPart, ModelGeometry, hosted_parts};
pub use cut::{clip_polygon, face_plane_segments};
pub use rooms::room_boundary;
pub use solid::{Face, Ray, Solid};
pub use wall::{Column, Opening, WallGeom};

pub use glam::{DVec2, DVec3};
