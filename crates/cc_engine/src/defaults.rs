//! Defaults applied when elements are created (shared by the drawing tools).

use cc_core::{Element, LevelId, Project, Wall, WallTop};
use cc_geom::polygon;
use glam::DVec2;

/// Top constraint for new columns and stairs: the level above, if any.
pub fn default_top(project: &Project, level: LevelId) -> WallTop {
    match project.level_above(level) {
        Some(above) => WallTop::Level { level: above.id, offset: 0.0 },
        None => WallTop::Unconnected,
    }
}

/// Default roof base: the top of the tallest wall on the level.
pub fn default_roof_base(project: &Project, level: LevelId) -> f64 {
    project
        .walls()
        .filter(|(e, _)| e.level == level)
        .map(|(e, w)| project.wall_top_z(e, w) - project.level_elevation(level))
        .fold(None, |m: Option<f64>, h| Some(m.map_or(h, |m| m.max(h))))
        .unwrap_or(3.0)
}

/// Elevation of a new pipe's center line above its level: supply pipes run
/// in the walls, drains under the floor.
pub fn pipe_offset(kind: Option<cc_core::SystemKind>) -> f64 {
    use cc_core::SystemKind as K;
    match kind {
        Some(K::ColdWater | K::HotWater | K::Vent) => 0.6,
        Some(K::Sanitary | K::Rainwater) => -0.3,
        _ => 0.0,
    }
}

/// Minimum fall of a horizontal drain of outside `diameter`, as in common
/// drainage practice: 2 % up to 75 mm, 1 % from 100 mm.
pub fn drain_slope(diameter: f64) -> f64 {
    if diameter <= 0.0751 { 0.02 } else { 0.01 }
}

/// Elevation of a new conduit above its level: in the slab above (the level height).
pub fn conduit_offset(project: &Project, level: LevelId) -> f64 {
    project.level_height(level, 2.8)
}

/// Usual mounting height of an electrical device above its level (lights:
/// the ceiling, at the level height).
pub fn device_height(project: &Project, level: LevelId, device: cc_core::DeviceKind) -> f64 {
    use cc_core::DeviceKind as D;
    match device {
        D::Outlet | D::DedicatedOutlet => 0.3,
        D::Switch => 1.1,
        D::WallLight => 2.1,
        D::Panel => 1.3,
        D::CeilingLight => project.level_height(level, 2.8),
    }
}

/// Next free number among `existing` names (1 when none is numeric).
pub fn next_number<'a>(existing: impl Iterator<Item = &'a str>) -> String {
    (existing.filter_map(|s| s.trim().parse::<u32>().ok()).max().unwrap_or(0) + 1).to_string()
}

/// Closed loop around the walls of `level`, offset to the exterior faces
/// (`outward > 0`) or interior faces (`outward < 0`): the outer boundary of
/// their location lines, so interior walls (even those meeting its corners)
/// and walls hanging off it are left out; with several separate loops, the
/// largest. The offset is half the thickest wall on the boundary.
pub fn walls_loop(project: &Project, level: LevelId, outward: f64) -> Option<Vec<DVec2>> {
    let walls: Vec<(&Element, &Wall)> = project.walls().filter(|(e, _)| e.level == level).collect();
    // Ends closer than this meet (drawn without snapping).
    const TOL: f64 = 0.01;
    let mut ends: Vec<DVec2> = Vec::new();
    let mut snap = |p: DVec2| match ends.iter().find(|q| q.distance(p) < TOL) {
        Some(q) => *q,
        None => {
            ends.push(p);
            p
        }
    };
    let lines: Vec<Vec<DVec2>> = walls
        .iter()
        .map(|(_, w)| {
            let (a, b) = (snap(w.start), snap(w.end));
            let mut pts = vec![a];
            pts.extend(along(w, true));
            if let Some(last) = pts.last_mut() {
                *last = b;
            }
            pts
        })
        .collect();
    let outline = cc_geom::outer_boundary(&lines)?;
    // The walls whose middle lies on the outline set the offset.
    let on_outline = |p: DVec2| {
        (0..outline.len()).any(|i| {
            let (q, _) = polygon::closest_on_segment(outline[i], outline[(i + 1) % outline.len()], p);
            q.distance(p) < TOL
        })
    };
    let half = walls
        .iter()
        .zip(&lines)
        .filter(|(_, line)| line.len() >= 2 && on_outline(mid_point(line)))
        .map(|((e, _), _)| project.thickness_of(e) * 0.5)
        .fold(0.0, f64::max);
    Some(polygon::offset(&outline, half * outward))
}

/// The point halfway along a polyline.
fn mid_point(line: &[DVec2]) -> DVec2 {
    let total: f64 = line.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total * 0.5;
    for w in line.windows(2) {
        let len = w[0].distance(w[1]);
        if len >= left && len > 0.0 {
            return w[0].lerp(w[1], left / len);
        }
        left -= len;
    }
    line.first().copied().unwrap_or_default()
}

/// Points of a wall's location line after its first end (`forward`: from
/// start to end): its other end, preceded by points along an arc.
fn along(w: &Wall, forward: bool) -> Vec<DVec2> {
    let mut pts = match w.arc() {
        Some(a) => a.points(0.0, a.length(), 0.0, cc_core::arc::ARC_STEP),
        None => vec![w.start, w.end],
    };
    if !forward {
        pts.reverse();
    }
    pts.remove(0);
    pts
}
