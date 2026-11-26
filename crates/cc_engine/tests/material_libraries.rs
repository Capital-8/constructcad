//! Material libraries: the built-in ones listed and imported, picks by
//! name, names the project has kept, files written and read back, and
//! hostile files refused.

use cc_engine::Session;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, args: Value) -> Value {
    s.execute(name, args).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn the_catalog_lists_the_libraries_and_their_materials() {
    let mut s = Session::default();
    let all = run(&mut s, "material_catalog", json!({}));
    let codes: Vec<&str> = all["libraries"].as_array().unwrap().iter().map(|l| l["code"].as_str().unwrap()).collect();
    assert_eq!(codes[..3], ["BR-ACO-ESTRUTURAL", "BR-ACO-ARMADURA", "BR-CONCRETO"]);
    for code in [
        "PAINTS",
        "CERAMICS",
        "STONES",
        "MORTARS-PLASTERS",
        "WOODS",
        "GLASS",
        "BR-CERAMICAS",
        "BR-PEDRAS",
        "BR-ARGAMASSAS-GESSO",
        "BR-MADEIRAS",
        "BR-ISOLANTES",
    ] {
        assert!(codes.contains(&code), "{code}");
    }
    let one = run(&mut s, "material_catalog", json!({"library": "br-concreto"}));
    let lib = &one["libraries"][0];
    assert_eq!(lib["region"], "brazil");
    assert!(!lib["sources"].as_array().unwrap().is_empty());
    let c30 = lib["materials"].as_array().unwrap().iter().find(|m| m["name"] == "Concreto C30 (NBR 6118)").unwrap();
    assert_eq!((c30["compressive_strength"].as_f64(), c30["in_project"].as_bool()), (Some(30.0), Some(false)));
    assert!(s.execute("material_catalog", json!({"library": "nowhere"})).is_err());
}

#[test]
fn materials_are_imported_by_library_and_name_once() {
    let mut s = Session::default();
    let depth = s.doc.undo_depth();
    let picks = json!({"library": "BR-ACO-ESTRUTURAL", "materials": ["Aço MR250 (NBR 7007)", "aço astm a36"]});
    let out = run(&mut s, "material_library_import", picks);
    assert_eq!(out["imported"], json!(["Aço MR250 (NBR 7007)", "Aço ASTM A36"]));
    assert_eq!(s.doc.undo_depth(), depth + 1, "one undo step");
    let mr250 = s.project().material("Aço MR250 (NBR 7007)").unwrap();
    assert_eq!((mr250.physical.yield_strength, mr250.physical.tensile_strength), (Some(250.0), Some(400.0)));
    assert_eq!(mr250.identity.class, cc_core::MaterialClass::Metal);
    // The whole library: what the project has is kept as it is.
    let out = run(&mut s, "material_library_import", json!({"library": "BR-ACO-ESTRUTURAL"}));
    assert_eq!(out["kept"].as_array().unwrap().len(), 2);
    assert_eq!(out["imported"].as_array().unwrap().len(), 10);
    let none = run(&mut s, "material_library_import", json!({"library": "BR-ACO-ESTRUTURAL"}));
    assert_eq!(none["imported"], json!([]));
    assert!(s.execute("material_library_import", json!({"library": "BR-CONCRETO", "materials": ["C31"]})).is_err());
    assert!(s.execute("material_library_import", json!({})).is_err(), "a library, path or text is needed");
}

#[test]
fn a_library_file_is_written_and_read_back() {
    let mut s = Session::default();
    run(&mut s, "material_library_import", json!({"library": "BR-CONCRETO", "materials": ["Concreto C40 (NBR 6118)"]}));
    let dir = std::env::temp_dir().join(format!("cc-material-library-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mine.json");
    let export = json!({
        "code": "MINE", "name": "My concretes", "region": "brazil",
        "materials": ["Concreto C40 (NBR 6118)"], "path": path,
    });
    let out = run(&mut s, "material_library_export", export);
    assert_eq!(out["materials"], 1);
    let mut other = Session::default();
    let back = run(&mut other, "material_library_import", json!({"path": path}));
    assert_eq!(back["library"], "MINE");
    assert_eq!(back["imported"], json!(["Concreto C40 (NBR 6118)"]));
    let a = s.project().material("Concreto C40 (NBR 6118)").unwrap();
    let b = other.project().material("Concreto C40 (NBR 6118)").unwrap();
    assert_eq!(a.physical, b.physical);
    let _ = std::fs::remove_dir_all(&dir);
    // A code is needed, and an unknown material is refused.
    assert!(s.execute("material_library_export", json!({"code": " ", "name": "X"})).is_err());
    assert!(s.execute("material_library_export", json!({"code": "X", "name": "X", "materials": ["Nope"]})).is_err());
}

#[test]
fn hostile_library_files_are_refused() {
    let mut s = Session::default();
    let before = s.project().materials.len();
    for text in [
        "not json",
        r#"{"format": 9, "code": "X", "name": "X", "materials": []}"#,
        r#"{"format": 1, "code": "X", "name": "X", "materials": [{"name": "A", "youngs_modulus": -5}]}"#,
        r#"{"format": 1, "code": "X", "name": "X", "materials": [{"name": "A"}, {"name": "a"}]}"#,
    ] {
        assert!(s.execute("material_library_import", json!({"text": text})).is_err(), "{text}");
    }
    assert_eq!(s.project().materials.len(), before, "nothing imported");
}

#[test]
fn finish_libraries_import_with_their_gloss() {
    let mut s = Session::default();
    let picks = json!({"library": "STONES", "materials": ["Granito preto polido"]});
    let out = run(&mut s, "material_library_import", picks);
    assert_eq!(out["imported"], json!(["Granito preto polido"]));
    let granite = s.project().material("Granito preto polido").unwrap();
    assert_eq!(granite.identity.class, cc_core::MaterialClass::Stone);
    assert_eq!(granite.appearance.finish, cc_core::material::Finish::Polished);
    assert_eq!(granite.physical.thermal_conductivity, Some(2.8));
    let all = run(&mut s, "material_library_import", json!({"library": "PAINTS"}));
    assert!(all["imported"].as_array().unwrap().len() > 20);
    let one = run(&mut s, "material_catalog", json!({"library": "PAINTS"}));
    let epoxy = one["libraries"][0]["materials"].as_array().unwrap().iter().find(|m| m["finish"] == "Glossy");
    assert!(epoxy.is_some_and(|m| m["in_project"] == true), "the catalog shows the finish");
}
