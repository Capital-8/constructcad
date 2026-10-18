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
    /// Boundary of each enclosed room (missing when the room is not enclosed).
    pub rooms: BTreeMap<ElementId, Vec<DVec2>>,
    /// Ridge and hip lines of pitched roofs, in plan.
    pub roof_ridges: BTreeMap<ElementId, Vec<(DVec2, DVec2)>>,
}

/// An axis-aligned box in a horizontal frame: `origin`, plan axes `u`/`v`
/// (unit, perpendicular) and extents along them and in absolute Z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxPart {
    pub origin: DVec2,
    pub u: DVec2,
    pub v: DVec2,
    pub u_range: (f64, f64),
    pub v_range: (f64, f64),
    pub z_range: (f64, f64),
}

impl BoxPart {
    pub fn add_to(&self, s: &mut Solid) {
        let (u, v, z) = (self.u_range, self.v_range, self.z_range);
        s.add_box(self.origin, self.u, self.v, u.0, u.1, v.0, v.1, z.0, z.1);
    }
}

/// Layout of a straight stair run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StairGeom {
    pub start: DVec2,
    /// Up direction of the run and the perpendicular across it.
    pub u: DVec2,
    pub v: DVec2,
    pub width: f64,
    pub risers: usize,
    pub riser: f64,
    pub tread: f64,
    pub base: f64,
    pub top: f64,
}

impl StairGeom {
    pub fn run(&self) -> f64 {
        self.tread * self.risers.saturating_sub(1) as f64
    }

    /// One box per tread, each from the base up to that tread's surface.
    pub fn steps(&self) -> Vec<BoxPart> {
        let w = self.width / 2.0;
        (0..self.risers.saturating_sub(1))
            .map(|k| BoxPart {
                origin: self.start,
                u: self.u,
                v: self.v,
                u_range: (k as f64 * self.tread, (k + 1) as f64 * self.tread),
                v_range: (-w, w),
                z_range: (self.base, self.base + (k + 1) as f64 * self.riser),
            })
            .collect()
    }

    /// Plan outline (counter-clockwise).
    pub fn outline(&self) -> [DVec2; 4] {
        let (w, l) = (self.width / 2.0, self.run());
        let p = |a: f64, b: f64| self.start + self.u * a + self.v * b;
        [p(0.0, -w), p(l, -w), p(l, w), p(0.0, w)]
    }
}

/// Layout of a stair element from its type and constraints.
pub fn stair_geom(project: &Project, element: &cc_core::Element, stair: &cc_core::Stair) -> Option<StairGeom> {
    let TypeData::Stair { width, max_riser, tread, .. } = project.element_type(element.type_id)?.data else {
        return None;
    };
    let (base, top) = project.stair_span(element, stair);
    let (risers, riser, _) = cc_core::stair_layout(top - base, max_riser, tread);
    let u = DVec2::from_angle(stair.direction_deg.to_radians());
    Some(StairGeom { start: stair.start, u, v: u.perp(), width, risers, riser, tread, base, top })
}

/// Boxes of a railing: a top rail along each segment and posts at the path
/// vertices and every `post_spacing` along it.
pub fn railing_parts(project: &Project, element: &cc_core::Element, railing: &cc_core::Railing) -> Vec<BoxPart> {
    let Some(TypeData::Railing { height, post_spacing, .. }) = project.element_type(element.type_id).map(|t| &t.data)
    else {
        return Vec::new();
    };
    let base = project.level_elevation(element.level) + railing.base_offset;
    let (rail, post) = (0.05, 0.04);
    let mut parts = Vec::new();
    for seg in railing.path.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let len = a.distance(b);
        let Some(u) = (b - a).try_normalize() else { continue };
        let v = u.perp();
        parts.push(BoxPart {
            origin: a,
            u,
            v,
            u_range: (0.0, len),
            v_range: (-rail / 2.0, rail / 2.0),
            z_range: (base + height - rail, base + height),
        });
        let count = (len / post_spacing.max(0.1)).ceil().max(1.0) as usize;
        for k in 0..count.min(1000) {
            let at = len * k as f64 / count as f64;
            parts.push(BoxPart {
                origin: a,
                u,
                v,
                u_range: (at - post / 2.0, at + post / 2.0),
                v_range: (-post / 2.0, post / 2.0),
                z_range: (base, base + height - rail),
            });
        }
    }
    // Closing post at the end of the path.
    if let [.., a, b] = railing.path.as_slice()
        && let Some(u) = (*b - *a).try_normalize()
    {
        parts.push(BoxPart {
            origin: *b,
            u,
            v: u.perp(),
            u_range: (-post / 2.0, post / 2.0),
            v_range: (-post / 2.0, post / 2.0),
            z_range: (base, base + height - rail),
        });
    }
    parts
}

/// Plan outline of a column section (counter-clockwise).
pub fn column_outline(section: cc_core::ColumnSection, position: DVec2, rotation_deg: f64) -> Vec<DVec2> {
    let u = DVec2::from_angle(rotation_deg.to_radians());
    let v = u.perp();
    match section {
        cc_core::ColumnSection::Rectangular { width, depth } => {
            let (w, d) = (width / 2.0, depth / 2.0);
            [(-w, -d), (w, -d), (w, d), (-w, d)].iter().map(|&(a, b)| position + u * a + v * b).collect()
        }
        cc_core::ColumnSection::Round { diameter } => (0..24)
            .map(|i| position + DVec2::from_angle(i as f64 / 24.0 * std::f64::consts::TAU) * (diameter / 2.0))
            .collect(),
    }
}

/// Pieces of a placed component in world coordinates: footprints rotated
/// and moved to the insertion point, elevations absolute. Pieces whose
/// expressions fail are left out (the component editor reports them).
pub fn component_parts(
    project: &Project,
    element: &cc_core::Element,
    component: &cc_core::Component,
) -> Vec<cc_core::component::ComponentPart> {
    let Some(TypeData::Component { def, .. }) = project.element_type(element.type_id).map(|t| &t.data) else {
        return Vec::new();
    };
    let rot = DVec2::from_angle(component.rotation_deg.to_radians());
    let z = project.level_elevation(element.level) + component.base_offset;
    let (parts, _) = def.resolve(&component.overrides);
    parts
        .into_iter()
        .map(|mut p| {
            p.footprint = p.footprint.iter().map(|q| component.position + rot.rotate(*q)).collect();
            p.z0 += z;
            p.z1 += z;
            p
        })
        .filter(|p| p.footprint.iter().all(|q| q.is_finite()) && p.z0.is_finite() && p.z1.is_finite())
        .collect()
}

impl ModelGeometry {
    pub fn build(project: &Project) -> Self {
        let mut geom = Self::default();
        geom.build_walls(project);
        geom.build_hosted(project);
        geom.build_slabs(project);
        geom.build_structure(project);
        geom.build_components(project);
        geom.build_rooms(project);
        geom
    }

    /// One solid per resolved piece of each component, colored by its material.
    fn build_components(&mut self, project: &Project) {
        for e in project.elements.values() {
            let ElementKind::Component(c) = &e.kind else { continue };
            for part in component_parts(project, e, c) {
                let mut s = Solid::of_material(e.id, e.category(), project, &part.material);
                s.add_prism(&part.footprint, |_| part.z0, |_| part.z1);
                if !s.faces.is_empty() {
                    self.solids.push(s);
                }
            }
        }
    }

    /// Columns, beams, stairs and railings.
    fn build_structure(&mut self, project: &Project) {
        for e in project.elements.values() {
            let Some(ty) = project.element_type(e.type_id) else { continue };
            let mut s = Solid::of_material(e.id, e.category(), project, ty.main_material());
            match (&e.kind, &ty.data) {
                (ElementKind::Column(c), TypeData::Column { section, .. }) => {
                    let (base, top) = project.column_span(e, c);
                    s.add_prism(&column_outline(*section, c.position, c.rotation_deg), |_| base, |_| top);
                }
                (ElementKind::Beam(b), TypeData::Beam { width, height, .. }) => {
                    let Some(u) = (b.end - b.start).try_normalize() else { continue };
                    let top = project.level_elevation(e.level) + b.top_offset;
                    BoxPart {
                        origin: b.start,
                        u,
                        v: u.perp(),
                        u_range: (0.0, b.length()),
                        v_range: (-width / 2.0, width / 2.0),
                        z_range: (top - height, top),
                    }
                    .add_to(&mut s);
                }
                (ElementKind::Stair(st), TypeData::Stair { .. }) => {
                    let Some(g) = stair_geom(project, e, st) else { continue };
                    g.steps().iter().for_each(|p| p.add_to(&mut s));
                }
                (ElementKind::Railing(r), TypeData::Railing { .. }) => {
                    railing_parts(project, e, r).iter().for_each(|p| p.add_to(&mut s));
                }
                _ => continue,
            }
            if !s.faces.is_empty() {
                self.solids.push(s);
            }
        }
    }

    /// Footprints of the room-bounding walls of `level`.
    pub fn bounding_footprints(&self, project: &Project, level: cc_core::LevelId) -> Vec<Vec<DVec2>> {
        project
            .walls()
            .filter(|(e, w)| e.level == level && w.room_bounding)
            .filter_map(|(e, _)| self.walls.get(&e.id))
            .map(|g| g.footprint().to_vec())
            .collect()
    }

    /// Boundary of the room that would enclose `point` on `level`.
    pub fn room_at(&self, project: &Project, level: cc_core::LevelId, point: DVec2) -> Option<Vec<DVec2>> {
        crate::room_boundary(&self.bounding_footprints(project, level), point)
    }

    fn build_rooms(&mut self, project: &Project) {
        let mut footprints: BTreeMap<cc_core::LevelId, Vec<Vec<DVec2>>> = BTreeMap::new();
        for e in project.elements.values() {
            let ElementKind::Room(room) = &e.kind else { continue };
            let walls = footprints.entry(e.level).or_insert_with(|| self.bounding_footprints(project, e.level));
            if let Some(boundary) = crate::room_boundary(walls, room.position) {
                self.rooms.insert(e.id, boundary);
            }
        }
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
                let material = ty.map_or("", |t| t.main_material());
                let mut solid = g.to_solid(project.material_color(material));
                solid.transparency = project.material_transparency(material);
                self.solids.push(solid);
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
                let mut panel = Solid::of_material(e.id, e.category(), project, ty.main_material());
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
            let mut s = Solid::of_material(e.id, e.category(), project, ty.main_material());
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
                        RoofShape::Gable { slope, .. } | RoofShape::Hip { slope } => {
                            let fp = polygon::ccw(&fp);
                            let ridge = match r.shape {
                                RoofShape::Gable { ridge_deg, .. } => Some(DVec2::from_angle(ridge_deg.to_radians())),
                                _ => None,
                            };
                            let eaves = crate::roofs::sloped_eaves(&fp, ridge);
                            if eaves.is_empty() {
                                s.add_prism(&fp, |_| base, |_| base + thk);
                            } else {
                                let vertical_thk = thk * (1.0 + slope * slope).sqrt();
                                crate::roofs::add_pitched_roof(&mut s, &fp, &eaves, slope, base, vertical_thk);
                                let regions = crate::roofs::roof_regions(&fp, &eaves);
                                self.roof_ridges.insert(e.id, crate::roofs::ridge_lines(&fp, &regions));
                            }
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
    fn stairs_columns_and_beams_have_exact_volumes() {
        use cc_core::{Beam, Column, Stair, WallTop};
        let mut doc = Document::new(Project::from_template());
        let (stair, column, beam) = doc.edit("structure", |e| {
            let level = e.project().levels[0].id;
            let upper = e.project().levels[1].id;
            let add = |e: &mut cc_core::EditCtx, category, kind| {
                let type_id = e.project().default_type(category).unwrap();
                let id = ElementId(e.alloc_id());
                let mark = String::new();
                e.add_element(Element {
                    id,
                    level,
                    type_id,
                    mark,
                    comments: String::new(),
                    kind,
                    uid: Default::default(),
                })
            };
            let top = WallTop::Level { level: upper, offset: 0.0 };
            let stair = add(
                e,
                Category::Stair,
                ElementKind::Stair(Stair {
                    start: DVec2::ZERO,
                    direction_deg: 0.0,
                    base_offset: 0.0,
                    height: 3.0,
                    top,
                }),
            );
            let column = add(
                e,
                Category::Column,
                ElementKind::Column(Column {
                    position: DVec2::ZERO,
                    rotation_deg: 30.0,
                    base_offset: 0.0,
                    height: 3.0,
                    top,
                }),
            );
            let beam = add(
                e,
                Category::Beam,
                ElementKind::Beam(Beam { start: DVec2::ZERO, end: DVec2::new(3.0, 4.0), top_offset: 3.0 }),
            );
            (stair, column, beam)
        });
        let g = ModelGeometry::build(&doc.project);
        // 3.00 m at most 0.18 per riser: 17 risers of 3/17 and 16 treads of 0.28.
        let stair_el = doc.project.element(stair).unwrap();
        let ElementKind::Stair(st) = &stair_el.kind else { panic!("not a stair") };
        let sg = stair_geom(&doc.project, stair_el, st).unwrap();
        assert_eq!(sg.risers, 17);
        assert!((sg.run() - 16.0 * 0.28).abs() < 1e-12);
        let expected = 0.28 * 1.0 * (3.0 / 17.0) * (1..=16).sum::<usize>() as f64;
        assert!((g.volume(stair) - expected).abs() < 1e-9, "{} vs {expected}", g.volume(stair));
        assert!((g.volume(column) - 0.2 * 0.2 * 3.0).abs() < 1e-9);
        assert!((g.volume(beam) - 0.15 * 0.4 * 5.0).abs() < 1e-9);
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
