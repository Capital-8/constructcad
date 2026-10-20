//! Derived geometry for ConstructCAD.
//!
//! [`ModelGeometry::build`] turns a [`cc_core::Project`] into boundary
//! representation [`Solid`]s (planar polygon faces) that feed the 3D renderer,
//! elevations and sections, plus [`WallGeom`] and [`BeamGeom`] data used by plan views.

pub mod analytical;
mod beam;
mod build;
pub mod csg;
mod cut;
mod mesh;
pub mod polygon;
pub mod rebar;
pub mod roofs;
mod rooms;
pub mod section;
mod solid;
mod wall;

pub use beam::{BeamEnd, BeamGeom};
pub use build::{
    BoxPart, FRAME_COLOR, FoundationGeom, HostedPart, ModelGeometry, StairGeom, column_outline, component_parts,
    hosted_parts, railing_parts, stair_geom,
};
pub use cut::{clip_polygon, face_plane_segments};
pub use mesh::{MeshPlan, mesh_plan, plan_edges};
pub use rooms::room_boundary;
pub use solid::{Face, Ray, Solid};
pub use wall::{Column, Opening, WallGeom};

pub use glam::{DVec2, DVec3};
