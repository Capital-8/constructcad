//! Every physical IFC class is imported: natively when a native element can
//! hold it, else as a generic object with its exact geometry and its class,
//! which export writes back.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::component::IfcClass;
use cc_core::{Category, ElementKind, Project, TypeData};
use cc_geom::ModelGeometry;
use cc_ifc::{ExportOptions, export, import};

const FIXTURE: &str = include_str!("fixtures/classes.ifc");

/// Volume of the one element whose type was imported from `entity`.
fn volume_of(p: &Project, g: &ModelGeometry, entity: &str) -> f64 {
    volume_where(p, g, entity, |c| c.entity == entity)
}

fn volume_where(p: &Project, g: &ModelGeometry, what: &str, pick: impl Fn(&IfcClass) -> bool) -> f64 {
    let ids: Vec<_> = p
        .elements
        .values()
        .filter(|e| match p.element_type(e.type_id).map(|t| &t.data) {
            Some(TypeData::Component { def, .. }) => def.ifc.as_ref().is_some_and(&pick),
            _ => false,
        })
        .map(|e| e.id)
        .collect();
    assert_eq!(ids.len(), 1, "{what}: {} elements", ids.len());
    g.volume(ids[0])
}

fn near(a: f64, b: f64, rel: f64, what: &str) {
    assert!((a - b).abs() <= rel * b.abs().max(1e-9), "{what}: {a} vs {b}");
}

fn check_volumes(p: &Project) {
    let g = ModelGeometry::build(p);
    use std::f64::consts::PI;
    near(volume_of(p, &g, "IFCPIPESEGMENT"), PI * 0.05 * 0.05 * 7.0, 0.05, "swept disk");
    near(volume_of(p, &g, "IFCSTAIRFLIGHT"), 2.25, 1e-9, "faceted B-rep");
    near(volume_of(p, &g, "IFCSLAB"), 3.0, 1e-9, "profile with a void");
    near(volume_of(p, &g, "IFCFLOWTERMINAL"), PI * 0.09 * 0.5, 0.02, "revolution in degrees");
    near(volume_of(p, &g, "IFCCOVERING"), 0.04, 1e-9, "covering");
    near(volume_of(p, &g, "IFCDOOR"), 0.9 * 0.05 * 2.1, 1e-9, "hostless door");
    let unknown =
        volume_where(p, &g, "unknown class", |c| c.entity == "IFCFOOBARELEMENT" || c.object_type == "IfcFoobarelement");
    near(unknown, 1.0 + PI * 0.04 * 0.99, 0.01, "CSG union");
    near(volume_of(p, &g, "IFCMEMBER"), (2.0 * 0.2 * 0.02 + 0.01 * 0.36) * 6.0, 1e-9, "I section");
    // Arcs trimmed in degrees: 90° of an annulus 4.8–5 m, 3 m tall.
    let wall = p
        .elements
        .values()
        .find(|e| p.element_type(e.type_id).is_some_and(|t| t.name == "Curved"))
        .map(|e| g.volume(e.id))
        .unwrap();
    // Minus an opening 2 m tall across x 0.5–1.5 m (Simpson's rule on the annulus strip).
    let strip = |x: f64| (25.0 - x * x).sqrt() - (23.04 - x * x).sqrt();
    let hole = 2.0 * (strip(0.5) + 4.0 * strip(1.0) + strip(1.5)) / 6.0;
    near(wall, PI / 4.0 * (25.0 - 23.04) * 3.0 - hole, 0.02, "curved wall with an opening");
    // A beam that would be native, but with a web opening it keeps its exact cut shape.
    near(volume_of(p, &g, "IFCBEAM"), 0.2 * 0.4 * 4.0 - 0.5 * 0.2 * 0.2, 1e-9, "beam with a web opening");
}

#[test]
fn every_class_is_imported() {
    let (p, report) = import(FIXTURE, Project::from_template()).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    let generic: Vec<(&str, usize)> = report.generic.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        generic,
        vec![
            ("IfcBeam", 1),
            ("IfcBuildingElementProxy", 2),
            ("IfcCovering", 1),
            ("IfcDoor", 1),
            ("IfcFlowTerminal", 1),
            ("IfcFoobarelement", 1),
            ("IfcMember", 1),
            ("IfcPipeSegment", 1),
            ("IfcSlab", 1),
            ("IfcStairFlight", 1),
            ("IfcWall", 1),
        ]
    );
    // The clipped straight wall stays a native wall, cut at 2.5 m.
    assert_eq!(report.imported.get(&Category::Wall), Some(&1));
    let (_, wall) = p.walls().next().unwrap();
    assert!((wall.height - 2.5).abs() < 1e-9, "{}", wall.height);
    assert_eq!(report.imported.get(&Category::Grid), Some(&3));
    let grids: Vec<&str> = p
        .elements
        .values()
        .filter_map(|e| match &e.kind {
            ElementKind::Grid(g) => Some(g.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(grids, ["A", "B", "1"]);
    // Both proxies share one type (one mapped geometry).
    let boxes: Vec<_> = p.types_of(Category::Component).filter(|t| t.name == "Box").collect();
    assert_eq!(boxes.len(), 1);
    // Classes and predefined types are kept.
    let pipe = p
        .types_of(Category::Component)
        .find_map(|t| match &t.data {
            TypeData::Component { def, .. } => def.ifc.clone().filter(|c| c.entity == "IFCPIPESEGMENT"),
            _ => None,
        })
        .unwrap();
    assert_eq!(pipe.predefined, "RIGIDSEGMENT");
    check_volumes(&p);
}

#[test]
fn classes_survive_export_and_import() {
    let (p, _) = import(FIXTURE, Project::from_template()).unwrap();
    let text = export(&p, &ExportOptions { time_stamp: Some("2026-01-01T00:00:00".into()), file_name: "t.ifc".into() });
    for needle in [
        "IFCPIPESEGMENT(",
        ".RIGIDSEGMENT.",
        "IFCPIPESEGMENTTYPE(",
        "IFCSTAIRFLIGHT(",
        "IFCSTAIRFLIGHTTYPE(",
        "IFCPOLYGONALFACESET(",
        "IFCFLOWTERMINAL(",
        // Unknown in IFC4: a proxy that names its class.
        "'IfcFoobarelement'",
    ] {
        assert!(text.contains(needle), "{needle} missing");
    }
    // Abstract type objects are not written.
    assert!(!text.contains("IFCFLOWTERMINALTYPE("));
    let (again, report) = import(&text, Project::from_template()).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.generic.get("IfcPipeSegment"), Some(&1));
    assert_eq!(report.generic.get("IfcBuildingElementProxy"), Some(&3));
    check_volumes(&again);
}
