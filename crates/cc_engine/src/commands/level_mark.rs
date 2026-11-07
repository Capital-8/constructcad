//! Level marks: the elevation of a point written in a plan, an elevation or
//! a section (see `cc_core::level_mark`).

use super::{add_element, created, edit};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, Ref, finite, point};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::level_mark::{LevelMark, MAX_LEVEL};
use cc_core::{Category, ElementKind, ViewKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Place a level mark in a plan, an elevation or a section: the finished
/// level of a point above a shelf line and its raw (structural) level under
/// it, each after the type's datum (the value written for the project's
/// zero) is added. Plans draw a circle with two filled quarters on the
/// point and measure the top of the highest floor of the plan's level under
/// it (the level where there is none), with the top of that floor's
/// structure as the raw level; elevations and sections draw a triangle
/// pointing down to the point and measure its height (`position` y), with
/// the raw level of a floor whose top the point lies on, in sections.
/// `element_get` gives the levels and texts (`derived`); `element_set`
/// changes the mark's `params`.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct LevelMarkCreate {
    /// View id or name: a floor or ceiling plan, an elevation or a section.
    pub view: Ref,
    /// The point measured, in the view's drawing coordinates (in elevations
    /// and sections `y` is the elevation).
    pub position: Point,
    /// Level mark type id or name (default: the default level mark type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Write the values on the left of the symbol (default: on the right).
    #[serde(default)]
    pub flip: Option<bool>,
    /// Finished level to write instead of the measured one, meters (an
    /// elevation of the project, before the datum).
    #[serde(default)]
    pub finished: Option<f64>,
    /// Raw level to write instead of the measured one, meters.
    #[serde(default)]
    pub raw: Option<f64>,
    /// Write the raw level when there is one (default true).
    #[serde(default)]
    pub show_raw: Option<bool>,
}

fn level_value(v: Option<f64>, name: &str) -> Result<Option<f64>> {
    let Some(v) = v else { return Ok(None) };
    let v = finite(v, name)?;
    if v.abs() > MAX_LEVEL {
        return Err(EngineError::invalid(format!("{name} must be within ±{MAX_LEVEL} m")));
    }
    Ok(Some(v))
}

impl Command for LevelMarkCreate {
    const NAME: &'static str = "level_mark_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let view_id = refs::view(p, &self.view)?;
        let view = p.view(view_id).ok_or_else(|| EngineError::not_found("view"))?;
        if !matches!(
            view.kind,
            ViewKind::FloorPlan { .. }
                | ViewKind::CeilingPlan { .. }
                | ViewKind::Elevation { .. }
                | ViewKind::Section { .. }
        ) {
            return Err(EngineError::invalid(t!("msg-level-mark-view")));
        }
        let level = view
            .level()
            .or_else(|| p.levels.first().map(|l| l.id))
            .ok_or_else(|| EngineError::rejected("no levels"))?;
        let type_id = refs::type_of(p, Category::LevelMark, self.type_ref.as_ref())?;
        let mark = LevelMark {
            flip: self.flip.unwrap_or(false),
            finished: level_value(self.finished, "finished")?,
            raw: level_value(self.raw, "raw")?,
            show_raw: self.show_raw.unwrap_or(true),
            ..LevelMark::new(view_id, point(self.position)?)
        };
        let kind = ElementKind::LevelMark(Box::new(mark));
        let id = edit(s, t!("tx-create-level-mark"), |e| Ok(add_element(e, level, type_id, kind)))?;
        Ok(created(s, id))
    }
}
