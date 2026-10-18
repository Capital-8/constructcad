//! Generates `examples/casa_simples.ccad`: a small house used for demos and manual testing.
//!
//! Run with `cargo run -p cc_core --example casa_simples`.

use cc_core::{
    Category, Dimension, Document, Element, ElementId, ElementKind, Floor, Grid, Hosted, Project, Roof, RoofShape,
    Room, View, ViewCategory, ViewId, ViewKind, Viewport, Wall, WallTop,
};
use glam::DVec2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Document::new(Project::from_template());
    doc.edit("Casa simples", |e| -> Result<(), String> {
        e.info_mut().name = "Casa simples".into();
        e.info_mut().client = "Exemplo".into();
        e.info_mut().author = "ConstructCAD".into();
        let p = e.project().clone();
        let (level, upper) = match p.levels.as_slice() {
            [ground, first, ..] => (ground.id, first.id),
            _ => return Err("template needs two levels".into()),
        };
        let wall_t = p.default_type(Category::Wall).ok_or("template lacks a wall type")?;
        let door_t = p.default_type(Category::Door).ok_or("template lacks a door type")?;
        let window_t = p.default_type(Category::Window).ok_or("template lacks a window type")?;
        let floor_t = p.default_type(Category::Floor).ok_or("template lacks a floor type")?;
        let roof_t = p.default_type(Category::Roof).ok_or("template lacks a roof type")?;
        let add = |e: &mut cc_core::EditCtx, type_id, mark: &str, kind| {
            let id = ElementId(e.alloc_id());
            e.add_element(Element {
                id,
                level,
                type_id,
                mark: mark.into(),
                comments: String::new(),
                kind,
                uid: Default::default(),
            })
        };

        // Clockwise outline so each wall's left face points outward.
        let outline = [DVec2::new(0.0, 0.0), DVec2::new(0.0, 6.0), DVec2::new(9.0, 6.0), DVec2::new(9.0, 0.0)];
        let walls: Vec<_> = (0..4)
            .map(|i| {
                // Exterior walls are load-bearing and rise to the upper level.
                let w = Wall {
                    start: outline[i],
                    end: outline[(i + 1) % 4],
                    base_offset: 0.0,
                    height: 3.0,
                    top: WallTop::Level { level: upper, offset: 0.0 },
                    structural: true,
                    ..Default::default()
                };
                add(e, wall_t, "", ElementKind::Wall(w))
            })
            .collect();
        // Interior partition meeting the north and south walls (T-joins).
        let inner = Wall {
            start: DVec2::new(5.0, 0.0),
            end: DVec2::new(5.0, 6.0),
            base_offset: 0.0,
            height: 3.0,
            ..Default::default()
        };
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

        // Rooms, grids and a dimension on the ground floor plan.
        let room_t = p.default_type(Category::Room).ok_or("template lacks a room type")?;
        let grid_t = p.default_type(Category::Grid).ok_or("template lacks a grid type")?;
        let dim_t = p.default_type(Category::Dimension).ok_or("template lacks a dimension type")?;
        for (x, number, name) in [(2.5, "1", "Sala"), (7.0, "2", "Quarto")] {
            let room = Room { position: DVec2::new(x, 3.0), number: number.into(), name: name.into(), height: 3.0 };
            add(e, room_t, "", ElementKind::Room(room));
        }
        for (x, name) in [(0.0, "1"), (5.0, "2"), (9.0, "3")] {
            let grid = Grid { start: DVec2::new(x, -1.5), end: DVec2::new(x, 7.5), name: name.into() };
            add(e, grid_t, "", ElementKind::Grid(grid));
        }
        for (y, name) in [(0.0, "A"), (6.0, "B")] {
            let grid = Grid { start: DVec2::new(-1.5, y), end: DVec2::new(10.5, y), name: name.into() };
            add(e, grid_t, "", ElementKind::Grid(grid));
        }
        let ground_plan = p
            .views_of(ViewCategory::FloorPlan)
            .find(|v| v.level() == Some(level))
            .map(|v| v.id)
            .ok_or("no ground plan")?;
        for (a, b, offset) in
            [((0.0, 0.0), (5.0, 0.0), -1.2), ((5.0, 0.0), (9.0, 0.0), -1.2), ((0.0, 0.0), (9.0, 0.0), -1.9)]
        {
            let dim = Dimension { view: ground_plan, a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1), offset };
            add(e, dim_t, "", ElementKind::Dimension(dim));
        }

        let section = ViewId(e.alloc_id());
        e.add_view(View {
            id: section,
            name: "Corte 1".into(),
            scale: 50,
            kind: ViewKind::Section { start: DVec2::new(3.0, -1.5), end: DVec2::new(3.0, 7.5), depth: 10.0 },
        });
        // Put the south elevation and section on a second sheet.
        let south =
            p.views_of(ViewCategory::Elevation).find(|v| v.name == "Sul").map(|v| v.id).ok_or("no south elevation")?;
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
        Ok(())
    })?;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/casa_simples.ccad");
    cc_core::save(&doc.project, &path)?;
    println!("{}", path.display());
    Ok(())
}
