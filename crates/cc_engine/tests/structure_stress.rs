//! The structure tab's automation (columns at grids, beams on grids, footings
//! under columns and walls, slabs from walls, joins, numbering) under
//! adversarial models: empty projects, repeated runs, overlapping and
//! degenerate elements, undo and redo. Every run must end in a value or an
//! error (never an internal error), and every view must still draw, as the
//! app redraws them after each command.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::ViewId;
use cc_engine::{ErrorCode, Session};
use serde_json::{Value, json};

/// Run `name`; an error is fine unless it is an internal one (a panic).
fn try_run(s: &mut Session, name: &str, args: Value) -> Option<Value> {
    match s.execute(name, args.clone()) {
        Ok(v) => Some(v),
        Err(e) => {
            assert_ne!(e.code, ErrorCode::Internal, "{name} {args}: {}", e.message);
            None
        }
    }
}

/// Everything the app derives after an edit: geometry, every view's drawing,
/// sheets, schedules and the analytical model.
fn redraw(s: &mut Session) {
    let views: Vec<ViewId> = s.project().views.keys().copied().collect();
    for v in views {
        s.drawing(v);
        s.sheet(v);
        s.schedule(v);
    }
    let (p, g) = s.project_and_geometry();
    cc_geom::analytical::analytical_model(p, g);
    // What the 3D view tessellates.
    for solid in &g.solids {
        for face in &solid.faces {
            face.triangulate();
            assert!(face.verts.iter().all(|v| v.is_finite()), "non-finite face of {:?}", solid.element);
        }
        solid.feature_edges();
    }
}

/// Every automation of the structure tab, as the app's buttons call them.
const AUTOMATION: &[(&str, &str)] = &[
    ("columns_at_grids", "{}"),
    ("beams_on_grids", "{}"),
    ("footings_under_columns", "{}"),
    ("strip_footings_under_walls", "{}"),
    ("floor_create", r#"{"from_walls": true}"#),
    ("foundation_slab_create", r#"{"from_walls": true}"#),
    ("slab_join_supports", "{}"),
    ("structure_number", "{}"),
];

/// Run every automation `times` times, redrawing after each, then undo and
/// redo all of it.
fn automate(s: &mut Session, times: usize) {
    let depth = s.doc.undo_depth();
    for _ in 0..times {
        for (name, args) in AUTOMATION {
            try_run(s, name, serde_json::from_str(args).unwrap());
            redraw(s);
        }
    }
    while s.doc.undo_depth() > depth {
        try_run(s, "undo", json!({})).unwrap();
        redraw(s);
    }
    while try_run(s, "redo", json!({})).is_some() {
        redraw(s);
    }
    redraw(s);
}

fn grid(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> Option<Value> {
    try_run(s, "grid_create", json!({"start": a, "end": b}))
}

fn walls(s: &mut Session, corners: &[[f64; 2]]) {
    for i in 0..corners.len() {
        try_run(s, "wall_create", json!({"start": corners[i], "end": corners[(i + 1) % corners.len()]}));
    }
}

/// Views of every kind the structure is seen in: structural plan (with
/// tags), sections across and along it, elevations, analytical and rebar 3D
/// views, structural schedules.
fn structure_views(s: &mut Session) {
    for args in [
        json!({"kind": "structural_plan"}),
        json!({"kind": "section", "start": [-3.0, 2.5], "end": [25.0, 2.5]}),
        json!({"kind": "section", "start": [2.5, -3.0], "end": [2.5, 25.0], "depth": 30.0}),
        json!({"kind": "section", "start": [0.0, 0.0], "end": [5.0, 0.0]}),
        json!({"kind": "elevation", "direction": "east"}),
        json!({"kind": "analytical"}),
        json!({"kind": "rebar"}),
        json!({"kind": "schedule", "category": "column"}),
        json!({"kind": "schedule", "category": "beam"}),
        json!({"kind": "schedule", "category": "foundation"}),
    ] {
        assert!(try_run(s, "view_create", args.clone()).is_some(), "{args}");
    }
}

#[test]
fn automation_on_an_empty_project() {
    let mut s = Session::default();
    structure_views(&mut s);
    automate(&mut s, 2);
}

#[test]
fn automation_on_a_grid_of_bays_twice() {
    let mut s = Session::default();
    structure_views(&mut s);
    for i in 0..5 {
        let x = f64::from(i) * 5.0;
        grid(&mut s, [x, -2.0], [x, 22.0]);
        grid(&mut s, [-2.0, x], [22.0, x]);
    }
    walls(&mut s, &[[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]]);
    walls(&mut s, &[[0.0, 10.0], [20.0, 10.0]]);
    automate(&mut s, 2);
    let columns = try_run(&mut s, "elements_list", json!({"category": "column"})).unwrap();
    assert_eq!(columns.as_array().unwrap().len(), 25);
}

#[test]
fn automation_with_overlapping_and_degenerate_grids() {
    let mut s = Session::default();
    structure_views(&mut s);
    // The same grid three times, a collinear overlapping one, a diagonal
    // through the crossings, grids that meet only at their ends, one that
    // meets nothing, and a nearly parallel pair.
    for _ in 0..3 {
        grid(&mut s, [0.0, 0.0], [10.0, 0.0]);
    }
    grid(&mut s, [5.0, 0.0], [15.0, 0.0]);
    grid(&mut s, [0.0, -5.0], [0.0, 5.0]);
    grid(&mut s, [10.0, -5.0], [10.0, 5.0]);
    grid(&mut s, [-1.0, -1.0], [11.0, 11.0]);
    grid(&mut s, [10.0, 5.0], [15.0, 5.0]);
    grid(&mut s, [100.0, 100.0], [101.0, 100.0]);
    grid(&mut s, [0.0, 20.0], [1000.0, 20.001]);
    grid(&mut s, [0.0, 20.0], [1000.0, 20.0]);
    grid(&mut s, [0.0, 0.0], [0.0, 0.0]);
    grid(&mut s, [0.0, 0.0], [0.000_1, 0.0]);
    automate(&mut s, 2);
}

#[test]
fn automation_with_stacked_columns_and_walls() {
    let mut s = Session::default();
    structure_views(&mut s);
    grid(&mut s, [0.0, -1.0], [0.0, 7.0]);
    grid(&mut s, [8.0, -1.0], [8.0, 7.0]);
    grid(&mut s, [-1.0, 0.0], [9.0, 0.0]);
    grid(&mut s, [-1.0, 6.0], [9.0, 6.0]);
    // Columns on top of each other, one a hair off a crossing.
    for _ in 0..3 {
        try_run(&mut s, "column_create", json!({"position": [0.0, 0.0]}));
    }
    try_run(&mut s, "column_create", json!({"position": [8.005, 0.0]}));
    try_run(&mut s, "column_create", json!({"position": [4.0, 0.0]}));
    // Walls over each other, a zero-length one, one inside another.
    for _ in 0..2 {
        walls(&mut s, &[[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]]);
    }
    try_run(&mut s, "wall_create", json!({"start": [2.0, 2.0], "end": [2.0, 2.0]}));
    try_run(&mut s, "wall_create", json!({"start": [1.0, 0.0], "end": [3.0, 0.0]}));
    try_run(&mut s, "wall_create", json!({"start": [4.0, 0.0], "end": [4.0, 6.0], "bulge": 0.5}));
    automate(&mut s, 3);
}

#[test]
fn automation_on_every_level() {
    let mut s = Session::default();
    structure_views(&mut s);
    for i in 0..3 {
        try_run(&mut s, "level_create", json!({"name": format!("L{i}"), "elevation": 3.0 * f64::from(i + 1)}));
    }
    // A level at the same height as another, and one below the ground.
    try_run(&mut s, "level_create", json!({"name": "Twin", "elevation": 3.0}));
    try_run(&mut s, "level_create", json!({"name": "Basement", "elevation": -3.0}));
    grid(&mut s, [0.0, -1.0], [0.0, 7.0]);
    grid(&mut s, [6.0, -1.0], [6.0, 7.0]);
    grid(&mut s, [-1.0, 0.0], [7.0, 0.0]);
    grid(&mut s, [-1.0, 6.0], [7.0, 6.0]);
    let levels: Vec<String> = try_run(&mut s, "levels_list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap().to_string())
        .collect();
    for level in &levels {
        for (name, args) in AUTOMATION {
            let mut args: Value = serde_json::from_str(args).unwrap();
            if *name != "structure_number" {
                args["level"] = json!(level);
            }
            try_run(&mut s, name, args);
            redraw(&mut s);
        }
    }
    automate(&mut s, 1);
}

#[test]
fn automation_after_deleting_and_moving_what_it_made() {
    let mut s = Session::default();
    structure_views(&mut s);
    grid(&mut s, [0.0, -1.0], [0.0, 7.0]);
    grid(&mut s, [6.0, -1.0], [6.0, 7.0]);
    grid(&mut s, [-1.0, 0.0], [7.0, 0.0]);
    walls(&mut s, &[[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]]);
    automate(&mut s, 1);
    // Delete the columns the beams frame into, then run it all again.
    let columns: Vec<u64> = try_run(&mut s, "elements_list", json!({"category": "column"}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_u64().unwrap())
        .collect();
    try_run(&mut s, "elements_delete", json!({"ids": columns}));
    redraw(&mut s);
    let grids: Vec<u64> = try_run(&mut s, "elements_list", json!({"category": "grid"}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_u64().unwrap())
        .collect();
    try_run(&mut s, "elements_move", json!({"ids": grids, "delta": [0.3, 0.0]}));
    redraw(&mut s);
    automate(&mut s, 2);
}
