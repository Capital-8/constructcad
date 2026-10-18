//! IFC import: maps the spatial structure and building elements of an IFC
//! file (IFC2X3 or IFC4) onto a ConstructCAD project.
//!
//! Geometry is read from extruded area solids (the usual representation of
//! walls, slabs, columns and beams), with faceted B-reps, face sets and
//! bounding boxes used for extents. Storeys become levels; walls, doors,
//! windows, slabs, ceilings, roofs, spaces, columns and beams become native
//! elements with matching or new types. Products that cannot be interpreted
//! are counted in the report instead of failing the import.

use crate::guid;
use crate::parse::{Entity, Param, ParseError, StepFile, parse};
use cc_core::{
    Beam, Category, Ceiling, Column, ColumnSection, Component, ComponentDef, ComponentItem, ComponentKind,
    ComponentShape, Element, ElementId, ElementKind, ElementType, ElementTypeId, Floor, Hosted, Layer, LayerFunction,
    Level, LevelId, Material, Project, Roof, RoofShape, Room, TypeData, Uid, View, ViewId, ViewKind, Wall,
    WallFunction, WallTop,
};
use cc_geom::polygon;
use glam::{DAffine3, DMat3, DVec2, DVec3};
use std::collections::{BTreeMap, HashMap};

const MAX_PLACEMENT_DEPTH: usize = 64;
const MAX_ITEM_DEPTH: usize = 16;
const MAX_PARENT_DEPTH: usize = 16;
/// Length tolerance when matching types (meters).
const TYPE_TOL: f64 = 0.001;

/// What an import brought in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportReport {
    /// IFC schema of the file (`IFC4`, `IFC2X3`…).
    pub schema: String,
    pub levels: usize,
    pub imported: BTreeMap<Category, usize>,
    /// Products that were not imported, by IFC entity name.
    pub skipped: BTreeMap<String, usize>,
}

impl ImportReport {
    pub fn imported_total(&self) -> usize {
        self.imported.values().sum()
    }

    pub fn skipped_total(&self) -> usize {
        self.skipped.values().sum()
    }
}

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    Parse(ParseError),
    /// The file has no `IFCPROJECT`.
    NotIfc,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Io(e) => write!(f, "{e}"),
            ImportError::Parse(e) => write!(f, "{e}"),
            ImportError::NotIfc => write!(f, "no IFCPROJECT found"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Import the IFC file at `path` into `base` (see [`import`]).
pub fn import_file(path: &std::path::Path, base: Project) -> Result<(Project, ImportReport), ImportError> {
    let bytes = std::fs::read(path).map_err(ImportError::Io)?;
    import(&String::from_utf8_lossy(&bytes), base)
}

/// Import IFC `text`. `base` supplies materials, types and project-wide
/// views (usually the default template); its levels, elements, plans and
/// sheets are replaced by the imported building.
pub fn import(text: &str, base: Project) -> Result<(Project, ImportReport), ImportError> {
    let file = parse(text).map_err(ImportError::Parse)?;
    let Some((project_id, _)) = file.of_type("IFCPROJECT").next() else { return Err(ImportError::NotIfc) };
    let mut im = Importer::new(&file, base, project_id);
    im.run();
    Ok((im.p, im.report))
}

/// An extruded profile in its plane: the polygon and, for parametric
/// profiles, its exact parameters.
#[derive(Debug, Clone)]
struct Profile {
    pts: Vec<DVec2>,
    kind: ProfileKind,
}

#[derive(Debug, Clone, Copy)]
enum ProfileKind {
    /// Rectangle `w` x `h` centered at `center`, with its width along `x`.
    Rect {
        center: DVec2,
        x: DVec2,
        w: f64,
        h: f64,
    },
    Circle {
        center: DVec2,
        r: f64,
    },
    Poly,
}

impl Profile {
    /// Exact rectangle or circle, else the bounding rectangle of the polygon.
    fn rect(&self) -> (DVec2, DVec2, f64, f64) {
        match self.kind {
            ProfileKind::Rect { center, x, w, h } => (center, x, w, h),
            ProfileKind::Circle { center, r } => (center, DVec2::X, 2.0 * r, 2.0 * r),
            ProfileKind::Poly => {
                let (lo, hi) = bounds2(&self.pts);
                ((lo + hi) * 0.5, DVec2::X, hi.x - lo.x, hi.y - lo.y)
            }
        }
    }
}

/// A body item resolved to world coordinates.
#[derive(Debug, Clone)]
struct Solid {
    /// Extruded profile, when the item is an extrusion.
    extrusion: Option<Extrusion>,
    /// World points of the item (both caps of an extrusion; B-rep vertices).
    points: Vec<DVec3>,
}

#[derive(Debug, Clone)]
struct Extrusion {
    profile: Profile,
    /// Profile plane to world.
    xf: DAffine3,
    /// Extrusion vector in world coordinates (direction times depth).
    vector: DVec3,
}

impl Extrusion {
    fn is_vertical(&self) -> bool {
        self.vector.length() > 1e-9 && self.vector.normalize().z.abs() > 0.999
    }

    /// World XY of the profile polygon (the bottom cap of a vertical extrusion).
    fn footprint(&self) -> Vec<DVec2> {
        self.profile.pts.iter().map(|p| self.xf.transform_point3(p.extend(0.0)).truncate()).collect()
    }

    fn plane_point(&self, p: DVec2) -> DVec3 {
        self.xf.transform_point3(p.extend(0.0))
    }

    fn plane_vector(&self, v: DVec2) -> DVec3 {
        self.xf.transform_vector3(v.extend(0.0))
    }
}

/// Body and axis of a product, in world coordinates.
#[derive(Debug, Clone, Default)]
struct Body {
    solids: Vec<Solid>,
    axis: Option<(DVec2, DVec2)>,
}

impl Body {
    fn points(&self) -> impl Iterator<Item = DVec3> + '_ {
        self.solids.iter().flat_map(|s| s.points.iter().copied())
    }

    fn z_range(&self) -> Option<(f64, f64)> {
        let mut it = self.points().map(|p| p.z).peekable();
        it.peek()?;
        Some(it.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), z| (lo.min(z), hi.max(z))))
    }

    /// The first extrusion (vertical ones preferred when `vertical`).
    fn extrusion(&self, vertical: bool) -> Option<&Extrusion> {
        let mut all = self.solids.iter().filter_map(|s| s.extrusion.as_ref());
        if vertical { all.find(|e| e.is_vertical()) } else { all.next() }
    }
}

struct Importer<'a> {
    f: &'a StepFile,
    p: Project,
    report: ImportReport,
    project_id: u32,
    /// Meters per file length unit.
    scale: f64,
    storeys: HashMap<u32, LevelId>,
    /// Product → its containing structure or aggregating whole.
    parent: HashMap<u32, u32>,
    type_of: HashMap<u32, u32>,
    material_of: HashMap<u32, u32>,
    psets_of: HashMap<u32, Vec<u32>>,
    /// Opening → host element; filler → opening.
    host_of_opening: HashMap<u32, u32>,
    opening_of_filler: HashMap<u32, u32>,
    /// Surface style colors by style name and by styled item.
    style_colors: HashMap<String, [u8; 3]>,
    item_colors: HashMap<u32, [u8; 3]>,
    /// Imported walls by IFC instance.
    walls: HashMap<u32, ElementId>,
    /// Products with parts aggregated into them (e.g. roofs made of slabs).
    has_parts: HashMap<u32, usize>,
}

fn bounds2(pts: &[DVec2]) -> (DVec2, DVec2) {
    pts.iter()
        .fold((DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
}

/// Convex hull (counter-clockwise), for footprints of B-rep bodies.
fn convex_hull(points: &[DVec2]) -> Vec<DVec2> {
    let mut pts: Vec<DVec2> = points.iter().copied().filter(|p| p.is_finite()).collect();
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| a.distance(*b) < 1e-9);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: DVec2, a: DVec2, b: DVec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<DVec2> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &DVec2>> =
            if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
        for &p in iter {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 1e-12 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// Drop a repeated closing point and consecutive duplicates.
fn clean_polygon(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    pts.dedup_by(|a, b| a.distance(*b) < 1e-9);
    if pts.len() > 1 && pts.first().zip(pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-9) {
        pts.pop();
    }
    pts
}

/// Type names from other tools often end in an instance number ("Wall:Generic 200mm:123456").
fn clean_name(name: &str) -> String {
    let name = name.trim();
    match name.rsplit_once(':') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => head.to_string(),
        _ => name.to_string(),
    }
}

fn affine(x: DVec3, y: DVec3, z: DVec3, origin: DVec3) -> DAffine3 {
    DAffine3 { matrix3: DMat3::from_cols(x, y, z), translation: origin }
}

/// A right-handed frame from a Z axis and an approximate X axis.
fn frame(z: Option<DVec3>, x: Option<DVec3>, origin: DVec3) -> DAffine3 {
    let z = z.and_then(|z| z.try_normalize()).unwrap_or(DVec3::Z);
    let fallback = if z.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let x = x
        .and_then(|x| (x - z * x.dot(z)).try_normalize())
        .or_else(|| (fallback - z * fallback.dot(z)).try_normalize())
        .unwrap_or(DVec3::X);
    affine(x, z.cross(x), z, origin)
}

impl<'a> Importer<'a> {
    fn new(f: &'a StepFile, base: Project, project_id: u32) -> Self {
        let mut im = Self {
            f,
            p: base,
            report: ImportReport { schema: f.schema.clone(), ..Default::default() },
            project_id,
            scale: 1.0,
            storeys: HashMap::new(),
            parent: HashMap::new(),
            type_of: HashMap::new(),
            material_of: HashMap::new(),
            psets_of: HashMap::new(),
            host_of_opening: HashMap::new(),
            opening_of_filler: HashMap::new(),
            style_colors: HashMap::new(),
            item_colors: HashMap::new(),
            walls: HashMap::new(),
            has_parts: HashMap::new(),
        };
        im.scale = im.length_unit();
        im.index_relationships();
        im.index_styles();
        im
    }

    fn run(&mut self) {
        self.prepare_base();
        self.read_project_info();
        self.read_storeys();
        let f = self.f;
        let mut handled = std::collections::HashSet::new();
        // Walls first: doors and windows need their hosts.
        for (id, e) in f.entities.iter() {
            let ok = match e.name.as_str() {
                "IFCWALL" | "IFCWALLSTANDARDCASE" | "IFCWALLELEMENTEDCASE" => Some(self.import_wall(*id, e)),
                _ => None,
            };
            if let Some(ok) = ok {
                handled.insert(*id);
                if !ok {
                    self.skip(e);
                }
            }
        }
        for (id, e) in f.entities.iter() {
            let ok = match e.name.as_str() {
                "IFCDOOR" | "IFCDOORSTANDARDCASE" => Some(self.import_filler(*id, e, true)),
                "IFCWINDOW" | "IFCWINDOWSTANDARDCASE" => Some(self.import_filler(*id, e, false)),
                "IFCSLAB" | "IFCSLABSTANDARDCASE" | "IFCSLABELEMENTEDCASE" => Some(self.import_slab(*id, e)),
                "IFCCOVERING" => Some(self.import_covering(*id, e)),
                "IFCROOF" => Some(self.import_roof(*id, e)),
                "IFCSPACE" => Some(self.import_space(*id, e)),
                "IFCCOLUMN" | "IFCCOLUMNSTANDARDCASE" => Some(self.import_column(*id, e)),
                "IFCBEAM" | "IFCBEAMSTANDARDCASE" => Some(self.import_beam(*id, e)),
                "IFCFURNITURE" | "IFCFURNISHINGELEMENT" | "IFCSYSTEMFURNITUREELEMENT" => {
                    Some(self.import_component(*id, e, ComponentKind::Furniture))
                }
                "IFCSANITARYTERMINAL" => Some(self.import_component(*id, e, ComponentKind::Plumbing)),
                "IFCLIGHTFIXTURE" => Some(self.import_component(*id, e, ComponentKind::Lighting)),
                "IFCBUILDINGELEMENTPROXY" | "IFCFLOWTERMINAL" => {
                    Some(self.import_component(*id, e, ComponentKind::Generic))
                }
                _ => None,
            };
            if let Some(ok) = ok {
                handled.insert(*id);
                if !ok {
                    self.skip(e);
                }
            }
        }
        // Any other placed product with a shape is reported as skipped.
        for (id, e) in f.entities.iter() {
            if handled.contains(id)
                || matches!(
                    e.name.as_str(),
                    "IFCOPENINGELEMENT" | "IFCSITE" | "IFCBUILDING" | "IFCBUILDINGSTOREY" | "IFCANNOTATION" | "IFCGRID"
                )
            {
                continue;
            }
            let placed = f.deref(e.arg(5)).is_some_and(|p| p.name == "IFCLOCALPLACEMENT");
            let shaped = f.deref(e.arg(6)).is_some_and(|s| s.name == "IFCPRODUCTDEFINITIONSHAPE");
            if placed && shaped {
                self.skip(e);
            }
        }
        self.finish();
    }

    fn skip(&mut self, e: &Entity) {
        *self.report.skipped.entry(e.name.clone()).or_default() += 1;
    }

    // --- Setup ---

    /// Clear what the import replaces: levels, elements, plans and sheets.
    fn prepare_base(&mut self) {
        self.p.elements.clear();
        self.p.levels.clear();
        self.p.views.retain(|_, v| {
            !matches!(v.kind, ViewKind::FloorPlan { .. } | ViewKind::CeilingPlan { .. } | ViewKind::Sheet { .. })
        });
    }

    fn read_project_info(&mut self) {
        let Some(e) = self.f.get(self.project_id) else { return };
        if let Some(name) = e.arg(2).as_str().filter(|s| !s.is_empty()) {
            self.p.info.name = name.to_string();
        }
        self.p.info.number = e.arg(5).as_str().unwrap_or_default().to_string();
        self.p.info.uid = e.arg(0).as_str().and_then(guid::decode).map(Uid).unwrap_or_default();
        self.p.info.client.clear();
        self.p.info.author.clear();
        self.p.info.address.clear();
        let address = self.f.of_type("IFCBUILDING").find_map(|(_, b)| self.f.deref(b.arg(11)));
        if let Some(lines) = address.map(|a| a.arg(4).as_list()) {
            let lines: Vec<&str> = lines.iter().filter_map(Param::as_str).collect();
            self.p.info.address = lines.join(", ");
        }
    }

    /// Meters per length unit of the file.
    fn length_unit(&self) -> f64 {
        let f = self.f;
        let assignment = f
            .get(self.project_id)
            .and_then(|p| f.deref(p.arg(8)))
            .or_else(|| f.of_type("IFCUNITASSIGNMENT").next().map(|(_, e)| e));
        let Some(units) = assignment else { return 1.0 };
        for unit in units.arg(0).as_list().iter().filter_map(|u| f.deref(u)) {
            if unit.arg(1).as_enum() != Some("LENGTHUNIT") {
                continue;
            }
            match unit.name.as_str() {
                "IFCSIUNIT" => return si_prefix(unit.arg(2).as_enum()),
                "IFCCONVERSIONBASEDUNIT" => {
                    let Some(measure) = f.deref(unit.arg(3)) else { continue };
                    let value = measure.arg(0).as_f64().unwrap_or(1.0);
                    let base = f.deref(measure.arg(1)).map_or(1.0, |u| si_prefix(u.arg(2).as_enum()));
                    let factor = value * base;
                    if factor.is_finite() && factor > 0.0 {
                        return factor;
                    }
                }
                _ => {}
            }
        }
        1.0
    }

    fn index_relationships(&mut self) {
        let f = self.f;
        let refs = |p: &Param| p.as_list().iter().filter_map(Param::as_ref).collect::<Vec<_>>();
        for e in f.entities.values() {
            match e.name.as_str() {
                "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                    let Some(whole) = e.arg(5).as_ref() else { continue };
                    for part in refs(e.arg(4)) {
                        self.parent.insert(part, whole);
                    }
                }
                "IFCRELAGGREGATES" | "IFCRELNESTS" => {
                    let Some(whole) = e.arg(4).as_ref() else { continue };
                    for part in refs(e.arg(5)) {
                        self.parent.entry(part).or_insert(whole);
                        *self.has_parts.entry(whole).or_default() += 1;
                    }
                }
                "IFCRELDEFINESBYTYPE" => {
                    let Some(ty) = e.arg(5).as_ref() else { continue };
                    for object in refs(e.arg(4)) {
                        self.type_of.insert(object, ty);
                    }
                }
                "IFCRELASSOCIATESMATERIAL" => {
                    let Some(material) = e.arg(5).as_ref() else { continue };
                    for object in refs(e.arg(4)) {
                        self.material_of.insert(object, material);
                    }
                }
                "IFCRELDEFINESBYPROPERTIES" => {
                    let Some(set) = e.arg(5).as_ref() else { continue };
                    for object in refs(e.arg(4)) {
                        self.psets_of.entry(object).or_default().push(set);
                    }
                }
                "IFCRELVOIDSELEMENT" => {
                    if let (Some(host), Some(opening)) = (e.arg(4).as_ref(), e.arg(5).as_ref()) {
                        self.host_of_opening.insert(opening, host);
                    }
                }
                "IFCRELFILLSELEMENT" => {
                    if let (Some(opening), Some(filler)) = (e.arg(4).as_ref(), e.arg(5).as_ref()) {
                        self.opening_of_filler.insert(filler, opening);
                    }
                }
                _ => {}
            }
        }
        // Type objects can also carry their property sets directly.
        for (id, e) in f.entities.iter() {
            if e.name.ends_with("TYPE") || e.name.ends_with("STYLE") {
                let sets = refs(e.arg(5));
                if !sets.is_empty() && sets.iter().all(|s| f.get(*s).is_some_and(|x| x.name == "IFCPROPERTYSET")) {
                    self.psets_of.entry(*id).or_default().extend(sets);
                }
            }
        }
    }

    fn index_styles(&mut self) {
        let f = self.f;
        let color_of_style = |style: &Entity| -> Option<[u8; 3]> {
            for s in style.arg(2).as_list().iter().filter_map(|s| f.deref(s)) {
                if s.name.starts_with("IFCSURFACESTYLE") {
                    let rgb = f.deref(s.arg(0))?;
                    let c = |i| (rgb.arg(i).as_f64().unwrap_or(0.8).clamp(0.0, 1.0) * 255.0).round() as u8;
                    return Some([c(1), c(2), c(3)]);
                }
            }
            None
        };
        for (_, style) in f.of_type("IFCSURFACESTYLE") {
            if let (Some(name), Some(color)) = (style.arg(0).as_str(), color_of_style(style)) {
                self.style_colors.entry(name.to_string()).or_insert(color);
            }
        }
        for (_, styled) in f.of_type("IFCSTYLEDITEM") {
            let Some(item) = styled.arg(0).as_ref() else { continue };
            // IFC2X3 wraps styles in presentation style assignments.
            let mut styles: Vec<&Entity> = Vec::new();
            for s in styled.arg(1).as_list().iter().filter_map(|s| f.deref(s)) {
                if s.name == "IFCPRESENTATIONSTYLEASSIGNMENT" {
                    styles.extend(s.arg(0).as_list().iter().filter_map(|x| f.deref(x)));
                } else {
                    styles.push(s);
                }
            }
            if let Some(color) = styles.iter().filter(|s| s.name == "IFCSURFACESTYLE").find_map(|s| color_of_style(s)) {
                self.item_colors.insert(item, color);
            }
        }
    }

    fn read_storeys(&mut self) {
        let f = self.f;
        let mut storeys: Vec<(u32, String, f64)> = Vec::new();
        for (id, e) in f.of_type("IFCBUILDINGSTOREY") {
            let world = self.placement(e.arg(5), 0);
            let attr = e.arg(9).as_f64().map(|z| z * self.scale);
            let z = world.translation.z;
            // Prefer the placement (consistent with element coordinates).
            let elevation = if z.abs() < 1e-9 { attr.unwrap_or(0.0) } else { z };
            let name = e.arg(2).as_str().or(e.arg(7).as_str()).unwrap_or_default().to_string();
            storeys.push((id, name, elevation));
        }
        storeys.sort_by(|a, b| a.2.total_cmp(&b.2));
        for (i, (id, name, elevation)) in storeys.into_iter().enumerate() {
            let level = LevelId(self.p.alloc_id());
            let name = if name.is_empty() { format!("{i}") } else { name };
            self.p.levels.push(Level { id: level, name, elevation });
            self.storeys.insert(id, level);
        }
        if self.p.levels.is_empty() {
            let level = LevelId(self.p.alloc_id());
            self.p.levels.push(Level { id: level, name: "0".into(), elevation: 0.0 });
        }
        self.report.levels = self.p.levels.len();
    }

    fn finish(&mut self) {
        let levels: Vec<(LevelId, String)> = self.p.levels.iter().map(|l| (l.id, l.name.clone())).collect();
        let add_view = |p: &mut Project, name: &str, kind: ViewKind| {
            let id = ViewId(p.alloc_id());
            p.views.insert(id, View { id, name: name.to_string(), scale: 50, kind });
        };
        for (level, name) in &levels {
            add_view(&mut self.p, name, ViewKind::FloorPlan { level: *level, cut_height: 1.2 });
        }
        for (level, name) in &levels {
            add_view(&mut self.p, name, ViewKind::CeilingPlan { level: *level, cut_height: 1.2 });
        }
        self.p.ensure_annotation_types();
        self.p.ensure_uids();
        self.p.repair();
    }

    // --- Lookups ---

    fn storey_of(&self, mut id: u32) -> Option<LevelId> {
        for _ in 0..MAX_PARENT_DEPTH {
            id = *self.parent.get(&id)?;
            if let Some(level) = self.storeys.get(&id) {
                return Some(*level);
            }
        }
        None
    }

    /// Level of a product: its storey, else the highest level at or below `z`.
    fn level_for(&self, id: u32, z: f64) -> (LevelId, f64) {
        let level = self.storey_of(id).or_else(|| {
            let below = self.p.levels.iter().filter(|l| l.elevation <= z + 1e-3);
            below.max_by(|a, b| a.elevation.total_cmp(&b.elevation)).map(|l| l.id)
        });
        let level = level.or_else(|| self.p.levels.first().map(|l| l.id)).unwrap_or(LevelId(0));
        (level, self.p.level_elevation(level))
    }

    fn type_entity(&self, id: u32) -> Option<&'a Entity> {
        self.type_of.get(&id).and_then(|t| self.f.get(*t))
    }

    /// Type name: the IFC type object's, else the product's name.
    fn type_name(&self, id: u32, e: &Entity) -> Option<String> {
        self.type_entity(id)
            .and_then(|t| t.arg(2).as_str())
            .or(e.arg(2).as_str())
            .map(clean_name)
            .filter(|s| !s.is_empty())
    }

    /// A property value from the product's (or its type's) property sets.
    fn property(&self, id: u32, set: &str, name: &str) -> Option<&'a Param> {
        let f = self.f;
        let mut owners = vec![id];
        owners.extend(self.type_of.get(&id));
        for owner in owners {
            for set_id in self.psets_of.get(&owner).into_iter().flatten() {
                let Some(pset) = f.get(*set_id).filter(|s| s.name == "IFCPROPERTYSET") else { continue };
                if pset.arg(2).as_str() != Some(set) {
                    continue;
                }
                for prop in pset.arg(4).as_list().iter().filter_map(|p| f.deref(p)) {
                    if prop.name == "IFCPROPERTYSINGLEVALUE" && prop.arg(0).as_str() == Some(name) {
                        return Some(prop.arg(2));
                    }
                }
            }
        }
        None
    }

    /// Numeric properties of a property set of the product.
    fn numeric_properties(&self, id: u32, set: &str) -> Vec<(String, f64, Option<String>)> {
        let f = self.f;
        let mut out = Vec::new();
        for set_id in self.psets_of.get(&id).into_iter().flatten() {
            let Some(pset) = f.get(*set_id).filter(|s| s.name == "IFCPROPERTYSET" && s.arg(2).as_str() == Some(set))
            else {
                continue;
            };
            for prop in pset.arg(4).as_list().iter().filter_map(|p| f.deref(p)) {
                let (Some(name), Some(v)) = (prop.arg(0).as_str(), prop.arg(2).as_f64()) else { continue };
                let measure = match prop.arg(2) {
                    Param::Typed(t, _) => Some(t.clone()),
                    _ => None,
                };
                out.push((name.to_string(), v, measure));
            }
        }
        out
    }

    /// Material layers (name, thickness) and the main material name of a
    /// product or its type.
    fn materials(&self, id: u32) -> (Vec<(String, f64)>, Option<String>) {
        let f = self.f;
        let mut owners = vec![id];
        owners.extend(self.type_of.get(&id));
        for owner in owners {
            let Some(mut m) = self.material_of.get(&owner).and_then(|m| f.get(*m)) else { continue };
            for _ in 0..4 {
                match m.name.as_str() {
                    "IFCMATERIALLAYERSETUSAGE" => match f.deref(m.arg(0)) {
                        Some(set) => m = set,
                        None => break,
                    },
                    "IFCMATERIALLAYERSET" => {
                        let layers: Vec<(String, f64)> = m
                            .arg(0)
                            .as_list()
                            .iter()
                            .filter_map(|l| f.deref(l))
                            .map(|l| {
                                let name = f.deref(l.arg(0)).and_then(|mat| mat.arg(0).as_str()).unwrap_or("IFC");
                                (name.to_string(), l.arg(1).as_f64().unwrap_or(0.0) * self.scale)
                            })
                            .collect();
                        let main = layers.first().map(|l| l.0.clone());
                        return (layers, main);
                    }
                    "IFCMATERIAL" => return (Vec::new(), m.arg(0).as_str().map(str::to_string)),
                    "IFCMATERIALLIST" => {
                        let first = m.arg(0).as_list().first().and_then(|x| f.deref(x));
                        return (Vec::new(), first.and_then(|x| x.arg(0).as_str()).map(str::to_string));
                    }
                    "IFCMATERIALCONSTITUENTSET" | "IFCMATERIALPROFILESET" => {
                        let first = m.arg(2).as_list().first().and_then(|x| f.deref(x));
                        let material = first.and_then(|x| f.deref(x.arg(2))).and_then(|x| x.arg(0).as_str());
                        return (Vec::new(), material.map(str::to_string));
                    }
                    "IFCMATERIALPROFILESETUSAGE" => match f.deref(m.arg(0)) {
                        Some(set) => m = set,
                        None => break,
                    },
                    _ => break,
                }
            }
        }
        (Vec::new(), None)
    }

    /// Ensure a project material named `name`, colored by a style of the same
    /// name or by `fallback`.
    fn material(&mut self, name: &str, fallback: Option<[u8; 3]>) -> String {
        if !self.p.materials.iter().any(|m| m.name == name) {
            let color = self.style_colors.get(name).copied().or(fallback).unwrap_or([200, 200, 200]);
            self.p.materials.push(Material { name: name.to_string(), color });
        }
        name.to_string()
    }

    /// Main material of a product (created if needed), colored like its body.
    fn main_material(&mut self, id: u32, e: &Entity) -> String {
        let (_, main) = self.materials(id);
        let color = self.body_color(e);
        let name = main.unwrap_or_else(|| "IFC".to_string());
        self.material(&name, color)
    }

    fn body_color(&self, e: &Entity) -> Option<[u8; 3]> {
        let f = self.f;
        let shape = f.deref(e.arg(6))?;
        for rep in shape.arg(2).as_list().iter().filter_map(|r| f.deref(r)) {
            for item in rep.arg(3).as_list().iter().filter_map(Param::as_ref) {
                if let Some(c) = self.item_colors.get(&item) {
                    return Some(*c);
                }
            }
        }
        None
    }

    /// Layers for a layered type of `thickness`: the file's layer set when it
    /// adds up, else one layer of the main material.
    fn layers(&mut self, id: u32, e: &Entity, thickness: f64) -> Vec<Layer> {
        let (layers, main) = self.materials(id);
        let color = self.body_color(e);
        let total: f64 = layers.iter().map(|l| l.1).sum();
        if !layers.is_empty() && (total - thickness).abs() < TYPE_TOL && layers.iter().all(|l| l.1 > 0.0) {
            let n = layers.len();
            return layers
                .into_iter()
                .enumerate()
                .map(|(i, (name, t))| {
                    let material = self.material(&name, color);
                    // Outer layers of a multi-layer set are finishes.
                    let function =
                        if n > 2 && (i == 0 || i == n - 1) { LayerFunction::Finish } else { LayerFunction::Structure };
                    Layer { material, thickness: t, function }
                })
                .collect();
        }
        let material = self.material(&main.unwrap_or_else(|| "IFC".to_string()), color);
        vec![Layer { material, thickness, function: LayerFunction::Structure }]
    }

    /// A type of `category` named `name` whose data satisfies `same`, else a
    /// new one from `make`.
    fn find_or_add_type(
        &mut self,
        category: Category,
        name: Option<String>,
        fallback_name: String,
        uid: Option<Uid>,
        same: impl Fn(&TypeData) -> bool,
        make: impl FnOnce() -> TypeData,
    ) -> ElementTypeId {
        let named = |t: &ElementType| name.as_deref().is_none_or(|n| t.name == n);
        if let Some(t) = self.p.types_of(category).find(|t| named(t) && same(&t.data)) {
            return t.id;
        }
        if name.is_none()
            && let Some(t) = self.p.types_of(category).find(|t| same(&t.data))
        {
            return t.id;
        }
        let mut base = name.unwrap_or(fallback_name);
        if self.p.types_of(category).any(|t| t.name == base) {
            base = format!("{base} (IFC)");
        }
        let mut unique = base.clone();
        let mut n = 2;
        while self.p.types_of(category).any(|t| t.name == unique) {
            unique = format!("{base} {n}");
            n += 1;
        }
        let id = ElementTypeId(self.p.alloc_id());
        let ty = ElementType { id, name: unique, data: make(), info: Default::default(), uid: uid.unwrap_or_default() };
        self.p.types.insert(id, ty);
        id
    }

    fn type_uid(&self, id: u32) -> Option<Uid> {
        self.type_entity(id).and_then(|t| t.arg(0).as_str()).and_then(guid::decode).map(Uid)
    }

    fn add_element(&mut self, e: &Entity, level: LevelId, type_id: ElementTypeId, mark: String, kind: ElementKind) {
        let id = ElementId(self.p.alloc_id());
        let uid = e.arg(0).as_str().and_then(guid::decode).map(Uid).unwrap_or_default();
        let comments = e.arg(3).as_str().unwrap_or_default().to_string();
        let element = Element { id, level, type_id, mark, comments, kind, uid };
        *self.report.imported.entry(element.category()).or_default() += 1;
        self.p.elements.insert(id, element);
    }

    fn tag(e: &Entity) -> String {
        e.arg(7).as_str().unwrap_or_default().to_string()
    }

    // --- Geometry ---

    fn point(&self, param: &Param) -> Option<DVec3> {
        let e = self.f.deref(param)?;
        if e.name != "IFCCARTESIANPOINT" {
            return None;
        }
        let c: Vec<f64> = e.arg(0).as_list().iter().filter_map(Param::as_f64).collect();
        let p = match c.as_slice() {
            [x, y] => DVec3::new(*x, *y, 0.0),
            [x, y, z, ..] => DVec3::new(*x, *y, *z),
            _ => return None,
        } * self.scale;
        p.is_finite().then_some(p)
    }

    fn direction(&self, param: &Param) -> Option<DVec3> {
        let e = self.f.deref(param)?;
        if e.name != "IFCDIRECTION" {
            return None;
        }
        let c: Vec<f64> = e.arg(0).as_list().iter().filter_map(Param::as_f64).collect();
        let d = match c.as_slice() {
            [x, y] => DVec3::new(*x, *y, 0.0),
            [x, y, z, ..] => DVec3::new(*x, *y, *z),
            _ => return None,
        };
        d.try_normalize()
    }

    /// `IfcAxis2Placement3D` / `2D` as an affine transform (identity if unset).
    fn axis_placement(&self, param: &Param) -> DAffine3 {
        let Some(e) = self.f.deref(param) else { return DAffine3::IDENTITY };
        let origin = self.point(e.arg(0)).unwrap_or(DVec3::ZERO);
        match e.name.as_str() {
            "IFCAXIS2PLACEMENT3D" => frame(self.direction(e.arg(1)), self.direction(e.arg(2)), origin),
            "IFCAXIS2PLACEMENT2D" => frame(None, self.direction(e.arg(1)), origin),
            _ => DAffine3::from_translation(origin),
        }
    }

    /// World transform of an `IfcLocalPlacement` chain.
    fn placement(&self, param: &Param, depth: usize) -> DAffine3 {
        if depth > MAX_PLACEMENT_DEPTH {
            return DAffine3::IDENTITY;
        }
        let Some(e) = self.f.deref(param) else { return DAffine3::IDENTITY };
        if e.name != "IFCLOCALPLACEMENT" {
            return DAffine3::IDENTITY;
        }
        let parent = if e.arg(0).as_ref().is_some() { self.placement(e.arg(0), depth + 1) } else { DAffine3::IDENTITY };
        parent * self.axis_placement(e.arg(1))
    }

    /// 2D placement of a profile: origin and X axis.
    fn placement2(&self, param: &Param) -> (DVec2, DVec2) {
        let xf = self.axis_placement(param);
        (xf.translation.truncate(), xf.matrix3.x_axis.truncate().try_normalize().unwrap_or(DVec2::X))
    }

    fn curve_points(&self, param: &Param, depth: usize) -> Vec<DVec2> {
        let f = self.f;
        let Some(e) = f.deref(param) else { return Vec::new() };
        if depth > MAX_ITEM_DEPTH {
            return Vec::new();
        }
        match e.name.as_str() {
            "IFCPOLYLINE" => e.arg(0).as_list().iter().filter_map(|p| self.point(p)).map(|p| p.truncate()).collect(),
            "IFCINDEXEDPOLYCURVE" => {
                let Some(list) = f.deref(e.arg(0)) else { return Vec::new() };
                list.arg(0)
                    .as_list()
                    .iter()
                    .filter_map(|c| {
                        let c: Vec<f64> = c.as_list().iter().filter_map(Param::as_f64).collect();
                        (c.len() >= 2).then(|| DVec2::new(c[0], c[1]) * self.scale)
                    })
                    .collect()
            }
            "IFCCOMPOSITECURVE" => e
                .arg(0)
                .as_list()
                .iter()
                .filter_map(|s| f.deref(s))
                .flat_map(|seg| self.curve_points(seg.arg(2), depth + 1))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn profile(&self, param: &Param, depth: usize) -> Option<Profile> {
        let e = self.f.deref(param)?;
        if depth > MAX_ITEM_DEPTH {
            return None;
        }
        let len = |i: usize| e.arg(i).as_f64().map(|v| v * self.scale).filter(|v| v.is_finite() && *v > 0.0);
        let rect = |center: DVec2, x: DVec2, w: f64, h: f64| {
            let y = x.perp();
            let pts = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .iter()
                .map(|(a, b)| center + x * (a * w / 2.0) + y * (b * h / 2.0))
                .collect();
            Profile { pts, kind: ProfileKind::Rect { center, x, w, h } }
        };
        let name = e.name.as_str();
        if name.starts_with("IFCRECTANGLE") {
            let (center, x) = self.placement2(e.arg(2));
            return Some(rect(center, x, len(3)?, len(4)?));
        }
        if name.starts_with("IFCCIRCLE") {
            let (center, _) = self.placement2(e.arg(2));
            let r = len(3)?;
            let pts =
                (0..24).map(|i| center + DVec2::from_angle(i as f64 / 24.0 * std::f64::consts::TAU) * r).collect();
            return Some(Profile { pts, kind: ProfileKind::Circle { center, r } });
        }
        match name {
            "IFCARBITRARYCLOSEDPROFILEDEF" | "IFCARBITRARYPROFILEDEFWITHVOIDS" => {
                let pts = clean_polygon(self.curve_points(e.arg(2), 0));
                (pts.len() >= 3).then_some(Profile { pts, kind: ProfileKind::Poly })
            }
            // Steel sections: their bounding rectangle.
            "IFCISHAPEPROFILEDEF" | "IFCASYMMETRICISHAPEPROFILEDEF" => {
                let (center, x) = self.placement2(e.arg(2));
                Some(rect(center, x, len(3)?, len(4)?))
            }
            "IFCLSHAPEPROFILEDEF"
            | "IFCTSHAPEPROFILEDEF"
            | "IFCUSHAPEPROFILEDEF"
            | "IFCCSHAPEPROFILEDEF"
            | "IFCZSHAPEPROFILEDEF" => {
                let (center, x) = self.placement2(e.arg(2));
                Some(rect(center, x, len(4)?, len(3)?))
            }
            "IFCDERIVEDPROFILEDEF" => self.profile(e.arg(2), depth + 1),
            _ => None,
        }
    }

    /// Resolve a representation item into world solids.
    fn items(&self, param: &Param, xf: DAffine3, depth: usize, out: &mut Vec<Solid>) {
        let f = self.f;
        let Some(e) = f.deref(param) else { return };
        if depth > MAX_ITEM_DEPTH {
            return;
        }
        match e.name.as_str() {
            "IFCEXTRUDEDAREASOLID" | "IFCEXTRUDEDAREASOLIDTAPERED" => {
                let Some(profile) = self.profile(e.arg(0), 0) else { return };
                let position = xf * self.axis_placement(e.arg(1));
                let Some(depth) = e.arg(3).as_f64().map(|d| d * self.scale).filter(|d| d.is_finite() && *d > 0.0)
                else {
                    return;
                };
                let dir = self.direction(e.arg(2)).unwrap_or(DVec3::Z);
                let vector = position.transform_vector3(dir * depth);
                let mut points = Vec::with_capacity(profile.pts.len() * 2);
                for p in &profile.pts {
                    let b = position.transform_point3(p.extend(0.0));
                    points.push(b);
                    points.push(b + vector);
                }
                out.push(Solid { extrusion: Some(Extrusion { profile, xf: position, vector }), points });
            }
            "IFCBOOLEANCLIPPINGRESULT" | "IFCBOOLEANRESULT" => self.items(e.arg(1), xf, depth + 1, out),
            "IFCMAPPEDITEM" => {
                let Some(source) = f.deref(e.arg(0)) else { return };
                let origin = self.axis_placement(source.arg(0));
                let target = self.operator(e.arg(1));
                let Some(rep) = f.deref(source.arg(1)) else { return };
                for item in rep.arg(3).as_list() {
                    self.items(item, xf * target * origin, depth + 1, out);
                }
            }
            "IFCFACETEDBREP" | "IFCFACETEDBREPWITHVOIDS" => {
                let mut points = Vec::new();
                self.shell_points(e.arg(0), xf, &mut points);
                if !points.is_empty() {
                    out.push(Solid { extrusion: None, points });
                }
            }
            "IFCSHELLBASEDSURFACEMODEL" => {
                let mut points = Vec::new();
                for shell in e.arg(0).as_list() {
                    self.shell_points(shell, xf, &mut points);
                }
                if !points.is_empty() {
                    out.push(Solid { extrusion: None, points });
                }
            }
            "IFCPOLYGONALFACESET" | "IFCTRIANGULATEDFACESET" => {
                let Some(list) = f.deref(e.arg(0)) else { return };
                let points: Vec<DVec3> = list
                    .arg(0)
                    .as_list()
                    .iter()
                    .filter_map(|c| {
                        let c: Vec<f64> = c.as_list().iter().filter_map(Param::as_f64).collect();
                        (c.len() >= 3).then(|| xf.transform_point3(DVec3::new(c[0], c[1], c[2]) * self.scale))
                    })
                    .collect();
                if !points.is_empty() {
                    out.push(Solid { extrusion: None, points });
                }
            }
            "IFCBOUNDINGBOX" => {
                let Some(corner) = self.point(e.arg(0)) else { return };
                let size = DVec3::new(
                    e.arg(1).as_f64().unwrap_or(0.0),
                    e.arg(2).as_f64().unwrap_or(0.0),
                    e.arg(3).as_f64().unwrap_or(0.0),
                ) * self.scale;
                let points = (0..8)
                    .map(|i| {
                        let pick = DVec3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64);
                        xf.transform_point3(corner + size * pick)
                    })
                    .collect();
                out.push(Solid { extrusion: None, points });
            }
            _ => {}
        }
    }

    fn shell_points(&self, param: &Param, xf: DAffine3, out: &mut Vec<DVec3>) {
        let f = self.f;
        let Some(shell) = f.deref(param) else { return };
        for face in shell.arg(0).as_list().iter().filter_map(|x| f.deref(x)) {
            for bound in face.arg(0).as_list().iter().filter_map(|x| f.deref(x)) {
                let Some(lp) = f.deref(bound.arg(0)) else { continue };
                out.extend(lp.arg(0).as_list().iter().filter_map(|p| self.point(p)).map(|p| xf.transform_point3(p)));
            }
        }
    }

    /// `IfcCartesianTransformationOperator3D` as an affine transform.
    fn operator(&self, param: &Param) -> DAffine3 {
        let Some(e) = self.f.deref(param) else { return DAffine3::IDENTITY };
        let x = self.direction(e.arg(0)).unwrap_or(DVec3::X);
        let y = self.direction(e.arg(1)).unwrap_or(DVec3::Y);
        let origin = self.point(e.arg(2)).unwrap_or(DVec3::ZERO);
        let scale = e.arg(3).as_f64().filter(|s| s.is_finite() && *s > 0.0).unwrap_or(1.0);
        let z = self.direction(e.arg(4)).unwrap_or_else(|| x.cross(y).try_normalize().unwrap_or(DVec3::Z));
        affine(x * scale, y * scale, z * scale, origin)
    }

    /// Body solids and axis of a product placed at `world`.
    fn body(&self, e: &Entity, world: DAffine3) -> Body {
        let f = self.f;
        let mut body = Body::default();
        let Some(shape) = f.deref(e.arg(6)) else { return body };
        let reps: Vec<&Entity> = shape.arg(2).as_list().iter().filter_map(|r| f.deref(r)).collect();
        let id = |r: &Entity| r.arg(1).as_str().unwrap_or_default().to_ascii_lowercase();
        let has_body = reps.iter().any(|r| id(r) == "body");
        for rep in &reps {
            let ident = id(rep);
            if ident == "axis" {
                let pts: Vec<DVec2> = rep
                    .arg(3)
                    .as_list()
                    .iter()
                    .flat_map(|c| self.curve_points(c, 0))
                    .map(|p| world.transform_point3(p.extend(0.0)).truncate())
                    .collect();
                if let (Some(a), Some(b)) = (pts.first(), pts.last())
                    && a.distance(*b) > 1e-6
                {
                    body.axis = Some((*a, *b));
                }
                continue;
            }
            let usable = if has_body {
                ident == "body"
            } else {
                !matches!(ident.as_str(), "footprint" | "annotation" | "box" | "clearance" | "profile")
            };
            if usable {
                for item in rep.arg(3).as_list() {
                    self.items(item, world, 0, &mut body.solids);
                }
            }
        }
        if body.solids.is_empty() {
            // Last resort: a bounding box representation.
            for rep in reps.iter().filter(|r| id(r) == "box") {
                for item in rep.arg(3).as_list() {
                    self.items(item, world, 0, &mut body.solids);
                }
            }
        }
        body
    }

    fn product(&self, e: &Entity) -> (DAffine3, Body) {
        let world = self.placement(e.arg(5), 0);
        let body = self.body(e, world);
        (world, body)
    }

    // --- Elements ---

    fn import_wall(&mut self, id: u32, e: &Entity) -> bool {
        let (world, body) = self.product(e);
        let Some(ex) = body.extrusion(true) else { return false };
        let Some((z0, z1)) = body.z_range() else { return false };
        let footprint = ex.footprint();
        // Wall direction: the axis representation, else the placement's X axis
        // (or Y when the footprint is longer that way).
        let mut dir = world.matrix3.x_axis.truncate().try_normalize().unwrap_or(DVec2::X);
        let mut origin = world.translation.truncate();
        if let Some((a, b)) = body.axis {
            dir = (b - a).try_normalize().unwrap_or(dir);
            origin = a;
        }
        let along = |d: DVec2| {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            let (mut vlo, mut vhi) = (f64::INFINITY, f64::NEG_INFINITY);
            for p in &footprint {
                let r = *p - origin;
                lo = lo.min(r.dot(d));
                hi = hi.max(r.dot(d));
                vlo = vlo.min(r.dot(d.perp()));
                vhi = vhi.max(r.dot(d.perp()));
            }
            (lo, hi, vlo, vhi)
        };
        let (mut u0, mut u1, mut v0, mut v1) = along(dir);
        if body.axis.is_none() && (v1 - v0) > (u1 - u0) {
            dir = dir.perp();
            (u0, u1, v0, v1) = along(dir);
        }
        let thickness = v1 - v0;
        let n = dir.perp() * ((v0 + v1) / 2.0);
        let (start, end) = match body.axis {
            // The axis keeps its own length (body ends include joins).
            Some((a, b)) => (a + n, b + n),
            None => (origin + dir * u0 + n, origin + dir * u1 + n),
        };
        if !(start.is_finite() && end.is_finite()) || start.distance(end) < 0.01 || thickness < 0.005 {
            return false;
        }
        let (level, elevation) = self.level_for(id, z0);
        let name = self.type_name(id, e);
        let fallback = format!("{:.0} cm", thickness * 100.0);
        let layers = self.layers(id, e, thickness);
        let external = self.property(id, "Pset_WallCommon", "IsExternal").and_then(Param::as_bool);
        let structural = self.property(id, "Pset_WallCommon", "LoadBearing").and_then(Param::as_bool);
        let function = if external == Some(false) { WallFunction::Interior } else { WallFunction::Exterior };
        let uid = self.type_uid(id);
        let type_id = self.find_or_add_type(
            Category::Wall,
            name,
            fallback,
            uid,
            |d| matches!(d, TypeData::Wall { layers, .. } if (layers.iter().map(|l| l.thickness).sum::<f64>() - thickness).abs() < TYPE_TOL),
            || TypeData::Wall { layers, function },
        );
        let wall = Wall {
            start,
            end,
            base_offset: z0 - elevation,
            height: z1 - z0,
            structural: structural.unwrap_or(false),
            ..Default::default()
        };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Wall(wall));
        if let Some(element) = self.p.elements.keys().next_back().copied() {
            self.walls.insert(id, element);
        }
        true
    }

    fn import_filler(&mut self, id: u32, e: &Entity, door: bool) -> bool {
        let opening = self.opening_of_filler.get(&id).copied();
        let host_ifc = opening.and_then(|o| self.host_of_opening.get(&o)).copied();
        let (world, body) = self.product(e);
        let opening_body = opening.and_then(|o| self.f.get(o)).map(|o| self.product(o).1);
        // Points that span the opening: the opening's body, else the filler's.
        let mut points: Vec<DVec3> = opening_body.as_ref().map(|b| b.points().collect()).unwrap_or_default();
        if points.is_empty() {
            points = body.points().collect();
        }
        let center = if points.is_empty() {
            world.translation
        } else {
            let (lo, hi) =
                points.iter().fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(lo, hi), p| {
                    (lo.min(*p), hi.max(*p))
                });
            (lo + hi) * 0.5
        };
        // Host: the voided wall, else the nearest wall within reach.
        let host = host_ifc.and_then(|h| self.walls.get(&h)).copied().or_else(|| {
            self.p
                .walls()
                .map(|(w, wall)| {
                    let p = polygon::closest_on_segment(wall.start, wall.end, center.truncate()).0;
                    (w.id, p.distance(center.truncate()), self.p.thickness_of(w))
                })
                .filter(|(_, d, t)| *d <= t / 2.0 + 0.1)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _, _)| id)
        });
        let Some(host) = host else { return false };
        let Some(host_element) = self.p.element(host).cloned() else { return false };
        let Some(wall) = host_element.as_wall().cloned() else { return false };
        let base_z = self.p.wall_base_z(&host_element, &wall);
        let dir = wall.direction();
        let (mut t0, mut t1, mut z0, mut z1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for p in &points {
            let t = (p.truncate() - wall.start).dot(dir);
            t0 = t0.min(t);
            t1 = t1.max(t);
            z0 = z0.min(p.z);
            z1 = z1.max(p.z);
        }
        if points.is_empty() {
            let t = (center.truncate() - wall.start).dot(dir);
            (t0, t1, z0, z1) = (t, t, center.z, center.z);
        }
        let attr = |i: usize| e.arg(i).as_f64().map(|v| v * self.scale).filter(|v| v.is_finite() && *v > 0.0);
        let width = attr(9).unwrap_or(t1 - t0);
        let height = attr(8).unwrap_or(z1 - z0);
        if !(width > 0.01 && height > 0.01) {
            return false;
        }
        let offset = (t0 + t1) / 2.0;
        let sill = if (z0 - base_z).abs() < 0.005 { 0.0 } else { (z0 - base_z).max(0.0) };
        let name = self.type_name(id, e);
        let material = self.main_material(id, e);
        let same = move |w: f64, h: f64| (w - width).abs() < TYPE_TOL && (h - height).abs() < TYPE_TOL;
        let uid = self.type_uid(id);
        let fallback = format!("{:.0} x {:.0}", width * 100.0, height * 100.0);
        let (category, type_id) = if door {
            let type_id = self.find_or_add_type(
                Category::Door,
                name,
                fallback,
                uid,
                |d| matches!(d, TypeData::Door { width, height, .. } if same(*width, *height)),
                || TypeData::Door { width, height, material },
            );
            (Category::Door, type_id)
        } else {
            let type_id = self.find_or_add_type(
                Category::Window,
                name,
                fallback,
                uid,
                |d| matches!(d, TypeData::Window { width, height, .. } if same(*width, *height)),
                || TypeData::Window { width, height, default_sill: sill, material },
            );
            (Category::Window, type_id)
        };
        let hosted = Hosted { host, offset, sill_height: sill, flip_facing: false, flip_hand: false };
        let kind = if category == Category::Door { ElementKind::Door(hosted) } else { ElementKind::Window(hosted) };
        self.add_element(e, host_element.level, type_id, Self::tag(e), kind);
        true
    }

    /// Footprint, z range and vertical thickness of a sketched body.
    fn sketch(body: &Body) -> Option<(Vec<DVec2>, f64, f64, f64)> {
        let (z0, z1) = body.z_range()?;
        let (footprint, thickness) = match body.extrusion(true) {
            Some(ex) => (ex.footprint(), ex.vector.z.abs()),
            None => (convex_hull(&body.points().map(|p| p.truncate()).collect::<Vec<_>>()), z1 - z0),
        };
        let footprint = clean_polygon(footprint);
        (footprint.len() >= 3 && polygon::area(&footprint) > 1e-4).then_some((footprint, z0, z1, thickness))
    }

    fn import_slab(&mut self, id: u32, e: &Entity) -> bool {
        let kind = e.arg(8).as_enum().unwrap_or("NOTDEFINED");
        let (_, body) = self.product(e);
        let Some((boundary, z0, z1, thickness)) = Self::sketch(&body) else { return false };
        if kind == "ROOF" {
            return self.add_roof(id, e, boundary, z0, thickness, RoofShape::Flat);
        }
        let (level, elevation) = self.level_for(id, z1);
        let name = self.type_name(id, e);
        let layers = self.layers(id, e, thickness);
        let uid = self.type_uid(id);
        let type_id = self.find_or_add_type(
            Category::Floor,
            name,
            format!("{:.0} cm", thickness * 100.0),
            uid,
            |d| matches!(d, TypeData::Floor { layers } if (layers.iter().map(|l| l.thickness).sum::<f64>() - thickness).abs() < TYPE_TOL),
            || TypeData::Floor { layers },
        );
        let floor = Floor { boundary, height_offset: z1 - elevation };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Floor(floor));
        true
    }

    fn import_covering(&mut self, id: u32, e: &Entity) -> bool {
        if e.arg(8).as_enum() != Some("CEILING") {
            return false;
        }
        let (_, body) = self.product(e);
        let Some((boundary, z0, _, thickness)) = Self::sketch(&body) else { return false };
        let (level, elevation) = self.level_for(id, z0);
        let name = self.type_name(id, e);
        let material = self.main_material(id, e);
        let uid = self.type_uid(id);
        let type_id = self.find_or_add_type(
            Category::Ceiling,
            name,
            format!("{:.0} cm", thickness * 100.0),
            uid,
            |d| matches!(d, TypeData::Ceiling { thickness: t, .. } if (t - thickness).abs() < TYPE_TOL),
            || TypeData::Ceiling { thickness, material },
        );
        let ceiling = Ceiling { boundary, height_offset: z0 - elevation };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Ceiling(ceiling));
        true
    }

    fn import_roof(&mut self, id: u32, e: &Entity) -> bool {
        let (_, body) = self.product(e);
        if body.solids.is_empty() {
            // Roofs made of aggregated slabs are imported through the slabs.
            return self.has_parts.contains_key(&id);
        }
        let predefined = e.arg(8).as_enum().unwrap_or("NOTDEFINED");
        let (layers, _) = self.materials(id);
        let layered: f64 = layers.iter().map(|l| l.1).sum();
        let tilted = |ex: &&Extrusion| ex.xf.matrix3.z_axis.try_normalize().is_some_and(|n| n.z.abs() < 0.9999);
        if let Some(ex) = body.solids.iter().filter_map(|s| s.extrusion.as_ref()).find(tilted) {
            // A sloped profile extruded vertically: the slope runs along the
            // steepest direction of the profile plane.
            let normal = ex.xf.matrix3.z_axis.normalize_or_zero();
            let downhill = normal.truncate();
            let run = downhill.length();
            if run < 1e-9 || normal.z.abs() < 1e-9 {
                return false;
            }
            let slope = run / normal.z.abs();
            // The roof rises away from the normal's horizontal component.
            let direction = -downhill * normal.z.signum();
            let footprint = clean_polygon(ex.footprint());
            let z0 = ex.profile.pts.iter().map(|p| ex.plane_point(*p).z).fold(f64::INFINITY, f64::min);
            let thickness = ex.vector.z.abs() / (1.0 + slope * slope).sqrt();
            let shape = RoofShape::Shed { slope, direction_deg: direction.y.atan2(direction.x).to_degrees() };
            if footprint.len() < 3 {
                return false;
            }
            return self.add_roof(id, e, footprint, z0, thickness, shape);
        }
        if body.extrusion(true).is_some() {
            let Some((footprint, z0, _, thickness)) = Self::sketch(&body) else { return false };
            return self.add_roof(id, e, footprint, z0, thickness, RoofShape::Flat);
        }
        // B-rep or other: footprint hull, slope from the rise over the half width.
        let Some((footprint, z0, z1, _)) = Self::sketch(&body) else { return false };
        let thickness = if layered > 0.0 { layered } else { 0.15 };
        let shape = match predefined {
            "GABLE_ROOF" | "HIP_ROOF" => {
                // Longest hull edge sets the ridge direction.
                let n = footprint.len();
                let (mut ridge, mut best) = (DVec2::X, 0.0);
                for i in 0..n {
                    let d = footprint[(i + 1) % n] - footprint[i];
                    if d.length() > best {
                        best = d.length();
                        ridge = d.normalize();
                    }
                }
                let across: Vec<f64> = footprint.iter().map(|p| p.dot(ridge.perp())).collect();
                let width = across.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                    - across.iter().cloned().fold(f64::INFINITY, f64::min);
                let half = (width / 2.0).max(0.01);
                let rise = z1 - z0;
                // rise = slope * half + thickness * sqrt(1 + slope²), solved by iteration.
                let mut slope = rise / half;
                for _ in 0..20 {
                    slope = ((rise - thickness * (1.0 + slope * slope).sqrt()) / half).max(0.0);
                }
                if predefined == "HIP_ROOF" {
                    RoofShape::Hip { slope }
                } else {
                    RoofShape::Gable { slope, ridge_deg: ridge.y.atan2(ridge.x).to_degrees() }
                }
            }
            _ => RoofShape::Flat,
        };
        let thickness = if matches!(shape, RoofShape::Flat) { z1 - z0 } else { thickness };
        self.add_roof(id, e, footprint, z0, thickness, shape)
    }

    fn add_roof(
        &mut self,
        id: u32,
        e: &Entity,
        footprint: Vec<DVec2>,
        z0: f64,
        thickness: f64,
        shape: RoofShape,
    ) -> bool {
        if thickness < 1e-3 {
            return false;
        }
        let (level, elevation) = self.level_for(id, z0);
        let name = self.type_name(id, e);
        let layers = self.layers(id, e, thickness);
        let uid = self.type_uid(id);
        let type_id = self.find_or_add_type(
            Category::Roof,
            name,
            format!("{:.0} cm", thickness * 100.0),
            uid,
            |d| matches!(d, TypeData::Roof { layers } if (layers.iter().map(|l| l.thickness).sum::<f64>() - thickness).abs() < TYPE_TOL),
            || TypeData::Roof { layers },
        );
        let roof = Roof { footprint, base_offset: z0 - elevation, overhang: 0.0, shape };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Roof(roof));
        true
    }

    fn import_space(&mut self, id: u32, e: &Entity) -> bool {
        let (world, body) = self.product(e);
        let (z0, z1) = body.z_range().unwrap_or((world.translation.z, world.translation.z));
        let outline = Self::sketch(&body).map(|s| s.0).unwrap_or_default();
        // A point inside the outline: its centroid when inside, else the
        // centroid of one of its triangles.
        let position = if outline.len() >= 3 {
            let area = polygon::signed_area(&outline);
            let n = outline.len();
            let mut c = DVec2::ZERO;
            for i in 0..n {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                c += (a + b) * a.perp_dot(b);
            }
            let centroid = if area.abs() > 1e-9 { c / (6.0 * area) } else { outline[0] };
            if polygon::contains(&outline, centroid) {
                centroid
            } else {
                polygon::triangulate(&outline)
                    .first()
                    .map(|t| (outline[t[0]] + outline[t[1]] + outline[t[2]]) / 3.0)
                    .unwrap_or(centroid)
            }
        } else {
            world.translation.truncate()
        };
        if !position.is_finite() {
            return false;
        }
        let (level, _) = self.level_for(id, z0);
        let height = if z1 - z0 > 0.1 { z1 - z0 } else { self.p.level_height(level, 2.8) };
        let Some(type_id) = self.p.default_type(Category::Room).or_else(|| {
            self.p.ensure_annotation_types();
            self.p.default_type(Category::Room)
        }) else {
            return false;
        };
        let room = Room {
            position,
            number: e.arg(2).as_str().unwrap_or_default().to_string(),
            name: e.arg(7).as_str().unwrap_or_default().to_string(),
            height,
        };
        self.add_element(e, level, type_id, String::new(), ElementKind::Room(room));
        true
    }

    fn import_column(&mut self, id: u32, e: &Entity) -> bool {
        let (_, body) = self.product(e);
        let Some(ex) = body.extrusion(true).cloned() else { return false };
        let Some((z0, z1)) = body.z_range() else { return false };
        let (section, center, rotation) = match ex.profile.kind {
            ProfileKind::Circle { center, r } => {
                (ColumnSection::Round { diameter: 2.0 * r }, ex.plane_point(center).truncate(), 0.0)
            }
            _ => {
                let (center, x, w, h) = ex.profile.rect();
                let xw = ex.plane_vector(x).truncate();
                (
                    ColumnSection::Rectangular { width: w, depth: h },
                    ex.plane_point(center).truncate(),
                    xw.y.atan2(xw.x).to_degrees(),
                )
            }
        };
        let (level, elevation) = self.level_for(id, z0);
        let name = self.type_name(id, e);
        let material = self.main_material(id, e);
        let fallback = match section {
            ColumnSection::Rectangular { width, depth } => format!("{:.0} x {:.0}", width * 100.0, depth * 100.0),
            ColumnSection::Round { diameter } => format!("Ø {:.0}", diameter * 100.0),
        };
        let same = move |s: &ColumnSection| match (s, section) {
            (ColumnSection::Rectangular { width: a, depth: b }, ColumnSection::Rectangular { width: c, depth: d }) => {
                (a - c).abs() < TYPE_TOL && (b - d).abs() < TYPE_TOL
            }
            (ColumnSection::Round { diameter: a }, ColumnSection::Round { diameter: b }) => (a - b).abs() < TYPE_TOL,
            _ => false,
        };
        let uid = self.type_uid(id);
        let type_id = self.find_or_add_type(
            Category::Column,
            name,
            fallback,
            uid,
            |d| matches!(d, TypeData::Column { section, .. } if same(section)),
            || TypeData::Column { section, material },
        );
        let column = Column {
            position: center,
            rotation_deg: rotation,
            base_offset: z0 - elevation,
            height: z1 - z0,
            top: WallTop::Unconnected,
        };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Column(column));
        true
    }

    /// Furniture, fixtures and proxies: pieces in the product's own frame
    /// (extrusions as profiles, anything else as its bounding box). An
    /// existing component type with the same name and size is reused, so
    /// parametric components survive a round trip.
    fn import_component(&mut self, id: u32, e: &Entity, kind: ComponentKind) -> bool {
        let (world, body) = self.product(e);
        if body.solids.is_empty() {
            return false;
        }
        let x = world.matrix3.x_axis.truncate().try_normalize().unwrap_or(DVec2::X);
        let rotation = x.y.atan2(x.x);
        let origin = world.translation;
        // Plan frame of the component: origin and rotation in plan, Z up.
        let frame = affine(x.extend(0.0), x.perp().extend(0.0), DVec3::Z, origin);
        let inv = frame.inverse();
        let material = self.main_material(id, e);
        let n = |v: f64| literal(v);
        let mut items = Vec::new();
        for (i, solid) in body.solids.iter().enumerate() {
            let name = format!("{} {}", self.type_name(id, e).unwrap_or_else(|| "IFC".into()), i + 1);
            let shape = match solid.extrusion.as_ref().filter(|ex| ex.is_vertical()) {
                Some(ex) => {
                    let pts: Vec<DVec2> =
                        ex.profile.pts.iter().map(|p| inv.transform_point3(ex.plane_point(*p)).truncate()).collect();
                    let z0 = ex
                        .profile
                        .pts
                        .iter()
                        .map(|p| inv.transform_point3(ex.plane_point(*p)).z)
                        .fold(f64::INFINITY, f64::min);
                    let z0 = z0.min(z0 + ex.vector.z);
                    let points = clean_polygon(pts).into_iter().map(|p| (n(p.x), n(p.y))).collect::<Vec<_>>();
                    if points.len() < 3 {
                        continue;
                    }
                    ComponentShape::Extrusion { points, z: n(z0), height: n(ex.vector.z.abs()) }
                }
                None => {
                    let local: Vec<DVec3> = solid.points.iter().map(|p| inv.transform_point3(*p)).collect();
                    let Some((lo, hi)) = local.iter().fold(None, |acc: Option<(DVec3, DVec3)>, p| {
                        Some(acc.map_or((*p, *p), |(lo, hi)| (lo.min(*p), hi.max(*p))))
                    }) else {
                        continue;
                    };
                    let size = hi - lo;
                    if size.min_element() < 1e-4 {
                        continue;
                    }
                    ComponentShape::Box {
                        x: n(lo.x),
                        y: n(lo.y),
                        z: n(lo.z),
                        width: n(size.x),
                        depth: n(size.y),
                        height: n(size.z),
                    }
                }
            };
            items.push(ComponentItem { name, shape, material: material.clone(), in_plan: true });
        }
        let def = ComponentDef { kind, params: Vec::new(), items };
        let Some((lo, hi)) = def_bounds(&def, &Default::default()) else { return false };
        // Instance parameter values written by ConstructCAD.
        let overrides: std::collections::BTreeMap<String, f64> = self
            .numeric_properties(id, crate::export::COMPONENT_PSET)
            .into_iter()
            .map(|(name, v, measure)| {
                let v = match measure.as_deref() {
                    Some("IFCLENGTHMEASURE") => v * self.scale,
                    Some("IFCPLANEANGLEMEASURE") => v.to_degrees(),
                    _ => v,
                };
                (name, v)
            })
            .filter(|(_, v)| v.is_finite())
            .collect();
        let check = overrides.clone();
        let (level, elevation) = self.level_for(id, origin.z + lo.z);
        let name = self.type_name(id, e);
        let fallback = format!("{} {}", e.name.trim_start_matches("IFC").to_lowercase(), id);
        let uid = self.type_uid(id);
        let component = name.clone().unwrap_or_else(|| fallback.clone());
        // Same name and same size: the existing (possibly parametric) type.
        let same = move |d: &TypeData| match d {
            TypeData::Component { def, .. } => def_bounds(def, &check)
                .is_some_and(|(a, b)| (a - lo).abs().max_element() < 1e-3 && (b - hi).abs().max_element() < 1e-3),
            _ => false,
        };
        let type_id = self.find_or_add_type(Category::Component, name, fallback, uid, same, || TypeData::Component {
            component,
            def,
        });
        // Overrides only make sense for a parametric type that declares them.
        let overrides = match self.p.element_type(type_id).map(|t| &t.data) {
            Some(TypeData::Component { def, .. }) => overrides
                .into_iter()
                .filter(|(k, v)| def.param(k).is_some_and(|p| p.instance && p.value != *v))
                .collect(),
            _ => Default::default(),
        };
        let c = Component {
            position: origin.truncate(),
            rotation_deg: rotation.to_degrees(),
            base_offset: origin.z - elevation,
            overrides,
        };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Component(c));
        true
    }

    fn import_beam(&mut self, id: u32, e: &Entity) -> bool {
        let (_, body) = self.product(e);
        let Some((_, z1)) = body.z_range() else { return false };
        let Some(ex) = body.extrusion(false).cloned() else { return false };
        let (center, x, w, h) = ex.profile.rect();
        let (start, end, width, height) = if ex.is_vertical() {
            // Plan rectangle extruded upwards: the long side is the beam.
            let c = ex.plane_point(center).truncate();
            let xw = ex.plane_vector(x).truncate().try_normalize().unwrap_or(DVec2::X);
            let (along, len, across) = if w >= h { (xw, w, h) } else { (xw.perp(), h, w) };
            (c - along * len / 2.0, c + along * len / 2.0, across, ex.vector.z.abs())
        } else if ex.vector.normalize().z.abs() < 0.1 {
            // Section extruded along the beam.
            let start = ex.plane_point(center);
            let end = start + ex.vector;
            let rx = ex.plane_vector(x);
            let (width, height) = if rx.z.abs() > rx.truncate().length() { (h, w) } else { (w, h) };
            (start.truncate(), end.truncate(), width, height)
        } else {
            return false;
        };
        if start.distance(end) < 0.01 || !(start.is_finite() && end.is_finite()) {
            return false;
        }
        let (level, elevation) = self.level_for(id, z1 - height);
        let name = self.type_name(id, e);
        let material = self.main_material(id, e);
        let uid = self.type_uid(id);
        let type_id = self.find_or_add_type(
            Category::Beam,
            name,
            format!("{:.0} x {:.0}", width * 100.0, height * 100.0),
            uid,
            |d| matches!(d, TypeData::Beam { width: a, height: b, .. } if (a - width).abs() < TYPE_TOL && (b - height).abs() < TYPE_TOL),
            || TypeData::Beam { width, height, material },
        );
        let beam = Beam { start, end, top_offset: z1 - elevation };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Beam(beam));
        true
    }
}

/// A number as an expression literal.
fn literal(v: f64) -> String {
    let s = format!("{:.6}", if v.abs() < 5e-7 { 0.0 } else { v });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".to_string() } else { s.to_string() }
}

/// Bounds of a definition's pieces in its own frame.
fn def_bounds(def: &ComponentDef, overrides: &BTreeMap<String, f64>) -> Option<(DVec3, DVec3)> {
    let (parts, errors) = def.resolve(overrides);
    if !errors.is_empty() {
        return None;
    }
    parts
        .iter()
        .flat_map(|p| p.footprint.iter().flat_map(move |q| [q.extend(p.z0), q.extend(p.z1)]))
        .fold(None, |acc, p| Some(acc.map_or((p, p), |(lo, hi): (DVec3, DVec3)| (lo.min(p), hi.max(p)))))
}

fn si_prefix(prefix: Option<&str>) -> f64 {
    match prefix {
        Some("MILLI") => 0.001,
        Some("CENTI") => 0.01,
        Some("DECI") => 0.1,
        Some("KILO") => 1000.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_and_names() {
        let pts = [DVec2::ZERO, DVec2::new(2.0, 0.0), DVec2::new(1.0, 0.5), DVec2::new(2.0, 1.0), DVec2::new(0.0, 1.0)];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4);
        assert!((polygon::signed_area(&hull) - 2.0).abs() < 1e-12);
        assert_eq!(clean_name("Basic Wall:Generic 200mm:123456"), "Basic Wall:Generic 200mm");
        assert_eq!(clean_name("Alvenaria 15 cm"), "Alvenaria 15 cm");
    }

    #[test]
    fn non_ifc_and_garbage_are_errors() {
        assert!(matches!(import("garbage", Project::empty()), Err(ImportError::Parse(_))));
        let empty = "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
        assert!(matches!(import(empty, Project::empty()), Err(ImportError::NotIfc)));
    }

    /// A minimal IFC2X3-style file in millimeters with one wall, a storey and
    /// dangling or cyclic references, as other tools may write.
    #[test]
    fn foreign_file_in_millimeters() {
        let text = "ISO-10303-21;
HEADER;
FILE_SCHEMA(('IFC2X3'));
ENDSEC;
DATA;
#1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'Foreign',$,$,$,$,(#2),#3);
#3=IFCUNITASSIGNMENT((#4));
#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
#10=IFCBUILDINGSTOREY('1XvctVUKr0kugbFTf53O9L',$,'Level 1',$,$,#11,$,$,.ELEMENT.,3000.);
#11=IFCLOCALPLACEMENT($,#12);
#12=IFCAXIS2PLACEMENT3D(#13,$,$);
#13=IFCCARTESIANPOINT((0.,0.,3000.));
#20=IFCWALLSTANDARDCASE('2XvctVUKr0kugbFTf53O9L',$,'Basic Wall:Generic 200mm:4711',$,$,#21,#30,'W1');
#21=IFCLOCALPLACEMENT(#11,#22);
#22=IFCAXIS2PLACEMENT3D(#23,#24,#25);
#23=IFCCARTESIANPOINT((1000.,2000.,0.));
#24=IFCDIRECTION((0.,0.,1.));
#25=IFCDIRECTION((0.,1.,0.));
#30=IFCPRODUCTDEFINITIONSHAPE($,$,(#31));
#31=IFCSHAPEREPRESENTATION(#2,'Body','SweptSolid',(#32));
#32=IFCEXTRUDEDAREASOLID(#33,$,#24,2700.);
#33=IFCRECTANGLEPROFILEDEF(.AREA.,$,#34,5000.,200.);
#34=IFCAXIS2PLACEMENT2D(#35,$);
#35=IFCCARTESIANPOINT((2500.,0.));
#40=IFCRELCONTAINEDINSPATIALSTRUCTURE('3XvctVUKr0kugbFTf53O9L',$,$,$,(#20,#99),#10);
#50=IFCLOCALPLACEMENT(#51,#12);
#51=IFCLOCALPLACEMENT(#50,#12);
#60=IFCCOLUMN('4XvctVUKr0kugbFTf53O9L',$,'C',$,$,#50,#61,$);
#61=IFCPRODUCTDEFINITIONSHAPE($,$,(#62));
#62=IFCSHAPEREPRESENTATION(#2,'Body','SweptSolid',(#63));
#63=IFCEXTRUDEDAREASOLID(#64,$,#24,3000.);
#64=IFCCIRCLEPROFILEDEF(.AREA.,$,$,150.);
#70=IFCFURNISHINGELEMENT('5XvctVUKr0kugbFTf53O9L',$,'Chair',$,$,#11,#30,$);
ENDSEC;
END-ISO-10303-21;
";
        let (p, report) = import(text, Project::from_template()).unwrap();
        assert_eq!(report.schema, "IFC2X3");
        assert_eq!(p.levels.len(), 1);
        assert_eq!(p.levels[0].name, "Level 1");
        assert!((p.levels[0].elevation - 3.0).abs() < 1e-9);
        let (wall_el, wall) = p.walls().next().unwrap();
        // The wall runs along the placement's X axis (world +Y) from (1, 2).
        assert!(wall.start.distance(DVec2::new(1.0, 2.0)) < 1e-9, "{}", wall.start);
        assert!(wall.end.distance(DVec2::new(1.0, 7.0)) < 1e-9, "{}", wall.end);
        assert!((wall.height - 2.7).abs() < 1e-9);
        assert!(wall.base_offset.abs() < 1e-9);
        assert!((p.thickness_of(wall_el) - 0.2).abs() < 1e-9);
        assert_eq!(p.type_name(wall_el.type_id), "Basic Wall:Generic 200mm");
        assert_eq!(wall_el.mark, "W1");
        // The cyclic placement does not hang; the column still imports.
        assert_eq!(report.imported.get(&Category::Column), Some(&1));
        // Furnishings become components (here a box from the wall's body).
        assert_eq!(report.imported.get(&Category::Component), Some(&1));
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert!(p.views_of(cc_core::ViewCategory::FloorPlan).count() == 1);
    }
}
