//! Interior elevations (see [`cc_core::interior`]): where a view of a
//! marker's side looks and how it is cropped, worked out from the space
//! closed by walls around the marker each time the view is drawn.
//!
//! The view looks from the marker along its side's direction, through the
//! space up to the far wall's outer face. Across, it keeps the space's
//! extent (the side walls' inner faces); up, from the level to the
//! underside of what covers the space above the marker (a ceiling, the
//! floor above or a roof), else the room's height, else the level above.

use crate::projected::Frame;
use cc_core::callout::CropBox;
use cc_core::interior::{Face, FaceMode};
use cc_core::{Category, ElementKind, Project, View, ViewKind};
use cc_geom::{ModelGeometry, polygon};
use glam::{DVec2, DVec3};

/// Height of a space with nothing above it, no room and no level above, meters.
const DEFAULT_HEIGHT: f64 = 3.0;
/// Least room between the floor and what covers the space, meters (lower
/// faces belong to furniture or the floor itself).
const MIN_HEADROOM: f64 = 0.5;
/// Thickest wall the view looks into past the space, meters.
const MAX_WALL: f64 = 3.0;

/// How an interior elevation is drawn: its frame and its crop.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub frame: Frame,
    /// The crop of the drawing (the space's, or the one drawn on the view).
    pub crop: Option<CropBox>,
}

/// The frame of an interior elevation without the space: it looks from its
/// origin along its direction (no depth or width limits). `None` for other views.
pub fn frame(view: &View) -> Option<Frame> {
    let ViewKind::InteriorElevation { origin, look, .. } = view.kind else { return None };
    let mut frame = Frame::looking(DVec3::new(origin.x, origin.y, 0.0), DVec3::new(look.x, look.y, 0.0));
    frame.near = Some(0.0);
    Some(frame)
}

/// How `view` (an interior elevation) is drawn now: `None` when it is not
/// one, its marker is gone, no walls close a space around the marker or
/// its wall is not one of that space's.
///
/// A side's view keeps the whole space across and looks to its far end; a
/// wall's view keeps the stretch of the boundary on that wall and looks to
/// the wall's outer face.
pub fn resolve(project: &Project, geom: &ModelGeometry, view: &View) -> Option<Resolved> {
    let ViewKind::InteriorElevation { marker, face, origin, look, auto_extents, .. } = view.kind else { return None };
    let e = project.element(marker)?;
    let ElementKind::InteriorMarker(m) = &e.kind else { return None };
    let look = look.try_normalize()?;
    let right = DVec2::new(look.y, -look.x);
    let across = |p: &DVec2| (*p - origin).dot(right);
    let ahead = |p: &DVec2| (*p - origin).dot(look);
    // The points the view must reach, and how thick the wall behind them may be.
    let (points, wall): (Vec<DVec2>, f64) = match face {
        Face::Side(_) => {
            (geom.room_at(project, e.level, m.position, Some(view.phase))?, thickest_wall(project, e.level))
        }
        Face::Wall(id) => {
            let edges = geom.room_edges_at(project, e.level, m.position, Some(view.phase))?;
            let points: Vec<DVec2> =
                edges.iter().filter(|edge| edge.wall == Some(id)).flat_map(|edge| [edge.a, edge.b]).collect();
            let thickness =
                project.element(id).and_then(|w| project.types.get(&w.type_id)).map_or(0.0, |t| t.thickness());
            (points, thickness)
        }
    };
    let (xa, xb) = points.iter().map(across).fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
    let far = points.iter().map(ahead).fold(0.0_f64, f64::max);
    let width = xb - xa;
    if points.is_empty() || !width.is_finite() || width <= 1e-6 || !far.is_finite() {
        return None;
    }
    let depth = far + if wall.is_finite() { wall.clamp(0.0, MAX_WALL) } else { 0.0 } + 0.01;
    let mut frame = frame(view)?;
    frame.far = Some(depth);
    frame.crop_x = Some((xa, xb));
    let crop = if auto_extents {
        let z0 = project.level_elevation(e.level);
        let top = covered_at(geom, m.position, z0)
            .or_else(|| room_top(project, m.room, z0))
            .or_else(|| level_above(project, z0))
            .unwrap_or(z0 + DEFAULT_HEIGHT);
        Some(CropBox::new(DVec2::new(xa, z0), DVec2::new(xb, top)))
    } else {
        view.crop
    };
    Some(Resolved { frame, crop: crop.filter(CropBox::is_valid) })
}

/// A wall a marker in walls mode can show: how much of it lies along the
/// space and the middle of its longest stretch there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallFace {
    pub wall: cc_core::ElementId,
    pub length: f64,
    pub mid: DVec2,
}

/// Most walls a marker lists.
pub const MAX_WALL_FACES: usize = 256;

/// The walls of the space around `marker` (an element on `level`) that its
/// walls mode can show: those with at least the mode's `min_length` along
/// the space (the default one in directions mode) that the marker does not
/// stand in, clockwise from north as seen from the marker (by the middle of
/// each wall's longest stretch). Empty where no walls close a space.
pub fn wall_faces(
    project: &Project,
    geom: &ModelGeometry,
    level: cc_core::LevelId,
    marker: &cc_core::interior::InteriorMarker,
) -> Vec<WallFace> {
    let Some(edges) = geom.room_edges_at(project, level, marker.position, None) else { return Vec::new() };
    let min_length = match marker.mode {
        FaceMode::Walls { min_length, .. } => min_length,
        FaceMode::Directions => cc_core::interior::DEFAULT_MIN_LENGTH,
    };
    // Per wall: its length along the space and its longest stretch.
    let mut walls: std::collections::BTreeMap<cc_core::ElementId, (f64, f64, DVec2)> = Default::default();
    for edge in &edges {
        let Some(id) = edge.wall else { continue };
        let length = edge.a.distance(edge.b);
        let entry = walls.entry(id).or_insert((0.0, 0.0, edge.a));
        entry.0 += length;
        if length > entry.1 {
            (entry.1, entry.2) = (length, (edge.a + edge.b) * 0.5);
        }
    }
    let bearing = |p: DVec2| {
        let d = p - marker.position;
        d.x.atan2(d.y).rem_euclid(std::f64::consts::TAU)
    };
    let mut faces: Vec<WallFace> = walls
        .into_iter()
        .filter(|(id, (length, ..))| {
            *length + 1e-9 >= min_length && project.interior_frame(marker, Face::Wall(*id)).is_some()
        })
        .map(|(wall, (length, _, mid))| WallFace { wall, length, mid })
        .collect();
    faces.sort_by(|a, b| bearing(a.mid).total_cmp(&bearing(b.mid)));
    faces.truncate(MAX_WALL_FACES);
    faces
}

/// The thickest type of the walls on `level`, meters.
fn thickest_wall(project: &Project, level: cc_core::LevelId) -> f64 {
    project
        .walls()
        .filter(|(e, _)| e.level == level)
        .filter_map(|(e, _)| project.types.get(&e.type_id).map(|t| t.thickness()))
        .filter(|t| t.is_finite())
        .fold(0.0, f64::max)
}

/// The lowest underside of a ceiling, floor or roof over `at`, at least
/// [`MIN_HEADROOM`] above `z0`.
fn covered_at(geom: &ModelGeometry, at: DVec2, z0: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for s in &geom.solids {
        if !matches!(s.category, Category::Ceiling | Category::Floor | Category::Roof) || s.secondary {
            continue;
        }
        for f in &s.faces {
            // Faces looking down, over the point.
            if f.normal.z > -0.1 || f.verts.len() < 3 {
                continue;
            }
            let plan: Vec<DVec2> = f.verts.iter().map(|v| v.truncate()).collect();
            if !polygon::contains(&plan, at) {
                continue;
            }
            let p0 = f.verts[0];
            let z = p0.z - (f.normal.x * (at.x - p0.x) + f.normal.y * (at.y - p0.y)) / f.normal.z;
            if z.is_finite() && z >= z0 + MIN_HEADROOM && best.is_none_or(|b| z < b) {
                best = Some(z);
            }
        }
    }
    best
}

/// The top of room `room`, its height above `z0`.
fn room_top(project: &Project, room: Option<cc_core::ElementId>, z0: f64) -> Option<f64> {
    match project.element(room?).map(|e| &e.kind) {
        Some(ElementKind::Room(r)) if r.height.is_finite() && r.height > MIN_HEADROOM => Some(z0 + r.height),
        _ => None,
    }
}

/// The elevation of the lowest level above `z0` by more than the headroom.
fn level_above(project: &Project, z0: f64) -> Option<f64> {
    project.levels.iter().map(|l| l.elevation).filter(|z| *z >= z0 + MIN_HEADROOM).reduce(f64::min)
}

/// Radius of a marker's circle on paper, millimeters.
const MARKER_RADIUS_MM: f64 = 3.0;
/// Height of the numbers at its arrows on paper, millimeters.
const NUMBER_SIZE_MM: f64 = 2.2;

/// The interior elevation markers on `level` in a plan at 1:`scale`: a
/// circle (the marker, to pick and move) and, for each of its views, a
/// filled arrow pointing at the wall it shows with the view's number (its
/// drawing number once on a sheet). A side's arrow stands on the circle; a
/// wall's stands where its view is cut, in front of the middle of the wall.
pub(crate) fn marks(
    d: &mut crate::Drawing2D,
    project: &Project,
    geom: &ModelGeometry,
    level: cc_core::LevelId,
    scale: u32,
) {
    let r = MARKER_RADIUS_MM * f64::from(scale.max(1)) / 1000.0;
    for e in project.elements.values().filter(|e| e.level == level) {
        let ElementKind::InteriorMarker(m) = &e.kind else { continue };
        let c = m.position;
        let circle: Vec<DVec2> =
            (0..24).map(|i| c + DVec2::from_angle(f64::from(i) / 24.0 * std::f64::consts::TAU) * r).collect();
        d.fill(circle.clone(), [255, 255, 255, 255], Some(e.id));
        d.polyline(&circle, true, crate::LineStyle::Annotation, Some(e.id));
        let walls = if m.mode.is_directions() { Vec::new() } else { wall_faces(project, geom, level, m) };
        for (face, number, view) in project.marker_faces(e.id) {
            let Some(v) = project.view(view) else { continue };
            let ViewKind::InteriorElevation { origin, look, .. } = v.kind else { continue };
            let Some(look) = look.try_normalize() else { continue };
            let across = look.perp();
            // Where the arrow's base stands, and where its number goes.
            let (base, text) = match face {
                Face::Side(_) => (c + look * (r * 0.3), c + look * (r * 0.55)),
                Face::Wall(id) => {
                    // In front of the middle of the wall's stretch, on the cut plane.
                    let mid = walls.iter().find(|w| w.wall == id).map_or(origin, |w| w.mid);
                    let on_plane = origin + across * (mid - origin).dot(across);
                    (on_plane, on_plane - look * (r * 0.7))
                }
            };
            let arrow = vec![base + across * (r * 0.95), base + look * (r * 1.6), base - across * (r * 0.95)];
            d.fill(arrow, [0, 0, 0, 255], Some(e.id));
            // Picked on the arrow's body, clear of the marker's circle.
            d.interior_arrows.push(crate::InteriorArrow { view, center: base + look * (r * 0.8), radius: r * 0.9 });
            let (drawing, sheet) = crate::plan::section_labels(project, v);
            let label = if sheet == "-" { number.to_string() } else { drawing };
            d.text_at(text, label, NUMBER_SIZE_MM, crate::Anchor::Center, 0.0);
        }
    }
}
