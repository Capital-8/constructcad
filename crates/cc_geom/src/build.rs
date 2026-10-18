use crate::polygon;
use crate::wall::{Opening, WallGeom, WallLine, resolve_join};
use crate::{Ray, Solid};
use cc_core::{Category, ElementId, ElementKind, Project, RoofShape, TypeData};
use glam::{DVec2, DVec3};
use std::collections::BTreeMap;

const FRAME: f64 = 0.05;
/// Shading color of door and window frames.
pub const FRAME_COLOR: [u8; 3] = [236, 236, 232];

/// One box of a door or window, in opening-local coordinates: `x` along the
/// wall from the start jamb, `y` across the wall (positive on its left), `z`
/// up from the sill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostedPart {
    pub min: DVec3,
    pub max: DVec3,
    /// Leaf or glazing (colored with the type material) rather than frame.
    pub panel: bool,
}

/// The boxes making up a door or window of `width` x `height` in a wall of
/// half thickness `half`. Empty for non-hosted types.
pub fn hosted_parts(data: &TypeData, width: f64, height: f64, half: f64) -> Vec<HostedPart> {
    let part = |x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64, panel: bool| HostedPart {
        min: DVec3::new(x0, y0, z0),
        max: DVec3::new(x1, y1, z1),
        panel,
    };
    let (w, h) = (width, height);
    match data {
        TypeData::Door { .. } => vec![
            part(0.0, FRAME, -half, half, 0.0, h, false),
            part(w - FRAME, w, -half, half, 0.0, h, false),
            part(FRAME, w - FRAME, -half, half, h - FRAME, h, false),
            part(FRAME, w - FRAME, -0.02, 0.02, 0.0, h - FRAME, true),
        ],
        TypeData::Window { .. } => {
            let d = 0.04;
            vec![
                part(0.0, FRAME, -d, d, 0.0, h, false),
                part(w - FRAME, w, -d, d, 0.0, h, false),
                part(FRAME, w - FRAME, -d, d, 0.0, FRAME, false),
                part(FRAME, w - FRAME, -d, d, h - FRAME, h, false),
                // Sill board spanning the wall depth.
                part(0.0, w, -half - 0.02, half, -0.02, 0.0, false),
                part(FRAME, w - FRAME, -0.006, 0.006, FRAME, h - FRAME, true),
            ]
        }
        _ => Vec::new(),
    }
}

/// All derived geometry of a project.
#[derive(Debug, Clone, Default)]
pub struct ModelGeometry {
    pub solids: Vec<Solid>,
    pub walls: BTreeMap<ElementId, WallGeom>,
}

impl ModelGeometry {
    pub fn build(project: &Project) -> Self {
        let mut geom = Self::default();
        geom.build_walls(project);
        geom.build_hosted(project);
        geom.build_slabs(project);
        geom
    }

    fn build_walls(&mut self, project: &Project) {
        // Joins are resolved among walls of the same level.
        let mut by_level: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (e, w) in project.walls() {
            if w.length() > 1e-6 {
                by_level.entry(e.level).or_default().push((e, w));
            }
        }
        for walls in by_level.values() {
            // Joins work on the body centerlines (location lines shifted by the location line setting).
            let lines: Vec<WallLine> = walls
                .iter()
                .map(|(e, w)| {
                    let (start, end) = project.wall_axis(e, w);
                    WallLine { start, end, half: project.thickness_of(e) * 0.5 }
                })
                .collect();
            for (i, (e, w)) in walls.iter().enumerate() {
                let base_z = project.wall_base_z(e, w);
                let top_z = project.wall_top_z(e, w);
                let ty = project.element_type(e.type_id);
                let mut layers = Vec::new();
                let mut v = lines[i].half;
                for layer in ty.map(|t| t.layers()).unwrap_or_default() {
                    layers.push((v, v - layer.thickness, layer.material.clone()));
                    v -= layer.thickness;
                }
                let mut g = WallGeom::new(
                    e.id,
                    lines[i],
                    base_z,
                    top_z,
                    resolve_join(&lines, i, true),
                    resolve_join(&lines, i, false),
                    layers,
                );
                let openings = project
                    .hosted_by(e.id)
                    .filter_map(|h| {
                        let host = h.as_hosted()?;
                        let (width, height) = project.element_type(h.type_id)?.opening_size()?;
                        let z0 = base_z + host.sill_height;
                        Some(Opening {
                            element: h.id,
                            u0: host.offset - width * 0.5,
                            u1: host.offset + width * 0.5,
                            z0,
                            z1: z0 + height,
                        })
                    })
                    .collect();
                g.set_openings(openings);
                let color = ty.map_or([200; 3], |t| project.material_color(t.main_material()));
                self.solids.push(g.to_solid(color));
                self.walls.insert(e.id, g);
            }
        }
    }

    fn build_hosted(&mut self, project: &Project) {
        for wall in self.walls.values() {
            for o in &wall.openings {
                let Some(e) = project.element(o.element) else { continue };
                let Some(ty) = project.element_type(e.type_id) else { continue };
                let parts = hosted_parts(&ty.data, o.u1 - o.u0, o.z1 - o.z0, wall.half);
                let mut frame = Solid::new(e.id, e.category(), FRAME_COLOR);
                let mut panel = Solid::new(e.id, e.category(), project.material_color(ty.main_material()));
                for p in &parts {
                    let s = if p.panel { &mut panel } else { &mut frame };
                    let (lo, hi) = (p.min, p.max);
                    s.add_box(
                        wall.origin,
                        wall.dir,
                        wall.normal,
                        o.u0 + lo.x,
                        o.u0 + hi.x,
                        lo.y,
                        hi.y,
                        o.z0 + lo.z,
                        o.z0 + hi.z,
                    );
                }
                for s in [frame, panel] {
                    if !s.faces.is_empty() {
                        self.solids.push(s);
                    }
                }
            }
        }
    }

    fn build_slabs(&mut self, project: &Project) {
        for e in project.elements.values() {
            let Some(ty) = project.element_type(e.type_id) else { continue };
            let level_z = project.level_elevation(e.level);
            let thk = ty.thickness();
            let color = project.material_color(ty.main_material());
            let mut s = Solid::new(e.id, e.category(), color);
            match &e.kind {
                ElementKind::Floor(f) => {
                    let top = level_z + f.height_offset;
                    s.add_prism(&f.boundary, |_| top - thk, |_| top);
                }
                ElementKind::Ceiling(c) => {
                    let bottom = level_z + c.height_offset;
                    s.add_prism(&c.boundary, |_| bottom, |_| bottom + thk);
                }
                ElementKind::Roof(r) => {
                    let fp = polygon::offset(&r.footprint, r.overhang);
                    let base = level_z + r.base_offset;
                    match r.shape {
                        RoofShape::Flat => s.add_prism(&fp, |_| base, |_| base + thk),
                        RoofShape::Shed { slope, direction_deg } => {
                            let a = direction_deg.to_radians();
                            let d = DVec2::new(a.cos(), a.sin());
                            let min = fp.iter().map(|p| p.dot(d)).fold(f64::INFINITY, f64::min);
                            let vertical_thk = thk * (1.0 + slope * slope).sqrt();
                            let bottom = move |p: DVec2| base + slope * (p.dot(d) - min);
                            s.add_prism(&fp, bottom, move |p| bottom(p) + vertical_thk);
                        }
                    }
                }
                _ => continue,
            }
            self.solids.push(s);
        }
    }

    pub fn solids_of(&self, id: ElementId) -> impl Iterator<Item = &Solid> {
        self.solids.iter().filter(move |s| s.element == id)
    }

    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        self.solids.iter().filter_map(Solid::bounds).reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
    }

    /// Volume of an element's main body (frames/glass excluded for openings).
    pub fn volume(&self, id: ElementId) -> f64 {
        if let Some(wall) = self.walls.get(&id) {
            return wall.volume();
        }
        self.solids_of(id).map(Solid::volume).sum()
    }

    /// Closest element hit by `ray`, skipping categories in `hidden`.
    pub fn pick(&self, ray: Ray, hidden: &[Category]) -> Option<(ElementId, f64)> {
        self.solids
            .iter()
            .filter(|s| !hidden.contains(&s.category))
            .filter_map(|s| s.raycast(ray).map(|t| (s.element, t)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::{Document, Element, Floor, Hosted, Wall};

    pub(crate) fn sample_house() -> Document {
        let mut doc = Document::new(Project::from_template());
        doc.edit("house", |e| {
            let level = e.project().levels[0].id;
            let wall_t = e.project().default_type(Category::Wall).unwrap();
            let door_t = e.project().default_type(Category::Door).unwrap();
            let floor_t = e.project().default_type(Category::Floor).unwrap();
            let pts = [DVec2::new(0.0, 0.0), DVec2::new(8.0, 0.0), DVec2::new(8.0, 6.0), DVec2::new(0.0, 6.0)];
            let mut walls = vec![];
            for i in 0..4 {
                let id = ElementId(e.alloc_id());
                walls.push(id);
                e.add_element(Element {
                    id,
                    level,
                    type_id: wall_t,
                    mark: String::new(),
                    comments: String::new(),
                    uid: Default::default(),
                    kind: ElementKind::Wall(Wall {
                        start: pts[i],
                        end: pts[(i + 1) % 4],
                        base_offset: 0.0,
                        height: 3.0,
                        ..Default::default()
                    }),
                });
            }
            let id = ElementId(e.alloc_id());
            e.add_element(Element {
                id,
                level,
                type_id: door_t,
                mark: "1".into(),
                comments: String::new(),
                uid: Default::default(),
                kind: ElementKind::Door(Hosted {
                    host: walls[0],
                    offset: 2.0,
                    sill_height: 0.0,
                    flip_facing: false,
                    flip_hand: false,
                }),
            });
            let id = ElementId(e.alloc_id());
            e.add_element(Element {
                id,
                level,
                type_id: floor_t,
                mark: String::new(),
                comments: String::new(),
                uid: Default::default(),
                kind: ElementKind::Floor(Floor { boundary: pts.to_vec(), height_offset: 0.0 }),
            });
        });
        doc
    }

    #[test]
    fn house_builds_closed_walls() {
        let doc = sample_house();
        let g = ModelGeometry::build(&doc.project);
        assert_eq!(g.walls.len(), 4);
        assert!(g.walls.values().all(|w| w.start_joined && w.end_joined));
        // Mitered rectangle: total wall volume = (outer area - inner area) * height - door.
        let wall_volume: f64 = g.walls.keys().map(|id| g.volume(*id)).sum();
        let ring = 8.15 * 6.15 - 7.85 * 5.85;
        let expected = ring * 3.0 - 0.8 * 2.1 * 0.15;
        assert!((wall_volume - expected).abs() < 1e-6, "{wall_volume} vs {expected}");
        let (lo, hi) = g.bounds().unwrap();
        assert!(lo.z < 0.0 && (hi.z - 3.0).abs() < 1e-9);
    }

    #[test]
    fn each_wall_volume_is_exact() {
        let doc = sample_house();
        let g = ModelGeometry::build(&doc.project);
        let door = 0.8 * 2.1 * 0.15;
        // Mitered corners: each wall is a trapezoid with the centerline length as mean width.
        let expected = [8.0 * 0.15 * 3.0 - door, 6.0 * 0.15 * 3.0, 8.0 * 0.15 * 3.0, 6.0 * 0.15 * 3.0];
        for (id, want) in g.walls.keys().zip(expected) {
            let got = g.volume(*id);
            assert!((got - want).abs() < 1e-9, "{id}: {got} vs {want}");
        }
    }

    #[test]
    fn walls_follow_top_constraint_and_location_line() {
        use cc_core::{LocationLine, WallTop};
        let mut doc = sample_house();
        let wall_id = *ModelGeometry::build(&doc.project).walls.keys().next().unwrap();
        let upper = doc.project.levels[1].clone();
        doc.edit("constrain", |e| {
            if let Some(ElementKind::Wall(w)) = e.element_mut(wall_id).map(|x| &mut x.kind) {
                w.top = WallTop::Level { level: upper.id, offset: -0.5 };
                w.location_line = LocationLine::FinishExterior;
            }
        });
        let g = ModelGeometry::build(&doc.project);
        let wall = &g.walls[&wall_id];
        assert!((wall.top_z - (upper.elevation - 0.5)).abs() < 1e-9);
        // The first wall runs along +X from the origin; its exterior face (left, +Y)
        // now lies on the drawn line, so the body is centered half a thickness to the right.
        assert!((wall.origin.y - -0.075).abs() < 1e-9, "{}", wall.origin.y);

        // Moving the upper level moves the wall top with it.
        doc.edit("raise", |e| e.levels_mut()[1].elevation += 1.0);
        let g = ModelGeometry::build(&doc.project);
        assert!((g.walls[&wall_id].top_z - (upper.elevation + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn pick_hits_wall() {
        let doc = sample_house();
        let g = ModelGeometry::build(&doc.project);
        let ray = Ray { origin: DVec3::new(5.0, -10.0, 1.5), dir: DVec3::Y };
        let (id, t) = g.pick(ray, &[]).unwrap();
        assert_eq!(doc.project.element(id).unwrap().category(), Category::Wall);
        assert!((t - (10.0 - 0.075)).abs() < 1e-9);
    }
}
