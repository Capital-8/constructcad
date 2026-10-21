//! IFC import: maps the spatial structure and building elements of an IFC
//! file (IFC2X3 or IFC4) onto a ConstructCAD project.
//!
//! Geometry is read from extruded area solids (the usual representation of
//! walls, slabs, columns and beams), with faceted B-reps, face sets and
//! bounding boxes used for extents. Storeys become levels; walls, doors,
//! windows, slabs, ceilings, roofs, spaces, columns and beams become native
//! elements with matching or new types. Products that cannot be interpreted
//! are counted in the report instead of failing the import.

use crate::geometry::{Extrusion, Geometry, ProfileKind, Solid, affine, clean_loop};
use crate::guid;
use crate::parse::{Entity, Param, ParseError, StepFile, parse};
use cc_core::component::IfcClass;
use cc_core::component::convex_hull;
use cc_core::properties::{Atom, IfcValue, Property, PropertySet, PropertyValue};
use cc_core::{
    Beam, Category, Ceiling, Column, ColumnSection, Component, ComponentDef, ComponentItem, ComponentKind,
    ComponentShape, Element, ElementId, ElementKind, ElementType, ElementTypeId, Floor, Grid, Hosted, Layer,
    LayerFunction, Level, LevelId, Material, Project, Roof, RoofShape, Room, SectionShape, TypeData, Uid, View, ViewId,
    ViewKind, Wall, WallFunction, WallTop,
};
use cc_geom::polygon;
use glam::{DAffine3, DVec2, DVec3};
use std::collections::{BTreeMap, HashMap};

/// Deepest nesting of complex properties.
const MAX_PROPERTY_DEPTH: usize = 8;
/// Most openings cut from one product.
const MAX_OPENINGS: usize = 1000;
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
    /// Objects brought in as generic components keeping their IFC class, by class.
    pub generic: BTreeMap<String, usize>,
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

    /// The extrusion of a body that is nothing but one uncut extrusion of a
    /// solid profile (what slabs, columns and beams can hold exactly).
    fn sole_extrusion(&self) -> Option<&Extrusion> {
        match self.solids.as_slice() {
            [s] if !s.clipped => s.extrusion.as_ref().filter(|e| e.profile.closed && e.profile.holes.is_empty()),
            _ => None,
        }
    }
}

/// Entities that are not physical objects: spatial structure, zones,
/// features cut from their hosts, ports, annotations and analysis models.
fn is_non_physical(name: &str) -> bool {
    matches!(
        name,
        "IFCPROJECT"
            | "IFCBUILDING"
            | "IFCBUILDINGSTOREY"
            | "IFCSPATIALZONE"
            | "IFCEXTERNALSPATIALELEMENT"
            | "IFCOPENINGELEMENT"
            | "IFCOPENINGSTANDARDCASE"
            | "IFCVOIDINGFEATURE"
            | "IFCSURFACEFEATURE"
            | "IFCANNOTATION"
            | "IFCPORT"
            | "IFCDISTRIBUTIONPORT"
            | "IFCVIRTUALELEMENT"
            | "IFCREFERENT"
    ) || name.starts_with("IFCSTRUCTURAL")
        || name.starts_with("IFCALIGNMENT")
        || name.starts_with("IFCFACILITY")
}

/// Component kind of a generic object, by IFC class.
fn kind_of_class(name: &str) -> ComponentKind {
    match name {
        "IFCFURNITURE" | "IFCFURNISHINGELEMENT" | "IFCSYSTEMFURNITUREELEMENT" => ComponentKind::Furniture,
        "IFCSANITARYTERMINAL" | "IFCWASTETERMINAL" => ComponentKind::Plumbing,
        "IFCLIGHTFIXTURE" | "IFCLAMP" => ComponentKind::Lighting,
        _ => ComponentKind::Generic,
    }
}

struct Importer<'a> {
    f: &'a StepFile,
    g: Geometry<'a>,
    p: Project,
    report: ImportReport,
    project_id: u32,
    /// Meters per file length unit.
    scale: f64,
    /// SI values of the file's area and volume units.
    area: f64,
    volume: f64,
    storeys: HashMap<u32, LevelId>,
    /// Product → its containing structure or aggregating whole.
    parent: HashMap<u32, u32>,
    type_of: HashMap<u32, u32>,
    material_of: HashMap<u32, u32>,
    psets_of: HashMap<u32, Vec<u32>>,
    /// Opening → host element; filler → opening.
    host_of_opening: HashMap<u32, u32>,
    /// Host → the openings voiding it.
    openings_of: HashMap<u32, Vec<u32>>,
    opening_of_filler: HashMap<u32, u32>,
    /// Surface style colors by style name and by styled item.
    style_colors: HashMap<String, [u8; 3]>,
    item_colors: HashMap<u32, [u8; 3]>,
    /// Surface style name of styled items.
    item_styles: HashMap<u32, String>,
    /// Imported walls by IFC instance.
    walls: HashMap<u32, ElementId>,
    /// Products with parts aggregated into them (e.g. roofs made of slabs).
    has_parts: HashMap<u32, usize>,
}

fn clean_polygon(pts: Vec<DVec2>) -> Vec<DVec2> {
    clean_loop(pts, |a, b| a.distance(*b) < 1e-9)
}

/// Type names from other tools often end in an instance number ("Wall:Generic 200mm:123456").
fn clean_name(name: &str) -> String {
    let name = name.trim();
    match name.rsplit_once(':') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => head.to_string(),
        _ => name.to_string(),
    }
}

impl<'a> Importer<'a> {
    fn new(f: &'a StepFile, base: Project, project_id: u32) -> Self {
        let mut im = Self {
            f,
            g: Geometry::new(f, 1.0, 1.0),
            p: base,
            report: ImportReport { schema: f.schema.clone(), ..Default::default() },
            project_id,
            scale: 1.0,
            area: 1.0,
            volume: 1.0,
            storeys: HashMap::new(),
            parent: HashMap::new(),
            type_of: HashMap::new(),
            material_of: HashMap::new(),
            psets_of: HashMap::new(),
            host_of_opening: HashMap::new(),
            openings_of: HashMap::new(),
            opening_of_filler: HashMap::new(),
            style_colors: HashMap::new(),
            item_colors: HashMap::new(),
            item_styles: HashMap::new(),
            walls: HashMap::new(),
            has_parts: HashMap::new(),
        };
        im.scale = im.unit("LENGTHUNIT");
        im.area = im.unit_opt("AREAUNIT").unwrap_or(im.scale * im.scale);
        im.volume = im.unit_opt("VOLUMEUNIT").unwrap_or(im.scale.powi(3));
        im.g = Geometry::new(f, im.scale, im.unit("PLANEANGLEUNIT"));
        im.index_relationships();
        im.index_styles();
        im
    }

    fn run(&mut self) {
        self.prepare_base();
        self.read_project_info();
        self.read_storeys();
        let f = self.f;
        // Walls first: doors and windows need their hosts.
        for (id, e) in f.entities.iter() {
            if matches!(e.name.as_str(), "IFCWALL" | "IFCWALLSTANDARDCASE" | "IFCWALLELEMENTEDCASE") {
                let ok = self.import_wall(*id, e);
                self.or_generic(*id, e, ok);
            }
        }
        for (id, e) in f.entities.iter() {
            let (id, name) = (*id, e.name.as_str());
            let ok = match name {
                "IFCWALL" | "IFCWALLSTANDARDCASE" | "IFCWALLELEMENTEDCASE" => continue,
                "IFCDOOR" | "IFCDOORSTANDARDCASE" => self.import_filler(id, e, true),
                "IFCWINDOW" | "IFCWINDOWSTANDARDCASE" => self.import_filler(id, e, false),
                "IFCSLAB" | "IFCSLABSTANDARDCASE" | "IFCSLABELEMENTEDCASE" => self.import_slab(id, e),
                "IFCCOVERING" => self.import_covering(id, e),
                "IFCROOF" => self.import_roof(id, e),
                "IFCCOLUMN" | "IFCCOLUMNSTANDARDCASE" => self.import_column(id, e),
                "IFCBEAM" | "IFCBEAMSTANDARDCASE" => self.import_beam(id, e),
                "IFCSPACE" => {
                    if !self.import_space(id, e) {
                        self.skip(e);
                    }
                    continue;
                }
                "IFCGRID" => {
                    self.import_grid(id, e);
                    continue;
                }
                // Terrain and other site geometry.
                "IFCSITE" => {
                    if self.is_shaped(e) {
                        self.import_generic(id, e);
                    }
                    continue;
                }
                _ if is_non_physical(name) || !self.is_product(e) => continue,
                // Every other physical object keeps its class and geometry.
                _ => false,
            };
            self.or_generic(id, e, ok);
        }
        self.finish();
    }

    /// After a native import attempt: anything not imported natively comes in
    /// as a generic object, and is reported only when it has no usable geometry.
    fn or_generic(&mut self, id: u32, e: &Entity, native: bool) {
        if !native && !self.import_generic(id, e) {
            self.skip(e);
        }
    }

    /// A placed object with a shape (an `IfcProduct` with geometry).
    fn is_product(&self, e: &Entity) -> bool {
        let placed = self.f.deref(e.arg(5)).is_some_and(|p| {
            matches!(p.name.as_str(), "IFCLOCALPLACEMENT" | "IFCGRIDPLACEMENT" | "IFCLINEARPLACEMENT")
        });
        placed && self.is_shaped(e)
    }

    fn is_shaped(&self, e: &Entity) -> bool {
        self.f.deref(e.arg(6)).is_some_and(|s| s.name == "IFCPRODUCTDEFINITIONSHAPE")
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

    /// SI value of one file unit of `kind` (`LENGTHUNIT`: meters; `PLANEANGLEUNIT`: radians), 1 if unset.
    fn unit(&self, kind: &str) -> f64 {
        self.unit_opt(kind).unwrap_or(1.0)
    }

    /// SI value of one file unit of `kind`, if the file assigns one.
    fn unit_opt(&self, kind: &str) -> Option<f64> {
        let f = self.f;
        let assignment = f
            .get(self.project_id)
            .and_then(|p| f.deref(p.arg(8)))
            .or_else(|| f.of_type("IFCUNITASSIGNMENT").next().map(|(_, e)| e))?;
        assignment
            .arg(0)
            .as_list()
            .iter()
            .filter_map(|u| f.deref(u))
            .filter(|u| u.arg(1).as_enum() == Some(kind))
            .find_map(|u| self.unit_factor(u))
    }

    /// SI value of one `unit` (an `IfcSIUnit` or conversion-based unit).
    fn unit_factor(&self, unit: &Entity) -> Option<f64> {
        let power = |kind: Option<&str>| match kind {
            Some("AREAUNIT") => 2,
            Some("VOLUMEUNIT") => 3,
            _ => 1,
        };
        // A prefix applies to the metre before squaring or cubing (mm², mm³).
        let si = |u: &Entity| si_prefix(u.arg(2).as_enum()).powi(power(u.arg(1).as_enum()));
        let factor = match unit.name.as_str() {
            "IFCSIUNIT" => si(unit),
            "IFCCONVERSIONBASEDUNIT" | "IFCCONVERSIONBASEDUNITWITHOFFSET" => {
                let measure = self.f.deref(unit.arg(3));
                let base =
                    measure.and_then(|m| self.f.deref(m.arg(1))).filter(|b| b.name == "IFCSIUNIT").map_or(1.0, si);
                let value = measure.and_then(|m| m.arg(0).as_f64()).map(|v| v * base);
                match value {
                    Some(v) if v.is_finite() && v > 0.0 => v,
                    // Degrees without a usable conversion factor.
                    _ if unit.arg(1).as_enum() == Some("PLANEANGLEUNIT")
                        && unit.arg(2).as_str().is_some_and(|n| n.eq_ignore_ascii_case("degree")) =>
                    {
                        std::f64::consts::PI / 180.0
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        (factor.is_finite() && factor > 0.0).then_some(factor)
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
                        self.openings_of.entry(host).or_default().push(opening);
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
                let is_set =
                    |s: &u32| f.get(*s).is_some_and(|x| x.name == "IFCPROPERTYSET" || x.name == "IFCELEMENTQUANTITY");
                if !sets.is_empty() && sets.iter().all(is_set) {
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
            let surface = styles.iter().filter(|s| s.name == "IFCSURFACESTYLE");
            if let Some((style, color)) = surface.clone().find_map(|s| Some((s, color_of_style(s)?))) {
                self.item_colors.insert(item, color);
                if let Some(name) = style.arg(0).as_str().filter(|n| !n.trim().is_empty()) {
                    self.item_styles.insert(item, name.to_string());
                }
            }
        }
    }

    fn read_storeys(&mut self) {
        let f = self.f;
        let mut storeys: Vec<(u32, String, f64)> = Vec::new();
        for (id, e) in f.of_type("IFCBUILDINGSTOREY") {
            let world = self.g.placement(e.arg(5));
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
            add_view(&mut self.p, name, ViewKind::floor_plan(*level));
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
            self.p.materials.push(Material::new(name, color));
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
                    .flat_map(|c| self.g.curve(c, 0))
                    .map(|p| world.transform_point3(p).truncate())
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
                    self.g.items(item, world, &mut body.solids);
                }
            }
        }
        if body.solids.is_empty() {
            // Last resort: a bounding box representation.
            for rep in reps.iter().filter(|r| id(r) == "box") {
                for item in rep.arg(3).as_list() {
                    self.g.items(item, world, &mut body.solids);
                }
            }
        }
        body
    }

    fn product(&self, e: &Entity) -> (DAffine3, Body) {
        let world = self.g.placement(e.arg(5));
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
        let on_a_face = |p: &DVec2| {
            let v = (*p - origin).dot(dir.perp());
            (v - v0).abs() < 1e-3 || (v - v1).abs() < 1e-3
        };
        if !footprint.iter().all(on_a_face) {
            return false;
        }
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

    /// Footprint, z range and vertical thickness of a body that is one
    /// vertical extrusion; with `approximate`, other bodies give their hull.
    fn sketch(body: &Body, approximate: bool) -> Option<(Vec<DVec2>, f64, f64, f64)> {
        let (z0, z1) = body.z_range()?;
        let (footprint, thickness) = match body.sole_extrusion().filter(|e| e.is_vertical()) {
            Some(ex) => (ex.footprint(), ex.vector.z.abs()),
            None if approximate => (convex_hull(&body.points().map(|p| p.truncate()).collect::<Vec<_>>()), z1 - z0),
            None => return None,
        };
        let footprint = clean_polygon(footprint);
        (footprint.len() >= 3 && polygon::area(&footprint) > 1e-4).then_some((footprint, z0, z1, thickness))
    }

    /// Openings voiding a product that a native element (other than a wall,
    /// whose doors and windows cut it) could not show.
    fn has_openings(&self, id: u32) -> bool {
        self.openings_of.get(&id).is_some_and(|o| !o.is_empty())
    }

    fn import_slab(&mut self, id: u32, e: &Entity) -> bool {
        if self.has_openings(id) {
            return false;
        }
        let kind = e.arg(8).as_enum().unwrap_or("NOTDEFINED");
        let (_, body) = self.product(e);
        let Some((boundary, z0, z1, thickness)) = Self::sketch(&body, false) else { return false };
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
        if self.has_openings(id) {
            return false;
        }
        if e.arg(8).as_enum() != Some("CEILING") {
            return false;
        }
        let (_, body) = self.product(e);
        let Some((boundary, z0, _, thickness)) = Self::sketch(&body, false) else { return false };
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
        if self.has_openings(id) {
            return false;
        }
        let (_, body) = self.product(e);
        if body.solids.is_empty() {
            // Roofs made of aggregated slabs are imported through the slabs.
            return self.has_parts.contains_key(&id);
        }
        let predefined = e.arg(8).as_enum().unwrap_or("NOTDEFINED");
        let (layers, _) = self.materials(id);
        let layered: f64 = layers.iter().map(|l| l.1).sum();
        let tilted = |ex: &&Extrusion| ex.xf.matrix3.z_axis.try_normalize().is_some_and(|n| n.z.abs() < 0.9999);
        if let Some(ex) = body.sole_extrusion().filter(tilted) {
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
        if let Some((footprint, z0, _, thickness)) = Self::sketch(&body, false) {
            return self.add_roof(id, e, footprint, z0, thickness, RoofShape::Flat);
        }
        // Pitched roofs as written by this program (one body with the roof
        // shape named): footprint hull, slope from the rise over the half width.
        // Other bodies keep their exact geometry as generic objects.
        if !matches!(predefined, "GABLE_ROOF" | "HIP_ROOF") || body.solids.len() != 1 {
            return false;
        }
        let Some((footprint, z0, z1, _)) = Self::sketch(&body, true) else { return false };
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
        let outline = Self::sketch(&body, true).map(|s| s.0).unwrap_or_default();
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
        if self.has_openings(id) {
            return false;
        }
        let (_, body) = self.product(e);
        let Some(ex) = body.sole_extrusion().filter(|e| e.is_vertical() && e.profile.is_simple()).cloned() else {
            return false;
        };
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
            ColumnSection::Profile { width, depth, .. } => format!("{:.0} x {:.0}", width * 100.0, depth * 100.0),
            ColumnSection::Pipe { diameter, .. } => format!("Ø {:.0}", diameter * 100.0),
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
            structural: true,
        };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Column(column));
        true
    }

    /// Any physical object without a native counterpart (or whose geometry
    /// a native element cannot hold): a component that keeps its IFC class.
    fn import_generic(&mut self, id: u32, e: &Entity) -> bool {
        let ok = self.import_component(id, e, kind_of_class(&e.name));
        if ok {
            *self.report.generic.entry(crate::schema::display_name(&e.name)).or_default() += 1;
        }
        ok
    }

    /// IFC class, predefined type, object type and type property sets of a product.
    fn ifc_class(&self, id: u32, e: &Entity) -> IfcClass {
        let properties = self.type_of.get(&id).map(|t| self.property_sets(*t)).unwrap_or_default();
        // Site geometry (terrain) is written back as a geographic element.
        if e.name == "IFCSITE" {
            return IfcClass {
                entity: "IFCGEOGRAPHICELEMENT".into(),
                predefined: "TERRAIN".into(),
                object_type: String::new(),
                properties,
            };
        }
        // Predefined types are read from IFC4 files only (IFC2X3 enumerations differ).
        let predefined = crate::schema::element_class(&e.name)
            .and_then(|c| c.predefined)
            .filter(|_| self.report.schema.starts_with("IFC4"))
            .and_then(|i| e.arg(i).as_enum())
            .unwrap_or_default();
        IfcClass {
            entity: e.name.clone(),
            predefined: predefined.to_string(),
            object_type: e.arg(4).as_str().unwrap_or_default().to_string(),
            properties,
        }
    }

    /// Property and quantity sets attached to `owner` (an object or a type).
    /// The program's own component parameters are left out (they are read
    /// back as overrides).
    fn property_sets(&self, owner: u32) -> Vec<PropertySet> {
        let f = self.f;
        let mut out = Vec::new();
        for set in self.psets_of.get(&owner).into_iter().flatten().filter_map(|s| f.get(*s)) {
            let name = set.arg(2).as_str().unwrap_or_default().to_string();
            if name == crate::export::COMPONENT_PSET {
                continue;
            }
            let (quantities, properties) = match set.name.as_str() {
                "IFCPROPERTYSET" => {
                    (false, set.arg(4).as_list().iter().filter_map(|p| self.read_property(p, 0)).collect::<Vec<_>>())
                }
                "IFCELEMENTQUANTITY" => (true, set.arg(5).as_list().iter().filter_map(|q| self.quantity(q)).collect()),
                // Predefined property sets (door linings…) and others.
                _ => continue,
            };
            if !properties.is_empty() {
                out.push(PropertySet { name, quantities, properties });
            }
        }
        out
    }

    fn read_property(&self, param: &Param, depth: usize) -> Option<Property> {
        let p = self.f.deref(param)?;
        let name = p.arg(0).as_str()?.to_string();
        let values = |i: usize, unit: &Param| -> Vec<IfcValue> {
            p.arg(i).as_list().iter().filter_map(|v| self.ifc_value(v, unit)).collect()
        };
        let value = match p.name.as_str() {
            "IFCPROPERTYSINGLEVALUE" => PropertyValue::Single(self.ifc_value(p.arg(2), p.arg(3))),
            "IFCPROPERTYENUMERATEDVALUE" => PropertyValue::Enumerated(values(2, &Param::Null)),
            "IFCPROPERTYLISTVALUE" => PropertyValue::List(values(2, p.arg(3))),
            "IFCPROPERTYBOUNDEDVALUE" => PropertyValue::Bounded {
                upper: self.ifc_value(p.arg(2), p.arg(4)),
                lower: self.ifc_value(p.arg(3), p.arg(4)),
                set_point: self.ifc_value(p.arg(5), p.arg(4)),
            },
            "IFCPROPERTYTABLEVALUE" => {
                PropertyValue::Table { defining: values(2, p.arg(5)), defined: values(3, p.arg(6)) }
            }
            "IFCCOMPLEXPROPERTY" if depth < MAX_PROPERTY_DEPTH => PropertyValue::Complex {
                usage: p.arg(2).as_str().unwrap_or_default().to_string(),
                properties: p.arg(3).as_list().iter().filter_map(|x| self.read_property(x, depth + 1)).collect(),
            },
            _ => return None,
        };
        Some(Property { name, value })
    }

    fn quantity(&self, param: &Param) -> Option<Property> {
        let q = self.f.deref(param)?;
        let name = q.arg(0).as_str()?.to_string();
        let explicit = self.f.deref(q.arg(2)).and_then(|u| self.unit_factor(u));
        let factor = match q.name.as_str() {
            "IFCQUANTITYLENGTH" => explicit.unwrap_or(self.scale),
            "IFCQUANTITYAREA" => explicit.unwrap_or(self.area),
            "IFCQUANTITYVOLUME" => explicit.unwrap_or(self.volume),
            "IFCQUANTITYCOUNT" | "IFCQUANTITYWEIGHT" | "IFCQUANTITYTIME" => explicit.unwrap_or(1.0),
            _ => return None,
        };
        let value = q.arg(3).as_f64().map(|v| v * factor).filter(|v| v.is_finite())?;
        Some(Property { name, value: PropertyValue::Quantity { kind: q.name.clone(), value } })
    }

    /// A typed value converted to SI (lengths, areas, volumes, angles), by
    /// its explicit `unit` or the file's units.
    fn ifc_value(&self, param: &Param, unit: &Param) -> Option<IfcValue> {
        let Param::Typed(measure, inner) = param else { return None };
        let explicit = self.f.deref(unit).and_then(|u| self.unit_factor(u));
        let factor = match measure.as_str() {
            "IFCLENGTHMEASURE" | "IFCPOSITIVELENGTHMEASURE" | "IFCNONNEGATIVELENGTHMEASURE" => {
                explicit.unwrap_or(self.scale)
            }
            "IFCAREAMEASURE" => explicit.unwrap_or(self.area),
            "IFCVOLUMEMEASURE" => explicit.unwrap_or(self.volume),
            "IFCPLANEANGLEMEASURE" | "IFCPOSITIVEPLANEANGLEMEASURE" => explicit.unwrap_or(self.g.angle),
            _ => 1.0,
        };
        let value = match inner.as_ref() {
            Param::Real(x) => Atom::Real(x * factor),
            Param::Int(i) if factor == 1.0 => Atom::Integer(*i),
            Param::Int(i) => Atom::Real(*i as f64 * factor),
            Param::Str(s) => Atom::Text(s.clone()),
            Param::Enum(e) if e == "T" => Atom::Bool(true),
            Param::Enum(e) if e == "F" => Atom::Bool(false),
            Param::Enum(e) => Atom::Enum(e.clone()),
            _ => return None,
        };
        value.is_finite().then(|| IfcValue { measure: measure.clone(), value })
    }

    /// Furniture, fixtures and any other object: pieces in the product's own
    /// frame (vertical extrusions as profiles, anything else as a fixed mesh),
    /// colored by their item styles. An existing component type with the
    /// same name and size is reused, so parametric components survive a
    /// round trip.
    fn import_component(&mut self, id: u32, e: &Entity, kind: ComponentKind) -> bool {
        let (world, mut body) = self.product(e);
        if body.solids.is_empty() {
            return false;
        }
        self.cut_openings(id, &mut body);
        let x = world.matrix3.x_axis.truncate().try_normalize().unwrap_or(DVec2::X);
        let rotation = x.y.atan2(x.x);
        let origin = world.translation;
        // Plan frame of the component: origin and rotation in plan, Z up.
        let frame = affine(x.extend(0.0), x.perp().extend(0.0), DVec3::Z, origin);
        let inv = frame.inverse();
        let main = self.main_material(id, e);
        let n = |v: f64| literal(v);
        let base_name = self.type_name(id, e).unwrap_or_else(|| crate::schema::display_name(&e.name));
        let mut items = Vec::new();
        for (i, solid) in body.solids.iter().enumerate() {
            let name = format!("{base_name} {}", i + 1);
            let material = match self.item_styles.get(&solid.item).cloned() {
                Some(style) => {
                    let color = self.item_colors.get(&solid.item).copied();
                    self.material(&style, color)
                }
                None => main.clone(),
            };
            let plain = solid
                .extrusion
                .as_ref()
                .filter(|ex| !solid.clipped && ex.is_vertical() && ex.profile.closed && ex.profile.holes.is_empty());
            let shape = match plain {
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
                    let Some(mesh) = local_mesh(&solid.mesh, &inv) else { continue };
                    mesh
                }
            };
            items.push(ComponentItem { name, shape, material, in_plan: true });
        }
        let ifc = Some(self.ifc_class(id, e));
        let properties = self.property_sets(id);
        let def = ComponentDef { kind, params: Vec::new(), items, ifc };
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
        let fallback = format!("{} {}", crate::schema::display_name(&e.name), id);
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
            properties,
        };
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Component(c));
        true
    }

    /// Subtract the bodies of the openings voiding `id` from its solids
    /// (openings too complex to cut are left uncut).
    fn cut_openings(&self, id: u32, body: &mut Body) {
        let f = self.f;
        let openings = self.openings_of.get(&id).into_iter().flatten().filter_map(|o| f.get(*o));
        for opening in openings.take(MAX_OPENINGS) {
            let (_, cutter) = self.product(opening);
            for c in &cutter.solids {
                for s in &mut body.solids {
                    if let Some(mesh) = crate::geometry::subtract(&s.mesh, &c.mesh) {
                        s.replace_mesh(mesh);
                    }
                }
            }
        }
        body.solids.retain(|s| !s.mesh.is_empty());
    }

    /// Grid axes of an `IfcGrid` as grid lines.
    fn import_grid(&mut self, id: u32, e: &Entity) {
        let f = self.f;
        let world = self.g.placement(e.arg(5));
        let Some(type_id) = self.p.default_type(Category::Grid).or_else(|| {
            self.p.ensure_annotation_types();
            self.p.default_type(Category::Grid)
        }) else {
            return;
        };
        let (level, _) = self.level_for(id, world.translation.z);
        for axis in (7..10).flat_map(|i| e.arg(i).as_list().iter()).filter_map(|a| f.deref(a)) {
            let pts = self.g.curve(axis.arg(1), 0);
            let (Some(a), Some(b)) = (pts.first(), pts.last()) else { continue };
            let (mut start, mut end) = (world.transform_point3(*a).truncate(), world.transform_point3(*b).truncate());
            if axis.arg(2).as_bool() == Some(false) {
                std::mem::swap(&mut start, &mut end);
            }
            if start.distance(end) < 1e-6 || !(start.is_finite() && end.is_finite()) {
                continue;
            }
            let name = axis.arg(0).as_str().unwrap_or_default().to_string();
            let element = ElementId(self.p.alloc_id());
            let grid = Grid { start, end, name };
            let el = Element {
                id: element,
                level,
                type_id,
                mark: String::new(),
                comments: String::new(),
                kind: ElementKind::Grid(grid),
                uid: Uid::default(),
            };
            *self.report.imported.entry(Category::Grid).or_default() += 1;
            self.p.elements.insert(element, el);
        }
    }

    fn import_beam(&mut self, id: u32, e: &Entity) -> bool {
        if self.has_openings(id) {
            return false;
        }
        let (_, body) = self.product(e);
        let Some((_, z1)) = body.z_range() else { return false };
        // A plan footprint extruded upwards along an axis: the axis keeps the
        // beam's own length (the body's ends include its joins).
        if let Some((a, b)) = body.axis
            && let Some(ex) = body.sole_extrusion().filter(|e| e.is_vertical())
        {
            let Some(dir) = (b - a).try_normalize() else { return false };
            let footprint = ex.footprint();
            let across: Vec<f64> = footprint.iter().map(|p| (*p - a).dot(dir.perp())).collect();
            let (v0, v1) =
                across.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
            if !across.iter().all(|v| (v - v0).abs() < 1e-3 || (v - v1).abs() < 1e-3) {
                return false;
            }
            let n = dir.perp() * ((v0 + v1) / 2.0);
            return self.add_beam(id, e, (a + n, b + n), (v1 - v0, ex.vector.z.abs()), z1);
        }
        let Some(ex) = body.sole_extrusion().filter(|e| e.profile.is_simple()).cloned() else { return false };
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
        self.add_beam(id, e, (start, end), (width, height), z1)
    }

    /// A native beam along `start`-`end` with its top at `z1`.
    fn add_beam(
        &mut self,
        id: u32,
        e: &Entity,
        (start, end): (DVec2, DVec2),
        (width, height): (f64, f64),
        z1: f64,
    ) -> bool {
        if start.distance(end) < 0.01 || !(start.is_finite() && end.is_finite() && width > 1e-6 && height > 1e-6) {
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
            |d| matches!(d, TypeData::Beam { width: a, height: b, shape: SectionShape::Solid, .. } if (a - width).abs() < TYPE_TOL && (b - height).abs() < TYPE_TOL),
            || TypeData::Beam { width, height, material, shape: SectionShape::Solid },
        );
        let beam = Beam::new(start, end, z1 - elevation);
        self.add_element(e, level, type_id, Self::tag(e), ElementKind::Beam(beam));
        true
    }
}

/// A world mesh as a fixed component piece in the frame `inv` maps to.
fn local_mesh(mesh: &[Vec<DVec3>], inv: &DAffine3) -> Option<ComponentShape> {
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut index: HashMap<[i64; 3], u32> = HashMap::new();
    let mut faces = Vec::with_capacity(mesh.len());
    for face in mesh {
        let mut f: Vec<u32> = Vec::with_capacity(face.len());
        for p in face {
            let q = inv.transform_point3(*p);
            if !q.is_finite() {
                continue;
            }
            // Snap to 1/100 mm so shared corners share a vertex.
            let key = [(q.x * 1e5).round() as i64, (q.y * 1e5).round() as i64, (q.z * 1e5).round() as i64];
            let i = *index.entry(key).or_insert_with(|| {
                vertices.push([q.x, q.y, q.z]);
                (vertices.len() - 1) as u32
            });
            if f.last() != Some(&i) {
                f.push(i);
            }
        }
        if f.len() > 1 && f.first() == f.last() {
            f.pop();
        }
        if f.len() >= 3 {
            faces.push(f);
        }
    }
    (!faces.is_empty()).then_some(ComponentShape::Mesh { vertices, faces })
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

    /// Geometry a hostile file can throw at the importer: cyclic booleans and
    /// mapped items, out-of-range and negative indices, absurd knot
    /// multiplicities and angles, empty paths. Nothing may panic or hang.
    #[test]
    fn hostile_geometry_is_survived() {
        let text = "ISO-10303-21;
HEADER;
FILE_SCHEMA(('IFC4'));
ENDSEC;
DATA;
#1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'Hostile',$,$,$,$,$,$);
#2=IFCLOCALPLACEMENT($,#3);
#3=IFCAXIS2PLACEMENT3D(#4,$,$);
#4=IFCCARTESIANPOINT((0.,0.,0.));
#10=IFCPIPESEGMENT('1',$,'A',$,$,#2,#11,$,.RIGIDSEGMENT.);
#11=IFCPRODUCTDEFINITIONSHAPE($,$,(#12));
#12=IFCSHAPEREPRESENTATION($,'Body','X',(#13,#14,#15,#16,#17,#18,#19,#20,#21,#22));
#13=IFCBOOLEANRESULT(.UNION.,#13,#13);
#14=IFCMAPPEDITEM(#30,$);
#30=IFCREPRESENTATIONMAP(#3,#31);
#31=IFCSHAPEREPRESENTATION($,'Body','X',(#14));
#15=IFCPOLYGONALFACESET(#32,$,(#33,#34),(0,-5,99999999999));
#32=IFCCARTESIANPOINTLIST3D(((0.,0.,0.),(1.,0.,0.),(0.,1.,0.)));
#33=IFCINDEXEDPOLYGONALFACE((1,2,3));
#34=IFCINDEXEDPOLYGONALFACE((-1,0,1E300));
#16=IFCSWEPTDISKSOLID(#35,10.,$,$,$);
#35=IFCBSPLINECURVEWITHKNOTS(3,(#4,#4,#4,#4),.UNSPECIFIED.,.F.,.F.,(1E30,1E30),(0.,1.),.UNSPECIFIED.);
#17=IFCREVOLVEDAREASOLID(#36,$,#37,1E308);
#36=IFCCIRCLEPROFILEDEF(.AREA.,$,$,-5.);
#37=IFCAXIS1PLACEMENT(#4,$);
#18=IFCEXTRUDEDAREASOLID(#38,$,$,1.);
#38=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#39);
#39=IFCCOMPOSITECURVE((#40),.F.);
#40=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#39);
#19=IFCBOOLEANCLIPPINGRESULT(.DIFFERENCE.,#21,#41);
#41=IFCHALFSPACESOLID(#42,.T.);
#42=IFCPLANE($);
#20=IFCSWEPTDISKSOLID(#43,1.,$,$,$);
#43=IFCPOLYLINE((#4,#4,#4));
#21=IFCBLOCK($,1.,1.,1.);
#22=IFCFACETEDBREP(#44);
#44=IFCCLOSEDSHELL((#45));
#45=IFCFACE((#46));
#46=IFCFACEOUTERBOUND(#47,.T.);
#47=IFCEDGELOOP((#48));
#48=IFCORIENTEDEDGE(*,*,#49,.T.);
#49=IFCEDGECURVE(#50,#50,#51,.T.);
#50=IFCVERTEXPOINT(#4);
#51=IFCCIRCLE(#3,0.);
ENDSEC;
END-ISO-10303-21;
";
        let (p, report) = import(text, Project::from_template()).unwrap();
        // The block (clipped to nothing or not) still makes an object.
        assert_eq!(report.imported_total(), p.elements.len());
        assert!(report.generic.contains_key("IfcPipeSegment") || !report.skipped.is_empty());
    }
}
