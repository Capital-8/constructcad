//! Grid naming and alignment.
//!
//! **Naming.** A new grid continues the series of the grids parallel to it
//! (within [`SERIES_DEG`]): the name after that of the most recently placed
//! parallel grid ([`successor`]: `1` → `2`, `09` → `10`, `A` → `B`,
//! `Z` → `AA`, `A1` → `A2`, `1.1` → `1.2`). A grid with no parallel grid
//! starts a new series: numbers for the first series, letters for a series
//! across a numbered one (and numbers across a lettered one). The name is
//! then made unique among the existing grids by stepping the series on.
//!
//! **Alignment.** The bubble lines of a grid are the lines through its ends
//! perpendicular to it. [`align_end`] snaps a grid end being drawn or
//! dragged onto the bubble line of a parallel grid's end (keeping the grid
//! parallel to it), and [`aligned_ends`] gives a grid the extents of a
//! parallel reference grid.

use crate::{ElementId, Grid};
use glam::DVec2;

/// Grids whose directions differ by at most this many degrees (either way
/// along the line) belong to the same series when naming.
pub const SERIES_DEG: f64 = 10.0;

/// Grids whose directions differ by at most this many degrees are parallel
/// when aligning ends.
pub const PARALLEL_DEG: f64 = 1.0;

/// Longest run of trailing letters counted as a series (`A`..`ZZZ`): longer
/// words are names, not counters.
const MAX_LETTERS: usize = 3;

/// Whether the lines along `u` and `v` are within `deg` degrees of each
/// other, either way along the line. Degenerate directions are not parallel.
pub fn is_parallel(u: DVec2, v: DVec2, deg: f64) -> bool {
    let (u, v) = (u.normalize_or_zero(), v.normalize_or_zero());
    if u == DVec2::ZERO || v == DVec2::ZERO {
        return false;
    }
    u.perp_dot(v).abs() <= deg.to_radians().sin()
}

/// How a grid name counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Counter {
    Number,
    Letter,
}

/// The counter at the end of `name`, if any.
fn counter(name: &str) -> Option<Counter> {
    let last = name.chars().last()?;
    if last.is_ascii_digit() {
        Some(Counter::Number)
    } else if last.is_ascii_alphabetic() {
        let run = name.chars().rev().take_while(char::is_ascii_alphabetic).count();
        (run <= MAX_LETTERS).then_some(Counter::Letter)
    } else {
        None
    }
}

/// The name after `name` in its series: the trailing number counts up
/// keeping its zero padding (`A1` → `A2`, `1.9` → `1.10`, `09` → `10`), or
/// the trailing letters (up to three, all upper or all lower case) count
/// like spreadsheet columns (`A` → `B`, `Z` → `AA`, `AZ` → `BA`, `b` → `c`).
/// `None` when the name does not end in a counter (`North`, `Sul`, `A'`).
pub fn successor(name: &str) -> Option<String> {
    let kind = counter(name)?;
    let chars: Vec<char> = name.chars().collect();
    let in_run = |c: &char| match kind {
        Counter::Number => c.is_ascii_digit(),
        Counter::Letter => c.is_ascii_alphabetic(),
    };
    let len = chars.iter().rev().take_while(|c| in_run(c)).count();
    let (prefix, run) = chars.split_at(chars.len().checked_sub(len)?);
    let (lo, hi) = match kind {
        Counter::Number => ('0', '9'),
        Counter::Letter if run.iter().all(char::is_ascii_uppercase) => ('A', 'Z'),
        Counter::Letter if run.iter().all(char::is_ascii_lowercase) => ('a', 'z'),
        // Mixed case is a word, not a counter.
        Counter::Letter => return None,
    };
    let mut run = run.to_vec();
    // Count up from the right; a carry out of the first place adds one.
    let mut carry = true;
    for c in run.iter_mut().rev() {
        if *c == hi {
            *c = lo;
        } else {
            *c = char::from_u32(*c as u32 + 1)?;
            carry = false;
            break;
        }
    }
    let mut out: String = prefix.iter().collect();
    if carry {
        out.push(if kind == Counter::Number { '1' } else { lo });
    }
    out.extend(run);
    Some(out)
}

/// The first name of a new series beside grids named like `across` (the
/// most recent grid of another direction): letters across numbers,
/// numbers otherwise.
fn series_start(across: Option<&str>) -> String {
    match across.and_then(counter) {
        Some(Counter::Number) => "A".into(),
        _ => "1".into(),
    }
}

/// Name for a new grid from `start` to `end` among `grids` (see the module
/// documentation): it continues the series of the most recently placed
/// (highest id) parallel grid and is unique among `grids`.
pub fn next_name<'a>(grids: impl IntoIterator<Item = (ElementId, &'a Grid)>, start: DVec2, end: DVec2) -> String {
    let grids: Vec<(ElementId, &Grid)> = grids.into_iter().collect();
    let dir = end - start;
    let parallel = |g: &Grid| is_parallel(g.end - g.start, dir, SERIES_DEG);
    let last = |want: bool| grids.iter().filter(|(_, g)| parallel(g) == want).max_by_key(|(id, _)| *id);
    let across = last(false).map(|(_, g)| g.name.trim());
    let mut name = last(true).and_then(|(_, g)| successor(g.name.trim())).unwrap_or_else(|| series_start(across));
    let taken = |n: &str| grids.iter().any(|(_, g)| g.name.trim() == n);
    // Each step gives a new name, so this ends after at most `grids.len()` steps.
    for _ in 0..=grids.len() {
        if !taken(&name) {
            return name;
        }
        match successor(&name) {
            Some(next) => name = next,
            None => break,
        }
    }
    // Last resort: a number above every numeric name.
    let max = grids.iter().filter_map(|(_, g)| g.name.trim().parse::<u64>().ok()).max().unwrap_or(0);
    max.saturating_add(1).to_string()
}

/// A grid end snapped onto the bubble line of another grid's end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndAlignment {
    /// The snapped end.
    pub point: DVec2,
    /// The end of the other grid it lines up with.
    pub reference: DVec2,
}

/// Snap a grid end near `raw` to the bubble lines of the grids `others`
/// (start and end points), within `tol` (model units):
///
/// - with the grid's other end `fixed` (drawing the second point, or
///   dragging an end handle), the end goes where the line from `fixed`
///   parallel to another grid meets the bubble line of that grid's start or
///   end, so the grid comes out parallel with its end lined up;
/// - without it (the first point), the end goes onto the nearest bubble line.
///
/// The closest candidate wins; `None` when none is within `tol`.
pub fn align_end(others: &[(DVec2, DVec2)], fixed: Option<DVec2>, raw: DVec2, tol: f64) -> Option<EndAlignment> {
    if !(tol.is_finite() && tol > 0.0) || !raw.is_finite() {
        return None;
    }
    let mut best: Option<(EndAlignment, f64)> = None;
    let mut consider = |point: DVec2, reference: DVec2| {
        let d = point.distance(raw);
        if point.is_finite() && d <= tol && best.is_none_or(|(_, b)| d < b) {
            best = Some((EndAlignment { point, reference }, d));
        }
    };
    for &(a, b) in others {
        let d = (b - a).normalize_or_zero();
        if d == DVec2::ZERO || !d.is_finite() {
            continue;
        }
        for e in [a, b] {
            match fixed {
                Some(f) => {
                    // Along the other grid, the way the pointer is from `fixed`.
                    let d = if (raw - f).dot(d) < 0.0 { -d } else { d };
                    let t = (e - f).dot(d);
                    if t > tol {
                        consider(f + d * t, e);
                    }
                }
                None => consider(raw - d * (raw - e).dot(d), e),
            }
        }
    }
    best.map(|(a, _)| a)
}

/// New `(start, end)` of `grid` with the extents of the parallel grid
/// `reference`: each end moves along the grid's own line onto the bubble
/// line of the reference end on its side (`start`/`end` pick which ends
/// move). `None` when the grids are not parallel (within [`PARALLEL_DEG`])
/// or the result would be shorter than 1 cm.
pub fn aligned_ends(grid: &Grid, reference: &Grid, start: bool, end: bool) -> Option<(DVec2, DVec2)> {
    let g = grid.end - grid.start;
    let d = (reference.end - reference.start).normalize_or_zero();
    if !is_parallel(g, d, PARALLEL_DEG) {
        return None;
    }
    let gd = g.dot(d);
    // The point of the grid's line at `t` along the reference direction.
    let at = |t: f64| grid.start + g * ((t - grid.start.dot(d)) / gd);
    let (ts, te) = (reference.start.dot(d), reference.end.dot(d));
    let (to_start, to_end) = if gd > 0.0 { (ts, te) } else { (te, ts) };
    let new_start = if start { at(to_start) } else { grid.start };
    let new_end = if end { at(to_end) } else { grid.end };
    (new_start.is_finite() && new_end.is_finite() && new_start.distance(new_end) >= 0.01)
        .then_some((new_start, new_end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(name: &str, a: [f64; 2], b: [f64; 2]) -> Grid {
        Grid { start: DVec2::from(a), end: DVec2::from(b), name: name.into() }
    }

    fn name_after(grids: &[Grid], a: [f64; 2], b: [f64; 2]) -> String {
        let ids = grids.iter().enumerate().map(|(i, g)| (ElementId(i as u64 + 1), g));
        next_name(ids, DVec2::from(a), DVec2::from(b))
    }

    #[test]
    fn successors() {
        let next = |s: &str| successor(s);
        assert_eq!(next("1").as_deref(), Some("2"));
        assert_eq!(next("9").as_deref(), Some("10"));
        assert_eq!(next("09").as_deref(), Some("10"));
        assert_eq!(next("007").as_deref(), Some("008"));
        assert_eq!(next("A").as_deref(), Some("B"));
        assert_eq!(next("Z").as_deref(), Some("AA"));
        assert_eq!(next("AZ").as_deref(), Some("BA"));
        assert_eq!(next("ZZ").as_deref(), Some("AAA"));
        assert_eq!(next("b").as_deref(), Some("c"));
        assert_eq!(next("z").as_deref(), Some("aa"));
        assert_eq!(next("A1").as_deref(), Some("A2"));
        assert_eq!(next("A9").as_deref(), Some("A10"));
        assert_eq!(next("1.1").as_deref(), Some("1.2"));
        assert_eq!(next("1A").as_deref(), Some("1B"));
        assert_eq!(next("Grid C").as_deref(), Some("Grid D"));
        assert_eq!(next("Norte"), None);
        assert_eq!(next("Sul"), None);
        assert_eq!(next("AA").as_deref(), Some("AB"));
        assert_eq!(next("A'"), None);
        assert_eq!(next(""), None);
    }

    #[test]
    fn first_grids_and_series() {
        assert_eq!(name_after(&[], [0.0, 0.0], [0.0, 10.0]), "1");
        // Parallel grids continue the series; one across starts letters.
        let ones = [grid("1", [0.0, 0.0], [0.0, 10.0]), grid("2", [5.0, 0.0], [5.0, 10.0])];
        assert_eq!(name_after(&ones, [10.0, 0.0], [10.0, 10.0]), "3");
        // Drawn the other way along the line, still the same series.
        assert_eq!(name_after(&ones, [10.0, 10.0], [10.0, 0.0]), "3");
        assert_eq!(name_after(&ones, [-1.0, 2.0], [11.0, 2.0]), "A");
        let mixed = [ones[0].clone(), ones[1].clone(), grid("A", [-1.0, 2.0], [11.0, 2.0])];
        assert_eq!(name_after(&mixed, [-1.0, 6.0], [11.0, 6.0]), "B");
        assert_eq!(name_after(&mixed, [12.0, 0.0], [12.0, 10.0]), "3");
        // Across a lettered series, numbers.
        let letters = [grid("A", [0.0, 0.0], [10.0, 0.0])];
        assert_eq!(name_after(&letters, [0.0, -1.0], [0.0, 5.0]), "1");
        // Slightly skewed grids are still the same series.
        assert_eq!(name_after(&ones, [10.0, 0.0], [10.5, 10.0]), "3");
    }

    #[test]
    fn continues_the_last_placed_and_stays_unique() {
        // The most recent parallel grid sets the series, even if renamed.
        let g = [grid("A1", [0.0, 0.0], [0.0, 10.0]), grid("A2", [5.0, 0.0], [5.0, 10.0])];
        assert_eq!(name_after(&g, [10.0, 0.0], [10.0, 10.0]), "A3");
        // An intermediate grid placed last continues its own series.
        let g =
            [grid("1", [0.0, 0.0], [0.0, 9.0]), grid("2", [4.0, 0.0], [4.0, 9.0]), grid("1.1", [2.0, 0.0], [2.0, 9.0])];
        assert_eq!(name_after(&g, [3.0, 0.0], [3.0, 9.0]), "1.2");
        // A taken successor steps on.
        let g = [grid("2", [0.0, 0.0], [0.0, 9.0]), grid("1", [4.0, 0.0], [4.0, 9.0])];
        assert_eq!(name_after(&g, [8.0, 0.0], [8.0, 9.0]), "3");
        // Names that do not count fall back to a new series start, kept unique.
        let g = [grid("North", [0.0, 0.0], [0.0, 9.0]), grid("1", [9.0, 0.0], [9.0, 9.0])];
        assert_eq!(name_after(&g, [4.0, 0.0], [4.0, 9.0]), "2");
        let g = [grid("North", [0.0, 0.0], [0.0, 9.0])];
        assert_eq!(name_after(&g, [4.0, 0.0], [4.0, 9.0]), "1");
        // A new series across skips taken names.
        let g =
            [grid("1", [0.0, 0.0], [0.0, 9.0]), grid("A", [20.0, 0.0], [20.0, 9.0]), grid("2", [4.0, 0.0], [4.0, 9.0])];
        assert_eq!(name_after(&g, [-1.0, 3.0], [9.0, 3.0]), "B");
        let g = [grid("1", [0.0, 0.0], [0.0, 9.0]), grid("A", [20.0, 0.0], [20.0, 9.0])];
        assert_eq!(name_after(&g, [-1.0, 3.0], [9.0, 3.0]), "2");
    }

    #[test]
    fn parallel() {
        assert!(is_parallel(DVec2::X, DVec2::NEG_X, 1.0));
        assert!(is_parallel(DVec2::X, DVec2::new(1.0, 0.1), 10.0));
        assert!(!is_parallel(DVec2::X, DVec2::new(1.0, 0.1), 1.0));
        assert!(!is_parallel(DVec2::X, DVec2::Y, 10.0));
        assert!(!is_parallel(DVec2::ZERO, DVec2::Y, 10.0));
    }

    #[test]
    fn aligning_the_second_point() {
        // Grid 1 from y = 0 (bubble) to y = 10; drawing a grid from (5, 0.02).
        let others = [(DVec2::new(0.0, 0.0), DVec2::new(0.0, 10.0))];
        let fixed = Some(DVec2::new(5.0, 0.02));
        let a = align_end(&others, fixed, DVec2::new(5.1, 9.9), 0.3).unwrap();
        assert!(a.point.distance(DVec2::new(5.0, 10.0)) < 1e-9, "{a:?}");
        assert_eq!(a.reference, DVec2::new(0.0, 10.0));
        // Drawn downwards, the bubble end lines up.
        let a = align_end(&others, Some(DVec2::new(5.0, 8.0)), DVec2::new(5.05, 0.1), 0.3).unwrap();
        assert!(a.point.distance(DVec2::new(5.0, 0.0)) < 1e-9, "{a:?}");
        // Too far from any bubble line, or not parallel enough.
        assert_eq!(align_end(&others, fixed, DVec2::new(5.0, 7.0), 0.3), None);
        assert_eq!(align_end(&others, fixed, DVec2::new(8.0, 10.0), 0.3), None);
        // Bad tolerances give nothing.
        assert_eq!(align_end(&others, fixed, DVec2::new(5.0, 10.0), f64::NAN), None);
        assert_eq!(align_end(&[], fixed, DVec2::new(5.0, 10.0), 0.3), None);
    }

    #[test]
    fn aligning_the_first_point() {
        let others = [(DVec2::new(0.0, -1.0), DVec2::new(0.0, 10.0)), (DVec2::new(4.0, -1.5), DVec2::new(4.0, 9.0))];
        let a = align_end(&others, None, DVec2::new(8.0, -0.9), 0.3).unwrap();
        assert!(a.point.distance(DVec2::new(8.0, -1.0)) < 1e-9, "{a:?}");
        let a = align_end(&others, None, DVec2::new(8.0, -1.4), 0.3).unwrap();
        assert!(a.point.distance(DVec2::new(8.0, -1.5)) < 1e-9, "{a:?}");
        assert_eq!(align_end(&others, None, DVec2::new(8.0, 3.0), 0.3), None);
    }

    #[test]
    fn extents_of_a_reference() {
        let reference = grid("1", [0.0, -1.0], [0.0, 10.0]);
        let g = grid("2", [5.0, 0.5], [5.0, 7.0]);
        let (s, e) = aligned_ends(&g, &reference, true, true).unwrap();
        assert_eq!((s, e), (DVec2::new(5.0, -1.0), DVec2::new(5.0, 10.0)));
        // Only the bubble end.
        let (s, e) = aligned_ends(&g, &reference, true, false).unwrap();
        assert_eq!((s, e), (DVec2::new(5.0, -1.0), DVec2::new(5.0, 7.0)));
        // Drawn the other way: each end goes to the reference end on its side.
        let back = grid("3", [8.0, 7.0], [8.0, 0.5]);
        let (s, e) = aligned_ends(&back, &reference, true, true).unwrap();
        assert_eq!((s, e), (DVec2::new(8.0, 10.0), DVec2::new(8.0, -1.0)));
        // Not parallel.
        assert_eq!(aligned_ends(&grid("A", [-1.0, 2.0], [9.0, 2.0]), &reference, true, true), None);
        // Degenerate reference.
        assert_eq!(aligned_ends(&g, &grid("x", [1.0, 1.0], [1.0, 1.0]), true, true), None);
    }
}
