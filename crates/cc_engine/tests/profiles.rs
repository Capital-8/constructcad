//! Structural profiles: columns and beams built and drawn from their
//! profile, at the view's detail level.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::structural_profile::{ProfileDef, ProfileMaterial, ProfileShape};
use cc_core::{
    ColumnSection, Document, ElementId, ElementKind, ElementType, ElementTypeId, ProfileId, SectionShape, TypeData,
    Uid, ViewId,
};
use cc_engine::Session;
use cc_views::{LineStyle, Prim};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

/// IPE 200 (EN 10365): depth, flange width, web, flange, root radius.
const IPE200: ProfileShape = ProfileShape::I { d: 0.2, bf: 0.1, tw: 0.0056, tf: 0.0085, r: 0.012 };

/// A session whose project has a steel profile of `shape` and a column type
/// pointing to it (until profile commands exist).
fn with_column_type(shape: ProfileShape) -> (Session, ElementTypeId) {
    let mut p = cc_engine::new_project();
    let pid = ProfileId(p.alloc_id());
    p.profiles.insert(
        pid,
        ProfileDef {
            id: pid,
            uid: Uid::generate(),
            name: "IPE 200".into(),
            material: ProfileMaterial::Steel,
            shape,
            catalog: None,
            description: String::new(),
        },
    );
    let steel = p
        .types
        .values()
        .find(|t| matches!(t.data, TypeData::Column { section: ColumnSection::Profile { .. }, .. }))
        .unwrap()
        .clone();
    let tid = ElementTypeId(p.alloc_id());
    let TypeData::Column { material, .. } = steel.data else { panic!("not a column type") };
    let data = TypeData::Column {
        section: ColumnSection::Profile {
            width: 0.1,
            depth: 0.2,
            shape: SectionShape::I { web: 0.0056, flange: 0.0085 },
        },
        material,
        profile: Some(pid),
        profile_rotation_deg: 0.0,
        profile_mirror: false,
    };
    p.types.insert(
        tid,
        ElementType { id: tid, name: "IPE 200".into(), data, info: Default::default(), uid: Uid::generate() },
    );
    (Session::new(Document::new(p)), tid)
}

#[test]
fn a_column_with_an_ipe_profile_has_its_area_times_height() {
    let (mut s, tid) = with_ipe_column_type();
    let col = run(&mut s, "column_create", json!({"type": tid.0, "position": [0.0, 0.0]}))["id"].as_u64().unwrap();
    let id = ElementId(col);
    let (base, top) = {
        let p = s.project();
        let e = p.element(id).unwrap();
        let ElementKind::Column(c) = &e.kind else { panic!("not a column") };
        p.column_span(e, c)
    };
    let height = top - base;
    let volume = s.geometry().volume(id);
    let area =
        2.0 * 0.1 * 0.0085 + (0.2 - 0.017) * 0.0056 + 4.0 * (1.0 - std::f64::consts::FRAC_PI_4) * 0.012f64.powi(2);
    assert!((volume - area * height).abs() / (area * height) < 0.003, "{volume} vs {}", area * height);
}

/// Cut lines of element `id` in the view's drawing.
fn cut_lines(s: &mut Session, view: u64, id: u64) -> usize {
    let d = s.drawing(ViewId(view)).unwrap();
    d.prims
        .iter()
        .filter(|p| matches!(p, Prim::Line { style: LineStyle::Cut, element: Some(e), .. } if e.0 == id))
        .count()
}

#[test]
fn plans_cut_a_profile_column_at_their_detail_level() {
    let (mut s, tid) = with_ipe_column_type();
    let view = run(&mut s, "views_list", json!({"category": "floor_plan"}))[0]["id"].as_u64().unwrap();
    let col = run(&mut s, "column_create", json!({"type": tid.0, "position": [0.0, 0.0]}))["id"].as_u64().unwrap();
    run(&mut s, "view_set", json!({"view": view, "detail": "coarse"}));
    assert_eq!(cut_lines(&mut s, view, col), 4, "the bounding rectangle");
    run(&mut s, "view_set", json!({"view": view, "detail": "medium"}));
    assert_eq!(cut_lines(&mut s, view, col), 12, "the I without its root radii");
    run(&mut s, "view_set", json!({"view": view, "detail": "fine"}));
    assert!(cut_lines(&mut s, view, col) > 12, "the I with its root radii");
}

fn with_ipe_column_type() -> (Session, ElementTypeId) {
    with_column_type(IPE200)
}

#[test]
fn a_beam_ending_between_the_flanges_frames_into_the_column_box() {
    // The I stands with its web along y; the beam comes along x and ends
    // between the flanges, off the web by more than the join tolerance.
    let (mut s, tid) = with_ipe_column_type();
    run(&mut s, "column_create", json!({"type": tid.0, "position": [0.0, 0.0]}));
    let beam = run(&mut s, "beam_create", json!({"start": [5.0, 0.0], "end": [0.03, 0.0]}))["id"].as_u64().unwrap();
    let g = s.geometry().beams.get(&ElementId(beam)).cloned().unwrap();
    assert!(matches!(g.end, cc_geom::BeamEnd::Column(_)), "{:?}", g.end);
    // Trimmed at the face of the box, x = bf / 2.
    let face = 5.0 - 0.05;
    assert!((g.end_ext.0 - face).abs() < 1e-9 && (g.end_ext.1 - face).abs() < 1e-9, "{:?}", g.end_ext);
}

#[test]
fn a_tube_column_is_cut_with_its_hole() {
    let tube = ProfileShape::RectTube { h: 0.15, b: 0.15, t: 0.006, ro: 0.0 };
    let (mut s, tid) = with_column_type(tube);
    let view = run(&mut s, "views_list", json!({"category": "floor_plan"}))[0]["id"].as_u64().unwrap();
    let col = run(&mut s, "column_create", json!({"type": tid.0, "position": [0.0, 0.0]}))["id"].as_u64().unwrap();
    run(&mut s, "view_set", json!({"view": view, "detail": "fine"}));
    assert_eq!(cut_lines(&mut s, view, col), 8, "outside and hole outlines");
    let d = s.drawing(ViewId(view)).unwrap();
    let fills: Vec<&Vec<glam::DVec2>> = d
        .prims
        .iter()
        .filter_map(|p| match p {
            Prim::Fill { pts, element: Some(e), .. } if e.0 == col => Some(pts),
            _ => None,
        })
        .collect();
    assert!(!fills.is_empty());
    let filled = |q: glam::DVec2| fills.iter().any(|f| cc_geom::polygon::contains(f, q));
    assert!(filled(glam::DVec2::new(0.072, 0.0)), "the wall is filled");
    assert!(!filled(glam::DVec2::ZERO), "the hole is not");
    assert!(!filled(glam::DVec2::new(0.05, 0.05)), "the hole is not");
}

/// W 310x38.7 (ASTM A6): depth, flange width, web, flange, root radius.
const W310: ProfileShape = ProfileShape::I { d: 0.31, bf: 0.165, tw: 0.0058, tf: 0.0097, r: 0.008 };

/// Add a steel profile of `shape` and a beam type pointing to it.
fn add_beam_type(s: &mut Session, shape: ProfileShape) -> ElementTypeId {
    let mut p = s.project().clone();
    let pid = ProfileId(p.alloc_id());
    p.profiles.insert(
        pid,
        ProfileDef {
            id: pid,
            uid: Uid::generate(),
            name: "W 310x38.7".into(),
            material: ProfileMaterial::Steel,
            shape: shape.clone(),
            catalog: None,
            description: String::new(),
        },
    );
    let material = p
        .types
        .values()
        .find_map(|t| match &t.data {
            TypeData::Beam { material, .. } => Some(material.clone()),
            _ => None,
        })
        .unwrap();
    let ProfileShape::I { d, bf, tw, tf, .. } = shape else { panic!("not an I") };
    let tid = ElementTypeId(p.alloc_id());
    let data = TypeData::Beam {
        width: bf,
        height: d,
        material,
        shape: SectionShape::I { web: tw, flange: tf },
        profile: Some(pid),
        profile_rotation_deg: 0.0,
        profile_mirror: false,
    };
    p.types.insert(
        tid,
        ElementType { id: tid, name: "W 310x38.7".into(), data, info: Default::default(), uid: Uid::generate() },
    );
    *s = Session::new(Document::new(p));
    tid
}

#[test]
fn beams_with_w_profiles_meet_in_a_mitered_l() {
    let (mut s, _) = with_ipe_column_type();
    let tid = add_beam_type(&mut s, W310);
    let make = |s: &mut Session, a: [f64; 2], b: [f64; 2]| {
        let id = run(s, "beam_create", json!({"type": tid.0, "start": a, "end": b}))["id"].as_u64().unwrap();
        ElementId(id)
    };
    let a = make(&mut s, [0.0, 0.0], [4.0, 0.0]);
    let b = make(&mut s, [4.0, 0.0], [4.0, 4.0]);
    let area = W310.outline().unwrap().properties(0.0).area;
    let length = 4.0;
    let g = s.geometry();
    let (ga, gb) = (&g.beams[&a], &g.beams[&b]);
    assert_eq!((ga.end, gb.start), (cc_geom::BeamEnd::Beam, cc_geom::BeamEnd::Beam));
    assert!(ga.curved, "the root radii are arcs");
    let (va, vb) = (g.volume(a), g.volume(b));
    assert!(va > 0.0 && vb > 0.0, "{va} {vb}");
    // Mitered on their axes the symmetric W's keep area x axis length each:
    // the corner the square ends would overlap is cut away.
    let square = 2.0 * area * (length + ga.half);
    assert!(va + vb < square, "{} vs {square}", va + vb);
    assert!(
        (va + vb - 2.0 * area * length).abs() / (2.0 * area * length) < 1e-6,
        "{} vs {}",
        va + vb,
        2.0 * area * length
    );
}

/// The linear-mass cell of the only beam of a beams schedule.
fn beam_schedule_cells(s: &mut Session) -> Vec<String> {
    let schedule = run(s, "view_create", json!({"kind": "schedule", "category": "beam"}));
    run(
        s,
        "view_set",
        json!({"view": schedule["id"], "params": {"def": {"fields": ["Type", "Profile", "LinearMass"]}}}),
    );
    let table = run(s, "schedule_table", json!({"view": schedule["id"]}));
    let rows = table["rows"].as_array().unwrap();
    rows[0]["cells"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect()
}

#[test]
fn schedules_show_the_profile_and_its_linear_mass() {
    let (mut s, _) = with_ipe_column_type();
    let tid = add_beam_type(&mut s, W310);
    // Give the beam type the template's steel.
    let mut p = s.project().clone();
    let steel = p.materials.iter().find(|m| m.identity.class == cc_core::MaterialClass::Metal).unwrap().name.clone();
    let TypeData::Beam { material, .. } = &mut p.types.get_mut(&tid).unwrap().data else { panic!("not a beam") };
    *material = steel.clone();
    let mut s = Session::new(Document::new(p.clone()));
    run(&mut s, "beam_create", json!({"type": tid.0, "start": [0.0, 0.0], "end": [4.0, 0.0]}));
    // The template's steel carries a density of 7850 kg/m3.
    let cells = beam_schedule_cells(&mut s);
    assert!(cells[1].contains("W 310x38.7"), "{cells:?}");
    let expected = W310.outline().unwrap().properties(7850.0).linear_mass;
    let shown = cells[2].trim_end_matches(" kg/m").replace(',', ".");
    let got: f64 = shown.parse().unwrap_or_else(|_| panic!("{cells:?}"));
    assert!((got - expected).abs() / expected < 0.01, "{got} vs {expected}");

    // Without a density nothing is guessed.
    let mut p = s.project().clone();
    p.materials.iter_mut().find(|m| m.name == steel).unwrap().physical.density = None;
    let mut s = Session::new(Document::new(p));
    let cells = beam_schedule_cells(&mut s);
    assert_eq!(cells[2], "", "{cells:?}");
}

fn run_err(s: &mut Session, name: &str, args: Value) -> cc_engine::EngineError {
    match s.execute(name, args) {
        Ok(v) => panic!("{name} should fail, gave {v}"),
        Err(e) => e,
    }
}

#[test]
fn create_get_set_delete() {
    let mut s = Session::default();
    let out = run(
        &mut s,
        "profile_create",
        json!({"material": "steel", "shape": "i", "name": "PS 300",
            "params": {"d": 0.3, "bf": 0.15, "tw": 0.008, "tf": 0.0125, "r": 0.0}}),
    );
    let id = out["id"].as_u64().unwrap();
    let got = run(&mut s, "profile_get", json!({"profile": "ps 300"}));
    assert_eq!(got["id"].as_u64(), Some(id));
    assert_eq!(got["shape"], "i");
    assert_eq!(got["material"], "steel");
    let area = got["properties"]["area"].as_f64().unwrap();
    assert!((area - (2.0 * 0.15 * 0.0125 + 0.275 * 0.008)).abs() < 1e-9);

    let steps = s.doc.undo_depth();
    let err = run_err(&mut s, "profile_set", json!({"profile": id, "params": {"tw": 0.5}}));
    assert_eq!(err.code, cc_engine::ErrorCode::InvalidArguments);
    assert_eq!(s.doc.undo_depth(), steps, "a refused change leaves no undo step");
    let err = run_err(&mut s, "profile_set", json!({"profile": id, "params": {"zz": 0.5}}));
    assert!(err.message.contains("tw"), "{}", err.message);
    run(&mut s, "profile_set", json!({"profile": id, "params": {"tw": 0.01}}));
    assert_eq!(run(&mut s, "profile_get", json!({"profile": id}))["params"]["tw"], 0.01);

    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "profile_get", json!({"profile": id}))["params"]["tw"], 0.008);
    let list = run(&mut s, "profiles_list", json!({"shape": "I"}));
    let row = list.as_array().unwrap().iter().find(|p| p["id"].as_u64() == Some(id)).unwrap();
    assert!((row["linear_mass_steel"].as_f64().unwrap() - area * 7850.0).abs() < 1e-9);
    assert!(list.as_array().unwrap().iter().all(|p| p["shape"] == "i"));
    run(&mut s, "profile_delete", json!({"profile": id}));
    let list = run(&mut s, "profiles_list", json!({}));
    assert!(list.as_array().unwrap().iter().all(|p| p["id"].as_u64() != Some(id)));
}

#[test]
fn create_checks_its_arguments() {
    let mut s = Session::default();
    let err = run_err(&mut s, "profile_create", json!({"material": "gold", "shape": "i"}));
    assert!(err.message.contains("steel"), "{}", err.message);
    let err = run_err(&mut s, "profile_create", json!({"material": "steel", "shape": "zz"}));
    assert!(err.message.contains("cold_lipped_u"), "{}", err.message);
    // Defaults, a name of its own, case-insensitive codes.
    let a = run(&mut s, "profile_create", json!({"material": "Steel", "shape": "RECT"}));
    let b = run(&mut s, "profile_create", json!({"material": "steel", "shape": "rect"}));
    assert_ne!(a["name"], b["name"]);
    let err = run_err(&mut s, "profile_create", json!({"material": "steel", "shape": "rect", "name": a["name"]}));
    assert_eq!(err.code, cc_engine::ErrorCode::InvalidArguments);
    let dup = run(&mut s, "profile_duplicate", json!({"profile": a["id"]}));
    assert_ne!(dup["id"], a["id"]);
    assert_ne!(dup["name"], a["name"]);
}

#[test]
fn sketch_profile_from_curves() {
    let mut s = Session::default();
    let out = run(
        &mut s,
        "profile_create",
        json!({"material": "steel", "name": "Box",
            "curves": [{"kind": "rectangle", "corner": [-0.1, -0.15], "opposite": [0.1, 0.15]}],
            "openings": [[{"kind": "rectangle", "corner": [-0.09, -0.14], "opposite": [0.09, 0.14]}]],
            "sketch_params": [{"name": "t", "value": 0.01}]}),
    );
    let got = run(&mut s, "profile_get", json!({"profile": out["id"]}));
    assert_eq!(got["shape"], "sketch");
    assert_eq!(got["outline"]["holes"].as_array().unwrap().len(), 1);
    assert_eq!(got["params"]["t"], 0.01);
    let area = got["properties"]["area"].as_f64().unwrap();
    assert!((area - (0.2 * 0.3 - 0.18 * 0.28)).abs() < 1e-9, "{area}");
    // The sketch is patched: no openings leaves the solid rectangle.
    run(&mut s, "profile_set", json!({"profile": out["id"], "sketch": {"openings": []}}));
    let got = run(&mut s, "profile_get", json!({"profile": out["id"]}));
    assert!(got["outline"]["holes"].as_array().unwrap().is_empty());
}

#[test]
fn deleting_a_used_profile_needs_a_replacement() {
    let mut s = Session::default();
    let used = run(&mut s, "profiles_list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["users"].as_u64().unwrap() > 0)
        .unwrap()
        .clone();
    let other =
        run(&mut s, "profile_create", json!({"material": "concrete", "shape": "rect", "name": "R"}))["id"].clone();
    let err = run_err(&mut s, "profile_delete", json!({"profile": used["id"]}));
    assert!(err.message.contains("replace_with"), "{}", err.message);
    run(&mut s, "profile_delete", json!({"profile": used["id"], "replace_with": other}));
    let r = run(&mut s, "profile_get", json!({"profile": "R"}));
    assert!(!r["used_by"].as_array().unwrap().is_empty());
}

/// The legacy box of a column or beam type: (profile, width, height).
fn type_box(s: &Session, id: ElementTypeId) -> (Option<ProfileId>, f64, f64) {
    match &s.project().types[&id].data {
        TypeData::Beam { profile, width, height, .. } => (*profile, *width, *height),
        TypeData::Column { profile, section, .. } => {
            let (w, h) = match *section {
                ColumnSection::Rectangular { width, depth } | ColumnSection::Profile { width, depth, .. } => {
                    (width, depth)
                }
                ColumnSection::Round { diameter } | ColumnSection::Pipe { diameter, .. } => (diameter, diameter),
            };
            (*profile, w, h)
        }
        _ => panic!("not a column or beam type"),
    }
}

#[test]
fn types_follow_an_edited_profile() {
    let mut s = Session::default();
    let list = run(&mut s, "profiles_list", json!({"shape": "rect"}));
    let used = list.as_array().unwrap().iter().find(|p| p["users"].as_u64().unwrap() > 0).unwrap().clone();
    let pid = ProfileId(used["id"].as_u64().unwrap());
    let users = s.project().profile_users(pid);
    assert!(!users.is_empty());
    run(&mut s, "profile_set", json!({"profile": pid.0, "params": {"b": 0.33, "h": 0.55}}));
    for t in users {
        let (profile, w, h) = type_box(&s, t);
        assert_eq!(profile, Some(pid), "the type still points to the edited profile");
        assert!((w - 0.33).abs() < 1e-9 && (h - 0.55).abs() < 1e-9, "{w} x {h}");
    }
}
