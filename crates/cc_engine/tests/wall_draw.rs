//! Walls drawn as shapes (issue #497): `walls_create` turns lines,
//! rectangles, polygons, circles, arcs, ellipses and splines into walls
//! joined end to end, in one undo step, with rounded corners and an offset.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::{ElementId, Wall};
use cc_engine::Session;
use glam::DVec2;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn walls(s: &mut Session, args: Value) -> Vec<Wall> {
    let out = run(s, "walls_create", args);
    out["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| s.project().element(ElementId(id.as_u64().unwrap())).and_then(|e| e.as_wall()).cloned().unwrap())
        .collect()
}

/// Every wall starts where the one before ends, and the last ends where the first starts.
fn closed_chain(w: &[Wall]) -> bool {
    (0..w.len()).all(|i| w[i].end.distance(w[(i + 1) % w.len()].start) < 1e-6)
}

#[test]
fn a_rectangle_makes_four_joined_walls_in_one_undo_step() {
    let mut s = Session::default();
    let before = s.doc.undo_depth();
    let w =
        walls(&mut s, json!({"curves": [{"kind": "rectangle", "corner": [0, 0], "opposite": [5, 4]}], "height": 2.8}));
    assert_eq!(w.len(), 4);
    assert!(closed_chain(&w));
    assert!(w.iter().all(|w| w.bulge == 0.0 && (w.height - 2.8).abs() < 1e-12 && w.join_start && w.join_end));
    assert_eq!(s.doc.undo_depth(), before + 1);
    s.doc.undo();
    assert_eq!(s.project().walls().count(), 0);
}

#[test]
fn polygons_circles_and_long_arcs() {
    let mut s = Session::default();
    let hex = walls(
        &mut s,
        json!({"curves": [{"kind": "polygon", "center": [0, 0], "radius": 3, "sides": 6, "inscribed": false}]}),
    );
    assert_eq!(hex.len(), 6);
    assert!(closed_chain(&hex));
    // Circumscribed: the middle of each side is 3 m from the center.
    assert!(hex.iter().all(|w| ((w.start + w.end) / 2.0).length() - 3.0 < 1e-9));

    let circle = walls(&mut s, json!({"curves": [{"kind": "circle", "center": [20, 0], "radius": 4}]}));
    assert_eq!(circle.len(), 2);
    assert!(closed_chain(&circle) && circle.iter().all(|w| (w.bulge - 1.0).abs() < 1e-12));

    // Three quarters of a turn: two walls of three eighths each.
    let bulge = (3.0 * std::f64::consts::PI / 2.0 / 4.0).tan();
    let arc = walls(&mut s, json!({"curves": [{"kind": "arc", "start": [40, 0], "end": [44, 0], "bulge": bulge}]}));
    assert_eq!(arc.len(), 2);
    assert!(arc[0].end.distance(arc[1].start) < 1e-9);
    let sweep: f64 = arc.iter().map(|w| 4.0 * w.bulge.atan()).sum();
    assert!((sweep - 3.0 * std::f64::consts::PI / 2.0).abs() < 1e-9);
}

#[test]
fn ellipses_and_closed_splines_become_spline_walls() {
    let mut s = Session::default();
    let e = walls(&mut s, json!({"curves": [{"kind": "ellipse", "center": [0, 0], "radii": [5, 3]}]}));
    assert_eq!(e.len(), 2, "a closed curve makes two walls");
    assert!(e.iter().all(|w| w.spline.is_some()) && closed_chain(&e));
    let sp = walls(
        &mut s,
        json!({"curves": [{"kind": "spline", "through": [[20, 0], [26, 2], [24, 7], [19, 5]], "closed": true}]}),
    );
    assert_eq!(sp.len(), 2);
    assert!(sp.iter().all(|w| w.spline.is_some()) && closed_chain(&sp));
}

#[test]
fn radius_rounds_corners_and_offset_moves_the_chain() {
    let mut s = Session::default();
    // An L: two lines joined at [4, 0], rounded with 1 m and moved 0.5 m to the left.
    let w = walls(
        &mut s,
        json!({
            "curves": [{"kind": "line", "start": [0, 0], "end": [4, 0]}, {"kind": "line", "start": [4, 0], "end": [4, 3]}],
            "radius": 1.0,
            "offset": 0.5,
            "join": false,
        }),
    );
    assert_eq!(w.len(), 3);
    assert!(w[0].end.distance(w[1].start) < 1e-9 && w[1].end.distance(w[2].start) < 1e-9);
    // Offset to the left (inside the turn): the arc's radius drops to 0.5 m.
    assert!(w[0].start.distance(DVec2::new(0.0, 0.5)) < 1e-9, "{}", w[0].start);
    assert!(w[2].end.distance(DVec2::new(3.5, 3.0)) < 1e-9, "{}", w[2].end);
    assert!(w[1].arc().is_some_and(|a| (a.radius - 0.5).abs() < 1e-9));
    assert!(w.iter().all(|w| !w.join_start && !w.join_end));
}

#[test]
fn bad_shapes_are_refused_and_change_nothing() {
    let mut s = Session::default();
    let rect = json!([{"kind": "rectangle", "corner": [0, 0], "opposite": [2, 2]}]);
    for args in [
        json!({"curves": []}),
        json!({"curves": rect, "radius": 1.5}),
        json!({"curves": rect, "radius": -1}),
        json!({"curves": [{"kind": "circle", "center": [0, 0], "radius": 1}], "offset": -2}),
    ] {
        assert!(s.execute("walls_create", args.clone()).is_err(), "{args}");
    }
    assert_eq!(s.project().walls().count(), 0);
}
