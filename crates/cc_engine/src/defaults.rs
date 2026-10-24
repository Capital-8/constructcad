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

/// Closed loop following the walls of `level`, offset to the exterior faces
/// (`outward > 0`) or interior faces (`outward < 0`).
pub fn walls_loop(project: &Project, level: LevelId, outward: f64) -> Option<Vec<DVec2>> {
    let walls: Vec<(&Element, &Wall)> = project.walls().filter(|(e, _)| e.level == level).collect();
    const TOL: f64 = 0.01;
    for start in 0..walls.len() {
        let mut used = vec![false; walls.len()];
        used[start] = true;
        let (first, w0) = (walls[start].1.start, walls[start].1.end);
        let mut pts = vec![first];
        pts.extend(along(walls[start].1, true));
        let mut cur = w0;
        let mut half = project.thickness_of(walls[start].0) * 0.5;
        loop {
            if cur.distance(first) < TOL && pts.len() > 3 {
                pts.pop();
                let pts = polygon::ccw(&pts);
                return Some(polygon::offset(&pts, half * outward));
            }
            let next = (0..walls.len())
                .find(|&i| !used[i] && (walls[i].1.start.distance(cur) < TOL || walls[i].1.end.distance(cur) < TOL));
            let Some(i) = next else { break };
            used[i] = true;
            let w = walls[i].1;
            let forward = w.start.distance(cur) < TOL;
            cur = if forward { w.end } else { w.start };
            half = half.max(project.thickness_of(walls[i].0) * 0.5);
            pts.extend(along(w, forward));
        }
    }
    None
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
