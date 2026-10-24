//! Weighted straight skeleton of a polygon with holes, for roofs.
//!
//! Every edge of the footprint is the eave of a roof plane rising inwards;
//! its weight is how far the plane's line moves inwards per unit of height
//! (`1 / slope`), and 0 for a gable (a vertical plane that stays put). The
//! skeleton follows the textbook wavefront simulation: the edges move in
//! at their speeds, each vertex along the line where its two planes meet,
//! and the topology changes at two kinds of events:
//!
//! * an *edge event*, when an edge shrinks to nothing and its neighbors meet;
//! * a *split event*, when a reflex vertex runs into another edge, which
//!   splits a wavefront loop in two (or joins a hole to the loop around it).
//!
//! Time is height above the eaves. Between events the wavefront moves
//! linearly, so each edge sweeps quads of its own roof plane; these are
//! merged into one polygon per plane. Vertex paths are the hips and
//! valleys, and loops that collapse leave the ridges. The simulation looks
//! for the next event among all vertices and edges at each step
//! (quadratic per event, cubic overall: footprints have tens of edges),
//! merges points closer than a tolerance relative to the footprint size,
//! and gives up with an error after a bounded number of events.

use crate::polygon;
use glam::DVec2;

/// Why a skeleton could not be computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkeletonError {
    /// Fewer than three distinct points, or numbers that are not finite.
    Invalid,
    /// The simulation did not finish within its event budget.
    Budget,
}

/// One roof plane: its eave line, inward normal and speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// A point of the eave and its direction (unit), interior on the left.
    pub origin: DVec2,
    pub dir: DVec2,
    /// Length of the eave.
    pub length: f64,
    /// Inward horizontal distance per unit of height (0: a vertical gable).
    pub weight: f64,
}

impl Plane {
    /// Height above the eaves of the plane at `p` (meaningless for gables).
    pub fn height(&self, p: DVec2) -> f64 {
        if self.weight <= 0.0 {
            return 0.0;
        }
        (p - self.origin).dot(self.dir.perp()) / self.weight
    }
}

/// The skeleton of a footprint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Skeleton {
    /// The planes, one per input edge (loops in order, edge `i` from point `i` to `i + 1`).
    pub planes: Vec<Plane>,
    /// The part of each sloped plane over the footprint: `(plane, polygon)`,
    /// counter-clockwise in plan.
    pub faces: Vec<(usize, Vec<DVec2>)>,
    /// Ridges, hips and valleys in plan.
    pub lines: Vec<(DVec2, DVec2)>,
    /// Where sloped planes end at a gable: segments along gable eaves with
    /// their heights, and the horizontal direction away from the sloped
    /// plane (the outside of the roof there).
    pub gables: Vec<(DVec2, f64, DVec2, f64, DVec2)>,
    /// Horizontal tops where the planes never close (all edges around them gables): `(height, polygon)`.
    pub flats: Vec<(f64, Vec<DVec2>)>,
}

impl Skeleton {
    /// Height above the eaves at a point of face `i`.
    pub fn face_height(&self, plane: usize, p: DVec2) -> f64 {
        self.planes.get(plane).map_or(0.0, |pl| pl.height(p))
    }
}

#[derive(Debug, Clone, Copy)]
struct Vertex {
    id: usize,
    pos: DVec2,
    /// Plane of the edge from this vertex to the next.
    out: usize,
}

/// A run of a vertex path being collected into one straight line.
#[derive(Debug, Clone, Copy)]
struct Run {
    start: DVec2,
    end: DVec2,
    dir: DVec2,
}

struct Sim {
    planes: Vec<Plane>,
    loops: Vec<Vec<Vertex>>,
    next_id: usize,
    /// Merge distance.
    tol: f64,
    /// Size of the footprint.
    size: f64,
    time: f64,
    /// Directed boundary segments of the quads swept by each plane.
    swept: Vec<Vec<(DVec2, DVec2)>>,
    runs: std::collections::BTreeMap<usize, Run>,
    out: Skeleton,
}

/// Velocity of a vertex between edges of planes `a` (incoming) and `b` (outgoing).
fn velocity(a: &Plane, b: &Plane) -> DVec2 {
    let (na, nb) = (a.dir.perp(), b.dir.perp());
    let det = na.perp_dot(nb);
    if det.abs() < 1e-9 {
        // Parallel edges: straight on (or a spike, cleaned up as it appears).
        return (na * a.weight + nb * b.weight) * 0.5;
    }
    // n_a · v = w_a and n_b · v = w_b.
    DVec2::new(a.weight * nb.y - b.weight * na.y, na.x * b.weight - nb.x * a.weight) / det
}

impl Sim {
    fn plane(&self, i: usize) -> Plane {
        self.planes.get(i).copied().unwrap_or(Plane { origin: DVec2::ZERO, dir: DVec2::X, length: 0.0, weight: 0.0 })
    }

    fn in_plane(&self, l: usize, k: usize) -> usize {
        let lp = &self.loops[l];
        lp[(k + lp.len() - 1) % lp.len()].out
    }

    fn velocities(&self) -> Vec<Vec<DVec2>> {
        self.loops
            .iter()
            .enumerate()
            .map(|(l, lp)| {
                (0..lp.len()).map(|k| velocity(&self.plane(self.in_plane(l, k)), &self.plane(lp[k].out))).collect()
            })
            .collect()
    }

    fn is_reflex(&self, l: usize, k: usize) -> bool {
        let a = self.plane(self.in_plane(l, k)).dir;
        let b = self.plane(self.loops[l][k].out).dir;
        a.perp_dot(b) < -1e-9
    }

    fn flush_run(&mut self, id: usize) {
        if let Some(run) = self.runs.remove(&id)
            && run.start.distance(run.end) > self.tol
        {
            self.out.lines.push((run.start, run.end));
        }
    }

    /// Record a straight piece of a vertex path, extending its current run.
    fn path(&mut self, id: usize, a: DVec2, b: DVec2) {
        let Some(dir) = (b - a).try_normalize() else { return };
        if let Some(run) = self.runs.get_mut(&id)
            && run.dir.dot(dir) > 1.0 - 1e-9
            && run.end.distance(a) <= self.tol
        {
            run.end = b;
            return;
        }
        self.flush_run(id);
        self.runs.insert(id, Run { start: a, end: b, dir });
    }

    /// The earliest event after now: its delay, or `None` when nothing moves towards anything.
    fn next_event(&self, vel: &[Vec<DVec2>]) -> Option<f64> {
        let mut best: Option<f64> = None;
        // Events now were handled by `cleanup` and `splits`.
        let soon = self.tol * 1e-3;
        let mut consider = |dt: f64| {
            if dt.is_finite() && dt > soon && best.is_none_or(|b| dt < b) {
                best = Some(dt);
            }
        };
        for (l, lp) in self.loops.iter().enumerate() {
            let n = lp.len();
            for k in 0..n {
                // Edge event: the edge k → k+1 shrinks to nothing.
                let j = (k + 1) % n;
                let d = self.plane(lp[k].out).dir;
                let len = (lp[j].pos - lp[k].pos).dot(d);
                let rate = (vel[l][j] - vel[l][k]).dot(d);
                if rate < -1e-12 {
                    consider(-len / rate);
                }
            }
            // Split events: a reflex vertex reaching another edge.
            for k in (0..n).filter(|&k| self.is_reflex(l, k)) {
                let (p, v) = (lp[k].pos, vel[l][k]);
                for (m, other) in self.loops.iter().enumerate() {
                    let on = other.len();
                    for e in 0..on {
                        let f = (e + 1) % on;
                        if m == l && (e == k || f == k) {
                            continue;
                        }
                        let pl = self.plane(other[e].out);
                        let normal = pl.dir.perp();
                        let gap = (p - other[e].pos).dot(normal);
                        let closing = pl.weight - normal.dot(v);
                        if gap < -self.tol || closing <= 1e-12 {
                            continue;
                        }
                        let dt = gap.max(0.0) / closing;
                        // Within the edge when it gets there.
                        let x = p + v * dt;
                        let (qa, qb) = (other[e].pos + vel[m][e] * dt, other[f].pos + vel[m][f] * dt);
                        let s = (x - qa).dot(pl.dir);
                        let length = (qb - qa).dot(pl.dir);
                        if s >= -self.tol && s <= length + self.tol {
                            consider(dt);
                        }
                    }
                }
            }
        }
        best
    }

    /// Move the wavefront by `dt`, recording what it sweeps.
    fn advance(&mut self, vel: &[Vec<DVec2>], dt: f64) {
        let t0 = self.time;
        let t1 = t0 + dt;
        for (l, vl) in vel.iter().enumerate() {
            let n = self.loops.get(l).map_or(0, Vec::len).min(vl.len());
            for k in 0..n {
                let j = (k + 1) % n;
                let (a, b) = (self.loops[l][k], self.loops[l][j]);
                let (a1, b1) = (a.pos + vl[k] * dt, b.pos + vl[j] * dt);
                let plane = self.plane(a.out);
                if plane.weight > 0.0 {
                    if let Some(s) = self.swept.get_mut(a.out) {
                        s.extend([(a.pos, b.pos), (b.pos, b1), (b1, a1), (a1, a.pos)]);
                    }
                } else {
                    // A gable: its ends rise along its eave, where the sloped planes
                    // beside it end (on the side their eaves run to).
                    let before = self.plane(self.in_plane(l, k));
                    let after = self.plane(b.out);
                    for (p, q, side) in [(a.pos, a1, -before.dir), (b.pos, b1, after.dir)] {
                        let across = side - plane.dir * side.dot(plane.dir);
                        if p.distance(q) > self.tol
                            && let Some(face) = across.try_normalize()
                        {
                            self.out.gables.push((p, t0, q, t1, -face));
                        }
                    }
                }
            }
            for (k, vk) in vl.iter().enumerate().take(n) {
                let v = self.loops[l][k];
                let along_gable = self.plane(self.in_plane(l, k)).weight <= 0.0 || self.plane(v.out).weight <= 0.0;
                let to = v.pos + *vk * dt;
                if along_gable {
                    self.flush_run(v.id);
                } else {
                    self.path(v.id, v.pos, to);
                }
            }
        }
        for (l, lp) in self.loops.iter_mut().enumerate() {
            for (k, v) in lp.iter_mut().enumerate() {
                v.pos += vel[l][k] * dt;
            }
        }
        self.time = t1;
    }

    fn ridge(&mut self, a: DVec2, b: DVec2) {
        if a.distance(b) <= self.tol {
            return;
        }
        let dup = self.out.lines.iter().any(|&(p, q)| {
            (p.distance(a) <= self.tol && q.distance(b) <= self.tol)
                || (p.distance(b) <= self.tol && q.distance(a) <= self.tol)
        });
        if !dup {
            self.out.lines.push((a, b));
        }
    }

    /// Merge vanished edges, remove spikes and drop collapsed loops (their
    /// remains are ridges). Returns whether anything changed.
    fn cleanup(&mut self) -> bool {
        let mut changed = false;
        let budget = 4 * self.loops.iter().map(Vec::len).sum::<usize>() + 16;
        for _ in 0..budget {
            let mut acted = false;
            let mut l = 0;
            while l < self.loops.len() {
                let n = self.loops[l].len();
                let area = polygon::signed_area(&self.loops[l].iter().map(|v| v.pos).collect::<Vec<_>>());
                if n < 3 || area.abs() <= self.tol * self.size * 10.0 {
                    let lp = self.loops.remove(l);
                    for k in 0..lp.len() {
                        self.flush_run(lp[k].id);
                        let next = lp[(k + 1) % lp.len()];
                        if lp.len() >= 2 {
                            self.ridge(lp[k].pos, next.pos);
                        }
                    }
                    acted = true;
                    continue;
                }
                // A vanished edge: its ends merge (the vertex keeps the next edge's plane).
                if let Some(k) =
                    (0..n).find(|&k| self.loops[l][k].pos.distance(self.loops[l][(k + 1) % n].pos) <= self.tol)
                {
                    let j = (k + 1) % n;
                    let next = self.loops[l][j];
                    let mid = (self.loops[l][k].pos + next.pos) * 0.5;
                    self.flush_run(self.loops[l][k].id);
                    self.flush_run(next.id);
                    let id = self.next_id;
                    self.next_id += 1;
                    self.loops[l][k] = Vertex { id, pos: mid, out: next.out };
                    self.loops[l].remove(j);
                    acted = true;
                    continue;
                }
                // A spike: the edges on both sides run back along each other.
                let spike = (0..n).find(|&k| {
                    let a = self.plane(self.in_plane(l, k)).dir;
                    let b = self.plane(self.loops[l][k].out).dir;
                    a.perp_dot(b).abs() < 1e-9 && a.dot(b) < 0.0
                });
                if let Some(k) = spike {
                    let lp = &self.loops[l];
                    let (prev, tip, next) = (lp[(k + n - 1) % n], lp[k], lp[(k + 1) % n]);
                    let near = if tip.pos.distance(prev.pos) < tip.pos.distance(next.pos) { prev } else { next };
                    self.flush_run(tip.id);
                    self.ridge(tip.pos, near.pos);
                    // The edge left between prev and next runs like the longer of the two.
                    let keep = if near.id == prev.id { tip.out } else { prev.out };
                    let p = (k + n - 1) % n;
                    self.loops[l][p].out = keep;
                    self.loops[l].remove(k);
                    acted = true;
                    continue;
                }
                l += 1;
            }
            if !acted {
                return changed;
            }
            changed = true;
        }
        changed
    }

    /// Split (or join) loops where a reflex vertex lies on another edge.
    fn splits(&mut self) -> bool {
        let tol = self.tol;
        for l in 0..self.loops.len() {
            let n = self.loops[l].len();
            for k in 0..n {
                if !self.is_reflex(l, k) {
                    continue;
                }
                let p = self.loops[l][k].pos;
                for m in 0..self.loops.len() {
                    let on = self.loops[m].len();
                    for e in 0..on {
                        let f = (e + 1) % on;
                        if m == l && (e == k || f == k) {
                            continue;
                        }
                        // On the edge, or at one of its ends (a vertex event).
                        let (a, b) = (self.loops[m][e].pos, self.loops[m][f].pos);
                        let (q, _) = polygon::closest_on_segment(a, b, p);
                        if q.distance(p) > tol {
                            continue;
                        }
                        self.split(l, k, m, e);
                        return true;
                    }
                }
            }
        }
        false
    }

    fn split(&mut self, l: usize, k: usize, m: usize, e: usize) {
        let r = self.loops[l][k];
        let plane_e = self.loops[m][e].out;
        self.flush_run(r.id);
        let first = Vertex { id: self.next_id, pos: r.pos, out: plane_e };
        let second = Vertex { id: self.next_id + 1, pos: r.pos, out: r.out };
        self.next_id += 2;
        if l == m {
            let lp = self.loops.remove(l);
            let n = lp.len();
            // [r, e+1 .. k-1] and [r', k+1 .. e].
            let mut one = vec![first];
            let mut i = (e + 1) % n;
            while i != k {
                one.push(lp[i]);
                i = (i + 1) % n;
            }
            let mut two = vec![second];
            let mut i = (k + 1) % n;
            while i != (e + 1) % n {
                two.push(lp[i]);
                i = (i + 1) % n;
            }
            self.loops.push(one);
            self.loops.push(two);
        } else {
            let (hi, lo) = if l > m { (l, m) } else { (m, l) };
            let a = self.loops.remove(hi);
            let b = self.loops.remove(lo);
            let (own, other) = if l > m { (a, b) } else { (b, a) };
            let (n, on) = (own.len(), other.len());
            let mut joined = vec![first];
            for i in 1..=on {
                joined.push(other[(e + i) % on]);
            }
            joined.push(second);
            for i in 1..n {
                joined.push(own[(k + i) % n]);
            }
            self.loops.push(joined);
        }
    }

    /// One polygon per sloped plane from the quads it swept.
    fn faces(&mut self) {
        let tol = self.tol;
        for (plane, segs) in std::mem::take(&mut self.swept).into_iter().enumerate() {
            let segs: Vec<(DVec2, DVec2)> = segs.into_iter().filter(|(a, b)| a.distance(*b) > tol).collect();
            // Split segments where others end on them, so shared pieces match.
            let points: Vec<DVec2> = segs.iter().flat_map(|(a, b)| [*a, *b]).collect();
            let mut pieces: Vec<(DVec2, DVec2)> = Vec::new();
            for &(a, b) in &segs {
                let mut ts: Vec<f64> = points
                    .iter()
                    .filter_map(|&p| {
                        let (q, s) = polygon::closest_on_segment(a, b, p);
                        (q.distance(p) <= tol && s > 1e-9 && s < 1.0 - 1e-9).then_some(s)
                    })
                    .collect();
                ts.push(0.0);
                ts.push(1.0);
                ts.sort_by(f64::total_cmp);
                ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
                for w in ts.windows(2) {
                    let (p, q) = (a.lerp(b, w[0]), a.lerp(b, w[1]));
                    if p.distance(q) > tol {
                        pieces.push((p, q));
                    }
                }
            }
            // Pieces shared by two quads (in opposite directions) are inside the face.
            let mut keep = vec![true; pieces.len()];
            for i in 0..pieces.len() {
                if !keep[i] {
                    continue;
                }
                let (a, b) = pieces[i];
                if let Some(j) = (i + 1..pieces.len())
                    .find(|&j| keep[j] && pieces[j].0.distance(b) <= tol && pieces[j].1.distance(a) <= tol)
                {
                    keep[i] = false;
                    keep[j] = false;
                }
            }
            let mut rest: Vec<(DVec2, DVec2)> =
                pieces.into_iter().zip(keep).filter(|(_, k)| *k).map(|(s, _)| s).collect();
            // Chain them into loops.
            let mut guard = rest.len() + 1;
            while let Some((start, mut at)) = rest.pop() {
                let mut ring = vec![start];
                while at.distance(start) > tol && guard > 0 {
                    guard -= 1;
                    let Some(i) = rest.iter().position(|(a, _)| a.distance(at) <= tol) else { break };
                    let (a, b) = rest.swap_remove(i);
                    ring.push(a);
                    at = b;
                }
                if ring.len() >= 3 && polygon::signed_area(&ring) > tol * tol {
                    self.out.faces.push((plane, ring));
                }
            }
        }
    }
}

/// The weighted straight skeleton of `loops` (the outer boundary
/// counter-clockwise, holes clockwise: the interior on the left of every
/// edge), with `weights[l][i]` the inward speed of edge `i` of loop `l`
/// per unit of height (0 for a gable).
pub fn skeleton(loops: &[Vec<DVec2>], weights: &[Vec<f64>]) -> Result<Skeleton, SkeletonError> {
    let mut planes = Vec::new();
    let mut wave = Vec::new();
    let mut next_id = 0;
    let mut lo = DVec2::splat(f64::INFINITY);
    let mut hi = DVec2::splat(f64::NEG_INFINITY);
    for (l, pts) in loops.iter().enumerate() {
        let mut pts: Vec<DVec2> = pts.clone();
        pts.dedup_by(|a, b| a.distance(*b) < 1e-9);
        while pts.len() > 1 && pts.first().zip(pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-9) {
            pts.pop();
        }
        if pts.len() < 3 {
            if l == 0 {
                return Err(SkeletonError::Invalid);
            }
            continue;
        }
        let mut lp = Vec::with_capacity(pts.len());
        for (i, &a) in pts.iter().enumerate() {
            let b = pts[(i + 1) % pts.len()];
            let w = weights.get(l).and_then(|w| w.get(i)).copied().unwrap_or(0.0);
            if !(a.is_finite() && b.is_finite() && w.is_finite()) {
                return Err(SkeletonError::Invalid);
            }
            let Some(dir) = (b - a).try_normalize() else { return Err(SkeletonError::Invalid) };
            lo = lo.min(a);
            hi = hi.max(a);
            planes.push(Plane { origin: a, dir, length: a.distance(b), weight: w.max(0.0) });
            lp.push(Vertex { id: next_id, pos: a, out: planes.len() - 1 });
            next_id += 1;
        }
        wave.push(lp);
    }
    let size = (hi - lo).length();
    if !(size.is_finite() && size > 1e-9) {
        return Err(SkeletonError::Invalid);
    }
    let n_planes = planes.len();
    let mut sim = Sim {
        out: Skeleton { planes: planes.clone(), ..Default::default() },
        planes,
        loops: wave,
        next_id,
        tol: size * 1e-9,
        size,
        time: 0.0,
        swept: vec![Vec::new(); n_planes],
        runs: Default::default(),
    };
    let budget = 8 * n_planes + 64;
    for _ in 0..budget {
        // Topology changes at the current time.
        let mut settle = 4 * n_planes + 16;
        while settle > 0 && (sim.cleanup() | sim.splits()) {
            settle -= 1;
        }
        if sim.loops.is_empty() {
            let ids: Vec<usize> = sim.runs.keys().copied().collect();
            ids.into_iter().for_each(|id| sim.flush_run(id));
            sim.faces();
            return Ok(sim.out);
        }
        let vel = sim.velocities();
        if vel.iter().flatten().any(|v| !v.is_finite()) {
            return Err(SkeletonError::Budget);
        }
        match sim.next_event(&vel) {
            Some(dt) => sim.advance(&vel, dt),
            None => {
                // Nothing closes: what remains is flat at this height.
                let time = sim.time;
                for lp in std::mem::take(&mut sim.loops) {
                    let pts: Vec<DVec2> = lp.iter().map(|v| v.pos).collect();
                    if polygon::signed_area(&pts) > 0.0 {
                        sim.out.flats.push((time, pts));
                    }
                }
            }
        }
    }
    Err(SkeletonError::Budget)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, d: f64) -> Vec<DVec2> {
        vec![DVec2::ZERO, DVec2::new(w, 0.0), DVec2::new(w, d), DVec2::new(0.0, d)]
    }

    fn faces_area(s: &Skeleton) -> f64 {
        s.faces.iter().map(|(_, f)| polygon::area(f)).sum::<f64>()
            + s.flats.iter().map(|(_, f)| polygon::area(f)).sum::<f64>()
    }

    fn max_height(s: &Skeleton) -> f64 {
        s.faces.iter().flat_map(|(i, f)| f.iter().map(|p| s.face_height(*i, *p))).fold(0.0, f64::max)
    }

    #[test]
    fn hip_roof_on_a_square_peaks_at_half_its_width() {
        let s = skeleton(&[rect(8.0, 8.0)], &[vec![1.0; 4]]).unwrap();
        assert_eq!(s.faces.len(), 4);
        assert!((faces_area(&s) - 64.0).abs() < 1e-9);
        assert!((max_height(&s) - 4.0).abs() < 1e-9);
        assert_eq!(s.lines.len(), 4, "four hips to the apex: {:?}", s.lines);
    }

    #[test]
    fn hip_roof_on_a_rectangle_has_a_ridge() {
        let s = skeleton(&[rect(10.0, 6.0)], &[vec![1.0; 4]]).unwrap();
        assert!((faces_area(&s) - 60.0).abs() < 1e-9);
        assert_eq!(s.lines.len(), 5, "{:?}", s.lines);
        assert!(
            s.lines.iter().any(|&(a, b)| (a.y - 3.0).abs() < 1e-9 && (b.y - 3.0).abs() < 1e-9 && a.distance(b) > 3.9)
        );
    }

    #[test]
    fn gables_stay_vertical() {
        // Ridge along X: the short edges are gables.
        let s = skeleton(&[rect(10.0, 6.0)], &[vec![1.0, 0.0, 1.0, 0.0]]).unwrap();
        assert_eq!(s.faces.len(), 2);
        assert!((faces_area(&s) - 60.0).abs() < 1e-9);
        // The ridge runs the whole length at half the depth.
        assert!(s.lines.iter().any(|&(a, b)| a.distance(b) > 9.99 && (a.y - 3.0).abs() < 1e-9), "{:?}", s.lines);
        let top = s.gables.iter().map(|g| g.1.max(g.3)).fold(0.0, f64::max);
        assert!((top - 3.0).abs() < 1e-9);
    }

    #[test]
    fn l_shape_has_a_valley() {
        let l = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 4.0),
            DVec2::new(4.0, 4.0),
            DVec2::new(4.0, 10.0),
            DVec2::new(0.0, 10.0),
        ];
        let s = skeleton(&[l], &[vec![1.0; 6]]).unwrap();
        assert!((faces_area(&s) - 64.0).abs() < 1e-6, "{}", faces_area(&s));
        // The valley runs from the inner corner at 45° into the roof.
        let valley = s.lines.iter().any(|&(a, b)| {
            let (p, q) = if a.distance(DVec2::new(4.0, 4.0)) < 1e-6 { (a, b) } else { (b, a) };
            p.distance(DVec2::new(4.0, 4.0)) < 1e-6
                && ((q - p).normalize() - DVec2::new(-1.0, -1.0).normalize()).length() < 1e-6
        });
        assert!(valley, "{:?}", s.lines);
        assert!((max_height(&s) - 2.0).abs() < 1e-9);
        // No point of the roof is below the eaves.
        assert!(s.faces.iter().all(|(i, f)| f.iter().all(|p| s.face_height(*i, *p) > -1e-9)));
    }

    #[test]
    fn courtyard_roof_slopes_to_both_eaves() {
        // A 12 x 12 square with a 4 x 4 courtyard in the middle (clockwise).
        let outer = rect(12.0, 12.0);
        let hole: Vec<DVec2> =
            [(4.0, 4.0), (4.0, 8.0), (8.0, 8.0), (8.0, 4.0)].iter().map(|&(x, y)| DVec2::new(x, y)).collect();
        let s = skeleton(&[outer, hole], &[vec![1.0; 4], vec![1.0; 4]]).unwrap();
        assert!((faces_area(&s) - 128.0).abs() < 1e-6, "{}", faces_area(&s));
        // The ring is 4 m wide: the ridge is 2 m up, midway.
        assert!((max_height(&s) - 2.0).abs() < 1e-9);
        assert_eq!(s.faces.len(), 8);
    }

    #[test]
    fn u_and_t_shapes_close() {
        let u: Vec<DVec2> =
            [(0.0, 0.0), (9.0, 0.0), (9.0, 8.0), (6.0, 8.0), (6.0, 3.0), (3.0, 3.0), (3.0, 8.0), (0.0, 8.0)]
                .iter()
                .map(|&(x, y)| DVec2::new(x, y))
                .collect();
        let area = polygon::area(&u);
        let s = skeleton(std::slice::from_ref(&u), &[vec![1.0; 8]]).unwrap();
        assert!((faces_area(&s) - area).abs() < 1e-6, "{} vs {area}", faces_area(&s));
        let t: Vec<DVec2> =
            [(0.0, 6.0), (3.0, 6.0), (3.0, 0.0), (7.0, 0.0), (7.0, 6.0), (10.0, 6.0), (10.0, 9.0), (0.0, 9.0)]
                .iter()
                .map(|&(x, y)| DVec2::new(x, y))
                .collect();
        let area = polygon::area(&t);
        let s = skeleton(&[t], &[vec![0.5; 8]]).unwrap();
        assert!((faces_area(&s) - area).abs() < 1e-6, "{} vs {area}", faces_area(&s));
    }

    /// Faces cover the footprint and never dip below the eaves.
    fn check_area(s: &Skeleton, area: f64) {
        assert!((faces_area(s) - area).abs() < 1e-6 * area.max(1.0), "{} vs {area}", faces_area(s));
        assert!(s.faces.iter().all(|(i, f)| f.iter().all(|p| s.face_height(*i, *p) > -1e-6)));
    }

    /// Faces cover the footprint, never dip below the eaves, and neighboring
    /// faces agree on the height of the points they share.
    fn check_consistent(s: &Skeleton, area: f64) {
        check_area(s, area);
        let pts: Vec<(DVec2, f64)> =
            s.faces.iter().flat_map(|(i, f)| f.iter().map(|p| (*p, s.face_height(*i, *p)))).collect();
        for &(p, h) in &pts {
            assert!(h > -1e-6, "{p} at {h}");
            for &(q, g) in &pts {
                if p.distance(q) < 1e-7 {
                    assert!((h - g).abs() < 1e-6, "{p}: {h} vs {g}");
                }
            }
        }
    }

    #[test]
    fn irregular_footprints_close_consistently() {
        // Stars and wobbly polygons (many reflex corners), deterministic.
        let mut seed = 12345u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64) / ((1u64 << 31) as f64)
        };
        for case in 0..40 {
            let n = 5 + case % 9;
            let pts: Vec<DVec2> = (0..n)
                .map(|i| {
                    let a = i as f64 / n as f64 * std::f64::consts::TAU;
                    let r = if case % 2 == 0 && i % 2 == 1 { 3.0 + rnd() } else { 6.0 + 3.0 * rnd() };
                    DVec2::from_angle(a) * r
                })
                .collect();
            let area = polygon::area(&pts);
            let weights: Vec<f64> =
                (0..n).map(|i| if case % 3 == 0 { 0.5 + (i % 3) as f64 * 0.5 } else { 1.0 }).collect();
            let s = skeleton(std::slice::from_ref(&pts), &[weights]).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
            check_consistent(&s, area);
            // With a small courtyard near the middle.
            let hole: Vec<DVec2> = (0..4)
                .rev()
                .map(|i| DVec2::from_angle(i as f64 * std::f64::consts::FRAC_PI_2 + 0.3) * 1.0 + DVec2::new(0.3, 0.2))
                .collect();
            let s = skeleton(&[pts.clone(), hole.clone()], &[vec![1.0; n], vec![1.0; 4]])
                .unwrap_or_else(|e| panic!("case {case} with a hole: {e:?}"));
            check_consistent(&s, area - polygon::area(&hole));
        }
    }

    #[test]
    fn rectilinear_footprints_with_simultaneous_events_close() {
        let shape = |c: &[(f64, f64)]| c.iter().map(|&(x, y)| DVec2::new(x, y)).collect::<Vec<_>>();
        let shapes = [
            // Plus.
            shape(&[
                (4.0, 0.0),
                (8.0, 0.0),
                (8.0, 4.0),
                (12.0, 4.0),
                (12.0, 8.0),
                (8.0, 8.0),
                (8.0, 12.0),
                (4.0, 12.0),
                (4.0, 8.0),
                (0.0, 8.0),
                (0.0, 4.0),
                (4.0, 4.0),
            ]),
            // H.
            shape(&[
                (0.0, 0.0),
                (4.0, 0.0),
                (4.0, 4.0),
                (8.0, 4.0),
                (8.0, 0.0),
                (12.0, 0.0),
                (12.0, 12.0),
                (8.0, 12.0),
                (8.0, 8.0),
                (4.0, 8.0),
                (4.0, 12.0),
                (0.0, 12.0),
            ]),
            // Comb.
            shape(&[
                (0.0, 0.0),
                (14.0, 0.0),
                (14.0, 8.0),
                (11.0, 8.0),
                (11.0, 3.0),
                (8.5, 3.0),
                (8.5, 8.0),
                (5.5, 8.0),
                (5.5, 3.0),
                (3.0, 3.0),
                (3.0, 8.0),
                (0.0, 8.0),
            ]),
            // Steps.
            shape(&[(0.0, 0.0), (9.0, 0.0), (9.0, 3.0), (6.0, 3.0), (6.0, 6.0), (3.0, 6.0), (3.0, 9.0), (0.0, 9.0)]),
        ];
        for (i, pts) in shapes.iter().enumerate() {
            let area = polygon::area(pts);
            let n = pts.len();
            let s = skeleton(std::slice::from_ref(pts), &[vec![1.0; n]]).unwrap_or_else(|e| panic!("shape {i}: {e:?}"));
            check_consistent(&s, area);
            // Gables on the edges along Y (the ridges run along X).
            let weights: Vec<f64> = (0..n)
                .map(|k| {
                    let d = pts[(k + 1) % n] - pts[k];
                    if d.x.abs() > d.y.abs() { 1.0 } else { 0.0 }
                })
                .collect();
            // A gable may rise above a lower roof beside it (a step in the surface).
            let s =
                skeleton(std::slice::from_ref(pts), &[weights]).unwrap_or_else(|e| panic!("gabled shape {i}: {e:?}"));
            check_area(&s, area);
        }
    }

    #[test]
    fn bad_input_is_an_error() {
        assert_eq!(skeleton(&[vec![DVec2::ZERO, DVec2::X]], &[vec![1.0; 2]]), Err(SkeletonError::Invalid));
        let nan = vec![DVec2::ZERO, DVec2::new(f64::NAN, 0.0), DVec2::Y];
        assert_eq!(skeleton(&[nan], &[vec![1.0; 3]]), Err(SkeletonError::Invalid));
    }
}
