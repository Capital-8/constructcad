//! Column and beam types by structural material, and the multi-storey grid frame.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::{ElementTypeId, StructuralMaterial};
use cc_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn run_err(s: &mut Session, name: &str, args: Value) -> cc_engine::EngineError {
    s.execute(name, args).expect_err(name)
}

fn type_for(s: &mut Session, category: &str, material: &str) -> Value {
    run(s, "structural_type_for", json!({"category": category, "material": material}))
}

#[test]
fn an_existing_type_is_returned_without_an_undo_step() {
    let mut s = Session::default();
    let depth = s.doc.undo_depth();
    for (category, material) in [("column", "steel"), ("beam", "steel"), ("column", "concrete")] {
        let out = type_for(&mut s, category, material);
        assert_eq!(out["created"], false, "{category} {material}: the template has one");
        let id = ElementTypeId(out["id"].as_u64().unwrap());
        let data = &s.project().element_type(id).unwrap().data;
        assert_eq!(s.project().structural_material(data), StructuralMaterial::from_code(material));
    }
    assert_eq!(s.doc.undo_depth(), depth);
}

#[test]
fn a_missing_material_gets_its_catalog_default_once() {
    let mut s = Session::default();
    for category in ["column", "beam"] {
        let depth = s.doc.undo_depth();
        let out = type_for(&mut s, category, "timber");
        assert_eq!(out["created"], true);
        assert_eq!(out["name"], "MLC 9x32");
        assert_eq!(s.doc.undo_depth(), depth + 1, "one undo step");
        let id = ElementTypeId(out["id"].as_u64().unwrap());
        let data = &s.project().element_type(id).unwrap().data;
        assert_eq!(s.project().structural_material(data), Some(StructuralMaterial::Timber));
        let again = type_for(&mut s, category, "timber");
        assert_eq!((again["id"].clone(), again["created"].clone()), (out["id"].clone(), json!(false)));
    }
    assert_eq!(s.project().profiles.values().filter(|p| p.name == "MLC 9x32").count(), 1, "one profile shared");
    run(&mut s, "undo", json!({}));
    assert!(s.project().types_of_material(cc_core::Category::Beam, StructuralMaterial::Timber).next().is_none());
}

#[test]
fn steel_and_concrete_defaults_are_made_when_missing() {
    let mut s = Session::default();
    // Delete every steel column type so steel is missing (concrete ones remain).
    let steel: Vec<u64> =
        s.project().types_of_material(cc_core::Category::Column, StructuralMaterial::Steel).map(|t| t.id.0).collect();
    for id in steel {
        run(&mut s, "type_delete", json!({"type": id}));
    }
    let out = type_for(&mut s, "column", "steel");
    assert_eq!((out["created"].clone(), out["name"].clone()), (json!(true), json!("W200x26,6")));
    // Concrete: delete them all but the new steel one, then ask for concrete.
    let concrete_ids: Vec<u64> = s
        .project()
        .types_of_material(cc_core::Category::Column, StructuralMaterial::Concrete)
        .map(|t| t.id.0)
        .collect();
    for id in concrete_ids {
        run(&mut s, "type_delete", json!({"type": id}));
    }
    let made = type_for(&mut s, "column", "concrete");
    assert_eq!(made["created"], true);
    let id = ElementTypeId(made["id"].as_u64().unwrap());
    let def = s.project().type_profile_def(&s.project().element_type(id).unwrap().data).unwrap();
    assert_eq!(def.shape, cc_core::ProfileShape::Rect { b: 0.2, h: 0.4, chamfer: 0.0 });
    assert_eq!(def.material, cc_core::ProfileMaterial::Concrete);
}

#[test]
fn bad_arguments_are_refused() {
    let mut s = Session::default();
    for (category, material) in [("wall", "steel"), ("column", "gold")] {
        let err = run_err(&mut s, "structural_type_for", json!({"category": category, "material": material}));
        assert_eq!(err.code, cc_engine::ErrorCode::InvalidArguments, "{category} {material}");
    }
}
