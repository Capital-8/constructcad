//! Mapping of the ConstructCAD model to IFC4 entities.

use crate::args;
use crate::guid::{encode, guid};
use crate::step::{Header, Id, StepWriter, Value, opt_str, reals};
use cc_core::{Element, ElementId, ElementKind, ElementTypeId, LevelId, Project, RoofShape, TypeData, Uid};
use cc_geom::{FRAME_COLOR, HostedPart, ModelGeometry, WallGeom, hosted_parts, polygon};
use glam::{DVec2, DVec3};
use std::collections::{BTreeMap, HashMap};

/// Openings extend this far beyond each wall face so the boolean cut is clean.
const OPENING_CLEARANCE: f64 = 0.05;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExportOptions {
    /// `FILE_NAME` time stamp (ISO 8601). The current UTC time when `None`.
    pub time_stamp: Option<String>,
    /// `FILE_NAME` name, usually the target file name.
    pub file_name: String,
}

/// Serialize `project` as an IFC4 STEP physical file.
pub fn export(project: &Project, options: &ExportOptions) -> String {
    let geom = ModelGeometry::build(project);
    let mut x = Exporter::new(project, &geom);
    x.write_all();
    let generator = format!("ConstructCAD {}", env!("CARGO_PKG_VERSION"));
    let header = Header {
        description: vec!["ViewDefinition [DesignTransferView_V1.0]".into()],
        name: options.file_name.clone(),
        time_stamp: options.time_stamp.clone().unwrap_or_else(now_utc),
        author: vec![project.info.author.clone()],
        organization: vec![String::new()],
        preprocessor_version: generator.clone(),
        originating_system: generator,
        authorization: String::new(),
        schema: "IFC4".into(),
    };
    x.w.finish(&header)
}

/// Export `project` to an `.ifc` file at `path`.
pub fn export_to_file(project: &Project, path: &std::path::Path) -> std::io::Result<()> {
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = export(project, &ExportOptions { time_stamp: None, file_name });
    // Write to a temp file first so a failed export never truncates an existing file.
    let tmp = path.with_extension("ifc.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// A rigid placement: origin plus local X and Z axes (Y = Z × X).
#[derive(Debug, Clone, Copy)]
struct Frame {
    origin: DVec3,
    x: DVec3,
    z: DVec3,
}

impl Frame {
    fn at(origin: DVec3) -> Self {
        Self { origin, x: DVec3::X, z: DVec3::Z }
    }

    /// Placement with Z up and X along `dir` in plan.
    fn horizontal(origin: DVec3, dir: DVec2) -> Self {
        Self { origin, x: DVec3::new(dir.x, dir.y, 0.0), z: DVec3::Z }
    }
}

struct Exporter<'a> {
    w: StepWriter,
    project: &'a Project,
    geom: &'a ModelGeometry,
    seed: String,
    // Shared resources.
    origin3: Id,
    origin2: Id,
    dir_z: Id,
    dir_x: Id,
    identity: Id,
    body_ctx: Id,
    axis_ctx: Id,
    // Caches.
    styles: HashMap<String, Id>,
    materials: HashMap<String, Id>,
    layer_sets: BTreeMap<ElementTypeId, Id>,
    types: BTreeMap<ElementTypeId, Id>,
    layer_usages: BTreeMap<ElementTypeId, Id>,
    products: BTreeMap<ElementId, Id>,
    // Relationship collectors, emitted at the end.
    typed: BTreeMap<ElementTypeId, Vec<Id>>,
    contained: BTreeMap<LevelId, Vec<Id>>,
    usage_objects: BTreeMap<Id, Vec<Id>>,
    /// Storey and its placement per level.
    storeys: BTreeMap<LevelId, (Id, Id)>,
}

impl<'a> Exporter<'a> {
    fn new(project: &'a Project, geom: &'a ModelGeometry) -> Self {
        let mut w = StepWriter::new();
        let origin3 = w.add("IFCCARTESIANPOINT", args![reals(&[0.0, 0.0, 0.0])]);
        let origin2 = w.add("IFCCARTESIANPOINT", args![reals(&[0.0, 0.0])]);
        let dir_z = w.add("IFCDIRECTION", args![reals(&[0.0, 0.0, 1.0])]);
        let dir_x = w.add("IFCDIRECTION", args![reals(&[1.0, 0.0, 0.0])]);
        let identity = w.add("IFCAXIS2PLACEMENT3D", args![origin3, Value::Null, Value::Null]);
        let info = &project.info;
        Self {
            w,
            project,
            geom,
            // Ids of derived objects (relationships, openings…) hash the project id with a tag.
            seed: if info.uid.is_nil() { format!("{}\u{1f}{}", info.number, info.name) } else { info.uid.to_string() },
            origin3,
            origin2,
            dir_z,
            dir_x,
            identity,
            body_ctx: Id(0),
            axis_ctx: Id(0),
            styles: HashMap::new(),
            materials: HashMap::new(),
            layer_sets: BTreeMap::new(),
            types: BTreeMap::new(),
            layer_usages: BTreeMap::new(),
            products: BTreeMap::new(),
            usage_objects: BTreeMap::new(),
            typed: BTreeMap::new(),
            contained: BTreeMap::new(),
            storeys: BTreeMap::new(),
        }
    }

    fn guid(&self, tag: &str) -> Value {
        Value::Str(guid(&self.seed, tag))
    }

    /// GlobalId of an object with a persistent id, falling back to a derived one.
    fn guid_of(&self, uid: Uid, tag: &str) -> Value {
        if uid.is_nil() { self.guid(tag) } else { Value::Str(encode(uid.0)) }
    }

    /// `Pset_*` property set with single values, attached to `objects`.
    fn property_set(&mut self, tag: &str, name: &str, objects: Vec<Id>, props: Vec<(&str, Value)>) {
        if props.is_empty() || objects.is_empty() {
            return;
        }
        let n = Value::Null;
        let props: Vec<Id> = props
            .into_iter()
            .map(|(name, value)| self.w.add("IFCPROPERTYSINGLEVALUE", args![name, n.clone(), value, n.clone()]))
            .collect();
        let gid = self.guid(&format!("pset-{tag}"));
        let set = self.w.add("IFCPROPERTYSET", args![gid, n.clone(), name, n.clone(), props]);
        let gid = self.guid(&format!("pset-rel-{tag}"));
        self.w.add("IFCRELDEFINESBYPROPERTIES", args![gid, n.clone(), n.clone(), n, objects, set]);
    }

    fn write_all(&mut self) {
        let project_id = self.write_project();
        self.write_spatial_structure(project_id);
        let walls: Vec<_> = self.project.walls().map(|(e, _)| e.id).collect();
        for id in walls {
            self.write_wall(id);
        }
        let others: Vec<_> = self
            .project
            .elements
            .values()
            .filter(|e| matches!(e.kind, ElementKind::Floor(_) | ElementKind::Ceiling(_) | ElementKind::Roof(_)))
            .map(|e| e.id)
            .collect();
        for id in others {
            self.write_sketched(id);
        }
        self.write_relationships();
    }

    // --- Project, units, contexts, spatial structure ---

    fn write_project(&mut self) -> Id {
        let w = &mut self.w;
        let units: Vec<Id> = [
            ("LENGTHUNIT", "METRE"),
            ("AREAUNIT", "SQUARE_METRE"),
            ("VOLUMEUNIT", "CUBIC_METRE"),
            ("PLANEANGLEUNIT", "RADIAN"),
        ]
        .into_iter()
        .map(|(kind, name)| {
            w.add("IFCSIUNIT", args![Value::Derived, Value::Enum(kind), Value::Null, Value::Enum(name)])
        })
        .collect();
        let units = w.add("IFCUNITASSIGNMENT", args![units]);
        let wcs = w.add("IFCAXIS2PLACEMENT3D", args![self.origin3, self.dir_z, self.dir_x]);
        let north = w.add("IFCDIRECTION", args![reals(&[0.0, 1.0])]);
        let model = w.add("IFCGEOMETRICREPRESENTATIONCONTEXT", args![Value::Null, "Model", 3i64, 1e-5, wcs, north]);
        let sub = |w: &mut StepWriter, id: &str, view: &'static str| {
            let d = Value::Derived;
            let args = args![
                id,
                "Model",
                d.clone(),
                d.clone(),
                d.clone(),
                d,
                model,
                Value::Null,
                Value::Enum(view),
                Value::Null
            ];
            w.add("IFCGEOMETRICREPRESENTATIONSUBCONTEXT", args)
        };
        self.body_ctx = sub(w, "Body", "MODEL_VIEW");
        self.axis_ctx = sub(w, "Axis", "GRAPH_VIEW");
        let info = &self.project.info;
        let name = if info.name.is_empty() { "ConstructCAD" } else { info.name.as_str() };
        let gid = self.guid_of(self.project.info.uid, "project");
        self.w.add(
            "IFCPROJECT",
            args![
                gid,
                Value::Null,
                name,
                Value::Null,
                Value::Null,
                opt_str(&info.number),
                Value::Null,
                vec![model],
                units
            ],
        )
    }

    fn write_spatial_structure(&mut self, project_id: Id) {
        let site_placement = self.local_placement(None, Frame::at(DVec3::ZERO));
        let gid = self.guid("site");
        let n = Value::Null;
        let site = self.w.add(
            "IFCSITE",
            args![
                gid,
                n.clone(),
                n.clone(),
                n.clone(),
                n.clone(),
                site_placement,
                n.clone(),
                n.clone(),
                Value::Enum("ELEMENT"),
                n.clone(),
                n.clone(),
                n.clone(),
                n.clone(),
                n.clone()
            ],
        );
        let address = (!self.project.info.address.is_empty()).then(|| {
            let lines = vec![self.project.info.address.clone()];
            let args = args![
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                lines,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null
            ];
            self.w.add("IFCPOSTALADDRESS", args)
        });
        let building_placement = self.local_placement(Some(site_placement), Frame::at(DVec3::ZERO));
        let gid = self.guid("building");
        let name = opt_str(&self.project.info.name);
        let building = self.w.add(
            "IFCBUILDING",
            args![
                gid,
                n.clone(),
                name,
                n.clone(),
                n.clone(),
                building_placement,
                n.clone(),
                n.clone(),
                Value::Enum("ELEMENT"),
                n.clone(),
                n.clone(),
                address
            ],
        );
        let mut storeys = Vec::new();
        for level in &self.project.levels {
            let placement =
                self.local_placement(Some(building_placement), Frame::at(DVec3::new(0.0, 0.0, level.elevation)));
            let gid = self.guid(&format!("storey-{}", level.id));
            let storey = self.w.add(
                "IFCBUILDINGSTOREY",
                args![
                    gid,
                    n.clone(),
                    level.name.as_str(),
                    n.clone(),
                    n.clone(),
                    placement,
                    n.clone(),
                    n.clone(),
                    Value::Enum("ELEMENT"),
                    level.elevation
                ],
            );
            self.storeys.insert(level.id, (storey, placement));
            storeys.push(storey);
        }
        self.aggregate("agg-project", project_id, vec![site]);
        self.aggregate("agg-site", site, vec![building]);
        if !storeys.is_empty() {
            self.aggregate("agg-building", building, storeys);
        }
    }

    fn aggregate(&mut self, tag: &str, whole: Id, parts: Vec<Id>) {
        let gid = self.guid(tag);
        self.w.add("IFCRELAGGREGATES", args![gid, Value::Null, Value::Null, Value::Null, whole, parts]);
    }

    // --- Geometry helpers ---

    fn point3(&mut self, p: DVec3) -> Id {
        self.w.add("IFCCARTESIANPOINT", args![reals(&[p.x, p.y, p.z])])
    }

    fn point2(&mut self, p: DVec2) -> Id {
        self.w.add("IFCCARTESIANPOINT", args![reals(&[p.x, p.y])])
    }

    fn direction(&mut self, d: DVec3) -> Id {
        if d == DVec3::Z {
            return self.dir_z;
        }
        if d == DVec3::X {
            return self.dir_x;
        }
        self.w.add("IFCDIRECTION", args![reals(&[d.x, d.y, d.z])])
    }

    fn placement3(&mut self, f: Frame) -> Id {
        if f.origin == DVec3::ZERO && f.x == DVec3::X && f.z == DVec3::Z {
            return self.identity;
        }
        let location = if f.origin == DVec3::ZERO { self.origin3 } else { self.point3(f.origin) };
        if f.x == DVec3::X && f.z == DVec3::Z {
            return self.w.add("IFCAXIS2PLACEMENT3D", args![location, Value::Null, Value::Null]);
        }
        let z = self.direction(f.z);
        let x = self.direction(f.x);
        self.w.add("IFCAXIS2PLACEMENT3D", args![location, z, x])
    }

    fn local_placement(&mut self, relative_to: Option<Id>, f: Frame) -> Id {
        let rel = self.placement3(f);
        self.w.add("IFCLOCALPLACEMENT", args![relative_to, rel])
    }

    /// Closed polyline through `pts` (the first point is repeated at the end).
    fn polyline(&mut self, pts: &[DVec2]) -> Id {
        let mut ids: Vec<Id> = pts.iter().map(|&p| self.point2(p)).collect();
        ids.push(ids[0]);
        self.w.add("IFCPOLYLINE", args![ids])
    }

    /// Extrusion of a closed polygon along `dir` (in `position` coordinates).
    fn extrude_polygon(&mut self, pts: &[DVec2], position: Id, dir: Id, depth: f64) -> Id {
        let curve = self.polyline(pts);
        let profile = self.w.add("IFCARBITRARYCLOSEDPROFILEDEF", args![Value::Enum("AREA"), Value::Null, curve]);
        self.w.add("IFCEXTRUDEDAREASOLID", args![profile, position, dir, depth])
    }

    /// Axis-aligned box between `min` and `max`, as a rectangle extruded upwards.
    fn extrude_box(&mut self, min: DVec3, max: DVec3) -> Id {
        let center = DVec2::new(min.x + max.x, min.y + max.y) * 0.5;
        let location = self.point2(center);
        let position2 = self.w.add("IFCAXIS2PLACEMENT2D", args![location, Value::Null]);
        let (dx, dy) = (max.x - min.x, max.y - min.y);
        let profile = self.w.add("IFCRECTANGLEPROFILEDEF", args![Value::Enum("AREA"), Value::Null, position2, dx, dy]);
        let position = self.placement3(Frame::at(DVec3::new(0.0, 0.0, min.z)));
        self.w.add("IFCEXTRUDEDAREASOLID", args![profile, position, self.dir_z, max.z - min.z])
    }

    /// Surface style named after `name` with the given shading color.
    fn style(&mut self, name: &str, color: [u8; 3]) -> Id {
        if let Some(&id) = self.styles.get(name) {
            return id;
        }
        let [r, g, b] = color.map(|c| c as f64 / 255.0);
        let rgb = self.w.add("IFCCOLOURRGB", args![name, r, g, b]);
        let shading = self.w.add("IFCSURFACESTYLESHADING", args![rgb, 0.0]);
        let style = self.w.add("IFCSURFACESTYLE", args![name, Value::Enum("BOTH"), vec![shading]]);
        self.styles.insert(name.to_string(), style);
        style
    }

    fn styled(&mut self, item: Id, style: Id) {
        self.w.add("IFCSTYLEDITEM", args![item, vec![style], Value::Null]);
    }

    fn material_style(&mut self, material: &str) -> Id {
        let color = self.project.material_color(material);
        self.style(material, color)
    }

    /// Product shape from body items and an optional 2D axis curve.
    fn shape(&mut self, body: Vec<Id>, axis: Option<Id>) -> Id {
        let mut reps = Vec::new();
        if let Some(curve) = axis {
            reps.push(self.w.add("IFCSHAPEREPRESENTATION", args![self.axis_ctx, "Axis", "Curve2D", vec![curve]]));
        }
        reps.push(self.w.add("IFCSHAPEREPRESENTATION", args![self.body_ctx, "Body", "SweptSolid", body]));
        self.w.add("IFCPRODUCTDEFINITIONSHAPE", args![Value::Null, Value::Null, reps])
    }

    // --- Materials and types ---

    fn material(&mut self, name: &str) -> Id {
        if let Some(&id) = self.materials.get(name) {
            return id;
        }
        let id = self.w.add("IFCMATERIAL", args![name, Value::Null, Value::Null]);
        self.materials.insert(name.to_string(), id);
        id
    }

    /// Layer set of a layered type, or `None` for single-material types.
    fn layer_set(&mut self, type_id: ElementTypeId) -> Option<Id> {
        if let Some(&id) = self.layer_sets.get(&type_id) {
            return Some(id);
        }
        let ty = self.project.element_type(type_id)?;
        if ty.layers().is_empty() {
            return None;
        }
        let (name, layers) = (ty.name.clone(), ty.layers().to_vec());
        let layers: Vec<Id> = layers
            .iter()
            .map(|l| {
                let m = self.material(&l.material);
                let n = Value::Null;
                self.w.add("IFCMATERIALLAYER", args![m, l.thickness, n.clone(), n.clone(), n.clone(), n.clone(), n])
            })
            .collect();
        let id = self.w.add("IFCMATERIALLAYERSET", args![layers, name, Value::Null]);
        self.layer_sets.insert(type_id, id);
        Some(id)
    }

    /// Assign the layer set of `type_id` to `product`, with layers running in
    /// the negative `direction` from `offset` (one usage shared per type).
    fn layer_usage(&mut self, type_id: ElementTypeId, product: Id, direction: &'static str, offset: f64) {
        let Some(set) = self.layer_set(type_id) else { return };
        let usage = match self.layer_usages.get(&type_id) {
            Some(&usage) => usage,
            None => {
                let args = args![set, Value::Enum(direction), Value::Enum("NEGATIVE"), offset, Value::Null];
                let usage = self.w.add("IFCMATERIALLAYERSETUSAGE", args);
                self.layer_usages.insert(type_id, usage);
                usage
            }
        };
        self.usage_objects.entry(usage).or_default().push(product);
    }

    /// The IFC type object for `type_id`, created with its material on first use.
    fn element_type(&mut self, type_id: ElementTypeId) -> Option<Id> {
        if let Some(&id) = self.types.get(&type_id) {
            return Some(id);
        }
        let ty = self.project.element_type(type_id)?.clone();
        let gid = self.guid_of(ty.uid, &format!("type-{type_id}"));
        let n = Value::Null;
        let description = opt_str(&ty.info.description);
        let base =
            args![gid, n.clone(), ty.name.as_str(), description, n.clone(), n.clone(), n.clone(), n.clone(), n.clone()];
        let (entity, extra): (&str, Vec<Value>) = match ty.data {
            TypeData::Wall { .. } => ("IFCWALLTYPE", args![Value::Enum("STANDARD")]),
            TypeData::Floor { .. } => ("IFCSLABTYPE", args![Value::Enum("FLOOR")]),
            TypeData::Ceiling { .. } => ("IFCCOVERINGTYPE", args![Value::Enum("CEILING")]),
            TypeData::Roof { .. } => ("IFCROOFTYPE", args![Value::Enum("NOTDEFINED")]),
            TypeData::Door { .. } => {
                ("IFCDOORTYPE", args![Value::Enum("DOOR"), Value::Enum("NOTDEFINED"), Value::Null, Value::Null])
            }
            TypeData::Window { .. } => {
                ("IFCWINDOWTYPE", args![Value::Enum("WINDOW"), Value::Enum("SINGLE_PANEL"), Value::Null, Value::Null])
            }
        };
        let id = self.w.add(entity, base.into_iter().chain(extra).collect());
        self.types.insert(type_id, id);
        let info = &ty.info;
        let label = |s: &str| Value::Typed("IFCLABEL", Box::new(s.into()));
        let manufacturer: Vec<(&str, Value)> = [("Manufacturer", &info.manufacturer), ("ModelLabel", &info.model)]
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (k, label(v)))
            .collect();
        self.property_set(&format!("mfr-{type_id}"), "Pset_ManufacturerTypeInformation", vec![id], manufacturer);
        let material = match &ty.data {
            TypeData::Door { material, .. }
            | TypeData::Window { material, .. }
            | TypeData::Ceiling { material, .. } => Some(self.material(material)),
            _ => self.layer_set(type_id),
        };
        if let Some(m) = material {
            self.associate_material(&format!("mat-type-{type_id}"), vec![id], m);
        }
        Some(id)
    }

    fn associate_material(&mut self, tag: &str, objects: Vec<Id>, material: Id) {
        let gid = self.guid(tag);
        self.w.add("IFCRELASSOCIATESMATERIAL", args![gid, Value::Null, Value::Null, Value::Null, objects, material]);
    }

    /// Register `product` as an occurrence of its element's type and as contained in its level.
    fn register(&mut self, e: &Element, product: Id) {
        if self.element_type(e.type_id).is_some() {
            self.typed.entry(e.type_id).or_default().push(product);
        }
        self.contained.entry(e.level).or_default().push(product);
    }

    /// Common leading attributes of an element occurrence.
    fn element_args(&self, e: &Element, placement: Id, shape: Id) -> Vec<Value> {
        let gid = self.guid_of(e.uid, &format!("element-{}", e.id));
        let name = self.project.type_name(e.type_id);
        args![gid, Value::Null, name, opt_str(&e.comments), Value::Null, placement, shape, opt_str(&e.mark)]
    }

    fn quantities(&mut self, e: &Element, set_name: &str, quantities: &[(&str, Quantity)]) {
        let qs: Vec<Id> = quantities
            .iter()
            .map(|&(name, q)| {
                let (entity, value) = match q {
                    Quantity::Length(v) => ("IFCQUANTITYLENGTH", v),
                    Quantity::Area(v) => ("IFCQUANTITYAREA", v),
                    Quantity::Volume(v) => ("IFCQUANTITYVOLUME", v),
                };
                self.w.add(entity, args![name, Value::Null, Value::Null, value, Value::Null])
            })
            .collect();
        let gid = self.guid(&format!("qto-{}", e.id));
        let n = Value::Null;
        let set = self.w.add("IFCELEMENTQUANTITY", args![gid, n.clone(), set_name, n.clone(), n, qs]);
        let product = self.products.get(&e.id).copied();
        if let Some(product) = product {
            let gid = self.guid(&format!("qto-rel-{}", e.id));
            self.w.add(
                "IFCRELDEFINESBYPROPERTIES",
                args![gid, Value::Null, Value::Null, Value::Null, vec![product], set],
            );
        }
    }

    // --- Elements ---

    fn storey_placement(&self, level: LevelId) -> Option<Id> {
        self.storeys.get(&level).map(|s| s.1)
    }

    fn write_wall(&mut self, id: ElementId) {
        let project = self.project;
        let Some(e) = project.element(id) else { return };
        let Some(wall) = e.as_wall() else { return };
        let geom = self.geom;
        let Some(g) = geom.walls.get(&id) else { return };
        let Some(storey) = self.storey_placement(e.level) else { return };
        let level_z = project.level_elevation(e.level);
        let placement = self.local_placement(
            Some(storey),
            Frame::horizontal(DVec3::new(g.origin.x, g.origin.y, g.base_z - level_z), g.dir),
        );

        let h = g.half;
        let profile = [
            DVec2::new(g.start_ext.1, -h),
            DVec2::new(g.end_ext.1, -h),
            DVec2::new(g.end_ext.0, h),
            DVec2::new(g.start_ext.0, h),
        ];
        let body = self.extrude_polygon(&profile, self.identity, self.dir_z, g.top_z - g.base_z);
        if let Some(ty) = project.element_type(e.type_id) {
            let style = self.material_style(ty.main_material());
            self.styled(body, style);
        }
        let axis_start = self.origin2;
        let axis_end = self.point2(DVec2::new(g.length, 0.0));
        let axis = self.w.add("IFCPOLYLINE", args![vec![axis_start, axis_end]]);
        let shape = self.shape(vec![body], Some(axis));
        let mut attrs = self.element_args(e, placement, shape);
        attrs.push(Value::Enum("STANDARD"));
        let product = self.w.add("IFCWALL", attrs);
        self.products.insert(e.id, product);
        self.register(e, product);

        // Layers are listed from the left face (+Y) towards the right.
        self.layer_usage(e.type_id, product, "AXIS2", h);

        let ty = project.element_type(e.type_id);
        let mut props = vec![
            ("Reference", Value::Typed("IFCIDENTIFIER", Box::new(project.type_name(e.type_id).into()))),
            ("LoadBearing", Value::Typed("IFCBOOLEAN", Box::new(wall.structural.into()))),
        ];
        if let Some(function) = ty.and_then(|t| t.wall_function()) {
            props.push(("IsExternal", Value::Typed("IFCBOOLEAN", Box::new(function.is_external().into()))));
        }
        if let Some(rating) = ty.map(|t| t.info.fire_rating.clone()).filter(|r| !r.is_empty()) {
            props.push(("FireRating", Value::Typed("IFCLABEL", Box::new(rating.into()))));
        }
        self.property_set(&format!("wall-{}", e.id), "Pset_WallCommon", vec![product], props);

        let opening_area: f64 = g.openings.iter().map(|o| (o.u1 - o.u0) * (o.z1 - o.z0)).sum();
        let thickness = 2.0 * h;
        let height = project.wall_height(e, wall);
        self.quantities(
            e,
            "Qto_WallBaseQuantities",
            &[
                ("Length", Quantity::Length(wall.length())),
                ("Height", Quantity::Length(height)),
                ("Width", Quantity::Length(thickness)),
                ("GrossSideArea", Quantity::Area(wall.length() * height)),
                ("NetSideArea", Quantity::Area((wall.length() * height - opening_area).max(0.0))),
                ("NetVolume", Quantity::Volume(self.geom.volume(e.id))),
            ],
        );

        for o in g.openings.clone() {
            self.write_opening(g, placement, product, &o);
        }
    }

    fn write_opening(&mut self, g: &WallGeom, wall_placement: Id, wall: Id, o: &cc_geom::Opening) {
        let project = self.project;
        let Some(e) = project.element(o.element) else { return };
        let Some(ty) = project.element_type(e.type_id) else { return };
        let (width, height) = (o.u1 - o.u0, o.z1 - o.z0);
        let reach = g.half + OPENING_CLEARANCE;

        let placement = self.local_placement(Some(wall_placement), Frame::at(DVec3::new(o.u0, 0.0, o.z0 - g.base_z)));
        let body = self.extrude_box(DVec3::new(0.0, -reach, 0.0), DVec3::new(width, reach, height));
        let shape = self.shape(vec![body], None);
        let gid = self.guid(&format!("opening-{}", e.id));
        let n = Value::Null;
        let opening = self.w.add(
            "IFCOPENINGELEMENT",
            args![gid, n.clone(), "Opening", n.clone(), n.clone(), placement, shape, n, Value::Enum("OPENING")],
        );
        let gid = self.guid(&format!("voids-{}", e.id));
        self.w.add("IFCRELVOIDSELEMENT", args![gid, Value::Null, Value::Null, Value::Null, wall, opening]);

        // The filling shares the opening's frame.
        let fill_placement = self.local_placement(Some(placement), Frame::at(DVec3::ZERO));
        let frame_style = self.style("Frame", FRAME_COLOR);
        let panel_style = self.material_style(ty.main_material());
        let parts: Vec<HostedPart> = hosted_parts(&ty.data, width, height, g.half);
        let mut items = Vec::new();
        for p in parts {
            if (p.max - p.min).min_element() < 1e-6 {
                continue;
            }
            let item = self.extrude_box(p.min, p.max);
            self.styled(item, if p.panel { panel_style } else { frame_style });
            items.push(item);
        }
        let shape = self.shape(items, None);
        let (type_w, type_h) = ty.opening_size().unwrap_or((width, height));
        let mut attrs = self.element_args(e, fill_placement, shape);
        attrs.extend(args![type_h, type_w]);
        let (entity, set_name) = match ty.data {
            TypeData::Door { .. } => {
                attrs.extend(args![Value::Enum("DOOR"), Value::Enum("NOTDEFINED"), Value::Null]);
                ("IFCDOOR", "Qto_DoorBaseQuantities")
            }
            _ => {
                attrs.extend(args![Value::Enum("WINDOW"), Value::Enum("SINGLE_PANEL"), Value::Null]);
                ("IFCWINDOW", "Qto_WindowBaseQuantities")
            }
        };
        let product = self.w.add(entity, attrs);
        self.products.insert(e.id, product);
        self.register(e, product);
        let gid = self.guid(&format!("fills-{}", e.id));
        self.w.add("IFCRELFILLSELEMENT", args![gid, Value::Null, Value::Null, Value::Null, opening, product]);
        self.quantities(
            e,
            set_name,
            &[
                ("Width", Quantity::Length(type_w)),
                ("Height", Quantity::Length(type_h)),
                ("Area", Quantity::Area(type_w * type_h)),
            ],
        );
    }

    /// Floors, ceilings and roofs: extruded sketches.
    fn write_sketched(&mut self, id: ElementId) {
        let project = self.project;
        let Some(e) = project.element(id) else { return };
        let Some(ty) = project.element_type(e.type_id) else { return };
        let Some(storey) = self.storey_placement(e.level) else { return };
        let thickness = ty.thickness();
        if thickness < 1e-6 {
            return;
        }

        let (frame, profile, dir, depth, entity, kind) = match &e.kind {
            ElementKind::Floor(f) => {
                let z = f.height_offset - thickness;
                (Frame::at(DVec3::new(0.0, 0.0, z)), polygon::ccw(&f.boundary), DVec3::Z, thickness, "IFCSLAB", "FLOOR")
            }
            ElementKind::Ceiling(c) => {
                let z = c.height_offset;
                (
                    Frame::at(DVec3::new(0.0, 0.0, z)),
                    polygon::ccw(&c.boundary),
                    DVec3::Z,
                    thickness,
                    "IFCCOVERING",
                    "CEILING",
                )
            }
            ElementKind::Roof(r) => {
                let fp = polygon::ccw(&polygon::offset(&r.footprint, r.overhang));
                match r.shape {
                    RoofShape::Flat => (
                        Frame::at(DVec3::new(0.0, 0.0, r.base_offset)),
                        fp,
                        DVec3::Z,
                        thickness,
                        "IFCROOF",
                        "FLAT_ROOF",
                    ),
                    RoofShape::Shed { slope, direction_deg } => {
                        // The profile lies on the sloped underside; the extrusion is vertical,
                        // which is oblique in the profile's frame.
                        let a = direction_deg.to_radians();
                        let d = DVec2::new(a.cos(), a.sin());
                        let across = d.perp();
                        let min = fp.iter().map(|p| p.dot(d)).fold(f64::INFINITY, f64::min);
                        let stretch = (1.0 + slope * slope).sqrt();
                        let c = 1.0 / stretch;
                        let frame = Frame {
                            origin: DVec3::new(0.0, 0.0, r.base_offset - slope * min),
                            x: DVec3::new(d.x * c, d.y * c, slope * c),
                            z: DVec3::new(-slope * c * d.x, -slope * c * d.y, c),
                        };
                        let profile = fp.iter().map(|p| DVec2::new(p.dot(d) * stretch, p.dot(across))).collect();
                        (frame, profile, DVec3::new(slope * c, 0.0, c), thickness * stretch, "IFCROOF", "SHED_ROOF")
                    }
                }
            }
            _ => return,
        };
        if profile.len() < 3 {
            return;
        }
        let placement = self.local_placement(Some(storey), frame);
        let dir = self.direction(dir);
        let body = self.extrude_polygon(&profile, self.identity, dir, depth);
        let style = self.material_style(ty.main_material());
        self.styled(body, style);
        let shape = self.shape(vec![body], None);
        let mut attrs = self.element_args(e, placement, shape);
        attrs.push(Value::Enum(kind));
        let product = self.w.add(entity, attrs);
        self.products.insert(e.id, product);
        self.register(e, product);

        let volume = self.geom.volume(e.id);
        match &e.kind {
            ElementKind::Floor(f) => {
                // Layers are listed top first: start at the top face and go down.
                self.layer_usage(e.type_id, product, "AXIS3", thickness);
                let area = polygon::area(&f.boundary);
                self.quantities(
                    e,
                    "Qto_SlabBaseQuantities",
                    &[
                        ("Width", Quantity::Length(thickness)),
                        ("Perimeter", Quantity::Length(polygon::perimeter(&f.boundary))),
                        ("NetArea", Quantity::Area(area)),
                        ("NetVolume", Quantity::Volume(volume)),
                    ],
                );
            }
            ElementKind::Ceiling(c) => {
                let area = polygon::area(&c.boundary);
                self.quantities(
                    e,
                    "Qto_CoveringBaseQuantities",
                    &[("Width", Quantity::Length(thickness)), ("NetArea", Quantity::Area(area))],
                );
            }
            ElementKind::Roof(r) => {
                let projected = polygon::area(&polygon::offset(&r.footprint, r.overhang));
                let slope = match r.shape {
                    RoofShape::Flat => 0.0,
                    RoofShape::Shed { slope, .. } => slope,
                };
                let gross = projected * (1.0 + slope * slope).sqrt();
                self.quantities(
                    e,
                    "Qto_RoofBaseQuantities",
                    &[("GrossArea", Quantity::Area(gross)), ("ProjectedArea", Quantity::Area(projected))],
                );
            }
            _ => {}
        }
    }

    // --- Relationships collected while writing elements ---

    fn write_relationships(&mut self) {
        let n = Value::Null;
        for (type_id, objects) in std::mem::take(&mut self.typed) {
            let Some(&ty) = self.types.get(&type_id) else { continue };
            let gid = self.guid(&format!("deftype-{type_id}"));
            self.w.add("IFCRELDEFINESBYTYPE", args![gid, n.clone(), n.clone(), n.clone(), objects, ty]);
        }
        for (i, (usage, objects)) in std::mem::take(&mut self.usage_objects).into_iter().enumerate() {
            self.associate_material(&format!("mat-usage-{i}"), objects, usage);
        }
        for (level, objects) in std::mem::take(&mut self.contained) {
            let Some(&(storey, _)) = self.storeys.get(&level) else { continue };
            let gid = self.guid(&format!("contains-{level}"));
            self.w
                .add("IFCRELCONTAINEDINSPATIALSTRUCTURE", args![gid, n.clone(), n.clone(), n.clone(), objects, storey]);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Quantity {
    Length(f64),
    Area(f64),
    Volume(f64),
}

/// Current UTC time as `YYYY-MM-DDThh:mm:ss`.
fn now_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_core::Category;
    use std::collections::HashSet;

    fn sample() -> Project {
        cc_core::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap()
    }

    fn export_sample() -> String {
        let options = ExportOptions { time_stamp: Some("2026-01-01T00:00:00".into()), file_name: "casa.ifc".into() };
        export(&sample(), &options)
    }

    fn count(text: &str, entity: &str) -> usize {
        text.matches(&format!("={entity}(")).count()
    }

    #[test]
    fn exports_every_element() {
        let project = sample();
        let text = export_sample();
        assert!(text.contains("FILE_SCHEMA(('IFC4'));"));
        assert!(text.contains("FILE_NAME('casa.ifc','2026-01-01T00:00:00',"));
        assert_eq!(count(&text, "IFCPROJECT"), 1);
        assert_eq!(count(&text, "IFCBUILDINGSTOREY"), project.levels.len());
        assert_eq!(count(&text, "IFCWALL"), project.elements_of(Category::Wall).count());
        let doors = project.elements_of(Category::Door).count();
        let windows = project.elements_of(Category::Window).count();
        assert!(doors > 0 && windows > 0);
        assert_eq!(count(&text, "IFCDOOR"), doors);
        assert_eq!(count(&text, "IFCWINDOW"), windows);
        assert_eq!(count(&text, "IFCOPENINGELEMENT"), doors + windows);
        assert_eq!(count(&text, "IFCRELFILLSELEMENT"), doors + windows);
        assert_eq!(count(&text, "IFCSLAB"), project.elements_of(Category::Floor).count());
        assert_eq!(count(&text, "IFCCOVERING"), project.elements_of(Category::Ceiling).count());
        assert_eq!(count(&text, "IFCROOF"), project.elements_of(Category::Roof).count());
    }

    #[test]
    fn references_are_defined_and_ids_unique() {
        const ROOTED: &[&str] = &[
            "IFCPROJECT",
            "IFCSITE",
            "IFCBUILDING",
            "IFCBUILDINGSTOREY",
            "IFCWALL",
            "IFCOPENINGELEMENT",
            "IFCDOOR",
            "IFCWINDOW",
            "IFCSLAB",
            "IFCCOVERING",
            "IFCROOF",
            "IFCELEMENTQUANTITY",
        ];
        let text = export_sample();
        let data = &text[text.find("DATA;\n").unwrap()..];
        let mut defined = HashSet::new();
        let mut guids = HashSet::new();
        for line in data.lines().filter(|l| l.starts_with('#')) {
            let (name, rest) = line.split_once('=').unwrap();
            assert!(defined.insert(name.to_string()), "duplicate {name}");
            let (entity, args) = rest.split_once('(').unwrap();
            let rooted = entity.starts_with("IFCREL") || entity.ends_with("TYPE") || ROOTED.contains(&entity);
            if rooted {
                let gid = &args[1..23];
                assert_eq!(&args[23..24], "'", "{line}");
                assert!(guids.insert(gid.to_string()), "duplicate GlobalId {gid} in {line}");
            }
        }
        for line in data.lines().filter(|l| l.starts_with('#')) {
            let body = line.split_once('=').unwrap().1;
            let bytes = body.as_bytes();
            let mut i = 0;
            let mut in_str = false;
            while i < bytes.len() {
                match bytes[i] {
                    b'\'' => in_str = !in_str,
                    b'#' if !in_str => {
                        let end = (i + 1..bytes.len()).find(|&j| !bytes[j].is_ascii_digit()).unwrap_or(bytes.len());
                        let name = &body[i..end];
                        assert!(defined.contains(name), "undefined {name} in {line}");
                        i = end;
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }
        }
    }

    #[test]
    fn global_ids_come_from_persistent_ids() {
        let project = sample();
        let text = export(&project, &ExportOptions::default());
        let wall = project.elements_of(Category::Wall).next().unwrap();
        assert!(text.contains(&format!("IFCWALL('{}'", encode(wall.uid.0))));
        assert!(text.contains(&format!("IFCPROJECT('{}'", encode(project.info.uid.0))));
        assert!(text.contains("'Pset_WallCommon'"));
    }

    #[test]
    fn export_is_deterministic() {
        assert_eq!(export_sample(), export_sample());
    }

    #[test]
    fn non_ascii_names_are_encoded() {
        let text = export_sample();
        assert!(text.is_ascii());
        // "Térreo", the ground level of the sample.
        assert!(text.contains(r"'T\X2\00E9\X0\rreo'"));
    }

    #[test]
    fn time_stamp_format() {
        let ts = now_utc();
        assert_eq!(ts.len(), 19);
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
    }
}
