//! Export → import round trips: the imported project must reproduce the
//! levels and the geometry of every importable element.

// Test helpers outside #[test] functions may fail loudly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use cc_core::{Category, ColumnSection, Element, ElementKind, Project, RoofShape, TypeData, Uid};
use cc_ifc::{ExportOptions, export, import};
use glam::DVec2;
use std::collections::HashMap;

fn load(text: &str) -> Project {
    cc_core::from_ron(text).unwrap()
}

fn round_trip(original: &Project) -> (Project, cc_ifc::ImportReport) {
    let text =
        export(original, &ExportOptions { time_stamp: Some("2026-01-01T00:00:00".into()), file_name: "t.ifc".into() });
    import(&text, Project::from_template()).unwrap()
}

fn by_uid(p: &Project) -> HashMap<Uid, &Element> {
    p.elements.values().map(|e| (e.uid, e)).collect()
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

fn close2(a: DVec2, b: DVec2, tol: f64, what: &str) {
    assert!(a.distance(b) <= tol, "{what}: {a} vs {b}");
}

fn check(original: &Project) {
    let (imported, report) = round_trip(original);
    assert_eq!(report.schema, "IFC4");
    assert_eq!(imported.info.name, original.info.name);
    assert_eq!(imported.info.uid, original.info.uid);
    assert_eq!(imported.levels.len(), original.levels.len());
    for (a, b) in original.levels.iter().zip(&imported.levels) {
        assert_eq!(a.name, b.name);
        close(a.elevation, b.elevation, 1e-9, "level elevation");
    }
    let lookup = by_uid(&imported);
    let level_name = |p: &Project, e: &Element| p.level_name(e.level).to_string();
    const TOL: f64 = 1e-6;
    for e in original.elements.values() {
        let importable = matches!(
            e.category(),
            Category::Wall
                | Category::Door
                | Category::Window
                | Category::Floor
                | Category::Ceiling
                | Category::Roof
                | Category::Room
                | Category::Column
                | Category::Beam
        );
        if !importable {
            continue;
        }
        let i = lookup.get(&e.uid).unwrap_or_else(|| panic!("{:?} {} not imported", e.category(), e.id));
        assert_eq!(i.category(), e.category());
        assert_eq!(level_name(&imported, i), level_name(original, e), "level of {}", e.id);
        assert_eq!(imported.type_name(i.type_id), original.type_name(e.type_id), "type of {}", e.id);
        if e.category() != Category::Room {
            assert_eq!(i.mark, e.mark);
        }
        match (&e.kind, &i.kind) {
            (ElementKind::Wall(a), ElementKind::Wall(b)) => {
                let (s, t) = original.wall_axis(e, a);
                close2(b.start, s, TOL, "wall start");
                close2(b.end, t, TOL, "wall end");
                close(original.wall_height(e, a), imported.wall_height(i, b), TOL, "wall height");
                close(original.wall_base_z(e, a), imported.wall_base_z(i, b), TOL, "wall base");
                close(original.thickness_of(e), imported.thickness_of(i), TOL, "wall thickness");
                assert_eq!(a.structural, b.structural);
            }
            (ElementKind::Door(a), ElementKind::Door(b)) | (ElementKind::Window(a), ElementKind::Window(b)) => {
                let host = |p: &Project, id| p.element(id).map(|h| h.uid);
                assert_eq!(host(original, a.host), host(&imported, b.host), "host of {}", e.id);
                close(a.offset, b.offset, TOL, "opening offset");
                close(a.sill_height, b.sill_height, TOL, "sill");
            }
            (ElementKind::Floor(a), ElementKind::Floor(b)) => {
                close(cc_geom::polygon::area(&a.boundary), cc_geom::polygon::area(&b.boundary), TOL, "floor area");
                close(a.height_offset, b.height_offset, TOL, "floor offset");
            }
            (ElementKind::Ceiling(a), ElementKind::Ceiling(b)) => {
                close(cc_geom::polygon::area(&a.boundary), cc_geom::polygon::area(&b.boundary), TOL, "ceiling area");
                close(a.height_offset, b.height_offset, TOL, "ceiling offset");
            }
            (ElementKind::Roof(a), ElementKind::Roof(b)) => {
                let fp =
                    |r: &cc_core::Roof| cc_geom::polygon::area(&cc_geom::polygon::offset(&r.footprint, r.overhang));
                match (a.shape, b.shape) {
                    (RoofShape::Flat, RoofShape::Flat) => {
                        close(fp(a), fp(b), TOL, "flat roof area");
                        close(a.base_offset, b.base_offset, TOL, "roof base");
                    }
                    (
                        RoofShape::Shed { slope: s0, direction_deg: d0 },
                        RoofShape::Shed { slope: s1, direction_deg: d1 },
                    ) => {
                        close(s0, s1, TOL, "shed slope");
                        close(d0, d1, 1e-6, "shed direction");
                        close(fp(a), fp(b), TOL, "shed footprint");
                    }
                    (RoofShape::Gable { slope: s0, .. }, RoofShape::Gable { slope: s1, .. })
                    | (RoofShape::Hip { slope: s0 }, RoofShape::Hip { slope: s1 }) => {
                        // Pitched roofs come back from their B-rep: the slope is estimated.
                        close(s0, s1, 0.05, "pitched slope");
                        close(fp(a), fp(b), 0.05 * fp(a), "pitched footprint");
                    }
                    (x, y) => panic!("roof shape {x:?} came back as {y:?}"),
                }
            }
            (ElementKind::Room(a), ElementKind::Room(b)) => {
                assert_eq!(a.number, b.number);
                assert_eq!(a.name, b.name);
                close(a.height, b.height, TOL, "room height");
            }
            (ElementKind::Column(a), ElementKind::Column(b)) => {
                close2(a.position, b.position, TOL, "column position");
                let (b0, t0) = original.column_span(e, a);
                let (b1, t1) = imported.column_span(i, b);
                close(b0, b1, TOL, "column base");
                close(t0, t1, TOL, "column top");
                let section = |p: &Project, x: &Element| match p.element_type(x.type_id).map(|t| &t.data) {
                    Some(TypeData::Column { section, .. }) => *section,
                    _ => panic!("column without column type"),
                };
                match (section(original, e), section(&imported, i)) {
                    (ColumnSection::Rectangular { .. }, ColumnSection::Rectangular { .. }) => {
                        let diff = (a.rotation_deg - b.rotation_deg).rem_euclid(180.0);
                        assert!(
                            diff < 1e-6 || diff > 180.0 - 1e-6,
                            "column rotation {} vs {}",
                            a.rotation_deg,
                            b.rotation_deg
                        );
                    }
                    (ColumnSection::Round { diameter: d0 }, ColumnSection::Round { diameter: d1 }) => {
                        close(d0, d1, TOL, "diameter")
                    }
                    (x, y) => panic!("section {x:?} came back as {y:?}"),
                }
            }
            (ElementKind::Beam(a), ElementKind::Beam(b)) => {
                close2(a.start, b.start, TOL, "beam start");
                close2(a.end, b.end, TOL, "beam end");
                close(a.top_offset, b.top_offset, TOL, "beam top");
            }
            (x, y) => panic!("{x:?} came back as {y:?}"),
        }
    }
    // The imported project survives its own export and repair.
    let mut again = imported.clone();
    assert_eq!(again.repair(), 0, "imported project needed repairs");
    let text = export(&again, &ExportOptions::default());
    assert!(text.contains("IFCWALL("));
}

#[test]
fn sample_house_round_trips() {
    check(&load(include_str!("../../../examples/casa_simples.ccad")));
}

#[test]
fn feature_fixture_round_trips() {
    check(&load(include_str!("fixtures/features.ccad")));
}

#[test]
fn reimport_reuses_template_types() {
    let original = load(include_str!("../../../examples/casa_simples.ccad"));
    let (imported, _) = round_trip(&original);
    // Types with the same name and size are not duplicated.
    for category in [Category::Wall, Category::Door, Category::Window] {
        let mut names: Vec<&str> = imported.types_of(category).map(|t| t.name.as_str()).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate {category:?} type names");
    }
}
