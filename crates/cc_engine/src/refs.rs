//! Naming model objects in commands: levels, types, views and materials by
//! numeric id or by name; elements by id.

use crate::error::{EngineError, Result};
use cc_core::{Category, ElementId, ElementTypeId, LevelId, Project, ViewId};
use glam::DVec2;
use schemars::JsonSchema;
use serde::Deserialize;

/// A level, type, view or material: its numeric id or its name.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Ref {
    Id(u64),
    Name(String),
}

/// A point in plan, `[x, y]` in meters.
pub type Point = [f64; 2];

pub fn point(p: Point) -> Result<DVec2> {
    let v = DVec2::from_array(p);
    v.is_finite().then_some(v).ok_or_else(|| EngineError::invalid("points must be finite numbers"))
}

pub fn points(ps: &[Point]) -> Result<Vec<DVec2>> {
    ps.iter().map(|p| point(*p)).collect()
}

/// A finite number.
pub fn finite(v: f64, what: &str) -> Result<f64> {
    v.is_finite().then_some(v).ok_or_else(|| EngineError::invalid(format!("{what} must be a finite number")))
}

/// A positive finite number.
pub fn positive(v: f64, what: &str) -> Result<f64> {
    (v.is_finite() && v > 0.0).then_some(v).ok_or_else(|| EngineError::invalid(format!("{what} must be positive")))
}

fn matches(name: &str, wanted: &str) -> bool {
    name.trim().eq_ignore_ascii_case(wanted.trim())
}

/// One match for a name, or an error naming the ambiguity.
fn unique<T: Copy>(what: &str, wanted: &str, found: Vec<T>) -> Result<T> {
    match found.as_slice() {
        [one] => Ok(*one),
        [] => Err(EngineError::not_found(format!("no {what} named '{wanted}'"))),
        _ => Err(EngineError::invalid(format!("several {what}s are named '{wanted}'; use the id"))),
    }
}

pub fn level(p: &Project, r: &Ref) -> Result<LevelId> {
    match r {
        Ref::Id(id) => {
            p.level(LevelId(*id)).map(|l| l.id).ok_or_else(|| EngineError::not_found(format!("no level with id {id}")))
        }
        Ref::Name(n) => unique("level", n, p.levels.iter().filter(|l| matches(&l.name, n)).map(|l| l.id).collect()),
    }
}

/// The given level, else the lowest one.
pub fn level_or_first(p: &Project, r: Option<&Ref>) -> Result<LevelId> {
    match r {
        Some(r) => level(p, r),
        None => p.levels.first().map(|l| l.id).ok_or_else(|| EngineError::rejected("the project has no levels")),
    }
}

pub fn element_type(p: &Project, r: &Ref) -> Result<ElementTypeId> {
    match r {
        Ref::Id(id) => p
            .element_type(ElementTypeId(*id))
            .map(|t| t.id)
            .ok_or_else(|| EngineError::not_found(format!("no type with id {id}"))),
        Ref::Name(n) => unique("type", n, p.types.values().filter(|t| matches(&t.name, n)).map(|t| t.id).collect()),
    }
}

/// A type of `category`: the given one (checked), else the category's default.
pub fn type_of(p: &Project, category: Category, r: Option<&Ref>) -> Result<ElementTypeId> {
    let id = match r {
        Some(Ref::Name(n)) => {
            unique("type", n, p.types_of(category).filter(|t| matches(&t.name, n)).map(|t| t.id).collect())?
        }
        Some(r) => element_type(p, r)?,
        None => p
            .default_type(category)
            .ok_or_else(|| EngineError::rejected(format!("the project has no {category:?} type")))?,
    };
    match p.element_type(id) {
        Some(t) if t.category() == category => Ok(id),
        _ => Err(EngineError::invalid(format!("type {} is not a {category:?} type", id.0))),
    }
}

pub fn view(p: &Project, r: &Ref) -> Result<ViewId> {
    match r {
        Ref::Id(id) => {
            p.view(ViewId(*id)).map(|v| v.id).ok_or_else(|| EngineError::not_found(format!("no view with id {id}")))
        }
        Ref::Name(n) => unique("view", n, p.views.values().filter(|v| matches(&v.name, n)).map(|v| v.id).collect()),
    }
}

/// A material's name (materials are keyed by name; an index also works).
pub fn material(p: &Project, r: &Ref) -> Result<String> {
    match r {
        Ref::Id(i) => usize::try_from(*i)
            .ok()
            .and_then(|i| p.materials.get(i))
            .map(|m| m.name.clone())
            .ok_or_else(|| EngineError::not_found(format!("no material at index {i}"))),
        Ref::Name(n) => p
            .materials
            .iter()
            .find(|m| m.name == *n)
            .or_else(|| p.materials.iter().find(|m| matches(&m.name, n)))
            .map(|m| m.name.clone())
            .ok_or_else(|| EngineError::not_found(format!("no material named '{n}'"))),
    }
}

pub fn element(p: &Project, id: u64) -> Result<ElementId> {
    p.element(ElementId(id)).map(|e| e.id).ok_or_else(|| EngineError::not_found(format!("no element with id {id}")))
}

pub fn elements(p: &Project, ids: &[u64]) -> Result<Vec<ElementId>> {
    if ids.is_empty() {
        return Err(EngineError::invalid("no elements given"));
    }
    ids.iter().map(|id| element(p, *id)).collect()
}

/// A category by its English name (`wall`, `door`…), case-insensitive.
pub fn category(name: &str) -> Result<Category> {
    Category::ALL
        .into_iter()
        .find(|c| format!("{c:?}").eq_ignore_ascii_case(name.trim()))
        .ok_or_else(|| EngineError::invalid(format!("unknown category '{name}'")))
}

pub fn category_name(c: Category) -> String {
    format!("{c:?}").to_lowercase()
}
