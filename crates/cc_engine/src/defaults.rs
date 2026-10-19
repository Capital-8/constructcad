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
        let mut pts = vec![first, w0];
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
            cur = if w.start.distance(cur) < TOL { w.end } else { w.start };
            half = half.max(project.thickness_of(walls[i].0) * 0.5);
            pts.push(cur);
        }
    }
    None
}
