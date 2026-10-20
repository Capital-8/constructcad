use crate::{Category, ElementId, ElementTypeId, LevelId, Uid, ViewId};
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
    Room(Room),
    Grid(Grid),
    Dimension(Box<Dimension>),
    Text(Box<TextNote>),
    Column(Column),
    Beam(Beam),
    Stair(Stair),
    Railing(Railing),
    Component(Component),
    Foundation(Foundation),
    Rebar(Rebar),
    Pipe(crate::MepSegment),
    Conduit(crate::MepSegment),
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
            ElementKind::Room(_) => Category::Room,
            ElementKind::Grid(_) => Category::Grid,
            ElementKind::Dimension(_) => Category::Dimension,
            ElementKind::Text(_) => Category::Text,
            ElementKind::Column(_) => Category::Column,
            ElementKind::Beam(_) => Category::Beam,
            ElementKind::Stair(_) => Category::Stair,
            ElementKind::Railing(_) => Category::Railing,
            ElementKind::Component(_) => Category::Component,
            ElementKind::Foundation(_) => Category::Foundation,
            ElementKind::Rebar(_) => Category::Rebar,
            ElementKind::Pipe(_) => Category::Pipe,
            ElementKind::Conduit(_) => Category::Conduit,
        }
    }

    /// The pipe or conduit run of MEP segments.
    pub fn as_segment(&self) -> Option<&crate::MepSegment> {
        match &self.kind {
            ElementKind::Pipe(s) | ElementKind::Conduit(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_segment_mut(&mut self) -> Option<&mut crate::MepSegment> {
        match &mut self.kind {
            ElementKind::Pipe(s) | ElementKind::Conduit(s) => Some(s),
            _ => None,
        }
    }

    /// Systems (or circuits) the element belongs to.
    pub fn systems(&self) -> Vec<crate::SystemId> {
        match &self.kind {
            ElementKind::Pipe(s) | ElementKind::Conduit(s) => s.systems.clone(),
            ElementKind::Component(c) => c.system.into_iter().collect(),
            _ => Vec::new(),
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
            ElementKind::Room(r) => r.position += delta,
            ElementKind::Grid(g) => {
                g.start += delta;
                g.end += delta;
            }
            ElementKind::Dimension(d) => {
                d.a += delta;
                d.b += delta;
                if let DimensionKind::Angular { vertex, .. } = &mut d.kind {
                    *vertex += delta;
                }
            }
            ElementKind::Text(t) => {
                t.position += delta;
                for l in &mut t.leaders {
                    l.end += delta;
                    if let Some(e) = &mut l.elbow {
                        *e += delta;
                    }
                }
            }
            ElementKind::Column(c) => c.position += delta,
            ElementKind::Beam(b) => {
                b.start += delta;
                b.end += delta;
            }
            ElementKind::Stair(s) => s.start += delta,
            ElementKind::Railing(r) => r.path.iter_mut().for_each(|p| *p += delta),
            ElementKind::Component(c) => c.position += delta,
            ElementKind::Pipe(s) | ElementKind::Conduit(s) => {
                s.start += delta;
                s.end += delta;
            }
            ElementKind::Foundation(f) => f.shape.points_mut().into_iter().for_each(|p| *p += delta),
            // Placed by their hosts.
            ElementKind::Door(_) | ElementKind::Window(_) | ElementKind::Rebar(_) => {}
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
    /// The start (end) may join other walls; when off it stays square.
    #[serde(default = "yes")]
    pub join_start: bool,
    #[serde(default = "yes")]
    pub join_end: bool,
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
            join_start: true,
            join_end: true,
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
    /// Two slopes meeting at a ridge along `ridge_deg`; the eaves parallel to
    /// the ridge rise, the others become gable ends.
    Gable {
        slope: f64,
        ridge_deg: f64,
    },
    /// Every eave rises with the same slope (hips and, on long plans, a ridge).
    Hip {
        slope: f64,
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

/// A room: the space enclosed by room-bounding walls around `position`.
/// Its boundary is derived from the walls (see `cc_geom`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Room {
    /// A point inside the room; also where its tag is drawn.
    pub position: DVec2,
    pub number: String,
    pub name: String,
    /// Height of the room above its level (upper limit).
    pub height: f64,
}

/// A named grid line (datum), shown in every plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub start: DVec2,
    pub end: DVec2,
    pub name: String,
}

/// A text note in one view, in that view's drawing coordinates (like
/// dimensions), optionally with leaders pointing at what it describes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextNote {
    pub view: ViewId,
    /// Insertion point: the top left corner of the text, before rotation.
    pub position: DVec2,
    /// The text; line breaks start new lines.
    pub text: String,
    /// Wrapping width in paper millimeters for multiline text; `None` for
    /// single-line text (only explicit line breaks break it).
    #[serde(default)]
    pub width: Option<f64>,
    /// Rotation in degrees counter-clockwise.
    #[serde(default)]
    pub rotation_deg: f64,
    #[serde(default)]
    pub leaders: Vec<Leader>,
}

/// A leader line from a text to what it points at: one segment, or two
/// when it has an elbow (text → elbow → end).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Leader {
    /// The point it points at, where the symbol is drawn.
    pub end: DVec2,
    /// Bend point splitting the leader in two segments.
    #[serde(default)]
    pub elbow: Option<DVec2>,
    #[serde(default)]
    pub symbol: LeaderSymbol,
}

/// The mark at the end of a leader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LeaderSymbol {
    /// Filled arrowhead.
    #[default]
    Arrow,
    /// Open arrowhead (two strokes).
    OpenArrow,
    /// Outlined arrowhead.
    Triangle,
    /// Small filled circle.
    Dot,
    Circle,
    Square,
    FilledSquare,
    /// Slanted stroke (architectural tick).
    Tick,
    None,
}

impl LeaderSymbol {
    pub const ALL: [LeaderSymbol; 9] = [
        LeaderSymbol::Arrow,
        LeaderSymbol::OpenArrow,
        LeaderSymbol::Triangle,
        LeaderSymbol::Dot,
        LeaderSymbol::Circle,
        LeaderSymbol::Square,
        LeaderSymbol::FilledSquare,
        LeaderSymbol::Tick,
        LeaderSymbol::None,
    ];

    /// Translation key of the symbol's name.
    pub fn key(self) -> &'static str {
        match self {
            LeaderSymbol::Arrow => "leader-arrow",
            LeaderSymbol::OpenArrow => "leader-open-arrow",
            LeaderSymbol::Triangle => "leader-triangle",
            LeaderSymbol::Dot => "leader-dot",
            LeaderSymbol::Circle => "leader-circle",
            LeaderSymbol::Square => "leader-square",
            LeaderSymbol::FilledSquare => "leader-filled-square",
            LeaderSymbol::Tick => "leader-tick",
            LeaderSymbol::None => "leader-none",
        }
    }
}

/// A dimension in one view, in that view's drawing coordinates. What `a`,
/// `b` and `offset` mean depends on the [`DimensionKind`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    pub view: ViewId,
    pub a: DVec2,
    pub b: DVec2,
    /// Linear kinds: distance of the dimension line from `a`, positive on
    /// the left of the measured direction. Angular: radius of the arc.
    pub offset: f64,
    #[serde(default)]
    pub kind: DimensionKind,
    /// Elements the ends `a` and `b` follow (associative dimension).
    #[serde(default)]
    pub refs: [Option<crate::DimRef>; 2],
}

/// How a dimension measures.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum DimensionKind {
    /// Distance from `a` to `b`, along `a`-`b`.
    #[default]
    Aligned,
    /// Distance from `a` to `b` measured along `dir` (a unit vector,
    /// horizontal or vertical when placed).
    Linear { dir: DVec2 },
    /// Angle at `vertex` between the rays through `a` and through `b`.
    /// `rays` hold a second point of each measured line (the vertex, in the
    /// frame of that line's element), so the vertex follows the elements.
    Angular {
        vertex: DVec2,
        #[serde(default)]
        rays: [Option<crate::DimRef>; 2],
    },
    /// Radius of the arc centered at `a` through `b`.
    Radial,
    /// Diameter of the circle centered at `a` through `b`.
    Diameter,
}

impl Dimension {
    pub fn length(&self) -> f64 {
        self.a.distance(self.b)
    }

    /// Direction the dimension measures along (linear kinds), if defined.
    pub fn direction(&self) -> Option<DVec2> {
        match self.kind {
            DimensionKind::Linear { dir } => dir.try_normalize(),
            DimensionKind::Aligned => (self.b - self.a).try_normalize(),
            _ => None,
        }
    }

    /// Every element reference: the ends and, for angles, the rays.
    pub fn all_refs_mut(&mut self) -> impl Iterator<Item = &mut Option<crate::DimRef>> {
        let rays: &mut [Option<crate::DimRef>] = match &mut self.kind {
            DimensionKind::Angular { rays, .. } => rays,
            _ => &mut [],
        };
        self.refs.iter_mut().chain(rays.iter_mut())
    }

    /// Whether any end or ray follows an element.
    pub fn is_associated(&self) -> bool {
        let rays = match &self.kind {
            DimensionKind::Angular { rays, .. } => rays.iter().any(Option::is_some),
            _ => false,
        };
        rays || self.refs.iter().any(Option::is_some)
    }

    /// The measured value: meters for lengths, radians for angles.
    pub fn value(&self) -> f64 {
        match self.kind {
            DimensionKind::Aligned => self.length(),
            DimensionKind::Linear { dir } => (self.b - self.a).dot(dir.normalize_or_zero()).abs(),
            DimensionKind::Angular { vertex, .. } => (self.a - vertex).angle_to(self.b - vertex).abs(),
            DimensionKind::Radial => self.length(),
            DimensionKind::Diameter => 2.0 * self.length(),
        }
    }
}

/// A column placed at a point, from its base to its top constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub position: DVec2,
    /// Rotation of the section in plan, degrees counter-clockwise.
    pub rotation_deg: f64,
    pub base_offset: f64,
    /// Height when `top` is unconnected.
    pub height: f64,
    pub top: WallTop,
    /// A load-bearing column; an architectural one (off) is cut by the walls it is joined with.
    #[serde(default = "yes")]
    pub structural: bool,
}

/// Which line across a beam's section its `start`-`end` location line follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BeamLateral {
    /// The middle of the section.
    #[default]
    Center,
    /// The left face (looking from the start to the end).
    Left,
    /// The right face.
    Right,
}

impl BeamLateral {
    pub const ALL: [BeamLateral; 3] = [BeamLateral::Center, BeamLateral::Left, BeamLateral::Right];

    pub fn key(self) -> &'static str {
        match self {
            BeamLateral::Center => "just-center",
            BeamLateral::Left => "just-left",
            BeamLateral::Right => "just-right",
        }
    }

    /// The other side, as seen in a mirror.
    pub fn mirrored(self) -> Self {
        match self {
            BeamLateral::Center => BeamLateral::Center,
            BeamLateral::Left => BeamLateral::Right,
            BeamLateral::Right => BeamLateral::Left,
        }
    }
}

/// Which height of a beam's section sits at its location elevation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BeamVertical {
    #[default]
    Top,
    Center,
    Bottom,
}

impl BeamVertical {
    pub const ALL: [BeamVertical; 3] = [BeamVertical::Top, BeamVertical::Center, BeamVertical::Bottom];

    pub fn key(self) -> &'static str {
        match self {
            BeamVertical::Top => "just-top",
            BeamVertical::Center => "just-center",
            BeamVertical::Bottom => "just-bottom",
        }
    }
}

/// A horizontal beam between two points. Its location line is at the level
/// elevation plus `top_offset`; `vertical` picks which height of the section
/// sits there (the top by default) and `lateral` which line across it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub start: DVec2,
    pub end: DVec2,
    pub top_offset: f64,
    /// Location elevation of the end above the level when it differs from
    /// the start's (a sloped beam).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<f64>,
    #[serde(default)]
    pub lateral: BeamLateral,
    #[serde(default)]
    pub vertical: BeamVertical,
    /// The start (end) may join structural columns and other beams; when off it stays square.
    #[serde(default = "yes")]
    pub join_start: bool,
    #[serde(default = "yes")]
    pub join_end: bool,
}

impl Beam {
    /// Rise of the location line from the start to the end.
    pub fn rise(&self) -> f64 {
        self.end_offset.map_or(0.0, |end| end - self.top_offset)
    }

    /// Length along the slope.
    pub fn true_length(&self) -> f64 {
        self.length().hypot(self.rise())
    }

    /// A beam whose ends may join.
    pub fn new(start: DVec2, end: DVec2, top_offset: f64) -> Self {
        Self {
            start,
            end,
            top_offset,
            end_offset: None,
            lateral: BeamLateral::Center,
            vertical: BeamVertical::Top,
            join_start: true,
            join_end: true,
        }
    }

    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }
}

/// Where a foundation is in plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FoundationShape {
    /// A pad centered at `position`, its width along the rotated X axis.
    Pad { position: DVec2, rotation_deg: f64 },
    /// A strip centered on the line `start`-`end`.
    Strip { start: DVec2, end: DVec2 },
    /// A slab with this boundary.
    Slab { boundary: Vec<DVec2> },
}

impl FoundationShape {
    pub fn points_mut(&mut self) -> Vec<&mut DVec2> {
        match self {
            FoundationShape::Pad { position, .. } => vec![position],
            FoundationShape::Strip { start, end } => vec![start, end],
            FoundationShape::Slab { boundary } => boundary.iter_mut().collect(),
        }
    }

    pub fn points(&self) -> Vec<DVec2> {
        match self {
            FoundationShape::Pad { position, .. } => vec![*position],
            FoundationShape::Strip { start, end } => vec![*start, *end],
            FoundationShape::Slab { boundary } => boundary.clone(),
        }
    }
}

/// A foundation: its top is `top_offset` above the level; the type gives its
/// size and depth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Foundation {
    pub shape: FoundationShape,
    pub top_offset: f64,
}

/// Where a set of longitudinal bars lies in its host's section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BarFace {
    /// Along the top of a beam.
    Top,
    /// Along the bottom of a beam.
    #[default]
    Bottom,
    /// Around a column's section (corners first).
    Perimeter,
}

impl BarFace {
    pub fn key(self) -> &'static str {
        match self {
            BarFace::Top => "bars-top",
            BarFace::Bottom => "bars-bottom",
            BarFace::Perimeter => "bars-perimeter",
        }
    }
}

/// How a set of bars is laid out in its host.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RebarLayout {
    /// `count` bars running along the host, on `face`.
    Longitudinal { face: BarFace, count: u32 },
    /// Closed stirrups (ties) around the section every `spacing` along the host.
    Stirrups { spacing: f64 },
}

/// A set of reinforcing bars in a beam or column (`host`), `cover` from its
/// faces; the type gives the bar diameter. The bars follow their host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rebar {
    pub host: ElementId,
    pub layout: RebarLayout,
    pub cover: f64,
}

/// A straight stair run starting at `start` (bottom of the first riser,
/// center of the width) going up along `direction_deg`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stair {
    pub start: DVec2,
    pub direction_deg: f64,
    pub base_offset: f64,
    /// Height when `top` is unconnected.
    pub height: f64,
    pub top: WallTop,
}

/// A railing along an open path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Railing {
    pub path: Vec<DVec2>,
    pub base_offset: f64,
}

impl Railing {
    pub fn length(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].distance(w[1])).sum()
    }
}

/// A placed parametric component (see [`crate::component`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Component {
    /// Insertion point in plan.
    pub position: DVec2,
    /// Rotation in plan, degrees counter-clockwise.
    pub rotation_deg: f64,
    /// Elevation of the insertion point above the level.
    pub base_offset: f64,
    /// Values of instance parameters that differ from the type.
    #[serde(default)]
    pub overrides: std::collections::BTreeMap<String, f64>,
    /// Property and quantity sets of the object (from IFC import).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<crate::properties::PropertySet>,
    /// The system or circuit it is connected to (an electrical device's circuit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<crate::SystemId>,
}
