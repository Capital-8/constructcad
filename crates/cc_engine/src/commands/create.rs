//! Creating elements, with the same defaults as the drawing tools.

use super::{add_element, created, edit};
use crate::defaults::{default_roof_base, default_top, next_number, walls_loop};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, Ref, finite, point, points, positive};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{
    Beam, Category, Ceiling, Column, Component, ElementKind, Floor, Grid, Hosted, LevelId, LocationLine, Project,
    Railing, Roof, RoofShape, Room, Stair, TypeData, Wall, WallTop,
};
use cc_geom::polygon;
use glam::DVec2;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

/// A top constraint: attached to a level (with an offset) or a fixed height.
fn top(p: &Project, top_level: Option<&Ref>, top_offset: Option<f64>) -> Result<Option<WallTop>> {
    match top_level {
        Some(r) => Ok(Some(WallTop::Level {
            level: refs::level(p, r)?,
            offset: finite(top_offset.unwrap_or(0.0), "top_offset")?,
        })),
        None => Ok(None),
    }
}

fn create(
    s: &mut Session,
    tx: String,
    level: LevelId,
    category: Category,
    ty: Option<&Ref>,
    kind: ElementKind,
) -> Result<Value> {
    let type_id = refs::type_of(s.project(), category, ty)?;
    let id = edit(s, tx, |e| Ok(add_element(e, level, type_id, kind)))?;
    Ok(created(s, id))
}

/// Create a straight wall from `start` to `end` on a level. Its top is
/// `height` above its base, or attached to `top_level` (plus `top_offset`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WallCreate {
    pub start: Point,
    pub end: Point,
    /// Level id or name (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// Wall type id or name (default: the default wall type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Unconnected height in meters (default 3).
    #[serde(default)]
    pub height: Option<f64>,
    /// Level the top is attached to, instead of a height.
    #[serde(default)]
    pub top_level: Option<Ref>,
    #[serde(default)]
    pub top_offset: Option<f64>,
    /// Base offset from the level, meters.
    #[serde(default)]
    pub base_offset: Option<f64>,
    /// Which line of the wall `start`-`end` is: `Centerline` (default), `CoreCenter`,
    /// `FinishExterior`, `FinishInterior`, `CoreExterior` or `CoreInterior`.
    #[serde(default)]
    pub location_line: Option<String>,
}

impl Command for WallCreate {
    const NAME: &'static str = "wall_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let (start, end) = (point(self.start)?, point(self.end)?);
        if start.distance(end) < 1e-3 {
            return Err(EngineError::invalid("the wall has no length"));
        }
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let location_line = match self.location_line {
            Some(name) => serde_json::from_value::<LocationLine>(Value::String(name.clone()))
                .map_err(|_| EngineError::invalid(format!("unknown location line '{name}'")))?,
            None => LocationLine::default(),
        };
        let wall = Wall {
            start,
            end,
            base_offset: finite(self.base_offset.unwrap_or(0.0), "base_offset")?,
            height: positive(self.height.unwrap_or(3.0), "height")?,
            top: top(p, self.top_level.as_ref(), self.top_offset)?.unwrap_or(WallTop::Unconnected),
            location_line,
            ..Default::default()
        };
        create(s, t!("tx-create-wall"), level, Category::Wall, self.type_ref.as_ref(), ElementKind::Wall(wall))
    }
}

/// Parameters shared by doors and windows.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HostedArgs {
    /// Id of the host wall.
    pub wall: u64,
    /// Distance along the wall from its start to the opening's center, meters.
    #[serde(default)]
    pub offset: Option<f64>,
    /// A point near the wall: the opening is centered at its projection, on its side.
    #[serde(default)]
    pub point: Option<Point>,
    /// Sill height (default: the type's default sill for windows, 0 for doors).
    #[serde(default)]
    pub sill: Option<f64>,
    /// Door or window type id or name (default: the default type).
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Face the other side of the wall.
    #[serde(default)]
    pub flip_facing: Option<bool>,
    /// Hinge on the other jamb.
    #[serde(default)]
    pub flip_hand: Option<bool>,
}

fn place_hosted(s: &mut Session, a: HostedArgs, category: Category) -> Result<Value> {
    let p = s.project();
    let type_id = refs::type_of(p, category, a.type_ref.as_ref())?;
    let host = refs::element(p, a.wall)?;
    let host_el = p.element(host).ok_or_else(|| EngineError::not_found("wall"))?;
    let wall = host_el.as_wall().ok_or_else(|| EngineError::invalid(format!("element {} is not a wall", a.wall)))?;
    let ty = p.element_type(type_id).ok_or_else(|| EngineError::not_found("type"))?;
    let (width, _) = ty.opening_size().ok_or_else(|| EngineError::invalid("not a door or window type"))?;
    let default_sill = match ty.data {
        TypeData::Window { default_sill, .. } => default_sill,
        _ => 0.0,
    };
    let (start, end) = p.wall_axis(host_el, wall);
    let length = start.distance(end);
    let dir = (end - start).try_normalize().unwrap_or(DVec2::X);
    let (u, side) = match (a.offset, a.point) {
        (Some(o), _) => (finite(o, "offset")?, 1.0),
        (None, Some(q)) => {
            let q = point(q)? - start;
            (q.dot(dir), q.dot(dir.perp()))
        }
        (None, None) => (length / 2.0, 1.0),
    };
    let margin = 0.01;
    if length < width + 2.0 * margin {
        return Err(EngineError::rejected("the wall is too short for this opening"));
    }
    let hosted = Hosted {
        host,
        offset: u.clamp(width * 0.5 + margin, length - width * 0.5 - margin),
        sill_height: finite(a.sill.unwrap_or(default_sill), "sill")?.max(0.0),
        flip_facing: a.flip_facing.unwrap_or(side < 0.0),
        flip_hand: a.flip_hand.unwrap_or(false),
    };
    let level = host_el.level;
    let (kind, tx) = if category == Category::Door {
        (ElementKind::Door(hosted), t!("tx-create-door"))
    } else {
        (ElementKind::Window(hosted), t!("tx-create-window"))
    };
    let id = edit(s, tx, |e| Ok(super::add_element(e, level, type_id, kind)))?;
    Ok(created(s, id))
}

/// Place a door in a wall (centered on the wall unless `offset` or `point` is given).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DoorCreate {
    #[serde(flatten)]
    pub args: HostedArgs,
}

impl Command for DoorCreate {
    const NAME: &'static str = "door_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        place_hosted(s, self.args, Category::Door)
    }
}

/// Place a window in a wall (centered on the wall unless `offset` or `point` is given).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WindowCreate {
    #[serde(flatten)]
    pub args: HostedArgs,
}

impl Command for WindowCreate {
    const NAME: &'static str = "window_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        place_hosted(s, self.args, Category::Window)
    }
}

/// Parameters shared by floors, ceilings and roofs.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SketchArgs {
    /// Closed outline (at least 3 points); omit with `from_walls`.
    #[serde(default)]
    pub boundary: Option<Vec<Point>>,
    /// Follow the loop of walls of the level (exterior faces; interior faces for ceilings).
    #[serde(default)]
    pub from_walls: bool,
    #[serde(default)]
    pub level: Option<Ref>,
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Floors: top above the level (default 0). Ceilings: bottom above the level (default 2.6).
    /// Roofs: base above the level (default: top of the tallest wall).
    #[serde(default)]
    pub offset: Option<f64>,
}

fn sketch(s: &Session, a: &SketchArgs, category: Category) -> Result<(LevelId, Vec<DVec2>)> {
    let p = s.project();
    let level = refs::level_or_first(p, a.level.as_ref())?;
    let pts = match (&a.boundary, a.from_walls) {
        (Some(b), _) => points(b)?,
        (None, true) => {
            let outward = if category == Category::Ceiling { -1.0 } else { 1.0 };
            walls_loop(p, level, outward).ok_or_else(|| EngineError::rejected("the walls of the level form no loop"))?
        }
        (None, false) => return Err(EngineError::invalid("give a boundary or from_walls")),
    };
    if pts.len() < 3 || polygon::area(&pts) < 1e-4 {
        return Err(EngineError::invalid("the outline needs at least 3 points and an area"));
    }
    Ok((level, pts))
}

/// Create a floor slab from an outline (or the loop of walls).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FloorCreate {
    #[serde(flatten)]
    pub sketch: SketchArgs,
}

impl Command for FloorCreate {
    const NAME: &'static str = "floor_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (level, boundary) = sketch(s, &self.sketch, Category::Floor)?;
        let floor = Floor { boundary, height_offset: finite(self.sketch.offset.unwrap_or(0.0), "offset")? };
        create(
            s,
            t!("tx-create-floor"),
            level,
            Category::Floor,
            self.sketch.type_ref.as_ref(),
            ElementKind::Floor(floor),
        )
    }
}

/// Create a ceiling from an outline (or the inside of the loop of walls).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CeilingCreate {
    #[serde(flatten)]
    pub sketch: SketchArgs,
}

impl Command for CeilingCreate {
    const NAME: &'static str = "ceiling_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (level, boundary) = sketch(s, &self.sketch, Category::Ceiling)?;
        let ceiling = Ceiling { boundary, height_offset: finite(self.sketch.offset.unwrap_or(2.6), "offset")? };
        let kind = ElementKind::Ceiling(ceiling);
        create(s, t!("tx-create-ceiling"), level, Category::Ceiling, self.sketch.type_ref.as_ref(), kind)
    }
}

/// Create a roof over an outline (or the loop of walls): flat by default; change its
/// shape (shed, gable, hip) with `element_set`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RoofCreate {
    #[serde(flatten)]
    pub sketch: SketchArgs,
    /// Overhang beyond the outline, meters (default 0.5).
    #[serde(default)]
    pub overhang: Option<f64>,
}

impl Command for RoofCreate {
    const NAME: &'static str = "roof_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (level, footprint) = sketch(s, &self.sketch, Category::Roof)?;
        let base_offset = match self.sketch.offset {
            Some(o) => finite(o, "offset")?,
            None => default_roof_base(s.project(), level),
        };
        let overhang = finite(self.overhang.unwrap_or(0.5), "overhang")?.max(0.0);
        let roof = Roof { footprint, base_offset, overhang, shape: RoofShape::Flat };
        create(s, t!("tx-create-roof"), level, Category::Roof, self.sketch.type_ref.as_ref(), ElementKind::Roof(roof))
    }
}

/// Create a room at a point enclosed by room-bounding walls (its boundary follows the walls).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoomCreate {
    pub point: Point,
    #[serde(default)]
    pub level: Option<Ref>,
    /// Default: the room default name.
    #[serde(default)]
    pub name: Option<String>,
    /// Default: the next free number.
    #[serde(default)]
    pub number: Option<String>,
    /// Upper limit above the level (default: the level's height).
    #[serde(default)]
    pub height: Option<f64>,
}

impl Command for RoomCreate {
    const NAME: &'static str = "room_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let position = point(self.point)?;
        let level = refs::level_or_first(s.project(), self.level.as_ref())?;
        let project = s.project().clone();
        if s.geometry().room_at(&project, level, position).is_none() {
            return Err(EngineError::rejected("the point is not enclosed by room-bounding walls"));
        }
        let p = s.project();
        let number = self.number.unwrap_or_else(|| {
            next_number(p.elements.values().filter_map(|e| match &e.kind {
                ElementKind::Room(r) => Some(r.number.as_str()),
                _ => None,
            }))
        });
        let height = positive(self.height.unwrap_or_else(|| p.level_height(level, 3.0)), "height")?;
        let room = Room { position, number, name: self.name.unwrap_or_else(|| t!("room-default-name")), height };
        create(s, t!("tx-create-room"), level, Category::Room, None, ElementKind::Room(room))
    }
}

/// Create a grid line (named with the next free number unless `name` is given).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GridCreate {
    pub start: Point,
    pub end: Point,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub level: Option<Ref>,
}

impl Command for GridCreate {
    const NAME: &'static str = "grid_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (start, end) = (point(self.start)?, point(self.end)?);
        if start.distance(end) < 1e-3 {
            return Err(EngineError::invalid("the grid has no length"));
        }
        let p = s.project();
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let name = self.name.unwrap_or_else(|| {
            next_number(p.elements.values().filter_map(|e| match &e.kind {
                ElementKind::Grid(g) => Some(g.name.as_str()),
                _ => None,
            }))
        });
        create(s, t!("tx-create-grid"), level, Category::Grid, None, ElementKind::Grid(Grid { start, end, name }))
    }
}

/// Place a column; its top goes to the level above (or `height` when there is none).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColumnCreate {
    pub position: Point,
    /// Rotation in plan, degrees.
    #[serde(default)]
    pub rotation: Option<f64>,
    #[serde(default)]
    pub level: Option<Ref>,
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Unconnected height (default: the level's height).
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub top_level: Option<Ref>,
    #[serde(default)]
    pub top_offset: Option<f64>,
}

impl Command for ColumnCreate {
    const NAME: &'static str = "column_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let column = Column {
            position: point(self.position)?,
            rotation_deg: finite(self.rotation.unwrap_or(0.0), "rotation")?,
            base_offset: 0.0,
            height: positive(self.height.unwrap_or_else(|| p.level_height(level, 3.0)), "height")?,
            top: top(p, self.top_level.as_ref(), self.top_offset)?.unwrap_or_else(|| default_top(p, level)),
        };
        create(s, t!("tx-create-column"), level, Category::Column, self.type_ref.as_ref(), ElementKind::Column(column))
    }
}

/// Create a beam from `start` to `end`; its top is `top_offset` above the level
/// (default: the level's height).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BeamCreate {
    pub start: Point,
    pub end: Point,
    #[serde(default)]
    pub level: Option<Ref>,
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    #[serde(default)]
    pub top_offset: Option<f64>,
}

impl Command for BeamCreate {
    const NAME: &'static str = "beam_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (start, end) = (point(self.start)?, point(self.end)?);
        if start.distance(end) < 1e-3 {
            return Err(EngineError::invalid("the beam has no length"));
        }
        let p = s.project();
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let top_offset = finite(self.top_offset.unwrap_or_else(|| p.level_height(level, 3.0)), "top_offset")?;
        let beam = Beam { start, end, top_offset };
        create(s, t!("tx-create-beam"), level, Category::Beam, self.type_ref.as_ref(), ElementKind::Beam(beam))
    }
}

/// Create a straight stair starting at `start` and rising along `direction`
/// (degrees) up to the level above (or `height`). Risers follow the type.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StairCreate {
    pub start: Point,
    /// Direction of the run in plan, degrees counter-clockwise from +X.
    #[serde(default)]
    pub direction: Option<f64>,
    #[serde(default)]
    pub level: Option<Ref>,
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    /// Height when there is no level above (default 3).
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub top_level: Option<Ref>,
}

impl Command for StairCreate {
    const NAME: &'static str = "stair_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let level = refs::level_or_first(p, self.level.as_ref())?;
        let top = top(p, self.top_level.as_ref(), None)?.unwrap_or_else(|| default_top(p, level));
        let height = match top {
            WallTop::Level { level: t, .. } => p.level_elevation(t) - p.level_elevation(level),
            WallTop::Unconnected => self.height.unwrap_or(3.0),
        };
        let stair = Stair {
            start: point(self.start)?,
            direction_deg: finite(self.direction.unwrap_or(0.0), "direction")?,
            base_offset: 0.0,
            height: positive(height, "height")?,
            top,
        };
        create(s, t!("tx-create-stair"), level, Category::Stair, self.type_ref.as_ref(), ElementKind::Stair(stair))
    }
}

/// Create a railing along a path (at least 2 points).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RailingCreate {
    pub path: Vec<Point>,
    #[serde(default)]
    pub level: Option<Ref>,
    #[serde(default, rename = "type")]
    pub type_ref: Option<Ref>,
    #[serde(default)]
    pub base_offset: Option<f64>,
}

impl Command for RailingCreate {
    const NAME: &'static str = "railing_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let mut path = points(&self.path)?;
        path.dedup_by(|a, b| a.distance(*b) < 1e-6);
        if path.len() < 2 {
            return Err(EngineError::invalid("the path needs at least 2 distinct points"));
        }
        let level = refs::level_or_first(s.project(), self.level.as_ref())?;
        let railing = Railing { path, base_offset: finite(self.base_offset.unwrap_or(0.0), "base_offset")? };
        let kind = ElementKind::Railing(railing);
        create(s, t!("tx-create-railing"), level, Category::Railing, self.type_ref.as_ref(), kind)
    }
}

/// Place a component (furniture, fixture…) of a component type.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComponentPlace {
    pub position: Point,
    /// Component type id or name.
    #[serde(rename = "type")]
    pub type_ref: Ref,
    /// Rotation in plan, degrees.
    #[serde(default)]
    pub rotation: Option<f64>,
    #[serde(default)]
    pub level: Option<Ref>,
    #[serde(default)]
    pub base_offset: Option<f64>,
}

impl Command for ComponentPlace {
    const NAME: &'static str = "component_place";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let level = refs::level_or_first(s.project(), self.level.as_ref())?;
        let component = Component {
            position: point(self.position)?,
            rotation_deg: finite(self.rotation.unwrap_or(0.0), "rotation")?,
            base_offset: finite(self.base_offset.unwrap_or(0.0), "base_offset")?,
            overrides: Default::default(),
            properties: Vec::new(),
        };
        let kind = ElementKind::Component(component);
        create(s, t!("tx-create-component"), level, Category::Component, Some(&self.type_ref), kind)
    }
}
