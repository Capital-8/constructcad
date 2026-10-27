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
    // The slab with a void in its profile is a native floor with an opening.
    let (slab, floor) = p
        .elements
        .values()
        .find_map(|e| match &e.kind {
            ElementKind::Floor(f) if f.boundary.iter().all(|q| q.y >= 20.0 - 1e-9) => Some((e.id, f)),
            _ => None,
        })
        .unwrap();
    assert_eq!(floor.openings.len(), 1);
    near(g.volume(slab), 3.0, 1e-9, "profile with a void");
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

/// The pipe's and the box type's property sets, values in SI units.
fn check_properties(p: &Project) {
    use cc_core::properties::{Atom, PropertySet, PropertyValue as V};
    let pipe = p
        .elements
        .values()
        .find_map(|e| match (&e.kind, p.element_type(e.type_id).map(|t| &t.data)) {
            (ElementKind::Component(c), Some(TypeData::Component { def, .. }))
                if def.ifc.as_ref().is_some_and(|i| i.entity == "IFCPIPESEGMENT") =>
            {
                Some(c)
            }
            _ => None,
        })
        .unwrap();
    let set = |name: &str| -> &PropertySet { pipe.properties.iter().find(|s| s.name == name).unwrap() };
    let get = |set: &PropertySet, name: &str| set.properties.iter().find(|p| p.name == name).unwrap().value.clone();
    let real = |v: V| match v {
        V::Single(Some(v)) => match v.value {
            Atom::Real(x) => x,
            other => panic!("{other:?}"),
        },
        V::Quantity { value, .. } => value,
        other => panic!("{other:?}"),
    };
    let custom = set("Pset_Custom");
    assert!(!custom.quantities);
    near(real(get(custom, "NominalDiameter")), 0.1, 1e-9, "length in mm");
    near(real(get(custom, "Slope")), std::f64::consts::FRAC_PI_2, 1e-9, "angle in degrees");
    assert!(matches!(get(custom, "IsExternal"), V::Single(Some(v)) if v.value == Atom::Bool(true)));
    assert!(
        matches!(get(custom, "Status"), V::Single(Some(v)) if v.measure == "IFCLABEL" && v.value == Atom::Text("New".into()))
    );
    assert!(matches!(get(custom, "Material"), V::Enumerated(v) if v.len() == 1));
    let V::Bounded { lower: Some(lower), upper: Some(upper), set_point: None } = get(custom, "Pressure") else {
        panic!("bounded")
    };
    assert_eq!((lower.value, upper.value), (Atom::Real(2.0), Atom::Real(10.0)));
    // A nested property with its own unit (centimeters).
    let V::Complex { usage, properties } = get(custom, "Fitting") else { panic!("complex") };
    assert_eq!(usage, "Usage");
    near(real(properties[0].value.clone()), 2.5, 1e-9, "explicit unit");
    let qto = set("Qto_PipeSegmentBaseQuantities");
    assert!(qto.quantities);
    near(real(get(qto, "Length")), 7.0, 1e-9, "quantity length");
    near(real(get(qto, "OuterSurfaceArea")), 2.2, 1e-9, "area unit m²");
    near(real(get(qto, "GrossVolume")), 0.055, 1e-9, "volume from mm");
    // The box type's own set travels with the type.
    let boxed = p.types_of(Category::Component).find(|t| t.name == "Box").unwrap();
    let TypeData::Component { def, .. } = &boxed.data else { panic!("not a component") };
    let sets = &def.ifc.as_ref().unwrap().properties;
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].name, "Pset_Box");
    assert!(matches!(&sets[0].properties[0].value, V::Single(Some(v)) if v.value == Atom::Text("ACME".into())));
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
    check_properties(&p);
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
    check_properties(&again);
}
