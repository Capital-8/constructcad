use crate::material::Material;
use crate::{
    CameraState, Category, ColumnSection, Direction, Element, ElementId, ElementKind, ElementType, ElementTypeId,
    Layer, LayerFunction, LevelId, SheetSize, TypeData, View, ViewCategory, ViewId, ViewKind, Viewport, WallFunction,
};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Persistent unique id of the project.
    #[serde(default)]
    pub uid: crate::Uid,
    pub name: String,
    pub number: String,
    pub client: String,
    pub address: String,
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub id: LevelId,
    pub name: String,
    /// Elevation in meters.
    pub elevation: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub info: ProjectInfo,
    /// Sorted by elevation.
    pub levels: Vec<Level>,
    pub materials: Vec<Material>,
    pub types: BTreeMap<ElementTypeId, ElementType>,
    pub elements: BTreeMap<ElementId, Element>,
    pub views: BTreeMap<ViewId, View>,
    /// How quantities are displayed and typed.
    #[serde(default)]
    pub units: crate::Units,
    /// Elements protected from moving and deleting.
    #[serde(default)]
    pub pinned: std::collections::BTreeSet<ElementId>,
    next_id: u64,
}

impl Project {
    /// An empty project with no levels, types or views.
    pub fn empty() -> Self {
        Self {
            info: ProjectInfo::default(),
            levels: Vec::new(),
            materials: Vec::new(),
            types: BTreeMap::new(),
            elements: BTreeMap::new(),
            views: BTreeMap::new(),
            units: crate::Units::default(),
            pinned: Default::default(),
            next_id: 1,
        }
    }

    /// A new project from the default architectural template (Brazilian Portuguese names).
    pub fn from_template() -> Self {
        Self::from_template_localized(&|_| None)
    }

    /// A new project from the template, naming levels, materials, types and views
    /// with `translate(key)` (keys `tpl-*`), falling back to Portuguese.
    pub fn from_template_localized(translate: &dyn Fn(&str) -> Option<String>) -> Self {
        let n = |key: &str| translate(key).unwrap_or_else(|| template_default(key).to_string());
        let mut p = Self::empty();
        p.info = ProjectInfo {
            uid: crate::Uid::default(),
            name: n("tpl-project-name"),
            number: "0001".into(),
            client: String::new(),
            address: String::new(),
            author: String::new(),
        };
        let ground = LevelId(p.alloc_id());
        let first = LevelId(p.alloc_id());
        p.levels = vec![
            Level { id: ground, name: n("tpl-level-ground"), elevation: 0.0 },
            Level { id: first, name: n("tpl-level-first"), elevation: 3.0 },
        ];
        use crate::material::MaterialClass as C;
        p.materials = [
            (&n("tpl-mat-masonry"), [196, 120, 92], C::Masonry),
            (&n("tpl-mat-plaster"), [232, 226, 214], C::Mortar),
            (&n("tpl-mat-concrete"), [170, 170, 165], C::Concrete),
            (&n("tpl-mat-wood"), [150, 104, 62], C::Wood),
            (&n("tpl-mat-glass"), [150, 200, 225], C::Glass),
            (&n("tpl-mat-gypsum"), [245, 245, 240], C::Gypsum),
            (&n("tpl-mat-roof-tile"), [168, 70, 48], C::Ceramic),
            (&n("tpl-mat-screed"), [190, 185, 175], C::Mortar),
            (&n("tpl-mat-fabric"), [120, 132, 150], C::Fabric),
            (&n("tpl-mat-porcelain"), [240, 240, 236], C::Ceramic),
        ]
        .into_iter()
        .map(|(name, color, class)| {
            let mut m = Material::new(name.as_str(), color);
            crate::material::template_properties(&mut m, class);
            m
        })
        .collect();

        let layer = |m: &str, t: f64| Layer { material: m.into(), thickness: t, function: LayerFunction::Structure };
        let finish = |m: &str, t: f64| Layer { material: m.into(), thickness: t, function: LayerFunction::Finish };
        let types = [
            (
                &n("tpl-type-masonry-15"),
                TypeData::Wall {
                    layers: vec![
                        finish(&n("tpl-mat-plaster"), 0.02),
                        layer(&n("tpl-mat-masonry"), 0.11),
                        finish(&n("tpl-mat-plaster"), 0.02),
                    ],
                    function: WallFunction::Exterior,
                },
            ),
            (
                &n("tpl-type-masonry-25"),
                TypeData::Wall {
                    layers: vec![
                        finish(&n("tpl-mat-plaster"), 0.025),
                        layer(&n("tpl-mat-masonry"), 0.20),
                        finish(&n("tpl-mat-plaster"), 0.025),
                    ],
                    function: WallFunction::Exterior,
                },
            ),
            (
                &n("tpl-type-drywall-10"),
                TypeData::Wall { layers: vec![layer(&n("tpl-mat-gypsum"), 0.10)], function: WallFunction::Interior },
            ),
            (&n("tpl-type-door-80"), TypeData::Door { width: 0.8, height: 2.1, material: n("tpl-mat-wood") }),
            (&n("tpl-type-door-90"), TypeData::Door { width: 0.9, height: 2.1, material: n("tpl-mat-wood") }),
            (
                &n("tpl-type-window-120"),
                TypeData::Window { width: 1.2, height: 1.2, default_sill: 1.0, material: n("tpl-mat-glass") },
            ),
            (
                &n("tpl-type-window-150"),
                TypeData::Window { width: 1.5, height: 1.0, default_sill: 1.1, material: n("tpl-mat-glass") },
            ),
            (
                &n("tpl-type-slab-12"),
                TypeData::Floor {
                    layers: vec![finish(&n("tpl-mat-screed"), 0.03), layer(&n("tpl-mat-concrete"), 0.12)],
                },
            ),
            (&n("tpl-type-ceiling-gypsum"), TypeData::Ceiling { thickness: 0.02, material: n("tpl-mat-gypsum") }),
            (
                &n("tpl-type-column-rect"),
                TypeData::Column {
                    section: ColumnSection::Rectangular { width: 0.2, depth: 0.2 },
                    material: n("tpl-mat-concrete"),
                },
            ),
            (
                &n("tpl-type-column-round"),
                TypeData::Column { section: ColumnSection::Round { diameter: 0.3 }, material: n("tpl-mat-concrete") },
            ),
            (&n("tpl-type-beam"), TypeData::Beam { width: 0.15, height: 0.4, material: n("tpl-mat-concrete") }),
            (
                &n("tpl-type-stair"),
                TypeData::Stair { width: 1.0, max_riser: 0.18, tread: 0.28, material: n("tpl-mat-concrete") },
            ),
            (&n("tpl-type-railing"), TypeData::Railing { height: 1.1, post_spacing: 1.2, material: n("tpl-mat-wood") }),
            (&n("tpl-type-room"), TypeData::Room {}),
            (&n("tpl-type-grid"), TypeData::Grid { bubble: 8.0 }),
            (&n("tpl-type-dimension"), TypeData::Dimension { text_size: 2.5 }),
            (&n("tpl-type-text"), TypeData::Text { size: 2.5, symbol: crate::LeaderSymbol::Arrow }),
            (&n("tpl-type-text-large"), TypeData::Text { size: 3.5, symbol: crate::LeaderSymbol::Arrow }),
            (
                &n("tpl-type-roof-tile"),
                TypeData::Roof { layers: vec![finish(&n("tpl-mat-roof-tile"), 0.05), layer(&n("tpl-mat-wood"), 0.10)] },
            ),
        ];
        for (name, data) in types {
            let id = ElementTypeId(p.alloc_id());
            p.types.insert(
                id,
                ElementType { id, name: name.into(), data, info: Default::default(), uid: Default::default() },
            );
        }
        for (component, name, def) in crate::component::library(&n) {
            let id = ElementTypeId(p.alloc_id());
            let data = TypeData::Component { component, def };
            p.types.insert(id, ElementType { id, name, data, info: Default::default(), uid: Default::default() });
        }

        let add_view = |p: &mut Project, name: &str, scale: u32, kind: ViewKind| {
            let id = ViewId(p.alloc_id());
            p.views.insert(id, View { id, name: name.into(), scale, kind });
            id
        };
        let plan = add_view(&mut p, &n("tpl-level-ground"), 50, ViewKind::FloorPlan { level: ground, cut_height: 1.2 });
        add_view(&mut p, &n("tpl-level-first"), 50, ViewKind::FloorPlan { level: first, cut_height: 1.2 });
        add_view(&mut p, &n("tpl-level-ground"), 50, ViewKind::CeilingPlan { level: ground, cut_height: 1.2 });
        add_view(&mut p, &n("tpl-level-first"), 50, ViewKind::CeilingPlan { level: first, cut_height: 1.2 });
        let threed = add_view(&mut p, "{3D}", 100, ViewKind::ThreeD { camera: CameraState::default() });
        for (name, direction) in [
            (&n("tpl-elev-north"), Direction::North),
            (&n("tpl-elev-south"), Direction::South),
            (&n("tpl-elev-east"), Direction::East),
            (&n("tpl-elev-west"), Direction::West),
        ] {
            add_view(&mut p, name, 100, ViewKind::Elevation { direction });
        }
        let all_types: Vec<_> = p.types.values().filter(|t| t.category().is_model()).map(|t| t.id).collect();
        add_view(&mut p, &n("tpl-legend"), 50, ViewKind::Legend { types: all_types });
        for (name, category) in [
            (&n("tpl-schedule-doors"), Category::Door),
            (&n("tpl-schedule-windows"), Category::Window),
            (&n("tpl-schedule-walls"), Category::Wall),
            (&n("tpl-schedule-rooms"), Category::Room),
        ] {
            add_view(&mut p, name, 1, ViewKind::Schedule { category, def: Default::default() });
        }
        add_view(
            &mut p,
            &n("tpl-sheet-plans"),
            1,
            ViewKind::Sheet {
                number: "A101".into(),
                size: SheetSize::A3,
                viewports: vec![
                    Viewport { view: plan, center: DVec2::new(130.0, 160.0) },
                    Viewport { view: threed, center: DVec2::new(320.0, 190.0) },
                ],
            },
        );
        p.ensure_uids();
        p
    }

    /// Assign persistent unique ids to the project, types and elements that
    /// lack one, and replace duplicates (e.g. from copied elements).
    pub fn ensure_uids(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut fresh = |uid: &mut crate::Uid| {
            if uid.is_nil() || !seen.insert(*uid) {
                *uid = crate::Uid::generate();
                seen.insert(*uid);
            }
        };
        fresh(&mut self.info.uid);
        self.types.values_mut().for_each(|t| fresh(&mut t.uid));
        self.elements.values_mut().for_each(|e| fresh(&mut e.uid));
    }

    /// Add default room, grid and dimension types to projects created before
    /// those categories existed.
    pub fn ensure_annotation_types(&mut self) {
        let defaults = [
            (
                Category::Column,
                "tpl-type-column-rect",
                TypeData::Column {
                    section: ColumnSection::Rectangular { width: 0.2, depth: 0.2 },
                    material: template_default("tpl-mat-concrete").to_string(),
                },
            ),
            (
                Category::Beam,
                "tpl-type-beam",
                TypeData::Beam { width: 0.15, height: 0.4, material: template_default("tpl-mat-concrete").to_string() },
            ),
            (
                Category::Stair,
                "tpl-type-stair",
                TypeData::Stair {
                    width: 1.0,
                    max_riser: 0.18,
                    tread: 0.28,
                    material: template_default("tpl-mat-concrete").to_string(),
                },
            ),
            (
                Category::Railing,
                "tpl-type-railing",
                TypeData::Railing {
                    height: 1.1,
                    post_spacing: 1.2,
                    material: template_default("tpl-mat-wood").to_string(),
                },
            ),
            (Category::Room, "tpl-type-room", TypeData::Room {}),
            (Category::Grid, "tpl-type-grid", TypeData::Grid { bubble: 8.0 }),
            (Category::Dimension, "tpl-type-dimension", TypeData::Dimension { text_size: 2.5 }),
            (Category::Text, "tpl-type-text", TypeData::Text { size: 2.5, symbol: crate::LeaderSymbol::Arrow }),
        ];
        for (category, key, data) in defaults {
            if self.default_type(category).is_none() {
                let id = ElementTypeId(self.alloc_id());
                let name = template_default(key).to_string();
                self.types
                    .insert(id, ElementType { id, name, data, info: Default::default(), uid: crate::Uid::generate() });
            }
        }
        // Projects created before components get the default library.
        if self.default_type(Category::Component).is_none() {
            let n = |key: &str| template_default(key).to_string();
            use crate::material::MaterialClass as C;
            for (key, color, class) in
                [("tpl-mat-fabric", [120, 132, 150], C::Fabric), ("tpl-mat-porcelain", [240, 240, 236], C::Ceramic)]
            {
                let name = n(key);
                if !self.materials.iter().any(|m| m.name == name) {
                    let mut m = Material::new(name, color);
                    crate::material::template_properties(&mut m, class);
                    self.materials.push(m);
                }
            }
            for (component, name, def) in crate::component::library(&n) {
                let id = ElementTypeId(self.alloc_id());
                let data = TypeData::Component { component, def };
                self.types
                    .insert(id, ElementType { id, name, data, info: Default::default(), uid: crate::Uid::generate() });
            }
        }
    }

    /// The level above `id`, or `id` itself plus `fallback` meters: the default upper limit of rooms.
    pub fn level_height(&self, id: LevelId, fallback: f64) -> f64 {
        self.level_above(id).map_or(fallback, |above| above.elevation - self.level_elevation(id))
    }

    /// Base elevation and top elevation of a column (top at least 1 cm above).
    pub fn column_span(&self, element: &Element, column: &crate::Column) -> (f64, f64) {
        let base = self.level_elevation(element.level) + column.base_offset;
        let top = match column.top {
            crate::WallTop::Unconnected => base + column.height,
            crate::WallTop::Level { level, offset } => self.level_elevation(level) + offset,
        };
        (base, top.max(base + 0.01))
    }

    /// Base and top elevation of a stair (the top at least one riser above).
    pub fn stair_span(&self, element: &Element, stair: &crate::Stair) -> (f64, f64) {
        let base = self.level_elevation(element.level) + stair.base_offset;
        let top = match stair.top {
            crate::WallTop::Unconnected => base + stair.height,
            crate::WallTop::Level { level, offset } => self.level_elevation(level) + offset,
        };
        (base, top.max(base + 0.05))
    }

    /// Elevation of a wall's base.
    pub fn wall_base_z(&self, element: &Element, wall: &crate::Wall) -> f64 {
        self.level_elevation(element.level) + wall.base_offset
    }

    /// Elevation of a wall's top, from its top constraint (at least 1 cm above the base).
    pub fn wall_top_z(&self, element: &Element, wall: &crate::Wall) -> f64 {
        let base = self.wall_base_z(element, wall);
        let top = match wall.top {
            crate::WallTop::Unconnected => base + wall.height,
            crate::WallTop::Level { level, offset } => self.level_elevation(level) + offset,
        };
        top.max(base + 0.01)
    }

    /// Effective height of a wall.
    pub fn wall_height(&self, element: &Element, wall: &crate::Wall) -> f64 {
        self.wall_top_z(element, wall) - self.wall_base_z(element, wall)
    }

    /// Start and end of a wall's body centerline: the location line shifted
    /// across the wall according to `location_line` and the type's layers
    /// (listed from the exterior face, on the left of the line).
    pub fn wall_axis(&self, element: &Element, wall: &crate::Wall) -> (DVec2, DVec2) {
        use crate::LocationLine as L;
        let Some(ty) = self.element_type(element.type_id) else { return (wall.start, wall.end) };
        let half = ty.thickness() * 0.5;
        let (core_ext, core_int) = ty.core_range();
        // Position of the location line across the wall, measured from the
        // centerline towards the exterior face.
        let line = match wall.location_line {
            L::Centerline => 0.0,
            L::FinishExterior => half,
            L::FinishInterior => -half,
            L::CoreExterior => half - core_ext,
            L::CoreInterior => half - core_int,
            L::CoreCenter => half - (core_ext + core_int) * 0.5,
        };
        let shift = wall.normal() * -line;
        (wall.start + shift, wall.end + shift)
    }

    /// Allocate a new unique id (shared by all id kinds).
    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn level(&self, id: LevelId) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    pub fn level_elevation(&self, id: LevelId) -> f64 {
        self.level(id).map_or(0.0, |l| l.elevation)
    }

    pub fn level_name(&self, id: LevelId) -> &str {
        self.level(id).map_or("?", |l| l.name.as_str())
    }

    pub fn is_pinned(&self, id: ElementId) -> bool {
        self.pinned.contains(&id)
    }

    pub fn element(&self, id: ElementId) -> Option<&Element> {
        self.elements.get(&id)
    }

    pub fn element_type(&self, id: ElementTypeId) -> Option<&ElementType> {
        self.types.get(&id)
    }

    pub fn type_name(&self, id: ElementTypeId) -> &str {
        self.types.get(&id).map_or("?", |t| t.name.as_str())
    }

    /// Wall thickness for an element's type (0 if not a layered type).
    pub fn thickness_of(&self, element: &Element) -> f64 {
        self.element_type(element.type_id).map_or(0.0, |t| t.thickness())
    }

    pub fn types_of(&self, category: Category) -> impl Iterator<Item = &ElementType> {
        self.types.values().filter(move |t| t.category() == category)
    }

    pub fn default_type(&self, category: Category) -> Option<ElementTypeId> {
        self.types_of(category).next().map(|t| t.id)
    }

    pub fn material_color(&self, name: &str) -> [u8; 3] {
        self.material(name).map_or([200, 200, 200], |m| m.shading_color())
    }

    /// Transparency of material `name` (0 opaque, 1 invisible).
    pub fn material_transparency(&self, name: &str) -> f64 {
        self.material(name).map_or(0.0, |m| m.graphics.transparency.clamp(0.0, 1.0))
    }

    pub fn material(&self, name: &str) -> Option<&Material> {
        self.materials.iter().find(|m| m.name == name)
    }

    /// Types that use the material `name`.
    pub fn material_users(&self, name: &str) -> Vec<ElementTypeId> {
        self.types.values().filter(|t| t.data.uses_material(name)).map(|t| t.id).collect()
    }

    /// Doors and windows hosted by `wall`.
    pub fn hosted_by(&self, wall: ElementId) -> impl Iterator<Item = &Element> {
        self.elements.values().filter(move |e| e.as_hosted().is_some_and(|h| h.host == wall))
    }

    pub fn elements_of(&self, category: Category) -> impl Iterator<Item = &Element> {
        self.elements.values().filter(move |e| e.category() == category)
    }

    pub fn walls(&self) -> impl Iterator<Item = (&Element, &crate::Wall)> {
        self.elements.values().filter_map(|e| match &e.kind {
            ElementKind::Wall(w) => Some((e, w)),
            _ => None,
        })
    }

    pub fn views_of(&self, category: ViewCategory) -> impl Iterator<Item = &View> {
        self.views.values().filter(move |v| v.category() == category)
    }

    pub fn view(&self, id: ViewId) -> Option<&View> {
        self.views.get(&id)
    }

    /// A fresh mark for a new element: the next number among its category.
    pub fn next_mark(&self, category: Category) -> String {
        let max = self.elements_of(category).filter_map(|e| e.mark.parse::<u32>().ok()).max().unwrap_or(0);
        (max + 1).to_string()
    }

    /// The level directly above `id`, if any.
    pub fn level_above(&self, id: LevelId) -> Option<&Level> {
        let elevation = self.level_elevation(id);
        self.levels.iter().filter(|l| l.elevation > elevation + 1e-9).min_by(|a, b| a.elevation.total_cmp(&b.elevation))
    }

    pub fn sort_levels(&mut self) {
        self.levels.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
    }
}

/// Portuguese names of the default template, keyed like the translation files.
fn template_default(key: &str) -> &'static str {
    match key {
        "tpl-project-name" => "Novo projeto",
        "tpl-level-ground" => "Térreo",
        "tpl-level-first" => "Pavimento 1",
        "tpl-type-masonry-15" => "Alvenaria 15 cm",
        "tpl-type-masonry-25" => "Alvenaria 25 cm",
        "tpl-type-drywall-10" => "Divisória gesso 10 cm",
        "tpl-type-door-80" => "Porta 80x210",
        "tpl-type-door-90" => "Porta 90x210",
        "tpl-type-window-120" => "Janela 120x120",
        "tpl-type-window-150" => "Janela 150x100",
        "tpl-type-slab-12" => "Laje 12 cm",
        "tpl-type-ceiling-gypsum" => "Forro de gesso",
        "tpl-type-roof-tile" => "Telhado cerâmico",
        "tpl-mat-roof-tile" => "Telha cerâmica",
        "tpl-mat-masonry" => "Alvenaria",
        "tpl-mat-plaster" => "Reboco",
        "tpl-mat-concrete" => "Concreto",
        "tpl-mat-wood" => "Madeira",
        "tpl-mat-glass" => "Vidro",
        "tpl-mat-gypsum" => "Gesso",
        "tpl-mat-screed" => "Contrapiso",
        "tpl-elev-north" => "Norte",
        "tpl-elev-south" => "Sul",
        "tpl-elev-east" => "Leste",
        "tpl-elev-west" => "Oeste",
        "tpl-legend" => "Legenda de tipos",
        "tpl-schedule-doors" => "Tabela de portas",
        "tpl-schedule-windows" => "Tabela de janelas",
        "tpl-schedule-walls" => "Tabela de paredes",
        "tpl-sheet-plans" => "Plantas",
        "tpl-type-room" => "Ambiente",
        "tpl-type-column-rect" => "Pilar 20 x 20 cm",
        "tpl-type-column-round" => "Pilar circular 30 cm",
        "tpl-type-beam" => "Viga 15 x 40 cm",
        "tpl-type-stair" => "Escada 1,00 m",
        "tpl-type-railing" => "Guarda-corpo 1,10 m",
        "tpl-type-grid" => "Eixo 8 mm",
        "tpl-type-dimension" => "Cota linear 2,5 mm",
        "tpl-type-text" => "Texto 2,5 mm",
        "tpl-type-text-large" => "Texto 3,5 mm",
        "tpl-schedule-rooms" => "Tabela de ambientes",
        "tpl-room-name" => "Ambiente",
        "tpl-mat-fabric" => "Tecido",
        "tpl-mat-porcelain" => "Louça",
        "tpl-param-width" => "Largura",
        "tpl-param-depth" => "Profundidade",
        "tpl-param-height" => "Altura",
        "tpl-param-seat-height" => "AlturaAssento",
        "tpl-part-top" => "Tampo",
        "tpl-part-leg" => "Perna",
        "tpl-part-seat" => "Assento",
        "tpl-part-back" => "Encosto",
        "tpl-part-arm" => "Braço",
        "tpl-part-mattress" => "Colchão",
        "tpl-part-headboard" => "Cabeceira",
        "tpl-part-bowl" => "Bacia",
        "tpl-part-tank" => "Caixa acoplada",
        "tpl-part-basin" => "Cuba",
        "tpl-part-pedestal" => "Coluna",
        "tpl-part-body" => "Corpo",
        "tpl-comp-table" => "Mesa",
        "tpl-comp-chair" => "Cadeira",
        "tpl-comp-bed" => "Cama",
        "tpl-comp-sofa" => "Sofá",
        "tpl-comp-toilet" => "Bacia sanitária",
        "tpl-comp-washbasin" => "Lavatório",
        "tpl-comp-wardrobe" => "Armário",
        "tpl-type-table" => "Mesa 120 x 80",
        "tpl-type-chair" => "Cadeira simples",
        "tpl-type-bed" => "Cama casal 140 x 190",
        "tpl-type-sofa" => "Sofá 3 lugares",
        "tpl-type-toilet" => "Bacia com caixa acoplada",
        "tpl-type-washbasin" => "Lavatório de coluna",
        "tpl-type-wardrobe" => "Armário 160 cm",
        _ => "?",
    }
}
