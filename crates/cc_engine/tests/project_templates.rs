//! Project templates: `project_new` starts a metric project by default or an
//! imperial one (feet and inches, round imperial sizes) with
//! `template: "imperial"`, and the imperial project works like the metric one.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::{Category, LengthUnit, MeasurementSystem, ViewKind};
use cc_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn imperial() -> Session {
    let mut s = Session::default();
    let out = run(&mut s, "project_new", json!({ "template": "imperial" }));
    assert_eq!(out["system"], json!("imperial"));
    s
}

#[test]
fn project_new_takes_a_template() {
    let mut s = imperial();
    assert_eq!(s.project().measurement_system(), MeasurementSystem::Imperial);
    assert_eq!(s.project().units.length.unit, LengthUnit::FeetInches);
    assert_eq!(run(&mut s, "project_info", json!({}))["system"], json!("imperial"));

    // No template is the metric one, as before.
    let out = run(&mut s, "project_new", json!({}));
    assert_eq!(out["system"], json!("metric"));
    assert_eq!(s.project().units, cc_core::Units::default());
    assert_eq!(s.project().levels[1].elevation, 3.0);
    let out = run(&mut s, "project_new", json!({ "template": "Metric" }));
    assert_eq!(out["system"], json!("metric"));

    // An unknown template is refused and the project is kept.
    let before = s.project().info.uid;
    let err = s.execute("project_new", json!({ "template": "us" })).expect_err("unknown template");
    assert!(err.message.contains("metric") && err.message.contains("imperial"), "{}", err.message);
    assert_eq!(s.project().info.uid, before);
}

#[test]
fn every_view_of_the_imperial_template_draws() {
    let mut s = imperial();
    s.sync();
    assert!(s.geometry().walls.is_empty());
    let views: Vec<_> = s.project().views.values().map(|v| (v.id, v.kind.clone())).collect();
    let mut drawn = 0;
    for (id, kind) in views {
        match kind {
            ViewKind::Schedule { .. } => assert!(s.schedule(id).is_some(), "schedule {id:?}"),
            ViewKind::Sheet { .. } => assert!(s.sheet(id).is_some(), "sheet {id:?}"),
            ViewKind::ThreeD { .. } => {}
            _ => {
                assert!(s.drawing(id).is_some(), "view {id:?}");
                drawn += 1;
            }
        }
    }
    assert!(drawn >= 9, "{drawn}");
    // Drawings follow the project's units.
    assert_eq!(cc_views::format::current().0.length.unit, LengthUnit::FeetInches);
}

#[test]
fn imperial_scales_read_the_customary_way() {
    let mut s = imperial();
    let plan = s.project().views.values().find(|v| matches!(v.kind, ViewKind::FloorPlan { .. })).expect("plan").id;
    let out = run(&mut s, "view_get", json!({ "view": plan.0 }));
    assert_eq!(out["scale"], json!(48));
    assert_eq!(out["scale_text"], json!("1/4\" = 1'-0\""));

    // A label, a ratio text or a number set the scale.
    let out = run(&mut s, "view_set", json!({ "view": plan.0, "scale": "1/8\" = 1'-0\"" }));
    assert_eq!(out["scale"], json!(96));
    assert_eq!(out["scale_text"], json!("1/8\" = 1'-0\""));
    let out = run(&mut s, "view_set", json!({ "view": plan.0, "scale": "1:50" }));
    assert_eq!((out["scale"].clone(), out["scale_text"].clone()), (json!(50), json!("1:50")));
    let out = run(&mut s, "view_set", json!({ "view": plan.0, "scale": 48 }));
    assert_eq!(out["scale"], json!(48));
    let err = s.execute("view_set", json!({ "view": plan.0, "scale": "1/5\" = 1'" })).expect_err("unreadable");
    assert_eq!(s.project().views[&plan].scale, 48, "{}", err.message);
    let out = run(&mut s, "view_create", json!({ "kind": "drafting", "scale": "3\" = 1'-0\"" }));
    assert_eq!(out["scale"], json!(4));

    // A callout's default detail scale is one of the imperial scales.
    let out = run(&mut s, "callout_create", json!({ "parent": plan.0, "rect": [[0, 0], [2, 2]] }));
    let scale = out["scale"].as_u64().expect("scale") as u32;
    assert!(cc_core::scale::choices(MeasurementSystem::Imperial).contains(&scale), "{scale}");
    assert_eq!(scale, 8, "1/4\" = 1'-0\" gives 1 1/2\" = 1'-0\"");

    // Metric projects are unchanged.
    run(&mut s, "project_new", json!({}));
    let plan = s.project().views.values().find(|v| matches!(v.kind, ViewKind::FloorPlan { .. })).expect("plan").id;
    let out = run(&mut s, "view_get", json!({ "view": plan.0 }));
    assert_eq!((out["scale"].clone(), out["scale_text"].clone()), (json!(50), json!("1:50")));
    let out = run(&mut s, "callout_create", json!({ "parent": plan.0, "rect": [[0, 0], [2, 2]] }));
    assert_eq!(out["scale"], json!(10));
}

#[test]
fn walls_take_the_imperial_default_type() {
    let mut s = imperial();
    let foot = cc_core::units::FOOT;
    let out = run(&mut s, "wall_create", json!({ "start": [0, 0], "end": [20.0 * foot, 0] }));
    let id = out["id"].as_u64().expect("wall id");
    let wall = s.project().elements.values().find(|e| e.id.0 == id).expect("wall");
    let t = s.project().element_type(wall.type_id).expect("type");
    assert_eq!(t.category(), Category::Wall);
    assert_eq!(t.name, cc_core::template_default("tpl-imp-type-masonry-6"));
    assert!((t.thickness() - 6.0 * cc_core::units::INCH).abs() < 1e-12);
    s.sync();
    assert_eq!(s.geometry().walls.len(), 1);
    let plan = s.project().views.values().find(|v| matches!(v.kind, ViewKind::FloorPlan { .. })).expect("plan").id;
    let drawing = s.drawing(plan).expect("plan drawing");
    assert!(!drawing.prims.is_empty());
}
