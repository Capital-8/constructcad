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

    // Any view as an image: a plan, the section, a 3D view, the sheet.
    let png_size = |bytes: &[u8]| {
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let be = |i: usize| u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        (be(16), be(20))
    };
    let views = run(&mut s, "views_list", json!({}));
    let id_of = |kind: &str| views.as_array().unwrap().iter().find(|v| v["category"] == kind).unwrap()["id"].clone();
    for (view, file) in
        [(id_of("floor_plan"), "plan.png"), (section["id"].clone(), "section.png"), (id_of("three_d"), "3d.png")]
    {
        let out = scratch(file);
        let r = run(&mut s, "view_export", json!({"view": view, "path": out, "dpi": 96}));
        let (w, h) = png_size(&std::fs::read(&out).unwrap());
        let paper = (r["paper_mm"][0].as_f64().unwrap(), r["paper_mm"][1].as_f64().unwrap());
        assert_eq!((w as f64, h as f64), ((paper.0 / 25.4 * 96.0).round(), (paper.1 / 25.4 * 96.0).round()), "{file}");
        assert!(w > 50 && h > 50, "{file}: {w} x {h}");
    }
    // A sheet keeps its paper: A-size sheets at 50 dpi.
    let out = scratch("sheet.png");
    let r = run(&mut s, "view_export", json!({"view": sheet["id"], "path": out, "dpi": 50}));
    let (w, _) = png_size(&std::fs::read(&out).unwrap());
    assert_eq!(w as f64, (r["paper_mm"][0].as_f64().unwrap() / 25.4 * 50.0).round());
    // The same view as SVG; doubling the resolution doubles the pixels.
    let svg = scratch("plan.svg");
    run(&mut s, "view_export", json!({"view": id_of("floor_plan"), "path": svg}));
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<svg"));
    let (a, b) = (scratch("lo.png"), scratch("hi.png"));
    run(&mut s, "view_export", json!({"view": id_of("floor_plan"), "path": a, "dpi": 40}));
    run(&mut s, "view_export", json!({"view": id_of("floor_plan"), "path": b, "dpi": 80}));
    let (wa, wb) = (png_size(&std::fs::read(&a).unwrap()).0, png_size(&std::fs::read(&b).unwrap()).0);
    assert!((wb as i64 - 2 * wa as i64).abs() <= 1, "{wa} {wb}");
    // Schedules have no drawing; absurd resolutions and unknown formats are refused.
    let bad = |file: &str, extra: Value| {
        let mut args = json!({"view": id_of("floor_plan"), "path": scratch(file)});
        args.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        args
    };
    assert_eq!(
        fails(&mut s, "view_export", json!({"view": schedule["id"], "path": scratch("x.png")})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(fails(&mut s, "view_export", bad("x.png", json!({"dpi": 1e6}))), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "view_export", bad("x.png", json!({"dpi": 0}))), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "view_export", bad("x.bmp", json!({}))), ErrorCode::InvalidArguments);
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

/// A window copied without its wall and pasted on another level goes into
/// the wall there (it used to stay on the wall it was copied from).
#[test]
fn a_lone_window_pasted_on_another_level_takes_the_wall_there() {
    let mut s = Session::default();
    let wall = run(&mut s, "wall_create", json!({"start": [0, 0], "end": [6, 0]}));
    let upper = run(&mut s, "level_create", json!({"name": "Superior", "elevation": 3.0}));
    let set = run(&mut s, "elements_export", json!({"ids": [wall["id"]]}));
    let copy = run(&mut s, "elements_paste", json!({"elements": set["elements"], "level": upper["id"]}));
    let upper_wall = copy["created"][0].clone();
    let window = run(&mut s, "window_create", json!({"wall": wall["id"], "point": [2.0, 0.0]}));
    let set = run(&mut s, "elements_export", json!({"ids": [window["id"]]}));
    let pasted = run(&mut s, "elements_paste", json!({"elements": set["elements"], "level": upper["id"]}));
    let new = &pasted["created"][0];
    let got = run(&mut s, "element_get", json!({"id": new}));
    assert_eq!(got["params"]["host"], upper_wall, "{got}");
    assert_eq!(got["level"]["id"], upper["id"], "{got}");
    // With no wall where it lands, nothing is pasted.
    let empty = run(&mut s, "level_create", json!({"name": "Vazio", "elevation": 6.0}));
    let none = run(&mut s, "elements_paste", json!({"elements": set["elements"], "level": empty["id"]}));
    assert_eq!(none["created"], json!([]));
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

/// Floors, roofs and ceilings from walls follow the outer boundary of the
/// walls: here the showcase house, an L with an arc wall, whose interior walls
/// meet the outer corners (once picked as the old 9 x 5 rectangle).
#[test]
fn outlines_from_walls_follow_the_outer_boundary() {
    let mut s = Session::default();
    let walls = [
        json!({"start": [0, 0], "end": [9, 0], "type": 3}),
        json!({"start": [9, 0], "end": [9, 5], "type": 3}),
        json!({"start": [9, 5], "end": [0, 5], "type": 3}),
        json!({"start": [0, 5], "end": [0, 0], "type": 3}),
        json!({"start": [5.5, 0], "end": [5.5, 5], "type": 5}),
        json!({"start": [3, 0], "end": [3, 5], "type": 5}),
        json!({"start": [9, 0], "end": [12, 0], "type": 3}),
        json!({"start": [12, 0], "end": [12, 10], "type": 3}),
        json!({"start": [12, 10], "end": [7, 10], "mid": [9.5, 11.6], "type": 3}),
        json!({"start": [7, 10], "end": [7, 5], "type": 3}),
        json!({"start": [9, 5], "end": [12, 5], "type": 3}),
        json!({"start": [9.5, 5], "end": [9.5, 7.5], "type": 5}),
        json!({"start": [9.5, 7.5], "end": [12, 7.5], "type": 5}),
    ];
    for wall in walls {
        run(&mut s, "wall_create", wall);
    }
    // Type 3 is 15 cm thick.
    let half = 0.075;
    // The L on the wall lines: 12 x 5 plus 5 x 5 beside the arc, plus the arc's segment.
    let arc = run(&mut s, "elements_list", json!({"category": "wall"}))
        .as_array()
        .unwrap()
        .iter()
        .map(|w| run(&mut s, "element_get", json!({"id": w["id"]})))
        .find(|w| w["params"]["bulge"].as_f64().unwrap_or(0.0) != 0.0)
        .unwrap();
    let (r, sweep) =
        (arc["derived"]["radius"].as_f64().unwrap(), arc["derived"]["sweep"].as_f64().unwrap().to_radians());
    let segment = r * r * 0.5 * (sweep.abs() - sweep.abs().sin());
    let on_lines = 12.0 * 5.0 + 5.0 * 5.0 + segment;
    let area = |v: &Value| {
        let outline = if v["params"]["boundary"].is_array() { "boundary" } else { "footprint" };
        let pts: Vec<(f64, f64)> = v["params"][outline]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
            .collect();
        let n = pts.len();
        (0..n).map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1).sum::<f64>().abs() / 2.0
    };
    // Exterior faces: grown by half a wall all around (perimeter 44 m on the lines, about).
    let floor = run(&mut s, "floor_create", json!({"from_walls": true}));
    let grown = area(&floor);
    assert!(grown > on_lines + 40.0 * half && grown < on_lines + 50.0 * half, "{grown} vs {on_lines}");
    let roof = run(&mut s, "roof_create", json!({"from_walls": true}));
    assert!((area(&roof) - grown).abs() < 1e-6);
    // Interior faces for ceilings.
    let ceiling = run(&mut s, "ceiling_create", json!({"from_walls": true}));
    let shrunk = area(&ceiling);
    assert!(shrunk < on_lines - 40.0 * half && shrunk > on_lines - 50.0 * half, "{shrunk} vs {on_lines}");
}

/// The showcase macro (the project of the README screenshots) keeps working,
/// and the ids its later steps name still point at what they expect.
#[test]
fn the_showcase_macro_builds_its_project() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/showcase.ccmacro");
    let steps = cc_engine::script::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut s = Session::default();
    cc_engine::script::run(&mut s, &steps).unwrap_or_else(|(_, e)| panic!("step {}: {}", e.index + 1, e.error));
    let count = |s: &mut Session, c: &str| run(s, "elements_list", json!({"category": c})).as_array().unwrap().len();
    assert_eq!([count(&mut s, "wall"), count(&mut s, "door"), count(&mut s, "room")], [13, 5, 5]);
    assert!(count(&mut s, "rebar") > 0 && count(&mut s, "pipe") > 0 && count(&mut s, "conduit") > 0);
    let get = |s: &mut Session, id: u64| run(s, "element_get", json!({"id": id}));
    assert!(get(&mut s, 97)["params"]["bulge"].as_f64().unwrap() > 0.0, "the arc wall");
    assert!(get(&mut s, 116)["params"]["shape"]["Hip"].is_object(), "the hip roof");
    assert_eq!(get(&mut s, 94)["phase_demolished"]["id"], 81, "the demolished partition");
    assert_eq!(get(&mut s, 247)["type"]["id"], 58, "the distribution board");
    let views = run(&mut s, "views_list", json!({}));
    let view = |id: u64| views.as_array().unwrap().iter().find(|v| v["id"] == id).map(|v| v["name"].clone());
    assert_eq!(view(333), Some(json!("Board diagram")));
    assert_eq!(view(341), Some(json!("3D North-east")));
    let loads = run(&mut s, "circuit_loads", json!({"panel": 247}));
    assert!(loads.to_string().contains("Kitchen and bath outlets"));
    for filter in [340, 342] {
        assert!(run(&mut s, "filter_elements", json!({"filter": filter, "view": 339}))["count"].as_u64().unwrap() > 0);
    }
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

#[test]
fn measuring_creates_nothing() {
    let mut s = Session::default();
    let before = run(&mut s, "project_info", json!({}));
    let m = run(&mut s, "measure", json!({"a": [1, 1], "b": [4, 5]}));
    assert!((m["distance"].as_f64().unwrap() - 5.0).abs() < 1e-12);
    assert_eq!((m["dx"].clone(), m["dy"].clone()), (json!(3.0), json!(4.0)));
    assert!((m["angle"].as_f64().unwrap() - 53.130102354).abs() < 1e-6);
    assert!(m["between_lines"].is_null());
    // Two parallel wall faces 0.2 apart, measured on a slant.
    let m = run(
        &mut s,
        "measure",
        json!({"a": [0, 0], "b": [3, 0.2], "line_a": [[-1, 0], [5, 0]], "line_b": [[0, 0.2], [9, 0.2]]}),
    );
    assert!((m["between_lines"].as_f64().unwrap() - 0.2).abs() < 1e-12);
    assert_eq!(run(&mut s, "project_info", json!({})), before);
}

#[test]
fn beams_frame_into_columns_and_each_other() {
    let mut s = Session::default();
    // Two 0.2 m columns 5 m apart with a 0.15 m beam between their centers,
    // a beam going north from the second and one along the north side.
    let c1 = id(&run(&mut s, "column_create", json!({"position": [0, 0]})));
    let c2 = id(&run(&mut s, "column_create", json!({"position": [5, 0]})));
    let span = id(&run(&mut s, "beam_create", json!({"start": [0, 0], "end": [5, 0]})));
    let side = id(&run(&mut s, "beam_create", json!({"start": [5, 0], "end": [5, 4]})));
    let north = id(&run(&mut s, "beam_create", json!({"start": [0, 4], "end": [5, 4]})));
    let derived = |s: &mut Session, beam: u64| run(s, "element_get", json!({"id": beam}))["derived"].clone();

    let d = derived(&mut s, span);
    assert_eq!((d["start_join"].clone(), d["end_join"].clone()), (json!({"column": c1}), json!({"column": c2})));
    assert!((d["length"].as_f64().unwrap() - 5.0).abs() < 1e-9);
    assert!((d["cut_length"].as_f64().unwrap() - 4.8).abs() < 1e-9, "{d}");
    assert!((d["volume"].as_f64().unwrap() - 0.15 * 0.4 * 4.8).abs() < 1e-9, "{d}");
    // A mitered corner between the side and north beams.
    let d = derived(&mut s, side);
    assert_eq!((d["start_join"].clone(), d["end_join"].clone()), (json!({"column": c2}), json!("beam")));
    assert_eq!(derived(&mut s, north)["end_join"], json!("beam"));

    // A disallowed end stays square at its location point, and undoes.
    run(&mut s, "wall_join", json!({"id": span, "end": "start", "joined": false}));
    let d = derived(&mut s, span);
    assert_eq!(d["start_join"], json!("free"));
    assert!((d["cut_length"].as_f64().unwrap() - 4.9).abs() < 1e-9, "{d}");
    assert_eq!(run(&mut s, "element_get", json!({"id": span}))["params"]["join_start"], json!(false));
    run(&mut s, "undo", json!({}));
    assert_eq!(derived(&mut s, span)["start_join"], json!({"column": c1}));
    assert_eq!(
        fails(&mut s, "wall_join", json!({"id": c1, "end": "start", "joined": false})),
        ErrorCode::InvalidArguments
    );
}

#[test]
fn columns_and_beams_from_grids() {
    let mut s = Session::default();
    // Two grids each way: a 6 x 4 m bay, plus a short grid crossing nothing.
    let a = id(&run(&mut s, "grid_create", json!({"start": [-1, 0], "end": [7, 0]})));
    let b = id(&run(&mut s, "grid_create", json!({"start": [-1, 4], "end": [7, 4]})));
    let one = id(&run(&mut s, "grid_create", json!({"start": [0, -1], "end": [0, 5]})));
    let two = id(&run(&mut s, "grid_create", json!({"start": [6, -1], "end": [6, 5]})));
    run(&mut s, "grid_create", json!({"start": [20, 20], "end": [21, 20]}));
    let columns = |s: &mut Session| run(s, "elements_list", json!({"category": "column"})).as_array().unwrap().len();

    let out = run(&mut s, "columns_at_grids", json!({}));
    assert_eq!(out["created"].as_array().unwrap().len(), 4, "{out}");
    assert_eq!(columns(&mut s), 4);
    // Once is enough: the intersections are taken now.
    let again = run(&mut s, "columns_at_grids", json!({"grids": [a, b, one, two]}));
    assert_eq!((again["created"].as_array().unwrap().len(), again["skipped"].as_u64()), (0, Some(4)));
    // All four columns came in one undo step.
    run(&mut s, "undo", json!({}));
    assert_eq!(columns(&mut s), 0);
    run(&mut s, "redo", json!({}));
    assert_eq!(fails(&mut s, "columns_at_grids", json!({"grids": [a]})), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "columns_at_grids", json!({"grids": [a, 999_999]})), ErrorCode::NotFound);

    // A beam between the neighboring columns on each grid, framed into them.
    let out = run(&mut s, "beams_on_grids", json!({}));
    let beams: Vec<u64> = out["created"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    assert_eq!(beams.len(), 4, "{out}");
    let d = run(&mut s, "element_get", json!({"id": beams[0]}))["derived"].clone();
    assert!(d["start_join"]["column"].is_u64() && d["end_join"]["column"].is_u64(), "{d}");
    let again = run(&mut s, "beams_on_grids", json!({"grids": [a]}));
    assert_eq!((again["created"].as_array().unwrap().len(), again["skipped"].as_u64()), (0, Some(1)));

    // Beams in a chain through chosen columns: the diagonal is new, the side exists.
    let ids: Vec<u64> = run(&mut s, "elements_list", json!({"category": "column"}))
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_u64().unwrap())
        .collect();
    let at = |s: &mut Session, id: u64| run(s, "element_get", json!({"id": id}))["params"]["position"].clone();
    let origin = *ids.iter().find(|&&c| at(&mut s, c) == json!([0.0, 0.0])).unwrap();
    let far = *ids.iter().find(|&&c| at(&mut s, c) == json!([6.0, 4.0])).unwrap();
    let side = *ids.iter().find(|&&c| at(&mut s, c) == json!([6.0, 0.0])).unwrap();
    let out = run(&mut s, "beams_between_columns", json!({"columns": [far, origin, side], "top_offset": 2.5}));
    assert_eq!((out["created"].as_array().unwrap().len(), out["skipped"].as_u64()), (1, Some(1)), "{out}");
    let diagonal = run(&mut s, "element_get", json!({"id": out["created"][0]}));
    assert_eq!(diagonal["params"]["top_offset"], json!(2.5));
    assert_eq!(fails(&mut s, "beams_between_columns", json!({"columns": [far]})), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "beams_between_columns", json!({"columns": [far, a]})), ErrorCode::InvalidArguments);
}

#[test]
fn beam_justification_moves_the_body() {
    let mut s = Session::default();
    let beam =
        id(&run(&mut s, "beam_create", json!({"start": [0, 0], "end": [4, 0], "top_offset": 3.0, "lateral": "left"})));
    let justified = |s: &mut Session| {
        let b = run(s, "element_get", json!({"id": beam}))["params"].clone();
        (b["lateral"].clone(), b["vertical"].clone())
    };
    assert_eq!(justified(&mut s), (json!("Left"), json!("Top")));
    run(&mut s, "element_set", json!({"id": beam, "params": {"vertical": "Bottom"}}));
    assert_eq!(justified(&mut s), (json!("Left"), json!("Bottom")));
    let d = run(&mut s, "element_get", json!({"id": beam}))["derived"].clone();
    assert!((d["volume"].as_f64().unwrap() - 0.15 * 0.4 * 4.0).abs() < 1e-9, "{d}");
    assert_eq!(
        fails(&mut s, "beam_create", json!({"start": [0, 0], "end": [1, 0], "lateral": "middle"})),
        ErrorCode::InvalidArguments
    );
}

#[test]
fn foundations_under_columns_and_walls() {
    let mut s = Session::default();
    let volume =
        |s: &mut Session, id: u64| run(s, "element_get", json!({"id": id}))["derived"]["volume"].as_f64().unwrap();
    // Two columns: a 1.00 x 1.00 x 0.40 pad under each, once.
    for x in [0, 5] {
        run(&mut s, "column_create", json!({"position": [x, 0]}));
    }
    let out = run(&mut s, "footings_under_columns", json!({}));
    let pads: Vec<u64> = out["created"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    assert_eq!(pads.len(), 2, "{out}");
    assert!((volume(&mut s, pads[0]) - 0.4).abs() < 1e-9);
    let pad = run(&mut s, "element_get", json!({"id": pads[0]}));
    assert_eq!(pad["category"], json!("foundation"));
    let again = run(&mut s, "footings_under_columns", json!({}));
    assert_eq!((again["created"].as_array().unwrap().len(), again["skipped"].as_u64()), (0, Some(2)));

    // A 6 x 4 m loop of walls: 0.20 m strips meeting at the corners tile 20 m of centerline.
    let corners = [[10, 0], [16, 0], [16, 4], [10, 4]];
    for i in 0..4 {
        run(&mut s, "wall_create", json!({"start": corners[i], "end": corners[(i + 1) % 4]}));
    }
    let out = run(&mut s, "strip_footings_under_walls", json!({}));
    let strips: Vec<u64> = out["created"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    assert_eq!(strips.len(), 4, "{out}");
    let total: f64 = strips.iter().map(|id| volume(&mut s, *id)).sum();
    assert!((total - 20.0 * 0.2 * 0.4).abs() < 1e-9, "{total}");

    // A raft under the loop of walls, 0.30 m thick.
    let raft = run(&mut s, "foundation_slab_create", json!({"boundary": [[10, 0], [16, 0], [16, 4], [10, 4]]}));
    assert!((volume(&mut s, id(&raft)) - 24.0 * 0.3).abs() < 1e-9);
    // A pad needs a pad type.
    assert_eq!(
        fails(&mut s, "footing_create", json!({"position": [0, 9], "type": "Radier 30 cm"})),
        ErrorCode::InvalidArguments
    );
    let pad = run(&mut s, "footing_create", json!({"position": [0, 9], "rotation": 45, "top_offset": -0.5}));
    assert_eq!(pad["params"]["top_offset"], json!(-0.5));
}

#[test]
fn sloped_beams_and_steel_sections() {
    let mut s = Session::default();
    let derived = |s: &mut Session, id: u64| run(s, "element_get", json!({"id": id}))["derived"].clone();
    // A beam rising 1 m over 4 m: its 0.15 x 0.40 section runs the true length.
    let beam =
        id(&run(&mut s, "beam_create", json!({"start": [0, 0], "end": [4, 0], "top_offset": 3.0, "end_offset": 4.0})));
    let d = derived(&mut s, beam);
    let true_length = 17f64.sqrt();
    assert!((d["true_length"].as_f64().unwrap() - true_length).abs() < 1e-9, "{d}");
    assert!((d["volume"].as_f64().unwrap() - 0.15 * 0.4 * true_length).abs() < 1e-9, "{d}");
    // Splitting keeps the slope: the parts meet at the elevation in between.
    let second = run(&mut s, "element_split", json!({"id": beam, "point": [2, 0]}));
    let first = run(&mut s, "element_get", json!({"id": beam}));
    assert_eq!(first["params"]["end_offset"], json!(3.5));
    assert_eq!(run(&mut s, "element_get", json!({"id": second["created"]}))["params"]["top_offset"], json!(3.5));

    // An I profile: two 0.02 flanges and a 0.01 web in a 0.15 x 0.40 box.
    let ty = run(&mut s, "type_duplicate", json!({"type": "Viga 15 x 40 cm", "name": "Perfil I", "elements": [beam]}));
    run(&mut s, "type_set", json!({"type": ty["id"], "params": {"shape": {"I": {"web": 0.01, "flange": 0.02}}}}));
    let area = 2.0 * 0.15 * 0.02 + 0.01 * (0.4 - 0.04);
    let d = derived(&mut s, beam);
    assert!((d["volume"].as_f64().unwrap() - area * true_length / 2.0).abs() < 1e-9, "{d}");
    // Back to a solid section: the variant is replaced, not merged.
    run(&mut s, "type_set", json!({"type": ty["id"], "params": {"shape": "Solid"}}));

    // A column switching to a tube profile and to a pipe.
    let column = id(&run(&mut s, "column_create", json!({"position": [9, 9], "height": 3.0})));
    let ty = run(&mut s, "type_duplicate", json!({"type": "Pilar 20 x 20 cm", "name": "Tubo", "elements": [column]}));
    let tube = json!({"Profile": {"width": 0.2, "depth": 0.2, "shape": {"Tube": {"wall": 0.01}}}});
    run(&mut s, "type_set", json!({"type": ty["id"], "params": {"section": tube}}));
    let volume = derived(&mut s, column)["volume"].as_f64().unwrap();
    assert!((volume - (0.04 - 0.18 * 0.18) * 3.0).abs() < 1e-9, "{volume}");
    run(
        &mut s,
        "type_set",
        json!({"type": ty["id"], "params": {"section": {"Pipe": {"diameter": 0.2, "wall": 0.01}}}}),
    );
    assert!(derived(&mut s, column)["volume"].as_f64().unwrap() < volume);
}

#[test]
fn slabs_join_their_supports_and_the_analytical_model() {
    let mut s = Session::default();
    // A slab over two columns and a beam hanging from it, and a beam well below it.
    for x in [0, 6] {
        run(&mut s, "column_create", json!({"position": [x, 0], "height": 3.0}));
    }
    let beam = id(&run(&mut s, "beam_create", json!({"start": [0, 0], "end": [6, 0], "top_offset": 3.0})));
    let low = id(&run(&mut s, "beam_create", json!({"start": [0, 2], "end": [6, 2], "top_offset": 1.5})));
    let slab = id(&run(
        &mut s,
        "floor_create",
        json!({"boundary": [[-1, -1], [7, -1], [7, 3], [-1, 3]], "level": "Pavimento 1", "offset": 0.0}),
    ));
    let before = run(&mut s, "element_get", json!({"id": slab}))["derived"]["volume"].as_f64().unwrap();
    let out = run(&mut s, "slab_join_supports", json!({}));
    let joined = out["joined"].as_array().unwrap();
    assert!(joined.iter().any(|j| j["cutting"] == json!(beam) && j["cut"] == json!(slab)), "{out}");
    assert!(!joined.iter().any(|j| j["cutting"] == json!(low)), "the low beam does not touch the slab");
    let after = run(&mut s, "element_get", json!({"id": slab}))["derived"]["volume"].as_f64().unwrap();
    assert!(after < before - 1e-6, "the beam took its share: {before} -> {after}");
    // Running it again joins nothing new.
    assert_eq!(run(&mut s, "slab_join_supports", json!({}))["joined"], json!([]));

    // The analytical model: the beam between the column axes, splitting both columns.
    let m = run(&mut s, "analytical_model", json!({}));
    let members = m["members"].as_array().unwrap();
    let of = |id: u64| members.iter().filter(|x| x["element"] == json!(id)).count();
    assert_eq!(of(beam), 1);
    assert_eq!(members.iter().filter(|x| x["kind"] == json!("column")).count(), 4, "{m}");
    let node = |i: &Value| m["nodes"][i.as_u64().unwrap() as usize].clone();
    let main = members.iter().find(|x| x["element"] == json!(beam)).unwrap();
    assert_eq!((node(&main["start"])["x"].clone(), node(&main["end"])["x"].clone()), (json!(0.0), json!(6.0)));
    assert!((main["length"].as_f64().unwrap() - 6.0).abs() < 1e-9);
    assert_eq!(m["nodes"].as_array().unwrap().iter().filter(|n| n["support"] == json!(true)).count(), 2);
    let path = scratch("model.analytical.json");
    let out = run(&mut s, "analytical_export", json!({"path": path}));
    assert_eq!(out["members"].as_u64(), Some(members.len() as u64));
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written["format"], json!("constructcad-analytical"));
    // An analytical 3D view.
    let view = run(&mut s, "view_create", json!({"kind": "analytical"}));
    assert_eq!(view["params"]["analytical"], json!(true), "{view}");
}

#[test]
fn reinforcement_of_beams_and_columns() {
    let mut s = Session::default();
    // A 0.15 x 0.40 beam, 5 m long, free; a 0.20 x 0.20 column, 3 m tall.
    let beam = id(&run(&mut s, "beam_create", json!({"start": [0, 0], "end": [5, 0], "top_offset": 3.0})));
    let column = id(&run(&mut s, "column_create", json!({"position": [9, 9], "height": 3.0})));
    let out = run(&mut s, "rebar_typical", json!({"elements": [beam, column]}));
    // Beam: bottom, top and stirrups; column: longitudinal and ties.
    assert_eq!(out["created"].as_array().unwrap().len(), 5, "{out}");
    let again = run(&mut s, "rebar_typical", json!({"elements": [beam, column]}));
    assert_eq!(again["skipped"].as_u64(), Some(2));

    let sets: Vec<Value> =
        out["created"].as_array().unwrap().iter().map(|i| run(&mut s, "element_get", json!({"id": i}))).collect();
    let bottom = sets.iter().find(|x| x["params"]["layout"]["Longitudinal"]["face"] == json!("Bottom")).unwrap();
    // Two bars from cover to cover: 5 m - 2 x 0.025.
    assert_eq!(bottom["derived"]["count"], json!(2));
    assert!((bottom["derived"]["bar_length"].as_f64().unwrap() - 4.95).abs() < 1e-9, "{bottom}");
    // 10 mm bars weigh 0.617 kg/m.
    let weight = bottom["derived"]["weight"].as_f64().unwrap();
    assert!((weight - 2.0 * 4.95 * std::f64::consts::PI * 0.01 * 0.01 / 4.0 * 7850.0).abs() < 1e-9);
    // Stirrups every 0.15 over 4.95 m: 33 spaces, 34 stirrups, each around a 0.10 x 0.35 loop.
    let stirrups =
        sets.iter().find(|x| x["params"]["layout"]["Stirrups"].is_object() && x["params"]["host"] == json!(beam));
    let stirrups = stirrups.unwrap();
    assert_eq!(stirrups["derived"]["count"], json!(34));
    assert!((stirrups["derived"]["bar_length"].as_f64().unwrap() - 2.0 * (0.095 + 0.345)).abs() < 1e-9, "{stirrups}");
    // A column's bars run its height.
    let bars =
        sets.iter().find(|x| x["params"]["host"] == json!(column) && x["params"]["layout"]["Longitudinal"].is_object());
    assert!((bars.unwrap()["derived"]["bar_length"].as_f64().unwrap() - 3.0).abs() < 1e-9);

    // Bars go with their host.
    let n = |s: &mut Session| run(s, "elements_list", json!({"category": "rebar"})).as_array().unwrap().len();
    assert_eq!(n(&mut s), 5);
    run(&mut s, "elements_delete", json!({"ids": [beam]}));
    assert_eq!(n(&mut s), 2);
    // Only beams and columns take reinforcement; layouts are checked.
    let wall = id(&run(&mut s, "wall_create", json!({"start": [0, 5], "end": [4, 5]})));
    assert_eq!(fails(&mut s, "rebar_create", json!({"host": wall, "layout": "top"})), ErrorCode::InvalidArguments);
    assert_eq!(fails(&mut s, "rebar_create", json!({"host": column, "layout": "zigzag"})), ErrorCode::InvalidArguments);
    let ties = run(&mut s, "rebar_create", json!({"host": column, "layout": "stirrups", "spacing": 0.2}));
    assert_eq!(ties["params"]["layout"], json!({"Stirrups": {"spacing": 0.2}}));
    // The steel schedule.
    run(&mut s, "view_create", json!({"kind": "schedule", "category": "rebar"}));
}

#[test]
fn trusses_braces_numbering_and_structural_plans() {
    let mut s = Session::default();
    // A Pratt truss, 6 m in 4 panels, 1 m deep: 2 chords, 4 diagonals, 5 posts.
    let out = run(
        &mut s,
        "truss_create",
        json!({"start": [0, 0], "end": [6, 0], "height": 1.0, "panels": 4, "bottom_offset": 2.0}),
    );
    assert_eq!(out["created"].as_array().unwrap().len(), 11, "{out}");
    let group = run(&mut s, "groups_list", json!({}));
    assert_eq!(group.as_array().unwrap().len(), 1);
    // Its analytical model: all members meet at the 10 panel points.
    let m = run(&mut s, "analytical_model", json!({}));
    assert_eq!(m["nodes"].as_array().unwrap().len(), 10, "{m}");
    assert_eq!(
        fails(&mut s, "truss_create", json!({"start": [0, 0], "end": [6, 0], "height": 1.0, "pattern": "k"})),
        ErrorCode::InvalidArguments
    );

    // X bracing between two columns: two members from base to top.
    let a = id(&run(&mut s, "column_create", json!({"position": [0, 10], "height": 3.0})));
    let b = id(&run(&mut s, "column_create", json!({"position": [4, 10], "height": 3.0})));
    let out = run(&mut s, "braces_between_columns", json!({"columns": [a, b]}));
    let braces = out["created"].as_array().unwrap();
    assert_eq!(braces.len(), 2);
    let brace = run(&mut s, "element_get", json!({"id": braces[0]}));
    assert_eq!(
        (brace["params"]["top_offset"].clone(), brace["params"]["end_offset"].clone()),
        (json!(0.0), json!(3.0))
    );
    assert_eq!(fails(&mut s, "braces_between_columns", json!({"columns": [a]})), ErrorCode::InvalidArguments);
    assert_eq!(
        fails(&mut s, "brace_create", json!({"start": [1, 1, 0], "end": [1, 1, 3]})),
        ErrorCode::InvalidArguments
    );

    // Numbering in reading order: the upper column first.
    let c = id(&run(&mut s, "column_create", json!({"position": [0, 20], "height": 3.0})));
    let out = run(&mut s, "structure_number", json!({"categories": ["column"]}));
    assert!(out["numbered"].as_u64().unwrap() >= 3);
    let mark = |s: &mut Session, id: u64| run(s, "element_get", json!({"id": id}))["mark"].clone();
    assert_eq!(mark(&mut s, c), json!("P1"));
    assert_eq!((mark(&mut s, a), mark(&mut s, b)), (json!("P2"), json!("P3")));

    // A structural plan: tagged with marks and sections.
    let plan = run(&mut s, "view_create", json!({"kind": "structural_plan"}));
    assert_eq!(plan["params"]["structural"], json!(true), "{plan}");
}

/// The component type of the template whose MEP data names `device` (or a fixture, for `"Fixture"`).
fn mep_type(s: &mut Session, device: &str) -> u64 {
    let types = run(s, "types_list", json!({"category": "component"}));
    types
        .as_array()
        .unwrap()
        .iter()
        .map(id)
        .find(|t| {
            let ty = run(s, "type_get", json!({"type": t}));
            let mep = &ty["params"]["def"]["mep"];
            mep["Device"]["device"] == json!(device) || (device == "Fixture" && mep["Fixture"].is_object())
        })
        .unwrap_or_else(|| panic!("no {device} type"))
}

#[test]
fn building_services_through_commands() {
    let mut s = Session::default();
    // The template's plumbing systems.
    let systems = run(&mut s, "systems_list", json!({}));
    assert_eq!(systems.as_array().unwrap().len(), 5, "{systems}");
    let sewer_code = systems.as_array().unwrap().iter().find(|x| x["kind"] == json!("sanitary")).unwrap()["code"]
        .as_str()
        .unwrap()
        .to_string();
    let pipes = run(&mut s, "types_list", json!({"category": "pipe"}));
    let drain_100 = pipes.as_array().unwrap().iter().map(id).find(|t| {
        (run(&mut s, "type_get", json!({"type": t}))["params"]["diameter"].as_f64().unwrap() - 0.1).abs() < 1e-9
    });

    // A 100 mm drain falls 1 % from under the floor; a riser goes up from its end.
    let drain = run(
        &mut s,
        "pipe_create",
        json!({"start": [0, 0], "end": [5, 0], "system": sewer_code, "type": drain_100.unwrap()}),
    );
    assert_eq!(drain["params"]["start_offset"], json!(-0.3), "{drain}");
    assert!((drain["params"]["end_offset"].as_f64().unwrap() + 0.35).abs() < 1e-9, "{drain}");
    assert!((drain["derived"]["slope"].as_f64().unwrap() - 0.01).abs() < 1e-9);
    let riser = run(
        &mut s,
        "pipe_create",
        json!({"start": [5, 0], "system": sewer_code, "offset": -0.35, "end_offset": 2.8, "type": drain_100.unwrap()}),
    );
    assert!((riser["derived"]["length"].as_f64().unwrap() - 3.15).abs() < 1e-9, "{riser}");
    // A supply pipe is level, in the wall.
    let supply = run(&mut s, "pipe_create", json!({"start": [0, 1], "end": [3, 1], "system": "cold_water"}));
    assert_eq!(
        (supply["params"]["start_offset"].clone(), supply["params"]["end_offset"].clone()),
        (json!(0.6), json!(0.6))
    );
    assert_eq!(fails(&mut s, "pipe_create", json!({"start": [0, 0]})), ErrorCode::InvalidArguments);

    // A board, a lighting circuit on it, a light and two outlets.
    let panel_t = mep_type(&mut s, "Panel");
    let panel = id(&run(&mut s, "component_place", json!({"position": [0, 3], "type": panel_t})));
    let circuit =
        run(&mut s, "system_create", json!({"kind": "lighting", "panel": panel, "voltage": 127.0, "name": "Luz"}));
    assert_eq!(circuit["code"], json!("1"));
    let power = run(&mut s, "system_create", json!({"kind": "power", "panel": panel}));
    assert_eq!(power["code"], json!("2"));
    let light_t = mep_type(&mut s, "CeilingLight");
    let light = run(&mut s, "component_place", json!({"position": [2, 2], "type": light_t, "system": id(&circuit)}));
    // Lights go on the ceiling: the template's levels are 3 m apart.
    assert_eq!(light["params"]["base_offset"], json!(3.0), "{light}");
    let outlet_t = mep_type(&mut s, "Outlet");
    for x in [1.0, 3.0] {
        let o = run(&mut s, "component_place", json!({"position": [x, 0], "type": outlet_t, "system": id(&power)}));
        assert_eq!(o["params"]["base_offset"], json!(0.3));
    }
    // Circuits take devices and conduits, not pipes.
    assert_eq!(
        fails(&mut s, "system_assign", json!({"elements": [id(&drain)], "system": id(&power)})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        fails(&mut s, "component_place", json!({"position": [1, 1], "type": outlet_t, "system": sewer_code})),
        ErrorCode::InvalidArguments
    );
    let conduit = run(&mut s, "conduit_create", json!({"start": [0, 3], "end": [2, 2], "circuits": ["1", "2"]}));
    assert_eq!(conduit["params"]["systems"].as_array().unwrap().len(), 2);
    assert_eq!(conduit["params"]["start_offset"], json!(3.0));
    assert_eq!(
        fails(&mut s, "conduit_create", json!({"start": [0, 3], "end": [2, 2], "circuits": [sewer_code]})),
        ErrorCode::InvalidArguments
    );

    // The load schedule: 100 VA of lighting, 2 x 100 VA of outlets at 127 V.
    let loads = run(&mut s, "circuit_loads", json!({"panel": panel}));
    let rows = loads["circuits"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0]["power_va"].clone(), rows[1]["power_va"].clone()), (json!(100.0), json!(200.0)));
    assert!((rows[1]["current_a"].as_f64().unwrap() - 200.0 / 127.0).abs() < 1e-9);
    assert_eq!(loads["panels"][0]["power_va"], json!(300.0));

    // Fixture units add up in their system.
    let fixture_t = mep_type(&mut s, "Fixture");
    let wc = id(&run(&mut s, "component_place", json!({"position": [4, 4], "type": fixture_t})));
    run(&mut s, "system_assign", json!({"elements": [wc], "system": sewer_code}));
    let systems = run(&mut s, "systems_list", json!({}));
    let sewer = systems.as_array().unwrap().iter().find(|x| x["code"] == json!(sewer_code)).unwrap().clone();
    assert_eq!(sewer["elements"], json!(3), "{sewer}");
    assert!(sewer["drain_units"].as_f64().unwrap() > 0.0);
    assert!((sewer["length"].as_f64().unwrap() - (5.0f64.hypot(0.05) + 3.15)).abs() < 1e-9);

    // Changing and deleting systems; members stay, outside it.
    let set = run(&mut s, "system_set", json!({"system": "2", "params": {"name": "Tomadas", "voltage": 220.0}}));
    assert_eq!((set["name"].clone(), set["voltage"].clone()), (json!("Tomadas"), json!(220.0)));
    assert_eq!(
        fails(&mut s, "system_set", json!({"system": "2", "params": {"kind": "cold_water"}})),
        ErrorCode::Rejected
    );
    run(&mut s, "system_delete", json!({"system": "Tomadas"}));
    let after = run(&mut s, "element_get", json!({"id": id(&conduit)}));
    assert_eq!(after["params"]["systems"].as_array().unwrap().len(), 1, "{after}");
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "systems_list", json!({})).as_array().unwrap().len(), 7);

    // Deleting the board leaves its circuits unfed.
    run(&mut s, "elements_delete", json!({"ids": [panel]}));
    let loads = run(&mut s, "circuit_loads", json!({}));
    assert!(loads["circuits"].as_array().unwrap().iter().all(|c| c["panel"].is_null()), "{loads}");

    // Services plans and schedules; IFC export with the systems.
    let plan = run(&mut s, "view_create", json!({"kind": "plumbing_plan"}));
    assert_eq!(plan["params"]["mep"], json!("Plumbing"), "{plan}");
    run(&mut s, "view_create", json!({"kind": "electrical_plan"}));
    let table = run(&mut s, "view_create", json!({"kind": "schedule", "category": "pipe"}));
    let rows = run(&mut s, "schedule_table", json!({"view": id(&table)}));
    assert_eq!(rows["rows"].as_array().unwrap().len(), 3, "{rows}");
    let path = scratch("mep.ifc");
    run(&mut s, "ifc_export", json!({"path": path.to_string_lossy()}));
    let ifc = std::fs::read_to_string(&path).unwrap();
    for entity in [
        "IFCPIPESEGMENT(",
        "IFCCABLECARRIERSEGMENT(",
        "IFCDISTRIBUTIONSYSTEM(",
        "IFCDISTRIBUTIONCIRCUIT(",
        "IFCOUTLET(",
        "IFCLIGHTFIXTURE(",
        "IFCRELASSIGNSTOGROUP(",
    ] {
        assert!(ifc.contains(entity), "{entity} missing");
    }
}

#[test]
fn fixtures_and_devices_connect_to_the_networks() {
    let mut s = Session::default();
    // A drain main under the floor and a cold water pipe in the wall, along y = 0.
    let drain_100 = run(&mut s, "types_list", json!({"category": "pipe"}))
        .as_array()
        .unwrap()
        .iter()
        .map(id)
        .find(|t| {
            (run(&mut s, "type_get", json!({"type": t}))["params"]["diameter"].as_f64().unwrap() - 0.1).abs() < 1e-9
        })
        .unwrap();
    run(&mut s, "pipe_create", json!({"start": [0, 0], "end": [6, 0], "system": "sanitary", "type": drain_100}));
    run(&mut s, "pipe_create", json!({"start": [0, 0.1], "end": [6, 0.1], "system": "cold_water"}));
    let toilet = mep_type(&mut s, "Fixture");
    let wc = id(&run(&mut s, "component_place", json!({"position": [3, 1.5], "type": toilet})));
    let out = run(&mut s, "mep_connect", json!({"elements": [wc]}));
    // Drain: a drop from the floor and a branch falling to the main; supply: one branch.
    assert_eq!(out["created"].as_array().unwrap().len(), 3, "{out}");
    let fittings = run(&mut s, "mep_fittings", json!({}));
    let kinds: Vec<&str> =
        fittings["fittings"].as_array().unwrap().iter().map(|f| f["kind"].as_str().unwrap()).collect();
    // Tees on both mains and the elbow at the bottom of the drop.
    assert_eq!(kinds.iter().filter(|k| **k == "tee").count(), 2, "{fittings}");
    assert!(kinds.contains(&"elbow"), "{fittings}");
    // The toilet's branch is 100 mm: no reducer at the drain tee.
    assert!(!kinds.contains(&"reducer"), "{fittings}");

    // A device drops from its height to the conduit in the slab, on its circuit.
    let panel_t = mep_type(&mut s, "Panel");
    run(&mut s, "component_place", json!({"position": [0, 3], "type": panel_t}));
    let circuit = id(&run(&mut s, "system_create", json!({"kind": "power"})));
    run(&mut s, "conduit_create", json!({"start": [0, 3], "end": [6, 3], "circuits": [circuit]}));
    let outlet_t = mep_type(&mut s, "Outlet");
    let outlet = id(&run(&mut s, "component_place", json!({"position": [4, 4], "type": outlet_t, "system": circuit})));
    let out = run(&mut s, "mep_connect", json!({"elements": [outlet]}));
    let created = out["created"].as_array().unwrap();
    assert_eq!(created.len(), 2, "{out}");
    let drop = run(&mut s, "element_get", json!({"id": created[0]}));
    assert_eq!(
        (drop["params"]["start_offset"].clone(), drop["params"]["end_offset"].clone()),
        (json!(0.3), json!(3.0))
    );
    assert_eq!(drop["params"]["systems"], json!([circuit]));
    // Walls are not fixtures.
    let wall = id(&run(&mut s, "wall_create", json!({"start": [0, 8], "end": [4, 8]})));
    assert_eq!(fails(&mut s, "mep_connect", json!({"elements": [wall]})), ErrorCode::InvalidArguments);
}

#[test]
fn components_from_profiles_revolutions_and_voids() {
    let mut s = Session::default();
    // A countertop basin: a slab drawn from the front with a rounded front
    // edge (seen from the side), and a round bowl turned about the vertical
    // axis, cut out of it.
    let def = json!({
        "kind": "Plumbing",
        "params": [
            {"name": "W", "value": 0.6, "kind": "Length"},
            {"name": "T", "value": 0.2, "kind": "Length"},
            {"name": "R", "value": 0.15, "kind": "Length"}
        ],
        "items": [
            {"name": "slab", "material": "", "shape": {"Profile": {
                "plane": "Front",
                "points": [{"u": "-W / 2", "v": "0"}, {"u": "W / 2", "v": "0"}, {"u": "W / 2", "v": "T"}, {"u": "-W / 2", "v": "T"}],
                "offset": "-0.25", "depth": "0.5"}}},
            {"name": "bowl", "void": true, "shape": {"Revolution": {
                "plane": "Front",
                "points": [{"u": "0", "v": "0.05"}, {"u": "R", "v": "0.05"}, {"u": "R", "v": "T + 0.1"}, {"u": "0", "v": "T + 0.1"}],
                "offset": "0", "axis": "0", "angle": "360"}}}
        ]
    });
    let ty = run(&mut s, "component_create", json!({"name": "Bancada com cuba", "def": def}));
    let shape = &ty["params"]["def"]["items"][1];
    assert_eq!(shape["void"], json!(true), "{ty}");
    assert_eq!(shape["shape"]["Revolution"]["plane"], json!("Front"));
    let basin = id(&run(&mut s, "component_place", json!({"position": [2, 2], "type": id(&ty)})));
    // 0.6 x 0.2 x 0.5 less a 48-sided bowl of radius 0.15, 0.15 deep.
    let volume = run(&mut s, "element_get", json!({"id": basin}))["derived"]["volume"].as_f64().unwrap();
    let bowl = 0.5 * 48.0 * 0.15f64.powi(2) * (std::f64::consts::TAU / 48.0).sin() * 0.15;
    assert!((volume - (0.06 - bowl)).abs() < 1e-6, "{volume}");

    // An arc on the side plane: a quarter round nosing of radius T; a
    // profile crossing its revolution axis is refused.
    let nosing = json!({"params": [{"name": "T", "value": 0.2, "kind": "Length"}], "items": [{"name": "n", "shape": {"Profile": {
        "plane": "Side",
        "points": [{"u": "0", "v": "0"}, {"u": "T", "v": "0", "radius": "T"}, {"u": "0", "v": "T"}],
        "offset": "0", "depth": "1"}}}]});
    let nosing = id(&run(&mut s, "component_create", json!({"name": "Boleado", "def": nosing})));
    let placed = id(&run(&mut s, "component_place", json!({"position": [5, 5], "type": nosing})));
    let volume = run(&mut s, "element_get", json!({"id": placed}))["derived"]["volume"].as_f64().unwrap();
    let quarter = std::f64::consts::PI * 0.04 / 4.0;
    assert!(volume < quarter && volume > quarter * 0.99, "{volume}");
    let crossing = json!({"items": [{"name": "x", "shape": {"Revolution": {
        "plane": "Front", "points": [{"u": "-1", "v": "0"}, {"u": "1", "v": "0"}, {"u": "1", "v": "1"}],
        "offset": "0", "axis": "0", "angle": "360"}}}]});
    assert_eq!(
        fails(&mut s, "component_create", json!({"name": "Errado", "def": crossing})),
        ErrorCode::InvalidArguments
    );
}

#[test]
fn runs_connect_through_risers_between_floors() {
    let mut s = Session::default();
    let first = run(&mut s, "levels_list", json!({}))[1]["id"].as_u64().unwrap();
    let elevation = |s: &mut Session, out: &Value| {
        let p = &run(s, "element_get", json!({"id": id(out)}))["params"];
        (p["start_offset"].as_f64().unwrap(), p["end_offset"].as_f64().unwrap())
    };
    // A cold water run on the ground floor; a riser from its end takes its elevation
    // and goes up to 0.6 m above the first floor (3 m).
    run(&mut s, "pipe_create", json!({"start": [0, 0], "end": [4, 0], "system": "cold_water", "offset": 0.4}));
    let riser = run(&mut s, "pipe_create", json!({"start": [4, 0], "system": "cold_water", "end_offset": 3.6}));
    assert_eq!(elevation(&mut s, &riser), (0.4, 3.6));
    // Upstairs, a run from the riser starts at its top, seen from the first floor.
    let upper =
        run(&mut s, "pipe_create", json!({"start": [4, 0], "end": [4, 3], "system": "cold_water", "level": first}));
    assert!((elevation(&mut s, &upper).0 - 0.6).abs() < 1e-9, "{upper}");
    let fittings = run(&mut s, "mep_fittings", json!({"level": first}));
    assert_eq!(fittings["fittings"][0]["kind"], json!("elbow"), "{fittings}");
    assert_eq!(run(&mut s, "mep_fittings", json!({}))["total"], json!(2));

    // A drain stack from under the ground floor to the first floor; a fixture
    // upstairs connects to it, and a conduit drop leaves a run at its end.
    run(&mut s, "pipe_create", json!({"start": [6, 0], "system": "sanitary", "offset": -0.3, "end_offset": 3.5}));
    let fixture_t = mep_type(&mut s, "Fixture");
    let wc = id(&run(&mut s, "component_place", json!({"position": [6, 1.5], "type": fixture_t, "level": first})));
    let out = run(&mut s, "mep_connect", json!({"elements": [wc], "system": "sanitary"}));
    assert_eq!(out["skipped"], json!(0), "{out}");
    let branch = out["created"].as_array().unwrap().last().unwrap().clone();
    let branch = run(&mut s, "element_get", json!({"id": branch}));
    assert_eq!(branch["params"]["end"], json!([6.0, 0.0]), "{branch}");
    let fittings = run(&mut s, "mep_fittings", json!({"level": first}));
    assert!(fittings["fittings"].as_array().unwrap().iter().any(|f| f["kind"] == json!("tee")), "{fittings}");

    run(&mut s, "conduit_create", json!({"start": [0, 5], "end": [3, 5], "offset": 2.9}));
    let drop = run(&mut s, "conduit_create", json!({"start": [3, 5], "end_offset": 0.3}));
    assert_eq!(elevation(&mut s, &drop), (2.9, 0.3));
}

#[test]
fn sizing_drains_water_pipes_and_circuits() {
    let mut s = Session::default();
    let pipe_types: Vec<(u64, f64)> = run(&mut s, "types_list", json!({"category": "pipe"}))
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (id(t), run(&mut s, "type_get", json!({"type": id(t)}))["params"]["diameter"].as_f64().unwrap()))
        .collect();
    let of = |d: f64| pipe_types.iter().find(|t| (t.1 - d).abs() < 1e-9).unwrap().0;
    // A 40 mm drain and a 20 mm cold water main, both too small for what they will carry.
    let drain = id(&run(
        &mut s,
        "pipe_create",
        json!({"start": [0, 0], "end": [8, 0], "system": "sanitary", "type": of(0.04)}),
    ));
    let water = id(&run(
        &mut s,
        "pipe_create",
        json!({"start": [0, 0.1], "end": [8, 0.1], "system": "cold_water", "type": of(0.02)}),
    ));
    let toilet = mep_type(&mut s, "Fixture");
    let mut fixtures = Vec::new();
    for x in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0] {
        fixtures.push(id(&run(&mut s, "component_place", json!({"position": [x, 1.5], "type": toilet}))));
    }
    run(&mut s, "mep_connect", json!({"elements": fixtures}));
    let sizing = run(&mut s, "mep_sizing", json!({}));
    let pipe = |id: u64| sizing["pipes"].as_array().unwrap().iter().find(|p| p["id"] == json!(id)).unwrap().clone();
    // Seven toilets: 42 fixture units at the outlet, 100 mm.
    let main = pipe(drain);
    assert_eq!(main["load"], json!(42.0), "{main}");
    assert_eq!((main["required_mm"].clone(), main["ok"].clone()), (json!(100.0), json!(false)));
    // Their weights add up to 2.1: 0.43 l/s, still within 20 mm at the source.
    let supply = pipe(water);
    assert!((supply["load"].as_f64().unwrap() - 2.1).abs() < 1e-9, "{supply}");
    assert!((supply["flow_l_s"].as_f64().unwrap() - 0.3 * 2.1f64.sqrt()).abs() < 1e-9);
    assert_eq!(supply["required_mm"], json!(20.0));
    // Applying widens the drain main; the water main stays.
    let applied = run(&mut s, "mep_size_apply", json!({}));
    assert!(applied["changed"].as_array().unwrap().contains(&json!(drain)), "{applied}");
    assert_eq!(run(&mut s, "element_get", json!({"id": drain}))["type"]["id"], json!(of(0.1)));
    assert_eq!(run(&mut s, "element_get", json!({"id": water}))["type"]["id"], json!(of(0.02)));

    // A 3000 VA circuit 51 m from its board (no conduits: measured straight).
    let panel_t = mep_type(&mut s, "Panel");
    run(&mut s, "component_place", json!({"position": [0, 0], "type": panel_t, "base_offset": 1.3}));
    let circuit = id(&run(&mut s, "system_create", json!({"kind": "power"})));
    let outlet = mep_type(&mut s, "DedicatedOutlet");
    for _ in 0..2 {
        run(&mut s, "component_place", json!({"position": [40, 10], "type": outlet, "system": circuit}));
    }
    let sizing = run(&mut s, "mep_sizing", json!({}));
    let c = sizing["circuits"].as_array().unwrap().iter().find(|c| c["id"] == json!(circuit)).unwrap().clone();
    // 23.6 A: a 25 A breaker; 4 mm² would carry it but drops 8.5 %, 10 mm² drops 3.4 %.
    assert_eq!((c["breaker_a"].clone(), c["conductor_mm2"].clone()), (json!(25.0), json!(10.0)), "{c}");
    assert!((c["length_m"].as_f64().unwrap() - 51.0).abs() < 1e-9);
    assert!(c["voltage_drop_pct"].as_f64().unwrap() < 4.0 && c["ok"] == json!(true));
}

#[test]
fn panel_diagrams_follow_their_board() {
    let mut s = Session::default();
    assert_eq!(fails(&mut s, "view_create", json!({"kind": "panel_diagram"})), ErrorCode::InvalidArguments);
    let panel_t = mep_type(&mut s, "Panel");
    let panel = id(&run(&mut s, "component_place", json!({"position": [0, 0], "type": panel_t})));
    run(&mut s, "system_create", json!({"kind": "lighting"}));
    let diagram = run(&mut s, "view_create", json!({"kind": "panel_diagram"}));
    assert_eq!(diagram["params"]["panel"], json!(panel), "{diagram}");
    // It goes on a sheet like any drawing.
    let sheet = id(&run(&mut s, "view_create", json!({"kind": "sheet"})));
    run(&mut s, "sheet_place_view", json!({"sheet": sheet, "view": id(&diagram)}));
    // Deleting the board deletes its diagram (and the viewport).
    run(&mut s, "elements_delete", json!({"ids": [panel]}));
    assert_eq!(fails(&mut s, "view_get", json!({"view": id(&diagram)})), ErrorCode::NotFound);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "view_get", json!({"view": id(&diagram)}))["params"]["panel"], json!(panel));
}

/// Arc walls: three points or a bulge; quantities along the arc; doors on
/// the arc; a room and floor from walls that include an arc; edits.
#[test]
fn arc_walls_through_commands() {
    use std::f64::consts::PI;
    let mut s = Session::default();
    // A half circle through (0, 4) closed by a straight wall: a "D".
    let arc = run(&mut s, "wall_create", json!({"start": [4.0, 0.0], "end": [-4.0, 0.0], "mid": [0.0, 4.0]}));
    let thickness = arc["derived"]["thickness"].as_f64().unwrap();
    assert!((arc["params"]["bulge"].as_f64().unwrap() - 1.0).abs() < 1e-9, "{arc}");
    assert!((arc["derived"]["radius"].as_f64().unwrap() - 4.0).abs() < 1e-9);
    assert!((arc["derived"]["sweep"].as_f64().unwrap() - 180.0).abs() < 1e-9);
    assert!((arc["derived"]["length"].as_f64().unwrap() - 4.0 * PI).abs() < 1e-9);
    let height = arc["derived"]["height"].as_f64().unwrap();
    let volume = arc["derived"]["volume"].as_f64().unwrap();
    assert!((volume - 4.0 * PI * thickness * height).abs() < 1e-9, "a free arc: length x thickness x height");
    run(&mut s, "wall_create", json!({"start": [-4.0, 0.0], "end": [4.0, 0.0]}));

    // A door by a point near the top of the arc lands there, centered on the arc.
    let door = run(&mut s, "door_create", json!({"point": [0.3, 4.5]}));
    let offset = door["params"]["offset"].as_f64().unwrap();
    let expected = (PI / 2.0 - (0.3f64 / 4.5).atan()) * 4.0;
    assert!((offset - expected).abs() < 1e-6, "{offset} vs {expected}");
    assert_eq!(door["params"]["host"], arc["id"]);
    assert_eq!(door["params"]["flip_facing"], true, "outside the arc is its right side");

    // The room inside follows the inner face of the arc.
    let room = run(&mut s, "room_create", json!({"point": [0.0, 1.0]}));
    let (r, d) = (4.0 - thickness / 2.0, thickness / 2.0);
    let segment = r * r * (d / r).acos() - d * (r * r - d * d).sqrt();
    let area = room["derived"]["area"].as_f64().unwrap();
    assert!((area - segment).abs() < 1e-3 * segment, "{area} vs {segment}");
    // A floor from the walls follows the arc too (its exterior faces).
    let floor = run(&mut s, "floor_create", json!({"from_walls": true}));
    let outer = PI * (4.0 + d) * (4.0 + d) / 2.0;
    let floor_el = run(&mut s, "element_get", json!({"id": id(&floor)}));
    let boundary = floor_el["params"]["boundary"].as_array().unwrap();
    assert!(boundary.len() > 30, "the arc is followed by many points");
    let pts: Vec<(f64, f64)> = boundary.iter().map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap())).collect();
    let n = pts.len();
    let shoelace: f64 =
        (0..n).map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1).sum::<f64>() / 2.0;
    let expected_floor = outer + 2.0 * (4.0 + d) * d;
    assert!((shoelace.abs() - expected_floor).abs() < 5e-3 * expected_floor, "{} vs {expected_floor}", shoelace.abs());

    // Bulge directly, and a clockwise arc; set back to straight with element_set.
    let cw = run(&mut s, "wall_create", json!({"start": [10.0, 0.0], "end": [14.0, 0.0], "bulge": -1.0}));
    assert!((cw["derived"]["sweep"].as_f64().unwrap() + 180.0).abs() < 1e-9);
    let straight = run(&mut s, "element_set", json!({"id": id(&cw), "params": {"bulge": 0.0}}));
    assert!((straight["derived"]["length"].as_f64().unwrap() - 4.0).abs() < 1e-9, "{straight}");
    assert!(straight["derived"]["radius"].is_null());

    // A radius: the shorter arc (or the longer), its sign the turn.
    let r = run(&mut s, "wall_create", json!({"start": [20.0, 0.0], "end": [24.0, 0.0], "radius": -2.0}));
    assert!((r["derived"]["sweep"].as_f64().unwrap() + 180.0).abs() < 1e-9, "{r}");
    let r = run(&mut s, "wall_create", json!({"start": [20.0, 5.0], "end": [24.0, 5.0], "radius": 4.0}));
    assert!((r["derived"]["sweep"].as_f64().unwrap() - 60.0).abs() < 1e-9, "{r}");
    assert!((r["derived"]["radius"].as_f64().unwrap() - 4.0).abs() < 1e-9, "{r}");
    let major = json!({"start": [20.0, 10.0], "end": [24.0, 10.0], "radius": 4.0, "major_arc": true});
    let r = run(&mut s, "wall_create", major);
    assert!((r["derived"]["sweep"].as_f64().unwrap() - 300.0).abs() < 1e-9, "{r}");
    let short = json!({"start": [20.0, 15.0], "end": [24.0, 15.0], "radius": 1.5});
    assert_eq!(fails(&mut s, "wall_create", short), ErrorCode::InvalidArguments);

    // Bad arcs are refused: collinear points, a bulge out of range, an arc too tight for the wall.
    let bad = json!({"start": [0.0, 10.0], "end": [4.0, 10.0], "mid": [2.0, 10.0]});
    assert_eq!(fails(&mut s, "wall_create", bad), ErrorCode::InvalidArguments);
    let bad = json!({"start": [0.0, 10.0], "end": [4.0, 10.0], "bulge": 50.0});
    assert_eq!(fails(&mut s, "wall_create", bad), ErrorCode::InvalidArguments);
    let tight = json!({"start": [0.0, 10.0], "end": [0.1, 10.0], "bulge": 1.0});
    assert_eq!(fails(&mut s, "wall_create", tight), ErrorCode::Rejected);

    // Split on the arc: two quarter circles.
    let split = run(&mut s, "element_split", json!({"id": id(&arc), "point": [0.0, 3.0]}));
    let parts = [split["original"].as_u64().unwrap(), split["created"].as_u64().unwrap()];
    for part in parts {
        let w = run(&mut s, "element_get", json!({"id": part}));
        assert!((w["derived"]["sweep"].as_f64().unwrap() - 90.0).abs() < 1e-9, "{w}");
    }

    // Away from the "D": an arc wall trims to a line along its circle, a line
    // extends to an arc's circle, and an arc aligns onto a concentric one.
    let quarter =
        run(&mut s, "wall_create", json!({"start": [24.0, 0.0], "end": [20.0, 4.0], "bulge": (PI / 8.0).tan()}));
    let post = run(&mut s, "wall_create", json!({"start": [22.0, -3.0], "end": [22.0, -1.0]}));
    let point = |w: &Value, end: &str| (w["params"][end][0].as_f64().unwrap(), w["params"][end][1].as_f64().unwrap());
    let near = |(x, y): (f64, f64), (u, v): (f64, f64)| (x - u).abs() < 1e-9 && (y - v).abs() < 1e-9;
    let r12 = 12f64.sqrt();
    let trim = json!({"id": id(&quarter), "pick": [20.0, 4.0], "reference": id(&post)});
    let trimmed = run(&mut s, "trim_extend", trim);
    assert!(near(point(&trimmed, "start"), (22.0, r12)) && near(point(&trimmed, "end"), (20.0, 4.0)), "{trimmed}");
    assert!((trimmed["derived"]["radius"].as_f64().unwrap() - 4.0).abs() < 1e-9);
    let extend = json!({"id": id(&post), "pick": [22.0, -2.0], "reference": id(&quarter)});
    let extended = run(&mut s, "trim_extend", extend);
    assert!(near(point(&extended, "end"), (22.0, r12)), "{extended}");
    let outer = run(&mut s, "wall_create", json!({"start": [26.0, 0.0], "end": [14.0, 0.0], "mid": [20.0, 6.0]}));
    let aligned = run(&mut s, "element_align", json!({"id": id(&quarter), "reference": id(&outer)}));
    assert!((aligned["derived"]["radius"].as_f64().unwrap() - 6.0).abs() < 1e-9, "{aligned}");
    let far = run(&mut s, "wall_create", json!({"start": [30.0, -1.0], "end": [30.0, 1.0]}));
    let corner = json!({"a": id(&far), "pick_a": [30.0, 0.0], "b": id(&outer), "pick_b": [20.0, 6.0]});
    assert_eq!(fails(&mut s, "trim_corner", corner), ErrorCode::Rejected, "x = 30 misses the circle of radius 6");
    // Rotating and mirroring keep the arc and its door.
    run(&mut s, "elements_rotate", json!({"ids": parts, "center": [0.0, 0.0], "angle": 90.0}));
    run(&mut s, "elements_mirror", json!({"ids": parts, "axis": [[0.0, 0.0], [0.0, 1.0]]}));
    for part in parts {
        let w = run(&mut s, "element_get", json!({"id": part}));
        assert!((w["derived"]["radius"].as_f64().unwrap() - 4.0).abs() < 1e-9, "{w}");
    }
    let door = run(&mut s, "element_get", json!({"id": id(&door)}));
    let host = run(&mut s, "element_get", json!({"id": door["params"]["host"]}));
    assert!(host["derived"]["radius"].is_number());
}

/// Pitched roofs over concave outlines and around courtyards: analytic
/// heights, a valley, exact volumes; bad openings are refused.
#[test]
fn roofs_over_any_footprint() {
    let mut s = Session::default();
    let thickness = |s: &mut Session, roof: &Value| {
        let t = run(s, "type_get", json!({"type": roof["type"]["id"]}));
        t["params"]["layers"].as_array().unwrap().iter().map(|l| l["thickness"].as_f64().unwrap()).sum::<f64>()
    };
    // An L: arms 4 m wide, a hip roof at 50 %.
    let l = json!([[0, 0], [10, 0], [10, 4], [4, 4], [4, 10], [0, 10]]);
    let roof = run(&mut s, "roof_create", json!({"boundary": l, "offset": 3.0, "overhang": 0.0}));
    let roof = run(&mut s, "element_set", json!({"id": id(&roof), "params": {"shape": {"Hip": {"slope": 0.5}}}}));
    let ridges = roof["derived"]["ridges"].as_array().unwrap();
    let valley = ridges.iter().any(|r| {
        let (a, b) = (
            (r[0][0].as_f64().unwrap(), r[0][1].as_f64().unwrap()),
            (r[1][0].as_f64().unwrap(), r[1][1].as_f64().unwrap()),
        );
        let at = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9;
        (at(a, (4.0, 4.0)) && at(b, (2.0, 2.0))) || (at(b, (4.0, 4.0)) && at(a, (2.0, 2.0)))
    });
    assert!(valley, "a valley from the inner corner: {ridges:?}");
    let t = thickness(&mut s, &roof);
    let volume = roof["derived"]["volume"].as_f64().unwrap();
    let expected = 64.0 * t * (1.0f64 + 0.25).sqrt();
    assert!((volume - expected).abs() < 1e-6, "{volume} vs {expected}");

    // A courtyard: a 12 m square around a 4 m one, with no overhang.
    let outer = json!([[20, 0], [32, 0], [32, 12], [20, 12]]);
    let hole = json!([[24, 4], [28, 4], [28, 8], [24, 8]]);
    let roof = run(&mut s, "roof_create", json!({"boundary": outer, "holes": [hole], "offset": 3.0, "overhang": 0.0}));
    assert!((roof["derived"]["plan_area"].as_f64().unwrap() - 128.0).abs() < 1e-9);
    let flat_volume = roof["derived"]["volume"].as_f64().unwrap();
    assert!((flat_volume - 128.0 * t).abs() < 1e-6, "{flat_volume}");
    let roof = run(&mut s, "element_set", json!({"id": id(&roof), "params": {"shape": {"Hip": {"slope": 1.0}}}}));
    let volume = roof["derived"]["volume"].as_f64().unwrap();
    assert!((volume - 128.0 * t * 2f64.sqrt()).abs() < 1e-6, "{volume}");
    // The ridge runs 2 m above the eaves, midway between them: around the courtyard.
    let ridges = roof["derived"]["ridges"].as_array().unwrap();
    assert!(ridges.len() >= 8, "{ridges:?}");

    // Openings must lie inside, apart.
    let outer = json!([[0, 20], [10, 20], [10, 30], [0, 30]]);
    let outside = json!({"boundary": outer, "holes": [[[8, 22], [12, 22], [12, 24], [8, 24]]]});
    assert_eq!(fails(&mut s, "roof_create", outside), ErrorCode::InvalidArguments);
    let overlapping = json!({"boundary": outer, "holes": [[[2, 22], [5, 22], [5, 25], [2, 25]], [[4, 24], [7, 24], [7, 27], [4, 27]]]});
    assert_eq!(fails(&mut s, "roof_create", overlapping), ErrorCode::InvalidArguments);
}

/// Floors with openings (shafts, stairwells): created with them, added and
/// removed one at a time; the area and volume leave them out.
#[test]
fn floor_openings() {
    let mut s = Session::default();
    let shaft = json!([[1, 1], [3, 1], [3, 3], [1, 3]]);
    let floor =
        run(&mut s, "floor_create", json!({"boundary": [[0, 0], [10, 0], [10, 6], [0, 6]], "openings": [shaft]}));
    let floor_id = id(&floor);
    let volume = |v: &Value| v["derived"]["volume"].as_f64().unwrap();
    let full = volume(&run(&mut s, "floor_create", json!({"boundary": [[20, 0], [30, 0], [30, 6], [20, 6]]})));
    assert!((volume(&floor) - full * 56.0 / 60.0).abs() < 1e-9, "{} vs {full}", volume(&floor));
    // A stairwell beside it; one overlapping or outside is refused.
    let stair = json!({"id": floor_id, "outline": [[5, 1], [9, 1], [9, 3], [5, 3]]});
    let floor = run(&mut s, "floor_opening_add", stair);
    assert_eq!(floor["params"]["openings"].as_array().unwrap().len(), 2);
    let overlapping = json!({"id": floor_id, "outline": [[2, 2], [6, 2], [6, 4], [2, 4]]});
    assert_eq!(fails(&mut s, "floor_opening_add", overlapping), ErrorCode::InvalidArguments);
    let outside = json!({"id": floor_id, "outline": [[9, 4], [12, 4], [12, 5], [9, 5]]});
    assert_eq!(fails(&mut s, "floor_opening_add", outside), ErrorCode::InvalidArguments);
    let floor = run(&mut s, "floor_opening_remove", json!({"id": floor_id, "index": 0}));
    assert_eq!(floor["params"]["openings"], json!([[[5.0, 1.0], [9.0, 1.0], [9.0, 3.0], [5.0, 3.0]]]));
    assert_eq!(fails(&mut s, "floor_opening_remove", json!({"id": floor_id, "index": 1})), ErrorCode::InvalidArguments);
    // An opening across the edge through element_set is refused too.
    let across = json!({"id": floor_id, "params": {"openings": [[[-1, 1], [1, 1], [1, 2], [-1, 2]]]}});
    assert_eq!(fails(&mut s, "element_set", across), ErrorCode::InvalidArguments);
}

/// A slope per edge: a hip roof with a gable end, a steeper eave; one
/// opening removed at a time.
#[test]
fn roof_edge_slopes_and_openings() {
    let mut s = Session::default();
    let rect = json!([[0, 0], [10, 0], [10, 6], [0, 6]]);
    let holes = json!([[[1, 1], [3, 1], [3, 3], [1, 3]], [[6, 2], [8, 2], [8, 4], [6, 4]]]);
    let roof = run(&mut s, "roof_create", json!({"boundary": rect, "holes": holes, "offset": 3.0, "overhang": 0.0}));
    let roof_id = id(&roof);
    // Flat roofs have no edge slopes.
    let edge = json!({"id": roof_id, "edge": 1, "slope": 0.0});
    assert_eq!(fails(&mut s, "roof_edge_set", edge.clone()), ErrorCode::InvalidArguments);

    // One opening goes, the other stays.
    let roof = run(&mut s, "roof_opening_remove", json!({"id": roof_id, "index": 0}));
    let left = roof["params"]["holes"].as_array().unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0][0], json!([6.0, 2.0]));
    assert_eq!(fails(&mut s, "roof_opening_remove", json!({"id": roof_id, "index": 1})), ErrorCode::InvalidArguments);
    run(&mut s, "roof_opening_remove", json!({"id": roof_id, "index": 0}));

    // A hip roof at 50 %, then a gable at its east end: the peak stays at 1.5 m.
    run(&mut s, "element_set", json!({"id": roof_id, "params": {"shape": {"Hip": {"slope": 0.5}}}}));
    let roof = run(&mut s, "roof_edge_set", edge);
    assert_eq!(roof["params"]["edge_slopes"], json!([null, 0.0]));
    let top = |s: &mut Session| {
        let b = run(s, "element_get", json!({"id": roof_id}));
        b["derived"]["ridges"].as_array().unwrap().iter().flat_map(|r| r.as_array().unwrap().clone()).count()
    };
    assert!(top(&mut s) >= 3, "a ridge and two hips");
    // A steeper south eave on a gable roof along X: the planes meet 2 m from it, 2 m up.
    run(&mut s, "roof_edge_set", json!({"id": roof_id, "edge": 1, "slope": null}));
    run(
        &mut s,
        "element_set",
        json!({"id": roof_id, "params": {"shape": {"Gable": {"slope": 0.5, "ridge_deg": 0.0}}}}),
    );
    let roof = run(&mut s, "roof_edge_set", json!({"id": roof_id, "edge": 0, "slope": 1.0}));
    assert_eq!(roof["params"]["edge_slopes"], json!([1.0]));
    let ridges = roof["derived"]["ridges"].as_array().unwrap();
    assert!(
        ridges
            .iter()
            .any(|r| (r[0][1].as_f64().unwrap() - 2.0).abs() < 1e-9 && (r[1][1].as_f64().unwrap() - 2.0).abs() < 1e-9),
        "{ridges:?}"
    );
    assert_eq!(
        fails(&mut s, "roof_edge_set", json!({"id": roof_id, "edge": 4, "slope": 1.0})),
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        fails(&mut s, "roof_edge_set", json!({"id": roof_id, "edge": 0, "slope": -1.0})),
        ErrorCode::InvalidArguments
    );
}

/// Doors and windows are components: the template's types are, a new door
/// component (a double leaf drawing its own plan symbol) cuts its opening in
/// the wall, mirrors with the door's flips, and saves to and loads from a
/// component file as a door type.
#[test]
fn doors_and_windows_are_components() {
    let mut s = Session::default();
    let doors = run(&mut s, "types_list", json!({"category": "door"}));
    let template_door = &doors.as_array().unwrap()[0];
    let t = run(&mut s, "type_get", json!({"type": template_door["id"]}));
    assert_eq!(t["params"]["def"]["kind"], "Door", "{t}");

    let def = json!({
        "kind": "Door",
        "params": [
            {"name": "L", "value": 1.6, "kind": "Length"},
            {"name": "A", "value": 2.1, "kind": "Length"}
        ],
        "items": [
            {"name": "Folha 1", "material": "", "in_plan": true,
             "shape": {"Box": {"x": "-L / 2", "y": "0", "z": "0", "width": "L / 2", "depth": "0.04", "height": "A"}}},
            {"name": "Folha 2", "material": "", "in_plan": true,
             "shape": {"Box": {"x": "0", "y": "0", "z": "0", "width": "L / 2", "depth": "0.04", "height": "A"}}},
            {"name": "Batente", "material": "", "in_plan": false,
             "shape": {"Box": {"x": "-L / 2", "y": "-WallThickness / 2", "z": "A", "width": "L", "depth": "WallThickness", "height": "0.05"}}},
            {"name": "Giro", "plan_only": true,
             "shape": {"Profile": {"plane": "Plan", "offset": "0", "depth": "0.001", "points": [
                 {"u": "-L / 2", "v": "0", "radius": ""},
                 {"u": "-L / 2", "v": "L / 2", "radius": "L / 2"},
                 {"u": "0", "v": "0", "radius": ""}]}}}
        ],
        "opening": {"width": "L", "height": "A", "own_symbol": true}
    });
    let double = run(&mut s, "component_create", json!({"name": "Porta dupla", "def": def}));
    assert_eq!(double["category"], "door", "{double}");
    // Without an opening it would not be a door.
    let mut bad = def.clone();
    bad["opening"]["width"] = json!("0");
    assert_eq!(fails(&mut s, "component_create", json!({"name": "X", "def": bad})), ErrorCode::InvalidArguments);

    let wall = run(&mut s, "wall_create", json!({"start": [0, 0], "end": [6, 0]}));
    let thickness = wall["derived"]["thickness"].as_f64().unwrap();
    let full = wall["derived"]["volume"].as_f64().unwrap();
    let door = run(&mut s, "door_create", json!({"wall": id(&wall), "offset": 3.0, "type": id(&double)}));
    let wall = run(&mut s, "element_get", json!({"id": id(&wall)}));
    let cut = full - wall["derived"]["volume"].as_f64().unwrap();
    assert!((cut - 1.6 * 2.1 * thickness).abs() < 1e-6, "the opening is cut: {cut}");

    // The leaves stand on the side the door opens to; flipping moves them across.
    let side = |s: &mut Session| {
        let fills = &s.geometry().opening_fills[&cc_core::ElementId(id(&door))];
        assert_eq!(fills.len(), 4);
        let leaves = fills.iter().filter(|p| p.in_plan && !p.plan_only);
        let ys: Vec<f64> = leaves.flat_map(|p| p.footprint.iter().map(|q| q.y)).collect();
        ys.iter().sum::<f64>() / ys.len() as f64
    };
    assert!(side(&mut s) > 0.0);
    run(&mut s, "element_flip", json!({"id": id(&door), "facing": true}));
    assert!(side(&mut s) < 0.0);

    // Plans draw the two leaves (its own symbol): two rectangles, no swing arc.
    let plan = cc_core::ViewId(run(&mut s, "views_list", json!({}))[0]["id"].as_u64().unwrap());
    let drawing = s.drawing(plan).unwrap();
    let lines = drawing
        .prims
        .iter()
        .filter(|p| {
            matches!(p, cc_views::Prim::Line { element: Some(e), style: cc_views::LineStyle::Projection, .. }
                if e.0 == id(&door))
        })
        .count();
    assert!(lines > 8, "the leaves' outlines and the swing symbol (the tag draws annotation lines): {lines}");
    // The swing is a plan symbol only: three solids (two leaves and the head).
    assert_eq!(s.geometry().solids_of(cc_core::ElementId(id(&door))).count(), 3);

    // A component file carries the door type.
    let path = scratch("porta-dupla.ccomp");
    run(&mut s, "component_save", json!({"component": "Porta dupla", "path": path}));
    let mut other = Session::default();
    run(&mut other, "component_load", json!({"path": path}));
    let doors = run(&mut other, "types_list", json!({"category": "door"}));
    assert!(doors.as_array().unwrap().iter().any(|t| t["name"] == "Porta dupla"), "{doors}");
}
