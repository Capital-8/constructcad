//! Reading the model: categories, levels, types, elements, views, materials, schedules.

use super::variant;
use crate::error::{EngineError, Result};
use crate::refs::{self, Ref};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use cc_core::{Category, ElementId, ElementKind, ViewCategory, ViewId};
use cc_geom::polygon;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// `FloorPlan` → `floor_plan`.
pub(crate) fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

pub(crate) fn view_category(name: &str) -> Result<ViewCategory> {
    ViewCategory::ALL
        .into_iter()
        .find(|c| snake(&format!("{c:?}")) == name.trim().to_ascii_lowercase())
        .ok_or_else(|| EngineError::invalid(format!("unknown view category '{name}'")))
}

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Element categories with their display names and how many elements and types each has.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CategoriesList {}

impl Command for CategoriesList {
    const NAME: &'static str = "categories_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        Ok(Category::ALL
            .into_iter()
            .map(|c| {
                json!({
                    "category": refs::category_name(c),
                    "label": crate::i18n::tr(c.key()),
                    "elements": p.elements.values().filter(|e| e.category() == c).count(),
                    "types": p.types_of(c).count(),
                })
            })
            .collect())
    }
}

/// Levels from the lowest, with their elevations (meters).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LevelsList {}

impl Command for LevelsList {
    const NAME: &'static str = "levels_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        Ok(s.project().levels.iter().map(|l| json!({"id": l.id.0, "name": l.name, "elevation": l.elevation})).collect())
    }
}

/// Types, optionally of one category.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypesList {
    /// Category name (`wall`, `door`, `component`…).
    #[serde(default)]
    pub category: Option<String>,
}

impl Command for TypesList {
    const NAME: &'static str = "types_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let category = self.category.as_deref().map(refs::category).transpose()?;
        let p = s.project();
        Ok(p.types
            .values()
            .filter(|t| category.is_none_or(|c| t.category() == c))
            .map(|t| {
                json!({
                    "id": t.id.0,
                    "name": t.name,
                    "category": refs::category_name(t.category()),
                    "default": p.default_type(t.category()) == Some(t.id),
                    "in_use": p.elements.values().filter(|e| e.type_id == t.id).count(),
                })
            })
            .collect())
    }
}

pub(crate) fn type_json(s: &Session, id: cc_core::ElementTypeId) -> Option<Value> {
    let t = s.project().element_type(id)?;
    let (_, params) = variant(to_json(&t.data));
    Some(json!({
        "id": t.id.0,
        "name": t.name,
        "category": refs::category_name(t.category()),
        "params": params,
        "info": to_json(&t.info),
    }))
}

/// A type with all its parameters (`params`, editable with `type_set`) and identity data (`info`).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypeGet {
    /// Type id or name.
    #[serde(rename = "type")]
    pub type_ref: Ref,
}

impl Command for TypeGet {
    const NAME: &'static str = "type_get";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element_type(s.project(), &self.type_ref)?;
        type_json(s, id).ok_or_else(|| EngineError::not_found("type"))
    }
}

/// Elements, optionally of one category and/or level, with a short summary each.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementsList {
    /// Category name (`wall`, `door`, `component`…); see `categories_list`.
    #[serde(default)]
    pub category: Option<String>,
    /// Level id or name.
    #[serde(default)]
    pub level: Option<Ref>,
}

impl Command for ElementsList {
    const NAME: &'static str = "elements_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let category = self.category.as_deref().map(refs::category).transpose()?;
        let level = self.level.as_ref().map(|l| refs::level(p, l)).transpose()?;
        Ok(p.elements
            .values()
            .filter(|e| category.is_none_or(|c| e.category() == c) && level.is_none_or(|l| e.level == l))
            .map(|e| {
                json!({
                    "id": e.id.0,
                    "category": refs::category_name(e.category()),
                    "type": p.type_name(e.type_id),
                    "level": p.level_name(e.level),
                    "mark": e.mark,
                })
            })
            .collect())
    }
}

/// Full description of an element: everything `element_set` can change, plus derived values.
pub(crate) fn element_json(s: &mut Session, id: ElementId) -> Option<Value> {
    let e = s.project().element(id)?.clone();
    let (_, params) = variant(to_json(&e.kind));
    let p = s.project();
    let mut derived = serde_json::Map::new();
    match &e.kind {
        ElementKind::Wall(w) => {
            derived.insert("length".into(), json!(w.length()));
            derived.insert("height".into(), json!(p.wall_height(&e, w)));
            derived.insert("thickness".into(), json!(p.thickness_of(&e)));
        }
        ElementKind::Beam(b) => {
            derived.insert("length".into(), json!(b.length()));
        }
        ElementKind::Railing(r) => {
            derived.insert("length".into(), json!(r.length()));
        }
        _ => {}
    }
    let pinned = p.is_pinned(id);
    let level = json!({"id": e.level.0, "name": p.level_name(e.level)});
    let type_ = json!({"id": e.type_id.0, "name": p.type_name(e.type_id)});
    let geom = s.geometry();
    if geom.solids_of(id).next().is_some() {
        derived.insert("volume".into(), json!(geom.volume(id)));
    }
    if let Some(boundary) = geom.rooms.get(&id) {
        derived.insert("area".into(), json!(polygon::area(boundary)));
        derived.insert("perimeter".into(), json!(polygon::perimeter(boundary)));
        derived.insert("boundary".into(), json!(boundary.iter().map(|q| [q.x, q.y]).collect::<Vec<_>>()));
    }
    Some(json!({
        "id": id.0,
        "category": refs::category_name(e.category()),
        "level": level,
        "type": type_,
        "mark": e.mark,
        "comments": e.comments,
        "pinned": pinned,
        "params": params,
        "derived": derived,
    }))
}

/// An element with all its parameters (`params`, editable with `element_set`) and derived values
/// (length, height, thickness, volume, room area and perimeter).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementGet {
    /// Element id.
    pub id: u64,
}

impl Command for ElementGet {
    const NAME: &'static str = "element_get";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::element(s.project(), self.id)?;
        element_json(s, id).ok_or_else(|| EngineError::not_found("element"))
    }
}

pub(crate) fn view_json(s: &Session, id: ViewId) -> Option<Value> {
    let v = s.project().view(id)?;
    let (_, params) = variant(to_json(&v.kind));
    Some(json!({
        "id": v.id.0,
        "name": v.name,
        "category": snake(&format!("{:?}", v.category())),
        "scale": v.scale,
        "params": params,
    }))
}

/// Views (plans, ceiling plans, elevations, sections, 3D, legends, schedules, sheets),
/// optionally of one category.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewsList {
    /// `floor_plan`, `ceiling_plan`, `three_d`, `elevation`, `section`, `legend`, `schedule` or `sheet`.
    #[serde(default)]
    pub category: Option<String>,
}

impl Command for ViewsList {
    const NAME: &'static str = "views_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let category = self.category.as_deref().map(view_category).transpose()?;
        Ok(s.project()
            .views
            .values()
            .filter(|v| category.is_none_or(|c| v.category() == c))
            .map(|v| json!({"id": v.id.0, "name": v.name, "category": snake(&format!("{:?}", v.category()))}))
            .collect())
    }
}

/// A view with all its parameters (`params`, editable with `view_set`).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewGet {
    /// View id or name.
    pub view: Ref,
}

impl Command for ViewGet {
    const NAME: &'static str = "view_get";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::view(s.project(), &self.view)?;
        view_json(s, id).ok_or_else(|| EngineError::not_found("view"))
    }
}

/// Materials with their colors and how many types use each.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialsList {}

impl Command for MaterialsList {
    const NAME: &'static str = "materials_list";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        Ok(p.materials
            .iter()
            .enumerate()
            .map(|(i, m)| {
                json!({
                    "index": i,
                    "name": m.name,
                    "color": m.color,
                    "used_by_types": p.material_users(&m.name).len(),
                })
            })
            .collect())
    }
}

/// A material with all its fields (editable with `material_set`).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialGet {
    /// Material name (or index in `materials_list`).
    pub material: Ref,
}

impl Command for MaterialGet {
    const NAME: &'static str = "material_get";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let name = refs::material(s.project(), &self.material)?;
        s.project().material(&name).map(to_json).ok_or_else(|| EngineError::not_found("material"))
    }
}

/// The table of a schedule view: headers, one row per element (with its id) and totals,
/// formatted with the project units.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScheduleTable {
    /// Schedule view id or name.
    pub view: Ref,
}

impl Command for ScheduleTable {
    const NAME: &'static str = "schedule_table";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let id = refs::view(s.project(), &self.view)?;
        let table = s.schedule(id).ok_or_else(|| EngineError::invalid("not a schedule view"))?;
        let cell = |c: &cc_views::Cell| match c {
            cc_views::Cell::Text(t) => t.clone(),
            cc_views::Cell::Key(k) => crate::i18n::tr(k),
        };
        Ok(json!({
            "headers": table.headers.iter().map(|h| crate::i18n::tr(h)).collect::<Vec<_>>(),
            "rows": table.rows.iter().map(|r| json!({"element": r.element.0, "cells": r.cells.iter().map(cell).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "totals": table.totals,
        }))
    }
}
