//! Interior elevations: a marker in a space closed by walls and a view of
//! each of its faces, the four sides around it or the walls of the space
//! (see `cc_core::interior` and `cc_views::interior`).

use super::{add_element, edit};
use crate::error::{EngineError, Result};
use crate::refs::{self, Point, Ref, finite, point};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::interior::{
    DEFAULT_DISTANCE, DEFAULT_MIN_LENGTH, Face, FaceMode, InteriorMarker, MAX_DISTANCE, Side, next_number,
};
use cc_core::{
    Category, EditCtx, ElementId, ElementKind, ElementType, ElementTypeId, LevelId, Project, TypeData, View, ViewId,
    ViewKind,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Default scale of an interior elevation (1:50).
pub const DEFAULT_SCALE: u32 = 50;

/// The interior marker type a project gets with its first marker.
fn new_marker_type(id: ElementTypeId) -> ElementType {
    ElementType {
        id,
        name: t!("tpl-type-interior-marker"),
        data: TypeData::InteriorMarker {},
        info: Default::default(),
        uid: Default::default(),
    }
}

/// The sides named in `codes` (`north`, `east`, `south`, `west`), each once.
fn sides_of(codes: &[String]) -> Result<Vec<Side>> {
    let mut sides = Vec::new();
    for code in codes {
        let side = Side::from_code(code)
            .ok_or_else(|| EngineError::invalid(format!("unknown side '{code}' (north, east, south or west)")))?;
        if !sides.contains(&side) {
            sides.push(side);
        }
    }
    Ok(sides)
}

/// The mode named `code` (`directions` or `walls`) with the distance and
/// minimum length given, or the defaults.
fn mode_of(code: Option<&str>, distance: Option<f64>, min_length: Option<f64>) -> Result<FaceMode> {
    let within = |v: f64, name: &str| {
        let v = finite(v, name)?;
        if (0.0..=MAX_DISTANCE).contains(&v) {
            Ok(v)
        } else {
            Err(EngineError::invalid(format!("{name} must be between 0 and {MAX_DISTANCE} m")))
        }
    };
    match code.map(|c| c.trim().to_ascii_lowercase()).as_deref() {
        None | Some("directions") => {
            if distance.is_some() || min_length.is_some() {
                return Err(EngineError::invalid("distance and min_length belong to the walls mode"));
            }
            Ok(FaceMode::Directions)
        }
        Some("walls") => Ok(FaceMode::Walls {
            distance: within(distance.unwrap_or(DEFAULT_DISTANCE), "distance")?,
            min_length: within(min_length.unwrap_or(DEFAULT_MIN_LENGTH), "min_length")?,
        }),
        Some(other) => Err(EngineError::invalid(format!("unknown mode '{other}' (directions or walls)"))),
    }
}

/// The walls `marker` (on `level`) can show, clockwise from north.
fn walls_around(s: &mut Session, level: LevelId, marker: &InteriorMarker) -> Vec<ElementId> {
    let project = s.project().clone();
    cc_views::interior::wall_faces(&project, s.geometry(), level, marker).into_iter().map(|f| f.wall).collect()
}

/// The faces to show of a marker that can show `available` walls: the
/// `sides` named in directions mode (default all four), the `walls` named
/// in walls mode (default all of them; each must be one of `available`).
fn faces_of(
    mode: FaceMode,
    sides: Option<&[String]>,
    walls: Option<&[u64]>,
    available: &[ElementId],
) -> Result<Vec<Face>> {
    match mode {
        FaceMode::Directions => {
            if walls.is_some() {
                return Err(EngineError::invalid("walls are picked in the walls mode; this marker shows sides"));
            }
            let sides = match sides {
                Some(codes) => sides_of(codes)?,
                None => Side::ALL.to_vec(),
            };
            // Clockwise from north, so numbers follow the sides around the marker.
            Ok(Side::ALL.into_iter().filter(|s| sides.contains(s)).map(Face::Side).collect())
        }
        FaceMode::Walls { .. } => {
            if sides.is_some() {
                return Err(EngineError::invalid("sides are picked in the directions mode; this marker shows walls"));
            }
            let Some(ids) = walls else { return Ok(available.iter().copied().map(Face::Wall).collect()) };
            for id in ids {
                if !available.iter().any(|w| w.0 == *id) {
                    return Err(EngineError::invalid(format!(
                        "element {id} is not a wall of the marker's space long enough to show"
                    )));
                }
            }
            // In the order around the marker.
            Ok(available.iter().copied().filter(|w| ids.contains(&w.0)).map(Face::Wall).collect())
        }
    }
}

/// The name of view number `number` of a marker: the room's name and the
/// number, or a generic name and the number with no room.
fn view_name(p: &Project, room: Option<ElementId>, number: u32) -> String {
    let room = room.and_then(|r| match p.element(r).map(|e| &e.kind) {
        Some(ElementKind::Room(r)) if !r.name.trim().is_empty() => Some(r.name.trim().to_string()),
        _ => None,
    });
    match room {
        Some(name) => format!("{name} {number}"),
        None => format!("{} {number}", t!("default-interior-elevation-name")),
    }
}

/// Add the views of `faces` (in the order to number them) to marker
/// `marker`; those it has are kept. Numbers are the lowest free ones.
/// Returns the new views.
fn add_faces(e: &mut EditCtx, marker: ElementId, faces: &[Face]) -> Vec<ViewId> {
    let p = e.project();
    let Some(ElementKind::InteriorMarker(m)) = p.element(marker).map(|el| el.kind.clone()) else {
        return Vec::new();
    };
    let have = p.marker_faces(marker);
    let mut taken: Vec<u32> = have.iter().map(|(_, n, _)| *n).collect();
    let mut made = Vec::new();
    for face in faces.iter().copied().filter(|f| m.takes(*f)) {
        if have.iter().any(|(f, ..)| *f == face) {
            continue;
        }
        let Some((origin, look)) = e.project().interior_frame(&m, face) else { continue };
        let number = next_number(&taken);
        taken.push(number);
        let name = view_name(e.project(), m.room, number);
        let kind = ViewKind::InteriorElevation { marker, face, number, origin, look, auto_extents: true };
        let id = ViewId(e.alloc_id());
        made.push(e.add_view(View::new(id, name, DEFAULT_SCALE, kind)));
    }
    made
}

/// The marker's views as JSON, by number: `side` or `wall`, number, view id and name.
fn faces_json(p: &Project, marker: ElementId) -> Value {
    let mut faces = p.marker_faces(marker);
    faces.sort_by_key(|(_, number, _)| *number);
    json!(
        faces
            .iter()
            .map(|(face, number, view)| {
                let mut out = json!({
                    "number": number,
                    "view": view.0,
                    "name": p.view(*view).map(|v| v.name.clone()).unwrap_or_default(),
                });
                match face {
                    Face::Side(side) => out["side"] = json!(side.code()),
                    Face::Wall(wall) => out["wall"] = json!(wall.0),
                }
                out
            })
            .collect::<Vec<_>>()
    )
}

/// The marker `id` names, with its level.
fn marker_of(p: &Project, id: u64) -> Result<(ElementId, LevelId, InteriorMarker)> {
    let marker = refs::element(p, id)?;
    match p.element(marker).map(|e| (e.level, &e.kind)) {
        Some((level, ElementKind::InteriorMarker(m))) => Ok((marker, level, m.clone())),
        _ => Err(EngineError::invalid(format!("element {id} is not an interior elevation marker"))),
    }
}

/// Place an interior elevation marker in a space closed by walls (a room's,
/// or any other) and make a view of each of its faces. In the `directions`
/// mode (the default) the faces are up to four sides at right angles:
/// each view looks from the marker at the wall on that side, through the
/// whole space. In the `walls` mode there is a view per wall of the space
/// with at least `min_length` along it: each looks square on at its wall
/// from `distance` in front of it (from the marker, when it stands nearer),
/// showing that wall's stretch of the space. Every view is cropped across
/// to what it shows and from the level to the ceiling or what covers the
/// space above (else the room's height, else the level above), and follows
/// the walls, the marker and the room as they change. Views are named
/// after the room and numbered clockwise from north ("Kitchen 1",
/// "Kitchen 2"…), at 1:50.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct InteriorElevationsCreate {
    /// Room id: the marker stands at its point, on its level, and the views
    /// take its name. Or give `position`.
    #[serde(default)]
    pub room: Option<u64>,
    /// Where the marker stands in plan, `[x, y]` (with no `room`).
    #[serde(default)]
    pub position: Option<Point>,
    /// Level id or name, with `position` (default: the lowest level).
    #[serde(default)]
    pub level: Option<Ref>,
    /// `directions` (default: up to four sides) or `walls` (a view per wall).
    #[serde(default)]
    pub mode: Option<String>,
    /// Directions: the sides to show, `north`, `east`, `south`, `west` (default all four).
    #[serde(default)]
    pub sides: Option<Vec<String>>,
    /// Walls: the ids of the walls to show (default every wall of the space
    /// long enough).
    #[serde(default)]
    pub walls: Option<Vec<u64>>,
    /// Directions: turn of the sides counter-clockwise, degrees (default 0:
    /// facing north, east, south and west of the plan), to face slanted walls.
    #[serde(default)]
    pub rotation: Option<f64>,
    /// Walls: how far in front of its wall each view is cut, meters (default 1).
    #[serde(default)]
    pub distance: Option<f64>,
    /// Walls: the least length of a wall along the space for it to get a
    /// view, meters (default 0.3), leaving out column faces and short returns.
    #[serde(default)]
    pub min_length: Option<f64>,
}

impl Command for InteriorElevationsCreate {
    const NAME: &'static str = "interior_elevations_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let (position, level, room) = match (self.room, self.position) {
            (Some(_), Some(_)) => return Err(EngineError::invalid("give room or position, not both")),
            (Some(id), None) => {
                let room = refs::element(p, id)?;
                let el = p.element(room).ok_or_else(|| EngineError::not_found("room"))?;
                let ElementKind::Room(r) = &el.kind else {
                    return Err(EngineError::invalid(format!("element {id} is not a room")));
                };
                (r.position, el.level, Some(room))
            }
            (None, Some(at)) => (point(at)?, refs::level_or_first(p, self.level.as_ref())?, None),
            (None, None) => return Err(EngineError::invalid("give a room or a position")),
        };
        let mode = mode_of(self.mode.as_deref(), self.distance, self.min_length)?;
        let rotation_deg = finite(self.rotation.unwrap_or(0.0), "rotation")?.rem_euclid(360.0);
        let marker = InteriorMarker { position, room, rotation_deg, mode };
        if !marker.is_valid() {
            return Err(EngineError::invalid("the position is out of bounds"));
        }
        let project = s.project().clone();
        let phase = s.doc.creation_phase;
        if s.geometry().room_at(&project, level, position, phase).is_none() {
            return Err(EngineError::rejected("no walls close a space around the marker"));
        }
        let available = if mode.is_directions() { Vec::new() } else { walls_around(s, level, &marker) };
        let faces = faces_of(mode, self.sides.as_deref(), self.walls.as_deref(), &available)?;
        if faces.is_empty() {
            return Err(EngineError::invalid(if mode.is_directions() {
                "give at least one side"
            } else {
                "no wall of the space is long enough to show"
            }));
        }
        let ty = s.project().default_type(Category::InteriorMarker);
        let id = edit(s, t!("tx-create-interior-elevations"), |e| {
            let type_id = match ty {
                Some(t) => t,
                None => {
                    let id = ElementTypeId(e.alloc_id());
                    e.add_type(new_marker_type(id))
                }
            };
            let id = add_element(e, level, type_id, ElementKind::InteriorMarker(marker));
            add_faces(e, id, &faces);
            Ok(id)
        })?;
        let p = s.project();
        Ok(json!({ "marker": id.0, "views": faces_json(p, id) }))
    }
}

/// Set the faces an interior elevation marker shows: `sides` for a marker
/// in the directions mode, `walls` (ids) for one in the walls mode. Views
/// are made for the faces added (numbered with the lowest numbers free) and
/// deleted, with their viewports on sheets, for the faces left out. One
/// undo step.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct InteriorElevationSidesSet {
    /// Marker id.
    pub marker: u64,
    /// Directions mode: the sides to show, `north`, `east`, `south`, `west` (at least one).
    #[serde(default)]
    pub sides: Option<Vec<String>>,
    /// Walls mode: the ids of the walls to show (at least one), among the
    /// walls of the marker's space long enough (see `element_get`'s
    /// `derived.walls`).
    #[serde(default)]
    pub walls: Option<Vec<u64>>,
}

impl Command for InteriorElevationSidesSet {
    const NAME: &'static str = "interior_elevation_sides_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (marker, level, m) = marker_of(s.project(), self.marker)?;
        if self.sides.is_none() && self.walls.is_none() {
            return Err(EngineError::invalid("give sides (directions mode) or walls (walls mode)"));
        }
        let available = if m.mode.is_directions() { Vec::new() } else { walls_around(s, level, &m) };
        let faces = faces_of(m.mode, self.sides.as_deref(), self.walls.as_deref(), &available)?;
        if faces.is_empty() {
            return Err(EngineError::invalid("give at least one face (delete the marker to remove them all)"));
        }
        let gone: Vec<ViewId> = s
            .project()
            .marker_faces(marker)
            .into_iter()
            .filter(|(face, ..)| !faces.contains(face))
            .map(|(.., v)| v)
            .collect();
        edit(s, t!("tx-interior-elevation-sides"), |e| {
            for view in gone {
                e.remove_view(view);
            }
            add_faces(e, marker, &faces);
            Ok(())
        })?;
        let p = s.project();
        Ok(json!({ "marker": marker.0, "views": faces_json(p, marker) }))
    }
}

/// Set what an interior elevation marker shows: `directions` (up to four
/// sides at right angles) or `walls` (a view per wall of its space, with
/// `distance` and `min_length`). Changing the mode deletes the marker's
/// views and makes those of every face of the new mode. In the walls mode,
/// a new `distance` moves where its views are cut and a new `min_length`
/// deletes the views of walls now too short (walls it lets in are added
/// with `interior_elevation_sides_set`). One undo step.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct InteriorElevationModeSet {
    /// Marker id.
    pub marker: u64,
    /// `directions` or `walls`.
    pub mode: String,
    /// Walls: how far in front of its wall each view is cut, meters
    /// (default: as it is, else 1).
    #[serde(default)]
    pub distance: Option<f64>,
    /// Walls: the least length of a wall along the space for it to get a
    /// view, meters (default: as it is, else 0.3).
    #[serde(default)]
    pub min_length: Option<f64>,
}

impl Command for InteriorElevationModeSet {
    const NAME: &'static str = "interior_elevation_mode_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let (marker, level, old) = marker_of(s.project(), self.marker)?;
        // In the walls mode, values not given stay as they are.
        let (distance, min_length) = match old.mode {
            FaceMode::Walls { distance, min_length } if self.mode.trim().eq_ignore_ascii_case("walls") => {
                (Some(self.distance.unwrap_or(distance)), Some(self.min_length.unwrap_or(min_length)))
            }
            _ => (self.distance, self.min_length),
        };
        let mode = mode_of(Some(&self.mode), distance, min_length)?;
        let changed = old.mode.is_directions() != mode.is_directions();
        let new = InteriorMarker { mode, ..old };
        let available = if mode.is_directions() { Vec::new() } else { walls_around(s, level, &new) };
        // A new mode shows all its faces; the same one keeps those still there.
        let faces: Vec<Face> = if changed {
            faces_of(mode, None, None, &available)?
        } else {
            let all = faces_of(mode, None, None, &available)?;
            s.project().marker_faces(marker).into_iter().map(|(f, ..)| f).filter(|f| all.contains(f)).collect()
        };
        if changed && faces.is_empty() {
            return Err(EngineError::rejected("no wall of the space is long enough to show"));
        }
        let gone: Vec<ViewId> = s
            .project()
            .marker_faces(marker)
            .into_iter()
            .filter(|(face, ..)| !faces.contains(face))
            .map(|(.., v)| v)
            .collect();
        edit(s, t!("tx-interior-elevation-mode"), |e| {
            for view in gone {
                e.remove_view(view);
            }
            if let Some(ElementKind::InteriorMarker(m)) = e.element_mut(marker).map(|el| &mut el.kind) {
                m.mode = mode;
            }
            if changed {
                add_faces(e, marker, &faces);
            }
            Ok(())
        })?;
        let p = s.project();
        Ok(json!({ "marker": marker.0, "views": faces_json(p, marker) }))
    }
}

/// The views of a marker for `element_get`, by number.
pub(crate) fn marker_views(p: &Project, marker: ElementId) -> Value {
    faces_json(p, marker)
}
