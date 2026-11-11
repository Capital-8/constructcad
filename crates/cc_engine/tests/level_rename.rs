//! A level and its plans keep one name (issue #274): renaming the level
//! renames the plans named after it, renaming such a plan renames the level
//! and its other plans, each in one undo step.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::{LevelId, ViewId};
use cc_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

fn id(v: &Value) -> u64 {
    v["id"].as_u64().unwrap()
}

/// The names of the level's plans, sorted.
fn plan_names(s: &Session, level: u64) -> Vec<String> {
    let mut names: Vec<_> =
        s.project().views.values().filter(|v| v.level() == Some(LevelId(level))).map(|v| v.name.clone()).collect();
    names.sort();
    names
}

fn plans(s: &Session, level: u64) -> Vec<u64> {
    s.project().views.values().filter(|v| v.level() == Some(LevelId(level))).map(|v| v.id.0).collect()
}

/// A level with its floor and ceiling plans, a third plan named after it
/// (an MEP plan) and one named otherwise.
fn setup() -> (Session, u64, u64) {
    let mut s = Session::default();
    let level = id(&run(&mut s, "level_create", json!({ "name": "Mezanino", "elevation": 4.5 })));
    run(&mut s, "view_create", json!({ "kind": "electrical_plan", "level": level, "name": "Mezanino"}));
    let other = id(&run(&mut s, "view_create", json!({ "kind": "floor_plan", "level": level, "name": "Layout" })));
    (s, level, other)
}

fn level_name(s: &Session, level: u64) -> String {
    s.project().level_name(LevelId(level)).to_string()
}

#[test]
fn renaming_a_level_renames_its_plans_in_one_step() {
    let (mut s, level, _) = setup();
    assert_eq!(plan_names(&s, level), ["Layout", "Mezanino", "Mezanino", "Mezanino"]);
    run(&mut s, "level_set", json!({ "level": level, "name": "  Sobreloja " }));
    assert_eq!(level_name(&s, level), "Sobreloja");
    assert_eq!(plan_names(&s, level), ["Layout", "Sobreloja", "Sobreloja", "Sobreloja"]);
    // Plans of other levels keep their names.
    assert!(s.project().views.values().all(|v| v.level() == Some(LevelId(level)) || v.name != "Sobreloja"));

    run(&mut s, "undo", json!({}));
    assert_eq!(level_name(&s, level), "Mezanino");
    assert_eq!(plan_names(&s, level), ["Layout", "Mezanino", "Mezanino", "Mezanino"]);
}

#[test]
fn renaming_a_plan_named_after_its_level_renames_the_level_in_one_step() {
    let (mut s, level, _) = setup();
    let plan = *plans(&s, level).iter().find(|p| s.project().view(ViewId(**p)).unwrap().name == "Mezanino").unwrap();
    run(&mut s, "view_set", json!({ "view": plan, "name": "Sobreloja" }));
    assert_eq!(level_name(&s, level), "Sobreloja");
    assert_eq!(plan_names(&s, level), ["Layout", "Sobreloja", "Sobreloja", "Sobreloja"]);
    // Referred to by its new name.
    let got = run(&mut s, "level_set", json!({ "level": "Sobreloja", "elevation": 5.0 }));
    assert_eq!(id(&got), level);

    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));
    assert_eq!(level_name(&s, level), "Mezanino");
    assert_eq!(plan_names(&s, level), ["Layout", "Mezanino", "Mezanino", "Mezanino"]);
}

#[test]
fn a_plan_named_otherwise_or_asked_not_to_renames_only_itself() {
    let (mut s, level, other) = setup();
    run(&mut s, "view_set", json!({ "view": other, "name": "Layout 2" }));
    assert_eq!(level_name(&s, level), "Mezanino");
    assert_eq!(plan_names(&s, level), ["Layout 2", "Mezanino", "Mezanino", "Mezanino"]);

    let plan = *plans(&s, level).iter().find(|p| s.project().view(ViewId(**p)).unwrap().name == "Mezanino").unwrap();
    run(&mut s, "view_set", json!({ "view": plan, "name": "Mezanino - Mobiliário", "rename_level": false }));
    assert_eq!(level_name(&s, level), "Mezanino");
    assert_eq!(plan_names(&s, level), ["Layout 2", "Mezanino", "Mezanino", "Mezanino - Mobiliário"]);
}

/// Typing a name key by key, as the levels dialog and the properties
/// palette do (merged steps), keeps the plans following.
#[test]
fn renaming_key_by_key_keeps_the_plans_following() {
    let (mut s, level, _) = setup();
    for name in ["M", "Me", "Mez"] {
        run(&mut s, "level_set", json!({ "level": level, "name": name }));
    }
    assert_eq!(plan_names(&s, level), ["Layout", "Mez", "Mez", "Mez"]);
}
