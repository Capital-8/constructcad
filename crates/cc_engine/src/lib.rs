//! The ConstructCAD engine: every user-facing operation as a named command
//! that runs on a [`Session`] without a GUI.
//!
//! A command is a struct of typed parameters (deserialized from JSON, with a
//! JSON Schema generated from the same type) and a `run` method. The
//! [`catalog`] lists them; [`Session::execute`] runs one by name. The
//! command line (`ccad`), the MCP server and, area by area, the desktop app
//! all go through this one catalog, so an operation behaves the same
//! wherever it is called from.
//!
//! Lengths are in meters, angles in degrees, points are `[x, y]` in plan.
//! Edits go through `Document::edit`, so each command is one undo step.
//! Levels, types, views and materials can be named by id or by name;
//! elements by id.

mod defaults;
pub mod dimensions;
mod error;
pub mod i18n;
mod refs;
mod registry;
mod session;

pub mod commands;

pub use defaults::{default_roof_base, default_top, next_number, walls_loop};
pub use error::{EngineError, ErrorCode, Result};
pub use refs::Ref;
pub use registry::{Command, CommandInfo, CommandKind, catalog, describe};
pub use session::Session;
