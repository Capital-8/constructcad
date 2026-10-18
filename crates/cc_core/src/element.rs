use crate::{Category, ElementId, ElementTypeId, LevelId, Uid};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A model element instance. Coordinates are in meters; X east, Y north, Z up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: ElementId,
    pub level: LevelId,
    pub type_id: ElementTypeId,
    /// "Mark" instance parameter, shown in schedules and tags.
    #[serde(default)]
    pub mark: String,
    #[serde(default)]
    pub comments: String,
    pub kind: ElementKind,
    /// Persistent unique id (IFC GlobalId and similar).
    #[serde(default)]
    pub uid: Uid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ElementKind {
    Wall(Wall),
    Door(Hosted),
    Window(Hosted),
    Floor(Floor),
    Ceiling(Ceiling),
    Roof(Roof),
}

impl Element {
    pub fn category(&self) -> Category {
        match self.kind {
            ElementKind::Wall(_) => Category::Wall,
            ElementKind::Door(_) => Category::Door,
            ElementKind::Window(_) => Category::Window,
            ElementKind::Floor(_) => Category::Floor,
            ElementKind::Ceiling(_) => Category::Ceiling,
            ElementKind::Roof(_) => Category::Roof,
        }
    }

    pub fn as_wall(&self) -> Option<&Wall> {
        match &self.kind {
            ElementKind::Wall(w) => Some(w),
            _ => None,
        }
    }

    pub fn as_hosted(&self) -> Option<&Hosted> {
        match &self.kind {
            ElementKind::Door(h) | ElementKind::Window(h) => Some(h),
            _ => None,
        }
    }

    pub fn as_hosted_mut(&mut self) -> Option<&mut Hosted> {
        match &mut self.kind {
            ElementKind::Door(h) | ElementKind::Window(h) => Some(h),
            _ => None,
        }
    }

    /// Sketch boundary for floors, ceilings and roofs.
    pub fn sketch(&self) -> Option<&[DVec2]> {
        match &self.kind {
            ElementKind::Floor(f) => Some(&f.boundary),
            ElementKind::Ceiling(c) => Some(&c.boundary),
            ElementKind::Roof(r) => Some(&r.footprint),
            _ => None,
        }
    }

    /// Translate the element in plan. Hosted elements move with their host.
    pub fn translate(&mut self, delta: DVec2) {
        match &mut self.kind {
            ElementKind::Wall(w) => {
                w.start += delta;
                w.end += delta;
            }
            ElementKind::Floor(f) => f.boundary.iter_mut().for_each(|p| *p += delta),
            ElementKind::Ceiling(c) => c.boundary.iter_mut().for_each(|p| *p += delta),
            ElementKind::Roof(r) => r.footprint.iter_mut().for_each(|p| *p += delta),
            ElementKind::Door(_) | ElementKind::Window(_) => {}
        }
    }
}

/// How the top of a wall is defined.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum WallTop {
    /// `height` above the base.
    #[default]
    Unconnected,
    /// At a level's elevation plus `offset`; follows the level when it moves.
    Level { level: LevelId, offset: f64 },
}

/// Which line of the wall the drawn `start`-`end` line follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LocationLine {
    #[default]
    Centerline,
    CoreCenter,
    FinishExterior,
    FinishInterior,
    CoreExterior,
    CoreInterior,
}

impl LocationLine {
    pub const ALL: [LocationLine; 6] = [
        LocationLine::Centerline,
        LocationLine::CoreCenter,
        LocationLine::FinishExterior,
        LocationLine::FinishInterior,
        LocationLine::CoreExterior,
        LocationLine::CoreInterior,
    ];

    pub fn key(self) -> &'static str {
        match self {
            LocationLine::Centerline => "loc-centerline",
            LocationLine::CoreCenter => "loc-core-center",
            LocationLine::FinishExterior => "loc-finish-exterior",
            LocationLine::FinishInterior => "loc-finish-interior",
            LocationLine::CoreExterior => "loc-core-exterior",
            LocationLine::CoreInterior => "loc-core-interior",
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    /// Location line start point (see `location_line`).
    pub start: DVec2,
    pub end: DVec2,
    /// Offset of the wall base from its level elevation.
    pub base_offset: f64,
    /// Unconnected height (used when `top` is [`WallTop::Unconnected`]).
    pub height: f64,
    #[serde(default)]
    pub top: WallTop,
    #[serde(default)]
    pub location_line: LocationLine,
    /// The wall bounds rooms.
    #[serde(default = "yes")]
    pub room_bounding: bool,
    /// The wall is load-bearing.
    #[serde(default)]
    pub structural: bool,
}

impl Default for Wall {
    fn default() -> Self {
        Self {
            start: DVec2::ZERO,
            end: DVec2::X,
            base_offset: 0.0,
            height: 3.0,
            top: WallTop::Unconnected,
            location_line: LocationLine::Centerline,
            room_bounding: true,
            structural: false,
        }
    }
}

impl Wall {
    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }

    /// Unit direction from start to end (X axis if degenerate).
    pub fn direction(&self) -> DVec2 {
        (self.end - self.start).try_normalize().unwrap_or(DVec2::X)
    }

    /// Left-hand normal of the location line.
    pub fn normal(&self) -> DVec2 {
        self.direction().perp()
    }

    pub fn point_at(&self, distance: f64) -> DVec2 {
        self.start + self.direction() * distance
    }

    /// Distance along the wall of the projection of `p`, and the signed
    /// perpendicular distance (positive on the left).
    pub fn project(&self, p: DVec2) -> (f64, f64) {
        let d = p - self.start;
        (d.dot(self.direction()), d.dot(self.normal()))
    }
}

/// A door or window hosted by a wall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hosted {
    pub host: ElementId,
    /// Distance from the host wall start to the opening center, along the wall.
    pub offset: f64,
    /// Sill height above the wall base (0 for doors).
    pub sill_height: f64,
    /// Swing/facing to the wall's right side instead of the left.
    #[serde(default)]
    pub flip_facing: bool,
    /// Hinge at the opposite jamb.
    #[serde(default)]
    pub flip_hand: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Floor {
    /// Boundary sketch (closed polygon, either winding).
    pub boundary: Vec<DVec2>,
    /// Offset of the top face from the level elevation.
    pub height_offset: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ceiling {
    pub boundary: Vec<DVec2>,
    /// Height of the ceiling's underside above the level.
    pub height_offset: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RoofShape {
    Flat,
    /// Single slope rising towards `direction_deg` (0 = +X), slope as rise/run.
    Shed {
        slope: f64,
        direction_deg: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roof {
    pub footprint: Vec<DVec2>,
    /// Offset of the roof base above the level.
    pub base_offset: f64,
    pub overhang: f64,
    pub shape: RoofShape,
}
