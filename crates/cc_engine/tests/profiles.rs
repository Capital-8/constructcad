//! Structural profiles: columns built and drawn from their profile, at the
//! view's detail level.

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
