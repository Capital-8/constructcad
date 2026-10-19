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
