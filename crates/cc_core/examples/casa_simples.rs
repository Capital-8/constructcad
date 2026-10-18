//! Generates `examples/casa_simples.ccad`: a small house used for demos and manual testing.
//!
//! Run with `cargo run -p cc_core --example casa_simples`.

use cc_core::{
    Category, Document, Element, ElementId, ElementKind, Floor, Hosted, Project, Roof, RoofShape, View, ViewCategory,
    ViewId, ViewKind, Viewport, Wall,
};
use glam::DVec2;

fn main() {
    let mut doc = Document::new(Project::from_template());
    doc.edit("Casa simples", |e| {
        e.info_mut().name = "Casa simples".into();
        e.info_mut().client = "Exemplo".into();
        e.info_mut().author = "ConstructCAD".into();
        let p = e.project().clone();
        let level = p.levels[0].id;
        let wall_t = p.default_type(Category::Wall).unwrap();
        let door_t = p.default_type(Category::Door).unwrap();
        let window_t = p.default_type(Category::Window).unwrap();
        let floor_t = p.default_type(Category::Floor).unwrap();
        let roof_t = p.default_type(Category::Roof).unwrap();
        let add = |e: &mut cc_core::EditCtx, type_id, mark: &str, kind| {
            let id = ElementId(e.alloc_id());
            e.add_element(Element { id, level, type_id, mark: mark.into(), comments: String::new(), kind })
        };

        // Clockwise outline so each wall's left face points outward.
        let outline = [DVec2::new(0.0, 0.0), DVec2::new(0.0, 6.0), DVec2::new(9.0, 6.0), DVec2::new(9.0, 0.0)];
        let walls: Vec<_> = (0..4)
            .map(|i| {
                let w = Wall { start: outline[i], end: outline[(i + 1) % 4], base_offset: 0.0, height: 3.0 };
                add(e, wall_t, "", ElementKind::Wall(w))
            })
            .collect();
        // Interior partition meeting the north and south walls (T-joins).
        let inner = Wall { start: DVec2::new(5.0, 0.0), end: DVec2::new(5.0, 6.0), base_offset: 0.0, height: 3.0 };
        let partition = add(e, wall_t, "", ElementKind::Wall(inner));

        let hosted =
            |host, offset, sill| Hosted { host, offset, sill_height: sill, flip_facing: false, flip_hand: false };
        // South wall runs from (9,0) to (0,0).
        add(e, door_t, "1", ElementKind::Door(hosted(walls[3], 6.5, 0.0)));
        add(e, door_t, "2", ElementKind::Door(hosted(partition, 4.0, 0.0)));
        add(e, window_t, "1", ElementKind::Window(hosted(walls[3], 2.0, 1.0)));
        add(e, window_t, "2", ElementKind::Window(hosted(walls[1], 2.5, 1.0)));
        add(e, window_t, "3", ElementKind::Window(hosted(walls[1], 6.5, 1.0)));
        add(e, window_t, "4", ElementKind::Window(hosted(walls[2], 3.0, 1.0)));

        let slab: Vec<DVec2> = vec![
            DVec2::new(-0.075, -0.075),
            DVec2::new(9.075, -0.075),
            DVec2::new(9.075, 6.075),
            DVec2::new(-0.075, 6.075),
        ];
        add(e, floor_t, "", ElementKind::Floor(Floor { boundary: slab.clone(), height_offset: 0.0 }));
        let roof = Roof {
            footprint: slab,
            base_offset: 3.0,
            overhang: 0.5,
            shape: RoofShape::Shed { slope: 0.1, direction_deg: 90.0 },
        };
        add(e, roof_t, "", ElementKind::Roof(roof));

        let section = ViewId(e.alloc_id());
        e.add_view(View {
            id: section,
            name: "Corte 1".into(),
            scale: 50,
            kind: ViewKind::Section { start: DVec2::new(3.0, -1.5), end: DVec2::new(3.0, 7.5), depth: 10.0 },
        });
        // Put the south elevation and section on a second sheet.
        let south = p.views_of(ViewCategory::Elevation).find(|v| v.name == "Sul").map(|v| v.id).unwrap();
        let sheet = ViewId(e.alloc_id());
        e.add_view(View {
            id: sheet,
            name: "Elevação e corte".into(),
            scale: 1,
            kind: ViewKind::Sheet {
                number: "A102".into(),
                size: cc_core::SheetSize::A3,
                viewports: vec![
                    Viewport { view: south, center: DVec2::new(130.0, 170.0) },
                    Viewport { view: section, center: DVec2::new(310.0, 170.0) },
                ],
            },
        });
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/casa_simples.ccad");
    cc_core::save(&doc.project, &path).expect("save example");
    println!("{}", path.display());
}
