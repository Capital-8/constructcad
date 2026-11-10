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

/// A session whose project has an IPE 200 profile and a steel column type
/// pointing to it (until profile commands exist).
fn with_ipe_column_type() -> (Session, ElementTypeId) {
    let mut p = cc_engine::new_project();
    let pid = ProfileId(p.alloc_id());
    p.profiles.insert(
        pid,
        ProfileDef {
            id: pid,
            uid: Uid::generate(),
            name: "IPE 200".into(),
            material: ProfileMaterial::Steel,
            shape: IPE200,
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
