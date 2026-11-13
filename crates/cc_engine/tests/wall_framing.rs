//! Light steel and wood framing of walls: `wall_framing_create` makes one
//! host per wall, which keeps its studs, tracks, headers and sills in step
//! with its parameters, its wall and the wall's doors and windows.

use cc_core::wall_framing::{FramingRoleKind as K, WallFraming};
use cc_core::{ElementId, ElementKind};
use cc_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn id(v: &Value) -> u64 {
    v["id"].as_u64().unwrap()
}

fn types_named(s: &Session, name: &str) -> usize {
    s.project().types.values().filter(|t| t.name == name).count()
}

fn wall(s: &mut Session, start: [f64; 2], end: [f64; 2]) -> u64 {
    id(&run(s, "wall_create", json!({"start": start, "end": end, "height": 3.0})))
}

fn framing(s: &Session, host: u64) -> WallFraming {
    match s.project().element(ElementId(host)).map(|e| &e.kind) {
        Some(ElementKind::WallFraming(f)) => (**f).clone(),
        _ => panic!("not a wall framing"),
    }
}

fn count(f: &WallFraming, kind: K) -> usize {
    f.members.iter().filter(|m| m.role.kind == kind).count()
}

fn framings(s: &Session) -> usize {
    s.project().elements.values().filter(|e| matches!(e.kind, ElementKind::WallFraming(_))).count()
}

fn first_framing(out: &Value) -> u64 {
    id(&out["framings"][0])
}

#[test]
fn light_steel_defaults_make_the_catalog_types_once() {
    let mut s = Session::default();
    let a = wall(&mut s, [0.0, 0.0], [3.6, 0.0]);
    let b = wall(&mut s, [0.0, 5.0], [3.6, 5.0]);
    let depth = s.doc.undo_depth();
    let out = run(&mut s, "wall_framing_create", json!({"walls": [a, b]}));
    assert_eq!(s.doc.undo_depth(), depth + 1, "one undo step");
    let list = out["framings"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["wall"], json!(a));
    assert_eq!(list[0]["members"].as_array().unwrap().len(), 12, "10 studs and two tracks");
    let f = framing(&s, first_framing(&out));
    assert_eq!((count(&f, K::Stud), count(&f, K::TopTrack)), (10, 1));
    assert!((f.spacing - 0.4).abs() < 1e-9 && !f.double_top && !f.blocking);
    assert_eq!(types_named(&s, "Ue 90x40x12x0,95"), 1, "the studs' column type");
    assert_eq!(types_named(&s, "U 92x38x0,95"), 1, "the tracks' beam type");
    assert_eq!(types_named(&s, "Ue 140x40x12x0,95"), 1, "the headers' beam type");
    run(&mut s, "undo", json!({}));
    assert_eq!(framings(&s), 0);
    assert_eq!(types_named(&s, "Ue 90x40x12x0,95"), 0, "undone with the framings");
}

#[test]
fn wood_defaults_double_the_top_plate() {
    let mut s = Session::default();
    let a = wall(&mut s, [0.0, 0.0], [3.6, 0.0]);
    let out = run(&mut s, "wall_framing_create", json!({"walls": [a], "system": "wood", "blocking": true}));
    let f = framing(&s, first_framing(&out));
    assert!((f.spacing - 0.6).abs() < 1e-9 && f.double_top);
    assert_eq!((count(&f, K::TopTrack), count(&f, K::Stud), count(&f, K::Blocking)), (2, 7, 6));
    assert_eq!(types_named(&s, "Peça 6x8"), 2, "the studs' column type and the plates' beam type");
    assert_eq!(types_named(&s, "Peça 6x16"), 1, "the headers' beam type");
}

#[test]
fn studs_turn_their_depth_across_the_wall() {
    let mut s = Session::default();
    let a = wall(&mut s, [0.0, 0.0], [0.0, 3.6]);
    let host = first_framing(&run(&mut s, "wall_framing_create", json!({"walls": [a]})));
    let f = framing(&s, host);
    let stud = f.members.iter().find(|m| m.role.kind == K::Stud).unwrap().id;
    let e = s.project().element(stud).unwrap();
    let ElementKind::Column(c) = &e.kind else { panic!("a column") };
    assert!((c.rotation_deg - 90.0).abs() < 1e-9, "turned with the wall");
    // The section's width (local x) runs along the wall, its depth (local y) across it.
    let (width, depth) = cc_core::steel_building::section_box(s.project(), e.type_id);
    assert!(width < depth, "{width} x {depth}");
    assert!(c.position.x.abs() < 1e-9 && (c.position.y - width / 2.0).abs() < 1e-9, "flush with the wall start");
}

#[test]
fn refused_before_anything_is_made() {
    let mut s = Session::default();
    let a = wall(&mut s, [0.0, 0.0], [3.6, 0.0]);
    let curved = id(&run(&mut s, "wall_create", json!({"start": [0, 5], "end": [4, 5], "bulge": 0.3, "height": 3.0})));
    let depth = s.doc.undo_depth();
    for args in [
        json!({"walls": []}),
        json!({"walls": [curved]}),
        json!({"walls": [a, a]}),
        json!({"walls": [a], "spacing": 0.1}),
        json!({"walls": [a], "spacing": 1.3}),
        json!({"walls": [a], "system": "steel"}),
        json!({"walls": [a], "stud_type": "no such type"}),
        json!({"walls": [999_999]}),
    ] {
        assert!(s.execute("wall_framing_create", args.clone()).is_err(), "{args}");
    }
    assert_eq!(s.doc.undo_depth(), depth);
    assert_eq!(framings(&s), 0);
    assert_eq!(types_named(&s, "Ue 90x40x12x0,95"), 0);
    run(&mut s, "wall_framing_create", json!({"walls": [a]}));
    let depth = s.doc.undo_depth();
    assert!(s.execute("wall_framing_create", json!({"walls": [a]})).is_err(), "one framing per wall");
    assert_eq!(s.doc.undo_depth(), depth);
    assert_eq!(framings(&s), 1);
}

#[test]
fn element_set_of_spacing_regenerates_in_one_step() {
    let mut s = Session::default();
    let a = wall(&mut s, [0.0, 0.0], [3.6, 0.0]);
    let host = first_framing(&run(&mut s, "wall_framing_create", json!({"walls": [a]})));
    let before = framing(&s, host);
    let start = before.members.iter().find(|m| m.role.kind == K::Stud).unwrap().id;
    let depth = s.doc.undo_depth();
    run(&mut s, "element_set", json!({"id": host, "params": {"spacing": 0.6}}));
    assert_eq!(s.doc.undo_depth(), depth + 1);
    let after = framing(&s, host);
    assert_eq!(count(&after, K::Stud), 7);
    assert_eq!(after.members.iter().find(|m| m.role.kind == K::Stud).unwrap().id, start, "same id");
    let d = run(&mut s, "element_get", json!({"id": host}))["derived"].clone();
    assert_eq!(d["members"]["studs"].as_array().unwrap().len(), 7);
    assert_eq!(d["detached"], json!(0));
    for bad in [json!({"spacing": 2.0}), json!({"members": []}), json!({"wall": 0}), json!({"stud_type": 0})] {
        assert!(s.execute("element_set", json!({"id": host, "params": bad})).is_err(), "{bad}");
    }
    assert_eq!(s.doc.undo_depth(), depth + 1);
    run(&mut s, "undo", json!({}));
    assert_eq!(framing(&s, host), before);
}

#[test]
fn a_window_frames_its_opening_and_deleting_the_wall_removes_everything() {
    let mut s = Session::default();
    let a = wall(&mut s, [0.0, 0.0], [3.6, 0.0]);
    let host = first_framing(&run(&mut s, "wall_framing_create", json!({"walls": [a]})));
    run(&mut s, "window_create", json!({"wall": a, "offset": 1.8}));
    let f = framing(&s, host);
    assert_eq!((count(&f, K::KingStud), count(&f, K::JackStud), count(&f, K::Header)), (2, 2, 1));
    assert!(count(&f, K::Stud) < 10, "studs inside the opening went");
    let members: Vec<ElementId> = f.members.iter().map(|m| m.id).collect();
    run(&mut s, "elements_delete", json!({"ids": [a]}));
    assert_eq!(framings(&s), 0);
    assert!(members.iter().all(|m| s.project().element(*m).is_none()));
    run(&mut s, "undo", json!({}));
    assert_eq!(framings(&s), 1);
    assert!(members.iter().all(|m| s.project().element(*m).is_some()));
}

#[test]
fn ifc_export_leaves_the_framing_out() {
    let mut s = Session::default();
    let a = wall(&mut s, [0.0, 0.0], [3.6, 0.0]);
    let host = first_framing(&run(&mut s, "wall_framing_create", json!({"walls": [a]})));
    let text = cc_ifc::export(s.project(), &cc_ifc::ExportOptions::default());
    let uid = cc_ifc::guid::encode(s.project().element(ElementId(host)).unwrap().uid.0);
    assert!(!text.contains(&uid), "the framing itself is not exported");
}
