use crate::{
    CameraState, Category, Direction, Element, ElementId, ElementKind, ElementType, ElementTypeId, Layer, LevelId,
    SheetSize, TypeData, View, ViewCategory, ViewId, ViewKind, Viewport,
};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProjectInfo {
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
pub struct Material {
    pub name: String,
    /// sRGB shading color.
    pub color: [u8; 3],
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
        p.materials = [
            (&n("tpl-mat-masonry"), [196, 120, 92]),
            (&n("tpl-mat-plaster"), [232, 226, 214]),
            (&n("tpl-mat-concrete"), [170, 170, 165]),
            (&n("tpl-mat-wood"), [150, 104, 62]),
            (&n("tpl-mat-glass"), [150, 200, 225]),
            (&n("tpl-mat-gypsum"), [245, 245, 240]),
            (&n("tpl-mat-roof-tile"), [168, 70, 48]),
            (&n("tpl-mat-screed"), [190, 185, 175]),
        ]
        .into_iter()
        .map(|(name, color)| Material { name: name.into(), color })
        .collect();

        let layer = |m: &str, t: f64| Layer { material: m.into(), thickness: t };
        let types = [
            (
                &n("tpl-type-masonry-15"),
                TypeData::Wall {
                    layers: vec![
                        layer(&n("tpl-mat-plaster"), 0.02),
                        layer(&n("tpl-mat-masonry"), 0.11),
                        layer(&n("tpl-mat-plaster"), 0.02),
                    ],
                },
            ),
            (
                &n("tpl-type-masonry-25"),
                TypeData::Wall {
                    layers: vec![
                        layer(&n("tpl-mat-plaster"), 0.025),
                        layer(&n("tpl-mat-masonry"), 0.20),
                        layer(&n("tpl-mat-plaster"), 0.025),
                    ],
                },
            ),
            (&n("tpl-type-drywall-10"), TypeData::Wall { layers: vec![layer(&n("tpl-mat-gypsum"), 0.10)] }),
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
                    layers: vec![layer(&n("tpl-mat-screed"), 0.03), layer(&n("tpl-mat-concrete"), 0.12)],
                },
            ),
            (&n("tpl-type-ceiling-gypsum"), TypeData::Ceiling { thickness: 0.02, material: n("tpl-mat-gypsum") }),
            (
                &n("tpl-type-roof-tile"),
                TypeData::Roof { layers: vec![layer(&n("tpl-mat-roof-tile"), 0.05), layer(&n("tpl-mat-wood"), 0.10)] },
            ),
        ];
        for (name, data) in types {
            let id = ElementTypeId(p.alloc_id());
            p.types.insert(id, ElementType { id, name: name.into(), data });
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
        let all_types: Vec<_> = p.types.keys().copied().collect();
        add_view(&mut p, &n("tpl-legend"), 50, ViewKind::Legend { types: all_types });
        for (name, category) in [
            (&n("tpl-schedule-doors"), Category::Door),
            (&n("tpl-schedule-windows"), Category::Window),
            (&n("tpl-schedule-walls"), Category::Wall),
        ] {
            add_view(&mut p, name, 1, ViewKind::Schedule { category });
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
        p
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
        self.materials.iter().find(|m| m.name == name).map_or([200, 200, 200], |m| m.color)
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
        _ => "?",
    }
}
