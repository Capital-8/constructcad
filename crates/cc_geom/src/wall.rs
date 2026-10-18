use crate::polygon::line_intersect;
use crate::{Face, Solid};
use cc_core::{Category, ElementId};
use glam::{DVec2, DVec3};

/// A rectangular hole through a wall, in wall-local coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Opening {
    pub element: ElementId,
    pub u0: f64,
    pub u1: f64,
    pub z0: f64,
    pub z1: f64,
}

/// A vertical slice of a wall between opening boundaries.
///
/// Its footprint is a quad whose left (+v) edge spans `left` and right (-v)
/// edge spans `right` along the wall axis; end columns may be mitered.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub left: (f64, f64),
    pub right: (f64, f64),
    /// Solid z-intervals (absolute elevations), bottom to top.
    pub solid: Vec<(f64, f64)>,
}

impl Column {
    pub fn is_solid_at(&self, z: f64) -> bool {
        self.solid.iter().any(|&(a, b)| a <= z && z <= b)
    }

    pub fn top(&self) -> Option<f64> {
        self.solid.last().map(|s| s.1)
    }
}

/// Resolved wall geometry: frame, joins, openings and columns.
#[derive(Debug, Clone)]
pub struct WallGeom {
    pub element: ElementId,
    pub origin: DVec2,
    pub dir: DVec2,
    /// Left normal (+v).
    pub normal: DVec2,
    pub length: f64,
    pub half: f64,
    pub base_z: f64,
    pub top_z: f64,
    /// Axial extents `(left, right)` of the start/end faces after joins.
    pub start_ext: (f64, f64),
    pub end_ext: (f64, f64),
    pub start_joined: bool,
    pub end_joined: bool,
    pub openings: Vec<Opening>,
    pub columns: Vec<Column>,
    /// Layer boundaries as `(v_from, v_to, material)`, from the left face inward.
    pub layers: Vec<(f64, f64, String)>,
    /// Stretches of a face where another wall meets it in a T, as
    /// `(side, u0, u1)` with side +1 for the left face and -1 for the right.
    pub face_gaps: Vec<(f64, f64, f64)>,
}

/// How one end of a wall is cut by a neighbor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EndJoin {
    /// Distance along the ray (pointing into this wall) of the ray-left and ray-right faces.
    pub ray_left: f64,
    pub ray_right: f64,
    /// For a T-join, the wall whose face this end meets.
    pub host: Option<usize>,
}

/// Minimal wall description used while resolving joins.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WallLine {
    pub start: DVec2,
    pub end: DVec2,
    pub half: f64,
    /// Whether each end may join (a disallowed end stays square and is ignored by neighbors).
    pub join_start: bool,
    pub join_end: bool,
}

impl WallLine {
    fn dir(&self) -> DVec2 {
        (self.end - self.start).try_normalize().unwrap_or(DVec2::X)
    }
}

const JOIN_TOL: f64 = 0.01;

/// How far short of another wall's face an end may stop and still join it.
const GAP_TOL: f64 = 0.05;

/// The end of `walls[me]` at `p` (ray `da` into the wall) meeting the face of
/// `walls[host]`: a T-join cut along that face.
fn t_join(walls: &[WallLine], me: usize, p: DVec2, da: DVec2, host: usize) -> Option<EndJoin> {
    let a = walls[me];
    let b = walls[host];
    let max_ext = 10.0 * a.half.max(0.05);
    let clamp = |t: f64| t.clamp(-max_ext, max_ext);
    let na = da.perp();
    let db = b.dir();
    let nb = db.perp();
    // The face on the side this wall comes from.
    let side = if da.dot(nb) >= 0.0 { 1.0 } else { -1.0 };
    let face = b.start + nb * (side * b.half);
    let left = line_intersect(p + na * a.half, da, face, db)?;
    let right = line_intersect(p - na * a.half, da, face, db)?;
    Some(EndJoin { ray_left: clamp(left), ray_right: clamp(right), host: Some(host) })
}

/// Resolve the join at one end of `walls[me]` (`at_start` selects the end).
pub(crate) fn resolve_join(walls: &[WallLine], me: usize, at_start: bool) -> Option<EndJoin> {
    let a = walls[me];
    if !(if at_start { a.join_start } else { a.join_end }) {
        return None;
    }
    let (p, da) = if at_start { (a.start, a.dir()) } else { (a.end, -a.dir()) };
    let max_ext = 10.0 * a.half.max(0.05);
    let clamp = |t: f64| t.clamp(-max_ext, max_ext);
    let na = da.perp();
    let square = || Some(EndJoin { ray_left: 0.0, ray_right: 0.0, host: None });

    // Other wall ends at the same point (that allow joining), as rays into each wall.
    let ends: Vec<(usize, DVec2)> = walls
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != me)
        .filter_map(|(i, b)| {
            if b.start.distance(p) < JOIN_TOL && b.join_start {
                Some((i, b.dir()))
            } else if b.end.distance(p) < JOIN_TOL && b.join_end {
                Some((i, -b.dir()))
            } else {
                None
            }
        })
        .collect();

    // L-join: exactly one other wall ends here.
    if ends.len() == 1 {
        let (bi, db) = ends[0];
        if da.perp_dot(db).abs() < 1e-6 {
            // Collinear: a seamless butt when the thicknesses match.
            return if (a.half - walls[bi].half).abs() < 1e-4 && da.dot(db) < 0.0 { square() } else { None };
        }
        let hb = walls[bi].half;
        let nb = db.perp();
        // Pair this wall's ray-left face with the neighbor's ray-right face, and vice versa.
        let left = line_intersect(p + na * a.half, da, p - nb * hb, db)?;
        let right = line_intersect(p - na * a.half, da, p + nb * hb, db)?;
        return Some(EndJoin { ray_left: clamp(left), ray_right: clamp(right), host: None });
    }

    // Three or more ends: the thickest collinear pair runs through, the
    // others meet its face.
    if ends.len() >= 2 {
        let mut all: Vec<(usize, DVec2)> = vec![(me, da)];
        all.extend(ends.iter().copied());
        let mut through: Option<(usize, usize, f64)> = None;
        for (x, &(i, di)) in all.iter().enumerate() {
            for &(j, dj) in all.iter().skip(x + 1) {
                if di.dot(dj) < -0.9999 {
                    let weight = walls[i].half + walls[j].half;
                    if through.is_none_or(|t| weight > t.2 + 1e-9) {
                        through = Some((i, j, weight));
                    }
                }
            }
        }
        let (i, j, _) = through?;
        if me == i || me == j {
            return square();
        }
        return t_join(walls, me, p, da, i);
    }

    // T-join: the end lies on (or within a few centimeters of) another wall,
    // away from its ends.
    let mut best: Option<(usize, f64)> = None;
    for (i, b) in walls.iter().enumerate() {
        if i == me {
            continue;
        }
        let db = b.dir();
        let len = b.start.distance(b.end);
        let rel = p - b.start;
        let along = rel.dot(db);
        let off = rel.dot(db.perp()).abs();
        if along <= JOIN_TOL || along >= len - JOIN_TOL || off > b.half + GAP_TOL || da.perp_dot(db).abs() < 1e-6 {
            continue;
        }
        if best.is_none_or(|(_, o)| off < o) {
            best = Some((i, off));
        }
    }
    best.and_then(|(i, _)| t_join(walls, me, p, da, i))
}

/// Subtract `cut` from the sorted disjoint `intervals`.
fn subtract(intervals: &[(f64, f64)], cut: (f64, f64)) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &(a, b) in intervals {
        if cut.1 <= a || cut.0 >= b {
            out.push((a, b));
            continue;
        }
        if cut.0 > a + 1e-9 {
            out.push((a, cut.0));
        }
        if cut.1 < b - 1e-9 {
            out.push((cut.1, b));
        }
    }
    out
}

impl WallGeom {
    pub(crate) fn new(
        element: ElementId,
        line: WallLine,
        base_z: f64,
        top_z: f64,
        start: Option<EndJoin>,
        end: Option<EndJoin>,
        layers: Vec<(f64, f64, String)>,
    ) -> Self {
        let dir = line.dir();
        let length = line.start.distance(line.end);
        // At the start the ray runs along +u and ray-left is the wall's left;
        // at the end the ray runs along -u and ray-left is the wall's right.
        let start_ext = start.map_or((0.0, 0.0), |j| (j.ray_left, j.ray_right));
        let end_ext = end.map_or((length, length), |j| (length - j.ray_right, length - j.ray_left));
        Self {
            element,
            origin: line.start,
            dir,
            normal: dir.perp(),
            length,
            half: line.half,
            base_z,
            top_z,
            start_ext,
            end_ext,
            start_joined: start.is_some(),
            end_joined: end.is_some(),
            openings: Vec::new(),
            columns: Vec::new(),
            layers,
            face_gaps: Vec::new(),
        }
    }

    /// World plan position of wall-local `(u, v)`.
    pub fn world(&self, u: f64, v: f64) -> DVec2 {
        self.origin + self.dir * u + self.normal * v
    }

    fn world3(&self, u: f64, v: f64, z: f64) -> DVec3 {
        let p = self.world(u, v);
        DVec3::new(p.x, p.y, z)
    }

    fn local_dir3(&self, du: f64, dv: f64) -> DVec3 {
        let d = self.dir * du + self.normal * dv;
        DVec3::new(d.x, d.y, 0.0)
    }

    /// Range of u where both faces exist (where openings may be placed).
    pub fn inner_range(&self) -> (f64, f64) {
        (self.start_ext.0.max(self.start_ext.1), self.end_ext.0.min(self.end_ext.1))
    }

    /// The parts of the face on `side` (+1 left, -1 right) between `u0` and
    /// `u1` that no T-joined wall covers.
    pub fn face_segments(&self, side: f64, u0: f64, u1: f64) -> Vec<(f64, f64)> {
        let (lo, hi) = (u0.min(u1), u0.max(u1));
        let mut parts = vec![(lo, hi)];
        for &(s, g0, g1) in &self.face_gaps {
            if s == side {
                parts = subtract(&parts, (g0, g1));
            }
        }
        parts.retain(|(a, b)| b - a > 1e-6);
        parts
    }

    /// Plan footprint of a column, counter-clockwise.
    pub fn column_polygon(&self, c: &Column) -> [DVec2; 4] {
        let h = self.half;
        [self.world(c.right.0, -h), self.world(c.right.1, -h), self.world(c.left.1, h), self.world(c.left.0, h)]
    }

    /// Full plan footprint of the wall.
    pub fn footprint(&self) -> [DVec2; 4] {
        let h = self.half;
        [
            self.world(self.start_ext.1, -h),
            self.world(self.end_ext.1, -h),
            self.world(self.end_ext.0, h),
            self.world(self.start_ext.0, h),
        ]
    }

    /// Cut openings and split the wall into columns.
    pub(crate) fn set_openings(&mut self, mut openings: Vec<Opening>) {
        let (lo, hi) = self.inner_range();
        let eps = 1e-4;
        for o in &mut openings {
            o.u0 = o.u0.max(lo + eps);
            o.u1 = o.u1.min(hi - eps);
            o.z0 = o.z0.max(self.base_z);
            o.z1 = o.z1.min(self.top_z);
        }
        openings.retain(|o| o.u1 - o.u0 > eps && o.z1 - o.z0 > eps);

        let mut bounds: Vec<f64> = openings.iter().flat_map(|o| [o.u0, o.u1]).collect();
        bounds.sort_by(f64::total_cmp);
        bounds.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

        let mut cols = Vec::with_capacity(bounds.len() + 1);
        for i in 0..=bounds.len() {
            let left = (
                if i == 0 { self.start_ext.0 } else { bounds[i - 1] },
                if i == bounds.len() { self.end_ext.0 } else { bounds[i] },
            );
            let right = (
                if i == 0 { self.start_ext.1 } else { bounds[i - 1] },
                if i == bounds.len() { self.end_ext.1 } else { bounds[i] },
            );
            let mid = (left.0.max(right.0) + left.1.min(right.1)) * 0.5;
            let mut solid = vec![(self.base_z, self.top_z)];
            for o in openings.iter().filter(|o| o.u0 <= mid && mid <= o.u1) {
                solid = subtract(&solid, (o.z0, o.z1));
            }
            cols.push(Column { left, right, solid });
        }
        self.columns = cols;
        self.openings = openings;
    }

    /// Net volume: each column's plan area times its solid height. Computed
    /// analytically because the body solid omits end caps at joins.
    pub fn volume(&self) -> f64 {
        self.columns
            .iter()
            .map(|c| {
                let height: f64 = c.solid.iter().map(|(z0, z1)| z1 - z0).sum();
                crate::polygon::area(&self.column_polygon(c)) * height
            })
            .sum()
    }

    pub fn column_at(&self, u: f64) -> Option<&Column> {
        self.columns.iter().find(|c| c.left.0.max(c.right.0) <= u && u <= c.left.1.min(c.right.1))
    }

    /// Exterior faces of the wall body.
    pub fn to_solid(&self, color: [u8; 3]) -> Solid {
        let mut s = Solid::new(self.element, Category::Wall, color);
        let h = self.half;
        let left_n = self.local_dir3(0.0, 1.0);
        let along = self.local_dir3(1.0, 0.0);
        let quad = |u0: f64, v0: f64, u1: f64, v1: f64, z0: f64, z1: f64, n: DVec3| {
            Face::new(
                vec![
                    self.world3(u0, v0, z0),
                    self.world3(u1, v1, z0),
                    self.world3(u1, v1, z1),
                    self.world3(u0, v0, z1),
                ],
                n,
            )
        };
        for (ci, c) in self.columns.iter().enumerate() {
            let poly = self.column_polygon(c);
            for &(z0, z1) in &c.solid {
                s.faces.push(Face::new(poly.iter().map(|p| DVec3::new(p.x, p.y, z1)).collect(), DVec3::Z));
                s.faces.push(Face::new(poly.iter().rev().map(|p| DVec3::new(p.x, p.y, z0)).collect(), -DVec3::Z));
                s.faces.push(quad(c.right.0, -h, c.right.1, -h, z0, z1, -left_n));
                s.faces.push(quad(c.left.1, h, c.left.0, h, z0, z1, left_n));
            }
            // Faces on the boundary with the next column where only one side is solid.
            if let Some(next) = self.columns.get(ci + 1) {
                let u = c.left.1;
                for &iv in &c.solid {
                    let exposed = next
                        .solid
                        .iter()
                        .fold(vec![iv], |acc, &cut| acc.iter().flat_map(|&i| subtract(&[i], cut)).collect());
                    for (z0, z1) in exposed {
                        s.faces.push(quad(u, -h, u, h, z0, z1, along));
                    }
                }
                for &iv in &next.solid {
                    let exposed = c
                        .solid
                        .iter()
                        .fold(vec![iv], |acc, &cut| acc.iter().flat_map(|&i| subtract(&[i], cut)).collect());
                    for (z0, z1) in exposed {
                        s.faces.push(quad(u, h, u, -h, z0, z1, -along));
                    }
                }
            }
        }
        // End caps, omitted where another wall closes the joint.
        let cap = |s: &mut Solid, col: &Column, (ul, ur): (f64, f64), outward: f64| {
            let e = self.world(ul, h) - self.world(ur, -h);
            let mut n = e.perp().normalize_or_zero();
            if n.dot(self.dir * outward) < 0.0 {
                n = -n;
            }
            let n3 = DVec3::new(n.x, n.y, 0.0);
            for &(z0, z1) in &col.solid {
                s.faces.push(quad(ur, -h, ul, h, z0, z1, n3));
            }
        };
        if !self.start_joined
            && let Some(c) = self.columns.first()
        {
            cap(&mut s, c, self.start_ext, -1.0);
        }
        if !self.end_joined
            && let Some(c) = self.columns.last()
        {
            cap(&mut s, c, self.end_ext, 1.0);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> WallLine {
        WallLine { start: DVec2::new(a.0, a.1), end: DVec2::new(b.0, b.1), half: 0.1, join_start: true, join_end: true }
    }

    fn geom(walls: &[WallLine], i: usize) -> WallGeom {
        let s = resolve_join(walls, i, true);
        let e = resolve_join(walls, i, false);
        let mut g = WallGeom::new(ElementId(i as u64), walls[i], 0.0, 3.0, s, e, vec![]);
        g.set_openings(vec![]);
        g
    }

    #[test]
    fn free_wall_volume() {
        let g = geom(&[line((0.0, 0.0), (5.0, 0.0))], 0);
        assert!((g.to_solid([0; 3]).volume() - 5.0 * 0.2 * 3.0).abs() < 1e-9);
    }

    #[test]
    fn l_join_miters_both_walls() {
        let walls = [line((0.0, 0.0), (4.0, 0.0)), line((4.0, 0.0), (4.0, 3.0))];
        let a = geom(&walls, 0);
        // Wall A runs +X with left face towards +Y (the inside of the corner).
        assert!((a.end_ext.0 - 3.9).abs() < 1e-9, "{:?}", a.end_ext);
        assert!((a.end_ext.1 - 4.1).abs() < 1e-9, "{:?}", a.end_ext);
        assert!(a.end_joined && !a.start_joined);
        let b = geom(&walls, 1);
        // Wall B runs +Y with its left face towards -X (also the inside).
        assert!((b.start_ext.0 - 0.1).abs() < 1e-9 && (b.start_ext.1 - (-0.1)).abs() < 1e-9, "{:?}", b.start_ext);
    }

    #[test]
    fn t_join_stops_at_face() {
        let walls = [line((0.0, 0.0), (6.0, 0.0)), line((3.0, 0.0), (3.0, 4.0))];
        let b = geom(&walls, 1);
        assert!((b.start_ext.0 - 0.1).abs() < 1e-9 && (b.start_ext.1 - 0.1).abs() < 1e-9);
        let a = geom(&walls, 0);
        assert!(!a.start_joined && !a.end_joined);
    }

    #[test]
    fn opening_removes_volume() {
        let walls = [line((0.0, 0.0), (5.0, 0.0))];
        let mut g = geom(&walls, 0);
        g.set_openings(vec![Opening { element: ElementId(9), u0: 1.0, u1: 2.0, z0: 1.0, z1: 2.0 }]);
        assert_eq!(g.columns.len(), 3);
        assert_eq!(g.columns[1].solid, vec![(0.0, 1.0), (2.0, 3.0)]);
        let v = g.to_solid([0; 3]).volume();
        assert!((v - (3.0 - 0.2)).abs() < 1e-9, "{v}");
    }

    #[test]
    fn disallowed_ends_stay_square_and_neighbors_ignore_them() {
        // An L corner at (4, 0).
        let mut walls = [line((0.0, 0.0), (4.0, 0.0)), line((4.0, 0.0), (4.0, 3.0))];
        assert!(resolve_join(&walls, 0, false).is_some());
        assert!(resolve_join(&walls, 1, true).is_some());
        walls[0].join_end = false;
        assert!(resolve_join(&walls, 0, false).is_none());
        // The neighbor no longer miters against it either.
        assert!(resolve_join(&walls, 1, true).is_none());
    }

    #[test]
    fn ends_at_the_face_or_slightly_short_join_in_a_t() {
        // Half thickness 0.1: an end at the face (0.1) and one 3 cm short (0.13) both join.
        for y in [0.0, 0.1, 0.13] {
            let walls = [line((0.0, 0.0), (6.0, 0.0)), line((3.0, y), (3.0, 2.0))];
            let j = resolve_join(&walls, 1, true).unwrap_or_else(|| panic!("no join at {y}"));
            assert_eq!(j.host, Some(0));
            // Cut along the face y = 0.1, measured from the end along the wall.
            assert!((j.ray_left - (0.1 - y)).abs() < 1e-9 && (j.ray_right - (0.1 - y)).abs() < 1e-9, "{y}: {j:?}");
        }
        // Too far away: no join.
        let walls = [line((0.0, 0.0), (6.0, 0.0)), line((3.0, 0.3), (3.0, 2.0))];
        assert!(resolve_join(&walls, 1, true).is_none());
    }

    #[test]
    fn junctions_of_three_or_four_walls_run_one_pair_through() {
        // Collinear pair meeting at (2, 0) plus a wall going up.
        let walls = [line((0.0, 0.0), (2.0, 0.0)), line((2.0, 0.0), (4.0, 0.0)), line((2.0, 0.0), (2.0, 2.0))];
        let a = resolve_join(&walls, 0, false).unwrap();
        let b = resolve_join(&walls, 1, true).unwrap();
        assert_eq!((a.ray_left, a.ray_right, a.host), (0.0, 0.0, None));
        assert_eq!((b.ray_left, b.ray_right, b.host), (0.0, 0.0, None));
        let c = resolve_join(&walls, 2, true).unwrap();
        assert!(c.host.is_some());
        assert!((c.ray_left - 0.1).abs() < 1e-9 && (c.ray_right - 0.1).abs() < 1e-9);
        // A plus: the thicker pair runs through.
        let mut plus = [
            line((0.0, 0.0), (2.0, 0.0)),
            line((2.0, 0.0), (4.0, 0.0)),
            line((2.0, -2.0), (2.0, 0.0)),
            line((2.0, 0.0), (2.0, 2.0)),
        ];
        plus[2].half = 0.15;
        plus[3].half = 0.15;
        assert_eq!(resolve_join(&plus, 2, false).map(|j| j.host), Some(None));
        assert_eq!(resolve_join(&plus, 0, false).and_then(|j| j.host), Some(2));
        // Collinear walls of the same thickness meet without a seam; different ones keep it.
        let pair = [line((0.0, 0.0), (2.0, 0.0)), line((2.0, 0.0), (4.0, 0.0))];
        assert!(resolve_join(&pair, 0, false).is_some());
        let mut pair = pair;
        pair[1].half = 0.2;
        assert!(resolve_join(&pair, 0, false).is_none());
    }

    #[test]
    fn faces_open_where_walls_meet_them() {
        let walls = [line((0.0, 0.0), (6.0, 0.0)), line((3.0, 0.1), (3.0, 2.0))];
        let mut host = geom(&walls, 0);
        host.face_gaps.push((1.0, 2.9, 3.1));
        assert_eq!(host.face_segments(1.0, 0.0, 6.0), vec![(0.0, 2.9), (3.1, 6.0)]);
        assert_eq!(host.face_segments(-1.0, 0.0, 6.0), vec![(0.0, 6.0)]);
    }
}
