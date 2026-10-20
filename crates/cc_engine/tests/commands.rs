//! The engine without a GUI: the catalog is well formed, and a house can be
//! built, changed, queried and exported through commands alone.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_engine::{CommandKind, ErrorCode, Session, catalog};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn fails(s: &mut Session, name: &str, args: Value) -> ErrorCode {
    s.execute(name, args).map(|v| panic!("{name} should fail, gave {v}")).unwrap_err().code
}

fn id(v: &Value) -> u64 {
    v["id"].as_u64().unwrap_or_else(|| panic!("no id in {v}"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ccad-engine-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn catalog_is_well_formed() {
    let commands = catalog();
    let mut names: Vec<&str> = commands.iter().map(|c| c.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), commands.len(), "duplicate command names");
    for c in &commands {
        // Usable as tool names by MCP clients and model APIs.
        assert!(c.name.len() <= 64 && c.name.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'), "{}", c.name);
        assert!(!c.description.is_empty(), "{} has no description", c.name);
        assert_eq!(c.input_schema["type"], "object", "{}", c.name);
        assert!(!c.input_schema.to_string().contains("$ref"), "{} schema has references", c.name);
    }
}

/// Every command answers empty or wrong arguments with an error value, never a panic.
#[test]
fn every_command_survives_bad_arguments() {
    for c in catalog() {
        for args in [json!({}), json!({"bogus": 1}), json!([1, 2]), json!({"id": 999_999, "ids": [999_999]})] {
            let mut s = Session::default();
            if let Err(e) = s.execute(c.name, args.clone()) {
                assert_ne!(e.code, ErrorCode::Internal, "{} with {args}: {}", c.name, e.message);
                assert_ne!(e.code, ErrorCode::UnknownCommand);
            }
        }
    }
    assert_eq!(Session::default().execute("no_such_command", json!({})).unwrap_err().code, ErrorCode::UnknownCommand);
}

#[test]
fn a_house_through_commands() {
    let mut s = Session::default();
    let levels = run(&mut s, "levels_list", json!({}));
    let ground = levels[0]["name"].as_str().unwrap().to_string();

    // Four walls, by level name; the upper one attached to the level above.
    let corners = [[0.0, 0.0], [8.0, 0.0], [8.0, 6.0], [0.0, 6.0]];
    let walls: Vec<u64> = (0..4)
        .map(|i| {
            let w = run(
                &mut s,
                "wall_create",
                json!({"start": corners[i], "end": corners[(i + 1) % 4], "level": ground, "height": 3.0}),
            );
            id(&w)
        })
        .collect();
    let wall = run(&mut s, "element_get", json!({"id": walls[0]}));
    assert_eq!(wall["derived"]["length"], 8.0);

    // A door centered on the first wall, a window by a point near the second.
    let door = run(&mut s, "door_create", json!({"wall": walls[0]}));
    assert!((door["params"]["offset"].as_f64().unwrap() - 4.0).abs() < 1e-9);
    assert_eq!(door["mark"], "1");
    let window = run(&mut s, "window_create", json!({"wall": walls[1], "point": [8.5, 2.0]}));
    assert_eq!(window["params"]["flip_facing"], true, "the point is outside, to the right of the wall");

    // Floor and roof from the walls, a room inside, a grid, a column, a beam, a stair, a railing.
    let floor = run(&mut s, "floor_create", json!({"from_walls": true}));
    assert!(floor["derived"]["volume"].as_f64().unwrap() > 0.0);
    run(&mut s, "roof_create", json!({"from_walls": true}));
    let room = run(&mut s, "room_create", json!({"point": [4.0, 3.0], "name": "Sala"}));
    let area = room["derived"]["area"].as_f64().unwrap();
    assert!((area - 7.85 * 5.85).abs() < 1e-6, "{area}");
    assert_eq!(fails(&mut s, "room_create", json!({"point": [40.0, 3.0]})), ErrorCode::Rejected);
    let grid = run(&mut s, "grid_create", json!({"start": [0.0, -2.0], "end": [0.0, 8.0]}));
    assert_eq!(grid["params"]["name"], "1");
    let column = run(&mut s, "column_create", json!({"position": [4.0, 3.0], "rotation": 30.0}));
    assert!(column["params"]["top"]["Level"].is_object(), "attached to the level above");
    run(&mut s, "beam_create", json!({"start": [0.0, 3.0], "end": [8.0, 3.0]}));
    run(&mut s, "stair_create", json!({"start": [1.0, 1.0], "direction": 90.0}));
    run(&mut s, "railing_create", json!({"path": [[0.0, 7.0], [4.0, 7.0], [4.0, 9.0]]}));
    let types = run(&mut s, "types_list", json!({"category": "component"}));
    let table = types[0]["name"].as_str().unwrap().to_string();
    run(&mut s, "component_place", json!({"position": [2.0, 2.0], "type": table, "rotation": 90.0}));

    // Change things: a parameter, the type, a mark; invalid edits are refused untouched.
    let changed = run(&mut s, "element_set", json!({"id": walls[0], "params": {"height": 2.7}, "mark": "P1"}));
    assert_eq!((changed["params"]["height"].as_f64(), changed["mark"].as_str()), (Some(2.7), Some("P1")));
    let before = run(&mut s, "element_get", json!({"id": door["id"]}));
    assert_eq!(
        fails(&mut s, "element_set", json!({"id": door["id"], "params": {"host": 999_999}})),
        ErrorCode::Rejected
    );
    assert_eq!(
        fails(&mut s, "element_set", json!({"id": walls[0], "params": {"height": "high"}})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(run(&mut s, "element_get", json!({"id": door["id"]})), before);
    let roof_id = run(&mut s, "elements_list", json!({"category": "roof"}))[0]["id"].clone();
    let gable = run(
        &mut s,
        "element_set",
        json!({"id": roof_id, "params": {"shape": {"Gable": {"slope": 0.5, "ridge_deg": 0.0}}}}),
    );
    assert!(gable["params"]["shape"]["Gable"].is_object());

    // Move, copy as an array, rotate, mirror, pin, delete, undo and redo.
    let moved = run(&mut s, "elements_move", json!({"ids": [grid["id"]], "delta": [-1.0, 0.0]}));
    assert_eq!(moved["changed"].as_array().unwrap().len(), 1);
    let copies = run(&mut s, "elements_copy", json!({"ids": [column["id"]], "delta": [2.0, 0.0], "count": 3}));
    assert_eq!(copies["created"].as_array().unwrap().len(), 3);
    run(&mut s, "elements_rotate", json!({"ids": copies["created"], "angle": 90.0}));
    let mirrored = run(&mut s, "elements_mirror", json!({"ids": [column["id"]], "axis_element": walls[1]}));
    let mirror_id = mirrored["created"][0].as_u64().unwrap();
    let x = run(&mut s, "element_get", json!({"id": mirror_id}))["params"]["position"][0].as_f64().unwrap();
    assert!((x - 12.0).abs() < 1e-9, "mirrored across x = 8: {x}");
    run(&mut s, "elements_pin", json!({"ids": [mirror_id], "pinned": true}));
    let deleted = run(&mut s, "elements_delete", json!({"ids": [mirror_id]}));
    assert_eq!(deleted["pinned_skipped"], json!([mirror_id]));
    let count = |s: &mut Session| run(s, "project_info", json!({}))["elements"].as_u64().unwrap();
    let n = count(&mut s);
    run(&mut s, "elements_pin", json!({"ids": [mirror_id], "pinned": false}));
    run(&mut s, "elements_delete", json!({"ids": [mirror_id]}));
    assert_eq!(count(&mut s), n - 1);
    run(&mut s, "undo", json!({}));
    assert_eq!(count(&mut s), n);
    run(&mut s, "redo", json!({}));
    assert_eq!(count(&mut s), n - 1);

    // Wall edits: split, offset copy, trim to a corner, align, move an end, joins.
    let split = run(&mut s, "element_split", json!({"id": walls[2], "point": [4.0, 6.0]}));
    let offset = run(&mut s, "element_offset", json!({"id": split["created"], "distance": -1.0}));
    run(&mut s, "wall_join", json!({"id": walls[3], "end": "start", "joined": false}));
    run(&mut s, "element_move_end", json!({"id": offset["id"], "end": "end", "point": [0.5, 5.0]}));
    assert_eq!(
        fails(&mut s, "element_move_end", json!({"id": offset["id"], "end": "middle", "point": [0.0, 0.0]})),
        ErrorCode::InvalidArguments
    );

    // Types, materials, levels and views.
    let wall_type = run(&mut s, "type_get", json!({"type": wall["type"]["id"]}));
    let copy = run(
        &mut s,
        "type_duplicate",
        json!({"type": wall_type["id"], "name": "Parede grossa", "elements": [walls[0]]}),
    );
    let thick = run(
        &mut s,
        "type_set",
        json!({"type": "Parede grossa", "params": {"layers": [{"material": wall_type["params"]["layers"][0]["material"], "thickness": 0.25, "function": "Structure"}]}}),
    );
    assert_eq!(thick["params"]["layers"].as_array().unwrap().len(), 1);
    assert!(
        (run(&mut s, "element_get", json!({"id": walls[0]}))["derived"]["thickness"].as_f64().unwrap() - 0.25).abs()
            < 1e-9
    );
    assert_eq!(fails(&mut s, "type_delete", json!({"type": copy["id"]})), ErrorCode::Rejected);
    let spare = run(&mut s, "type_duplicate", json!({"type": wall_type["id"]}));
    run(&mut s, "type_delete", json!({"type": spare["id"]}));
    let mat = run(&mut s, "material_create", json!({"name": "Granito", "color": [90, 90, 90]}));
    assert_eq!(mat["name"], "Granito");
    run(&mut s, "material_set", json!({"material": "Granito", "values": {"description": "polido"}}));
    run(&mut s, "material_rename", json!({"material": "Granito", "name": "Granito cinza"}));
    run(&mut s, "material_delete", json!({"material": "Granito cinza"}));
    let level = run(&mut s, "level_create", json!({"name": "Cobertura", "elevation": 6.0}));
    run(&mut s, "level_set", json!({"level": level["id"], "elevation": 6.5}));
    assert_eq!(run(&mut s, "views_list", json!({"category": "floor_plan"})).as_array().unwrap().len(), 3);
    run(&mut s, "level_delete", json!({"level": "Cobertura"}));
    let section = run(&mut s, "view_create", json!({"kind": "section", "start": [-1.0, 3.0], "end": [9.0, 3.0]}));
    let schedule = run(&mut s, "view_create", json!({"kind": "schedule", "category": "wall"}));
    let rows = run(&mut s, "schedule_table", json!({"view": schedule["id"]}));
    assert!(rows["rows"].as_array().unwrap().len() >= 5);
    let sheet = run(&mut s, "view_create", json!({"kind": "sheet"}));
    run(&mut s, "sheet_place_view", json!({"sheet": sheet["id"], "view": section["id"]}));
    assert_eq!(
        fails(&mut s, "sheet_place_view", json!({"sheet": sheet["id"], "view": section["id"]})),
        ErrorCode::Rejected
    );
    run(&mut s, "view_set", json!({"view": section["id"], "scale": 100, "params": {"depth": 4.0}}));
    run(&mut s, "units_set", json!({"values": {"length": {"decimals": 3}}}));

    // Files: save, reopen, IFC round trip, PDF and SVG of the sheet, CSV.
    let path = scratch("house.ccad");
    run(&mut s, "project_save", json!({"path": path}));
    let elements = count(&mut s);
    let mut reopened = Session::default();
    run(&mut reopened, "project_open", json!({"path": path}));
    assert_eq!(count(&mut reopened), elements);
    let ifc = scratch("house.ifc");
    run(&mut s, "ifc_export", json!({"path": ifc}));
    let report = run(&mut reopened, "ifc_import", json!({"path": ifc}));
    assert!(report["imported"]["wall"].as_u64().unwrap() >= 5);
    for (file, magic) in [("sheet.pdf", "%PDF"), ("sheet.svg", "<svg")] {
        let out = scratch(file);
        run(&mut s, "sheet_export", json!({"sheet": sheet["id"], "path": out}));
        let text = String::from_utf8_lossy(&std::fs::read(&out).unwrap()).into_owned();
        assert!(text.contains(magic), "{file}");
    }
    let csv = scratch("walls.csv");
    run(&mut s, "schedule_export_csv", json!({"view": schedule["id"], "path": csv}));
    assert!(std::fs::read_to_string(&csv).unwrap().lines().count() >= 6);
}

#[test]
fn names_resolve_and_ambiguity_is_reported() {
    let mut s = Session::default();
    let level = run(&mut s, "levels_list", json!({}))[0]["id"].as_u64().unwrap();
    run(&mut s, "level_create", json!({"name": "Mezanino"}));
    run(&mut s, "level_create", json!({"name": "mezanino"}));
    assert_eq!(
        fails(&mut s, "wall_create", json!({"start": [0, 0], "end": [1, 0], "level": "MEZANINO"})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        fails(&mut s, "wall_create", json!({"start": [0, 0], "end": [1, 0], "level": "Nowhere"})),
        ErrorCode::NotFound
    );
    run(&mut s, "wall_create", json!({"start": [0, 0], "end": [1, 0], "level": level}));
    // A type of another category is refused.
    let door_type = run(&mut s, "types_list", json!({"category": "door"}))[0]["id"].clone();
    assert_eq!(
        fails(&mut s, "wall_create", json!({"start": [0, 0], "end": [1, 0], "type": door_type})),
        ErrorCode::InvalidArguments
    );
}

#[test]
fn queries_change_nothing() {
    let mut s = Session::default();
    let revision = s.doc.revision();
    for c in catalog().into_iter().filter(|c| c.kind == CommandKind::Query) {
        let _ = s.execute(c.name, json!({}));
    }
    assert_eq!(s.doc.revision(), revision);
}

#[test]
fn dimensions_follow_and_drive_what_they_measure() {
    let mut s = Session::default();
    let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/casa_simples.ccad");
    run(&mut s, "project_open", json!({"path": sample}));
    let plan = run(&mut s, "views_list", json!({"category": "floor_plan"}))[0]["id"].clone();
    // A 20 cm column far from the house; measure from 4 m left of it to its left face.
    let column = run(&mut s, "column_create", json!({"position": [50.0, 50.0]}));
    let face = [49.9, 50.0];
    let dim = run(&mut s, "dimension_create", json!({"view": plan, "a": [45.9, 50.0], "b": face, "kind": "linear"}));
    let refs = &dim["params"]["refs"];
    assert!(refs[0].is_null() && refs[1]["target"]["Element"] == column["id"], "{refs}");
    // The dimension follows the column...
    run(&mut s, "elements_move", json!({"ids": [column["id"]], "delta": [1.5, 0.0]}));
    let value = |s: &mut Session| {
        let d = run(s, "element_get", json!({"id": dim["id"]}));
        (d["params"]["b"][0].as_f64().unwrap() - d["params"]["a"][0].as_f64().unwrap()).abs()
    };
    assert!((value(&mut s) - 5.5).abs() < 1e-9);
    // ...and drives it: 4 m puts it back.
    run(&mut s, "dimension_set_value", json!({"id": dim["id"], "value": 4.0}));
    let x = run(&mut s, "element_get", json!({"id": column["id"]}))["params"]["position"][0].as_f64().unwrap();
    assert!((x - 50.0).abs() < 1e-9, "{x}");
    assert_eq!(
        fails(&mut s, "dimension_set_value", json!({"id": dim["id"], "value": -1.0})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        fails(&mut s, "dimension_set_value", json!({"id": column["id"], "value": 1.0})),
        ErrorCode::InvalidArguments
    );
    // A free dimension just takes the value; zero-length ones are refused.
    let free = run(&mut s, "dimension_create", json!({"view": plan, "a": [100.0, 100.0], "b": [103.0, 100.0]}));
    run(&mut s, "dimension_set_value", json!({"id": free["id"], "value": 5.0}));
    assert!(
        (run(&mut s, "element_get", json!({"id": free["id"]}))["params"]["b"][0].as_f64().unwrap() - 105.0).abs()
            < 1e-9
    );
    assert_eq!(
        fails(&mut s, "dimension_create", json!({"view": plan, "a": [1.0, 1.0], "b": [1.0, 1.0]})),
        ErrorCode::InvalidArguments
    );
    let three_d = run(&mut s, "view_create", json!({"kind": "three_d"}));
    assert_eq!(
        fails(&mut s, "dimension_create", json!({"view": three_d["id"], "a": [0, 0], "b": [1, 0]})),
        ErrorCode::InvalidArguments
    );
}

#[test]
fn turning_and_pinned_elements() {
    let mut s = Session::default();
    let wall = run(&mut s, "wall_create", json!({"start": [0, 0], "end": [6, 0]}));
    let door = run(&mut s, "door_create", json!({"wall": wall["id"]}));
    let column = run(&mut s, "column_create", json!({"position": [3, 3], "rotation": 300}));
    let out = run(&mut s, "elements_turn", json!({"ids": [door["id"], column["id"], wall["id"]]}));
    assert_eq!(out["changed"].as_array().unwrap().len(), 2, "walls do not turn");
    assert_eq!(run(&mut s, "element_get", json!({"id": column["id"]}))["params"]["rotation_deg"], 30.0);
    assert_eq!(run(&mut s, "element_get", json!({"id": door["id"]}))["params"]["flip_facing"], true);
    // Pinned elements refuse direct edits and stay out of group ones.
    run(&mut s, "elements_pin", json!({"ids": [wall["id"]], "pinned": true}));
    assert_eq!(fails(&mut s, "element_split", json!({"id": wall["id"], "point": [3, 0]})), ErrorCode::Rejected);
    assert_eq!(
        fails(&mut s, "element_move_end", json!({"id": wall["id"], "end": "end", "point": [8, 0]})),
        ErrorCode::Rejected
    );
    let moved = run(&mut s, "elements_move", json!({"ids": [wall["id"], column["id"]], "delta": [1, 0]}));
    assert_eq!(moved["pinned_skipped"], json!([wall["id"]]));
}

#[test]
fn export_and_paste_elements() {
    let mut s = Session::default();
    let wall = run(&mut s, "wall_create", json!({"start": [0, 0], "end": [5, 0]}));
    run(&mut s, "door_create", json!({"wall": wall["id"]}));
    let set = run(&mut s, "elements_export", json!({"ids": [wall["id"]]}));
    assert_eq!(set["elements"].as_array().unwrap().len(), 2, "the door comes along");
    let pasted = run(&mut s, "elements_paste", json!({"elements": set["elements"], "delta": [0, 4]}));
    let created = pasted["created"].as_array().unwrap();
    assert_eq!(created.len(), 2);
    let y = |s: &mut Session, id: &Value| run(s, "element_get", json!({"id": id}))["params"]["start"][1].as_f64();
    assert!(created.iter().any(|id| y(&mut s, id) == Some(4.0)));
    assert_eq!(fails(&mut s, "elements_paste", json!({"elements": [{"bogus": true}]})), ErrorCode::InvalidArguments);
}

#[test]
fn merged_edits_are_one_undo_step() {
    use cc_engine::commands::edit::ElementSet;
    let mut s = Session::default();
    let wall = run(&mut s, "wall_create", json!({"start": [0, 0], "end": [5, 0], "height": 3.0}));
    let id = wall["id"].as_u64().unwrap();
    let depth = s.doc.undo_depth();
    for h in [3.1, 3.2, 3.3] {
        let set = ElementSet {
            id,
            level: None,
            type_ref: None,
            mark: None,
            comments: None,
            params: Some(json!({"height": h})),
        };
        s.run_merged(7, set).unwrap();
    }
    assert_eq!(s.doc.undo_depth(), depth + 1);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "element_get", json!({"id": id}))["params"]["height"], 3.0);
}

#[test]
fn recording_a_macro() {
    use cc_engine::commands::edit::ElementSet;
    let mut s = Session::default();
    s.start_recording();
    let wall = run(&mut s, "wall_create", json!({"start": [0, 0], "end": [5, 0]}));
    run(&mut s, "elements_list", json!({}));
    assert!(s.execute("element_get", json!({"id": 424242})).is_err());
    let id = wall["id"].as_u64().unwrap();
    for h in [3.1, 3.2, 3.3] {
        let set = ElementSet {
            id,
            level: None,
            type_ref: None,
            mark: None,
            comments: None,
            params: Some(json!({"height": h})),
        };
        s.run_merged(9, set).unwrap();
    }
    // A cancelled dialog's edits are dropped from the recording.
    let depth = s.doc.undo_depth();
    run(&mut s, "level_create", json!({}));
    s.undo_to(depth);
    let steps = s.stop_recording();
    let names: Vec<&str> = steps.iter().map(|st| st.command.as_str()).collect();
    assert_eq!(names, ["wall_create", "element_set"], "queries, failures and cancelled edits are left out");
    assert_eq!(steps[1].args["params"]["height"], 3.3, "a dragged value is recorded once, at its end");
    // The macro replays on a new project.
    let text = cc_engine::script::to_text(&steps);
    let mut other = Session::default();
    cc_engine::script::run(&mut other, &cc_engine::script::parse(&text).unwrap()).unwrap();
    let walls = run(&mut other, "elements_list", json!({"category": "wall"}));
    let replayed = run(&mut other, "element_get", json!({"id": walls[0]["id"]}));
    assert_eq!(replayed["params"]["height"], 3.3);
}

/// The example macro in `examples/` keeps working.
#[test]
fn the_example_macro_builds_a_house() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/small_house.ccmacro");
    let steps = cc_engine::script::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut s = Session::default();
    cc_engine::script::run(&mut s, &steps).unwrap_or_else(|(_, e)| panic!("step {}: {}", e.index + 1, e.error));
    let count = |s: &mut Session, c: &str| run(s, "elements_list", json!({"category": c})).as_array().unwrap().len();
    assert_eq!(
        [count(&mut s, "wall"), count(&mut s, "door"), count(&mut s, "window"), count(&mut s, "room")],
        [5, 2, 3, 2]
    );
    let room = run(&mut s, "elements_list", json!({"category": "room"}))[0]["id"].clone();
    assert!(run(&mut s, "element_get", json!({"id": room}))["derived"]["area"].as_f64().unwrap() > 20.0);
}

#[test]
fn text_notes_with_leaders() {
    let mut s = Session::default();
    let plan = run(&mut s, "views_list", json!({"category": "floor_plan"}))[0]["id"].clone();
    // Single-line text, then multiline text with a straight leader and a two-segment one.
    let single = run(&mut s, "text_create", json!({"view": plan, "position": [1, 1], "text": "Living room"}));
    assert!(single["params"]["width"].is_null());
    let multi = run(
        &mut s,
        "text_create",
        json!({
            "view": plan, "position": [5, 5], "text": "Exposed concrete\nwith sealant", "width": 30.0,
            "leaders": [
                {"end": [2, 2]},
                {"end": [9, 2], "elbow": [8, 4], "symbol": "Circle"}
            ]
        }),
    );
    let leaders = &multi["params"]["leaders"];
    assert_eq!(leaders[0]["symbol"], "Arrow", "the type's symbol by default");
    assert_eq!(leaders[1]["elbow"], json!([8.0, 4.0]));
    assert_eq!(leaders[1]["symbol"], "Circle");
    // The view draws the lines of text and the leaders.
    let view = cc_core::ViewId(plan.as_u64().unwrap());
    let drawing = s.drawing(view).unwrap();
    let texts: Vec<&str> = drawing
        .prims
        .iter()
        .filter_map(|p| match p {
            cc_views::Prim::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for t in ["Living room", "Exposed concrete", "with sealant"] {
        assert!(texts.contains(&t), "{t} in {texts:?}");
    }
    // Edit the text, add and remove leaders, move it.
    let id = multi["id"].clone();
    let edited = run(&mut s, "element_set", json!({"id": id, "params": {"text": "Sealed concrete", "width": null}}));
    assert_eq!(edited["params"]["text"], "Sealed concrete");
    assert!(edited["params"]["width"].is_null(), "now single-line");
    let added = run(&mut s, "text_add_leader", json!({"id": id, "end": [0, 9], "symbol": "Tick"}));
    assert_eq!(added["params"]["leaders"].as_array().unwrap().len(), 3);
    let removed = run(&mut s, "text_remove_leader", json!({"id": id, "index": 0}));
    assert_eq!(removed["params"]["leaders"][0]["symbol"], "Circle");
    assert_eq!(fails(&mut s, "text_remove_leader", json!({"id": id, "index": 9})), ErrorCode::NotFound);
    let moved = run(&mut s, "elements_move", json!({"ids": [id], "delta": [1, 0]}));
    assert_eq!(moved["changed"], json!([id]));
    let after = run(&mut s, "element_get", json!({"id": id}));
    assert_eq!(after["params"]["position"], json!([6.0, 5.0]));
    assert_eq!(after["params"]["leaders"][0]["elbow"], json!([9.0, 4.0]), "leaders move with the text");
    // Texts and leaders survive saving and opening.
    let path = scratch("text.ccad");
    run(&mut s, "project_save", json!({"path": path}));
    let mut reopened = Session::default();
    run(&mut reopened, "project_open", json!({"path": path}));
    assert_eq!(run(&mut reopened, "element_get", json!({"id": id}))["params"], after["params"]);
    // Refusals.
    assert_eq!(
        fails(&mut s, "text_create", json!({"view": plan, "position": [0, 0], "text": "  "})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        fails(
            &mut s,
            "text_create",
            json!({"view": plan, "position": [0, 0], "text": "x", "leaders": [{"end": [1, 1], "symbol": "Star"}]})
        ),
        ErrorCode::InvalidArguments
    );
    let schedule = run(&mut s, "view_create", json!({"kind": "schedule"}));
    assert_eq!(
        fails(&mut s, "text_create", json!({"view": schedule["id"], "position": [0, 0], "text": "x"})),
        ErrorCode::InvalidArguments
    );
}

#[test]
fn joined_geometry_follows_the_cutting_order() {
    let mut s = Session::default();
    // A wall with a structural column in it, and an architectural one.
    let wall = id(&run(&mut s, "wall_create", json!({"start": [0, 0], "end": [6, 0], "height": 3.0})));
    let column = id(&run(&mut s, "column_create", json!({"position": [3, 0], "height": 3.0})));
    let deco = id(&run(&mut s, "column_create", json!({"position": [5, 0], "height": 3.0, "structural": false})));
    let far = id(&run(&mut s, "column_create", json!({"position": [3, 9], "height": 3.0})));
    let volume =
        |s: &mut Session, e: u64| run(s, "element_get", json!({"id": e}))["derived"]["volume"].as_f64().unwrap();
    let wall_v = volume(&mut s, wall);
    let deco_v = volume(&mut s, deco);

    // Structural elements cut hosts, whichever is picked first.
    let joined = run(&mut s, "geometry_join", json!({"first": wall, "second": column}));
    assert_eq!(joined, json!({"cutting": column, "cut": wall}));
    assert!(volume(&mut s, wall) < wall_v - 1e-6, "the wall gives up the column's volume");
    assert_eq!(run(&mut s, "element_get", json!({"id": wall}))["joined_with"], json!([column]));
    // Walls cut architectural columns.
    let joined = run(&mut s, "geometry_join", json!({"first": deco, "second": wall}));
    assert_eq!(joined, json!({"cutting": wall, "cut": deco}));
    assert!(volume(&mut s, deco) < deco_v - 1e-6);

    // Unjoin gives the volume back; undo joins again.
    run(&mut s, "geometry_unjoin", json!({"first": column, "second": wall}));
    run(&mut s, "geometry_unjoin", json!({"first": wall, "second": deco}));
    assert!((volume(&mut s, wall) - wall_v).abs() < 1e-9);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "element_get", json!({"id": deco}))["joined_with"], json!([wall]));

    // Deleting an element drops its joins.
    run(&mut s, "geometry_join", json!({"first": wall, "second": column}));
    run(&mut s, "elements_delete", json!({"ids": [column]}));
    assert_eq!(run(&mut s, "element_get", json!({"id": wall}))["joined_with"], json!([deco]));

    // Refusals: apart, itself, not joined, no body.
    assert_eq!(fails(&mut s, "geometry_join", json!({"first": wall, "second": far})), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "geometry_join", json!({"first": wall, "second": wall})), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "geometry_unjoin", json!({"first": wall, "second": far})), ErrorCode::NotFound);
    let grid = id(&run(&mut s, "grid_create", json!({"start": [0, -1], "end": [6, -1]})));
    assert_eq!(fails(&mut s, "geometry_join", json!({"first": wall, "second": grid})), ErrorCode::InvalidArguments);
}

#[test]
fn instances_of_a_type_shown_in_a_view() {
    let mut s = Session::default();
    let levels = run(&mut s, "levels_list", json!({}));
    let upper = levels[1]["id"].clone();
    let a = id(&run(&mut s, "wall_create", json!({"start": [0, 0], "end": [4, 0]})));
    let b = id(&run(&mut s, "wall_create", json!({"start": [4, 0], "end": [4, 4]})));
    let up = id(&run(&mut s, "wall_create", json!({"start": [0, 0], "end": [4, 0], "level": upper})));
    let ty = run(&mut s, "element_get", json!({"id": a}))["type"]["id"].clone();
    let plan = run(&mut s, "views_list", json!({"category": "floor_plan"}))[0]["id"].clone();
    let ids = |v: Value| -> Vec<u64> { v.as_array().unwrap().iter().map(|e| e["id"].as_u64().unwrap()).collect() };
    let all = ids(run(&mut s, "elements_list", json!({"type": ty})));
    assert!(all.contains(&a) && all.contains(&b) && all.contains(&up));
    let shown = ids(run(&mut s, "elements_list", json!({"type": ty, "view": plan})));
    assert_eq!(shown, vec![a, b], "the upper wall is not in the ground plan");
    let schedule = run(&mut s, "view_create", json!({"kind": "schedule"}));
    let refused = fails(&mut s, "elements_list", json!({"type": ty, "view": schedule["id"]}));
    assert_eq!(refused, ErrorCode::InvalidArguments);
}

#[test]
fn groups_keep_elements_together() {
    let mut s = Session::default();
    let a = id(&run(&mut s, "wall_create", json!({"start": [0, 0], "end": [4, 0]})));
    let b = id(&run(&mut s, "wall_create", json!({"start": [4, 0], "end": [4, 4]})));
    let c = id(&run(&mut s, "column_create", json!({"position": [8, 8]})));
    let group = run(&mut s, "group_create", json!({"ids": [a, b]}));
    let g = id(&group);
    assert_eq!(group["members"], json!([a, b]));
    assert_eq!(run(&mut s, "element_get", json!({"id": a}))["group"], g);
    assert_eq!(run(&mut s, "groups_list", json!({}))[0]["name"], group["name"]);
    // An element joins one group at most: regrouping takes it out of the first.
    let other = run(&mut s, "group_create", json!({"ids": [b, c], "name": "Core"}));
    assert_eq!(run(&mut s, "element_get", json!({"id": b}))["group"], other["id"]);
    // The first group is left with one member; deleting it empties and removes the group.
    run(&mut s, "elements_delete", json!({"ids": [a]}));
    assert_eq!(run(&mut s, "groups_list", json!({})).as_array().unwrap().len(), 1);
    run(&mut s, "group_ungroup", json!({"group": id(&other)}));
    assert!(run(&mut s, "element_get", json!({"id": b}))["group"].is_null());
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "element_get", json!({"id": c}))["group"], other["id"]);
    assert_eq!(fails(&mut s, "group_create", json!({"ids": [c]})), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "group_ungroup", json!({"group": 999})), ErrorCode::NotFound);
}
