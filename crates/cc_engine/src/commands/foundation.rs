//! Foundations: pads under columns, strips (ground beams) under walls and
//! slabs, placed one by one or under existing columns and walls.

use super::create::{SketchArgs, sketch};
use super::{add_element, created, edit, ids_json};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, Ref, finite, point};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{
    Category, ElementId, ElementKind, ElementTypeId, Foundation, FoundationKind, FoundationShape, LevelId, Project,
    TypeData,
};
use glam::DVec2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Points closer than this are the same place (1 cm).
const SAME: f64 = 0.01;

/// The foundation type named by `r` (which must be of `kind`), or the first type of that kind.
fn foundation_type(p: &Project, kind: FoundationKind, r: Option<&Ref>) -> Result<ElementTypeId> {
    let of_kind = |id: ElementTypeId| matches!(p.element_type(id).map(|t| &t.data), Some(TypeData::Foundation { kind: k, .. }) if *k == kind);
    match r {
        Some(r) => {
            let id = refs::type_of(p, Category::Foundation, Some(r))?;
            if of_kind(id) {
                Ok(id)
            } else {
                Err(EngineError::invalid(t!("msg-foundation-kind", "kind" => crate::i18n::tr(kind.key()))))
            }
        }
        None => p
            .types_of(Category::Foundation)
            .map(|t| t.id)
            .find(|id| of_kind(*id))
            .ok_or_else(|| EngineError::rejected(t!("msg-foundation-kind", "kind" => crate::i18n::tr(kind.key())))),
    }
}

fn foundation(shape: FoundationShape, top_offset: Option<f64>) -> Result<ElementKind> {
    Ok(ElementKind::Foundation(Foundation { shape, top_offset: finite(top_offset.unwrap_or(0.0), "top_offset")? }))
}

/// Foundations of `level` with their shapes.
fn foundations_on(p: &Project, level: LevelId) -> Vec<&FoundationShape> {
    p.elements
        .values()
        .filter(|e| e.level == level)
        .filter_map(|e| match &e.kind {
            ElementKind::Foundation(f) => Some(&f.shape),
            _ => None,
        })
        .collect()
}

/// Create foundations in one undo step; returns the new ids and how many were skipped.
fn create_all(
    s: &mut Session,
    type_id: ElementTypeId,
    todo: Vec<(LevelId, ElementKind)>,
    skipped: usize,
) -> Result<Value> {
    if todo.is_empty() {
        return Ok(json!({"created": [], "skipped": skipped}));
    }
    let created = edit(s, t!("tx-create-foundation"), |e| {
        Ok(todo.into_iter().map(|(level, kind)| add_element(e, level, type_id, kind)).collect::<Vec<_>>())
    })?;
    Ok(json!({"created": ids_json(&created), "skipped": skipped}))
}

/// Place a pad footing centered at `position`; its top is `top_offset` above the level (default 0).
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct FootingCreate {
    /// Center of the pad in plan.
    pub position: Point,
    /// Rotation in plan, degrees.
    #[serde(default)]
    pub rotation: Option<f64>,
    /// Level id or name (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// Pad footing type id or name (default: the first pad type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Top above the level, meters (default 0).
    #[serde(default)]
    pub top_offset: Option<f64>,
}

impl Command for FootingCreate {
    const NAME: &'static str = "footing_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let type_id = foundation_type(p, FoundationKind::Pad, self.type_ref.as_ref())?;
        let shape = FoundationShape::Pad {
            position: point(self.position)?,
            rotation_deg: finite(self.rotation.unwrap_or(0.0), "rotation")?,
        };
        let kind = foundation(shape, self.top_offset)?;
        let id = edit(s, t!("tx-create-foundation"), |e| Ok(add_element(e, level, type_id, kind)))?;
        Ok(created(s, id))
    }
}

/// Place a strip footing (a ground beam) centered on the line `start`-`end`;
/// its top is `top_offset` above the level (default 0). Strips meet at
/// corners and tees like beams.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct StripFootingCreate {
    /// Start of the strip's center line.
    pub start: Point,
    /// End of the strip's center line.
    pub end: Point,
    /// Level id or name (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// Strip footing type id or name (default: the first strip type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Top above the level, meters (default 0).
    #[serde(default)]
    pub top_offset: Option<f64>,
}

impl Command for StripFootingCreate {
    const NAME: &'static str = "strip_footing_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (start, end) = (point(self.start)?, point(self.end)?);
        if start.distance(end) < 1e-3 {
            return Err(EngineError::invalid("the strip has no length"));
        }
        let p = s.project();
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let type_id = foundation_type(p, FoundationKind::Strip, self.type_ref.as_ref())?;
        let kind = foundation(FoundationShape::Strip { start, end }, self.top_offset)?;
        let id = edit(s, t!("tx-create-foundation"), |e| Ok(add_element(e, level, type_id, kind)))?;
        Ok(created(s, id))
    }
}

/// Create a foundation slab (a raft) from an outline or under the loop of
/// walls of the level; `offset` is its top above the level (default 0).
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
pub struct FoundationSlabCreate {
    #[serde(flatten)]
    pub sketch: SketchArgs,
}

impl Command for FoundationSlabCreate {
    const NAME: &'static str = "foundation_slab_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (level, boundary) = sketch(s, &self.sketch, Category::Floor)?;
        let type_id = foundation_type(s.project(), FoundationKind::Slab, self.sketch.type_ref.as_ref())?;
        let kind = foundation(FoundationShape::Slab { boundary }, self.sketch.offset)?;
        let id = edit(s, t!("tx-create-foundation"), |e| Ok(add_element(e, level, type_id, kind)))?;
        Ok(created(s, id))
    }
}

/// Place a pad footing under each column (default: every column of the
/// level), centered and turned with it, its top at the column's base.
/// Columns that already stand on a pad are skipped.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct FootingsUnderColumns {
    /// Column ids (default: every column of `level`).
    #[serde(default)]
    pub columns: Vec<u64>,
    /// Level id or name, when no columns are given (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// Pad footing type id or name (default: the first pad type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
}

impl Command for FootingsUnderColumns {
    const NAME: &'static str = "footings_under_columns";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let type_id = foundation_type(p, FoundationKind::Pad, self.type_ref.as_ref())?;
        let ids: Vec<ElementId> = if self.columns.is_empty() {
            let level = refs::level_or_first(p, self.level.as_ref())?;
            p.elements.values().filter(|e| e.level == level && e.category() == Category::Column).map(|e| e.id).collect()
        } else {
            refs::elements(p, &self.columns)?
        };
        let (mut todo, mut skipped) = (Vec::new(), 0);
        for id in ids {
            let Some(e) = p.element(id) else { continue };
            let ElementKind::Column(c) = &e.kind else {
                return Err(EngineError::invalid(format!("element {} is not a column", id.0)));
            };
            let taken = |shapes: Vec<&FoundationShape>| {
                shapes
                    .iter()
                    .any(|f| matches!(f, FoundationShape::Pad { position, .. } if position.distance(c.position) < SAME))
            };
            let planned = todo.iter().any(|(l, k): &(LevelId, ElementKind)| {
                *l == e.level
                    && matches!(k, ElementKind::Foundation(f) if f.shape.points().first().is_some_and(|q| q.distance(c.position) < SAME))
            });
            if planned || taken(foundations_on(p, e.level)) {
                skipped += 1;
                continue;
            }
            let shape = FoundationShape::Pad { position: c.position, rotation_deg: c.rotation_deg };
            todo.push((e.level, foundation(shape, Some(c.base_offset))?));
        }
        create_all(s, type_id, todo, skipped)
    }
}

/// Place a strip footing under each wall (default: every wall of the level),
/// along its body's centerline, its top at the wall's base. Walls that
/// already have a strip are skipped; strips meet at corners like beams.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct StripFootingsUnderWalls {
    /// Wall ids (default: every wall of `level`).
    #[serde(default)]
    pub walls: Vec<u64>,
    /// Level id or name, when no walls are given (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// Strip footing type id or name (default: the first strip type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
}

impl Command for StripFootingsUnderWalls {
    const NAME: &'static str = "strip_footings_under_walls";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let type_id = foundation_type(p, FoundationKind::Strip, self.type_ref.as_ref())?;
        let ids: Vec<ElementId> = if self.walls.is_empty() {
            let level = refs::level_or_first(p, self.level.as_ref())?;
            p.walls().filter(|(e, _)| e.level == level).map(|(e, _)| e.id).collect()
        } else {
            refs::elements(p, &self.walls)?
        };
        let same = |a: (DVec2, DVec2), b: (DVec2, DVec2)| {
            (a.0.distance(b.0) < SAME && a.1.distance(b.1) < SAME)
                || (a.0.distance(b.1) < SAME && a.1.distance(b.0) < SAME)
        };
        let (mut todo, mut skipped) = (Vec::new(), 0);
        let mut planned: Vec<(LevelId, (DVec2, DVec2))> = Vec::new();
        for id in ids {
            let Some(e) = p.element(id) else { continue };
            let ElementKind::Wall(w) = &e.kind else {
                return Err(EngineError::invalid(format!("element {} is not a wall", id.0)));
            };
            // Strips are straight: arc walls get none.
            if w.arc().is_some() {
                skipped += 1;
                continue;
            }
            let line = p.wall_axis(e, w);
            let existing = foundations_on(p, e.level).into_iter().any(|f| match f {
                FoundationShape::Strip { start, end } => same((*start, *end), line),
                _ => false,
            });
            if existing || planned.iter().any(|(l, x)| *l == e.level && same(*x, line)) {
                skipped += 1;
                continue;
            }
            planned.push((e.level, line));
            let shape = FoundationShape::Strip { start: line.0, end: line.1 };
            todo.push((e.level, foundation(shape, Some(w.base_offset))?));
        }
        create_all(s, type_id, todo, skipped)
    }
}
