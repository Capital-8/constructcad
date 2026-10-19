//! Views and sheets: create, change, duplicate, delete, place on sheets.

use super::query::view_json;
use super::{edit, patch_variant};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, Ref, point, positive};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{CameraState, Direction, Project, SheetSize, View, ViewCategory, ViewId, ViewKind, Viewport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn view_out(s: &Session, id: ViewId) -> Value {
    view_json(s, id).unwrap_or(json!({ "id": id.0 }))
}

/// Create a view. `kind` is `floor_plan` or `ceiling_plan` (with `level`),
/// `section` (with `start`, `end` and optional `depth`), `elevation` (with
/// `direction`: north, south, east or west), `three_d`, `legend` (all types),
/// `schedule` (with `category`) or `sheet` (A3, numbered after the others).
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ViewCreate {
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub level: Option<Ref>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub start: Option<Point>,
    #[serde(default)]
    pub end: Option<Point>,
    /// Section depth beyond the cut line, meters (default 8).
    #[serde(default)]
    pub depth: Option<f64>,
    #[serde(default)]
    pub direction: Option<String>,
    /// Scale denominator (e.g. 50 for 1:50).
    #[serde(default)]
    pub scale: Option<u32>,
}

fn count(p: &Project, c: ViewCategory) -> usize {
    p.views_of(c).count()
}

impl Command for ViewCreate {
    const NAME: &'static str = "view_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let (name, scale, kind, tx) = match self.kind.as_str() {
            "floor_plan" | "ceiling_plan" => {
                let level = refs::level_or_first(p, self.level.as_ref())?;
                let kind = if self.kind == "floor_plan" {
                    ViewKind::FloorPlan { level, cut_height: 1.2 }
                } else {
                    ViewKind::CeilingPlan { level, cut_height: 1.2 }
                };
                (p.level_name(level).to_string(), 50, kind, t!("tx-new-view"))
            }
            "section" => {
                let (Some(a), Some(b)) = (self.start, self.end) else {
                    return Err(EngineError::invalid("a section needs start and end"));
                };
                let (start, end) = (point(a)?, point(b)?);
                if start.distance(end) < 1e-3 {
                    return Err(EngineError::invalid("the section line has no length"));
                }
                let depth = positive(self.depth.unwrap_or(8.0), "depth")?;
                let n = count(p, ViewCategory::Section) + 1;
                let name = t!("default-section-name", "n" => n);
                (name, 50, ViewKind::Section { start, end, depth }, t!("tx-create-section"))
            }
            "elevation" => {
                let d = self.direction.as_deref().unwrap_or("south");
                let direction: Direction = serde_json::from_value(Value::String(capitalized(d)))
                    .map_err(|_| EngineError::invalid(format!("unknown direction '{d}'")))?;
                let key = match direction {
                    Direction::North => "tpl-elev-north",
                    Direction::South => "tpl-elev-south",
                    Direction::East => "tpl-elev-east",
                    Direction::West => "tpl-elev-west",
                };
                (crate::i18n::tr(key), 100, ViewKind::Elevation { direction }, t!("tx-new-view"))
            }
            "three_d" => {
                let n = count(p, ViewCategory::ThreeD) + 1;
                (
                    t!("default-3d-name", "n" => n),
                    100,
                    ViewKind::ThreeD { camera: CameraState::default() },
                    t!("tx-new-view"),
                )
            }
            "legend" => {
                let types = p.types.keys().copied().collect();
                let n = count(p, ViewCategory::Legend) + 1;
                (t!("default-legend-name", "n" => n), 50, ViewKind::Legend { types }, t!("tx-new-view"))
            }
            "schedule" => {
                let category = refs::category(self.category.as_deref().unwrap_or("wall"))?;
                if !cc_core::Category::SCHEDULABLE.contains(&category) {
                    return Err(EngineError::invalid("this category has no schedules"));
                }
                let name = t!("default-schedule-name", "category" => crate::i18n::tr(category.key()));
                (name, 1, ViewKind::Schedule { category, def: Default::default() }, t!("tx-new-view"))
            }
            "sheet" => {
                let n = count(p, ViewCategory::Sheet) + 101;
                let kind = ViewKind::Sheet { number: format!("A{n}"), size: SheetSize::A3, viewports: Vec::new() };
                (t!("default-sheet-name"), 1, kind, t!("tx-new-sheet"))
            }
            other => return Err(EngineError::invalid(format!("unknown view kind '{other}'"))),
        };
        let name = self.name.unwrap_or(name);
        let scale = self.scale.unwrap_or(scale).max(1);
        let id = edit(s, tx, |e| {
            let id = ViewId(e.alloc_id());
            Ok(e.add_view(View { id, name, scale, kind }))
        })?;
        Ok(view_out(s, id))
    }
}

fn capitalized(s: &str) -> String {
    let lower = s.trim().to_ascii_lowercase();
    let mut c = lower.chars();
    c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
}

/// Change a view: name, scale and its `params` as `view_get` shows them
/// (merged: cut height, section depth, legend types, schedule fields, sort,
/// filter and totals, sheet number and size, viewport positions…).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewSet {
    pub view: Ref,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub scale: Option<u32>,
    #[serde(default)]
    pub params: Option<Value>,
}

impl Command for ViewSet {
    const NAME: &'static str = "view_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let id = refs::view(p, &self.view)?;
        let view = p.view(id).ok_or_else(|| EngineError::not_found("view"))?.clone();
        let kind = self.params.as_ref().map(|patch| patch_variant(&view.kind, patch)).transpose()?;
        if kind.as_ref().is_some_and(|k| std::mem::discriminant(k) != std::mem::discriminant(&view.kind)) {
            return Err(EngineError::invalid("params cannot change the kind of view"));
        }
        if let Some(n) = &self.name
            && n.trim().is_empty()
        {
            return Err(EngineError::invalid("the name is empty"));
        }
        edit(s, t!("tx-modify-view"), |e| {
            let v = e.view_mut(id).ok_or_else(|| EngineError::not_found("view"))?;
            if let Some(n) = self.name {
                v.name = n.trim().to_string();
            }
            if let Some(sc) = self.scale {
                v.scale = sc.max(1);
            }
            if let Some(k) = kind {
                v.kind = k;
            }
            Ok(())
        })?;
        Ok(view_out(s, id))
    }
}

/// Copy a view (a copied sheet starts without viewports).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewDuplicate {
    pub view: Ref,
}

impl Command for ViewDuplicate {
    const NAME: &'static str = "view_duplicate";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let id = refs::view(p, &self.view)?;
        let mut v = p.view(id).cloned().ok_or_else(|| EngineError::not_found("view"))?;
        v.name = t!("copy-of", "name" => v.name);
        if let ViewKind::Sheet { viewports, number, .. } = &mut v.kind {
            // A view can be placed on only one sheet.
            viewports.clear();
            number.push('a');
        }
        let new = edit(s, t!("tx-duplicate-view"), |e| {
            v.id = ViewId(e.alloc_id());
            Ok(e.add_view(v))
        })?;
        Ok(view_out(s, new))
    }
}

/// Delete a view (and its viewports on sheets; dimensions drawn in it go too).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewDelete {
    pub view: Ref,
}

impl Command for ViewDelete {
    const NAME: &'static str = "view_delete";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::view(s.project(), &self.view)?;
        edit(s, t!("tx-delete-view"), |e| {
            e.remove_view(id);
            Ok(())
        })?;
        Ok(json!({ "deleted": id.0 }))
    }
}

/// The sheet that shows `view`, if any.
pub fn sheet_of(project: &Project, view: ViewId) -> Option<ViewId> {
    project.views.values().find_map(|v| match &v.kind {
        ViewKind::Sheet { viewports, .. } if viewports.iter().any(|vp| vp.view == view) => Some(v.id),
        _ => None,
    })
}

fn sheet(p: &Project, r: &Ref) -> Result<ViewId> {
    let id = refs::view(p, r)?;
    match p.view(id).map(|v| &v.kind) {
        Some(ViewKind::Sheet { .. }) => Ok(id),
        _ => Err(EngineError::invalid("not a sheet")),
    }
}

/// Place a view on a sheet at `center` (paper millimeters; default left of
/// center). A view can be on only one sheet; sheets cannot be placed.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SheetPlaceView {
    pub sheet: Ref,
    pub view: Ref,
    #[serde(default)]
    pub center: Option<Point>,
}

impl Command for SheetPlaceView {
    const NAME: &'static str = "sheet_place_view";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let sheet_id = sheet(p, &self.sheet)?;
        let view = refs::view(p, &self.view)?;
        if !p.view(view).is_some_and(|v| v.placeable()) {
            return Err(EngineError::invalid("this view cannot be placed on a sheet"));
        }
        if sheet_of(p, view).is_some() {
            return Err(EngineError::rejected(t!("msg-view-already-placed")));
        }
        let center = self.center.map(point).transpose()?;
        edit(s, t!("tx-place-view"), |e| {
            if let Some(ViewKind::Sheet { viewports, size, .. }) = e.view_mut(sheet_id).map(|v| &mut v.kind) {
                let center = center.unwrap_or(size.size_mm() * glam::DVec2::new(0.4, 0.55));
                viewports.push(Viewport { view, center });
            }
            Ok(())
        })?;
        Ok(view_out(s, sheet_id))
    }
}

/// Remove a view from a sheet.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SheetRemoveViewport {
    pub sheet: Ref,
    pub view: Ref,
}

impl Command for SheetRemoveViewport {
    const NAME: &'static str = "sheet_remove_viewport";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let sheet_id = sheet(p, &self.sheet)?;
        let view = refs::view(p, &self.view)?;
        edit(s, t!("tx-remove-viewport"), |e| match e.view_mut(sheet_id).map(|v| &mut v.kind) {
            Some(ViewKind::Sheet { viewports, .. }) if viewports.iter().any(|vp| vp.view == view) => {
                viewports.retain(|vp| vp.view != view);
                Ok(())
            }
            _ => Err(EngineError::not_found("the view is not on this sheet")),
        })?;
        Ok(view_out(s, sheet_id))
    }
}
