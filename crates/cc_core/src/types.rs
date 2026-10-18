use crate::{ElementTypeId, Uid};
use serde::{Deserialize, Serialize};

/// BIM category of an element or element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Category {
    Wall,
    Door,
    Window,
    Floor,
    Ceiling,
    Roof,
    Room,
    Grid,
    Dimension,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Wall,
        Category::Door,
        Category::Window,
        Category::Floor,
        Category::Ceiling,
        Category::Roof,
        Category::Room,
        Category::Grid,
        Category::Dimension,
    ];

    /// Categories with physical elements (shown in legends and 3D).
    pub const MODEL: [Category; 6] =
        [Category::Wall, Category::Door, Category::Window, Category::Floor, Category::Ceiling, Category::Roof];

    /// Categories that can be listed in a schedule.
    pub const SCHEDULABLE: [Category; 7] = [
        Category::Wall,
        Category::Door,
        Category::Window,
        Category::Floor,
        Category::Ceiling,
        Category::Roof,
        Category::Room,
    ];

    pub fn is_model(self) -> bool {
        Category::MODEL.contains(&self)
    }

    /// Translation key for the category name.
    pub fn key(self) -> &'static str {
        match self {
            Category::Wall => "category-wall",
            Category::Door => "category-door",
            Category::Window => "category-window",
            Category::Floor => "category-floor",
            Category::Ceiling => "category-ceiling",
            Category::Roof => "category-roof",
            Category::Room => "category-room",
            Category::Grid => "category-grid",
            Category::Dimension => "category-dimension",
        }
    }
}

/// Role of a layer in a compound type. The core (structure layers) is the
/// reference for core-based wall location lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LayerFunction {
    #[default]
    Structure,
    Substrate,
    Insulation,
    Finish,
    Membrane,
}

impl LayerFunction {
    pub const ALL: [LayerFunction; 5] = [
        LayerFunction::Structure,
        LayerFunction::Substrate,
        LayerFunction::Insulation,
        LayerFunction::Finish,
        LayerFunction::Membrane,
    ];

    /// Translation key of the function name.
    pub fn key(self) -> &'static str {
        match self {
            LayerFunction::Structure => "layer-structure",
            LayerFunction::Substrate => "layer-substrate",
            LayerFunction::Insulation => "layer-insulation",
            LayerFunction::Finish => "layer-finish",
            LayerFunction::Membrane => "layer-membrane",
        }
    }
}

/// One material layer of a compound wall/floor/roof type, listed exterior first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub material: String,
    /// Thickness in meters.
    pub thickness: f64,
    #[serde(default)]
    pub function: LayerFunction,
}

/// Purpose of a wall type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WallFunction {
    #[default]
    Interior,
    Exterior,
    Foundation,
    Retaining,
}

impl WallFunction {
    pub const ALL: [WallFunction; 4] =
        [WallFunction::Interior, WallFunction::Exterior, WallFunction::Foundation, WallFunction::Retaining];

    pub fn key(self) -> &'static str {
        match self {
            WallFunction::Interior => "wall-fn-interior",
            WallFunction::Exterior => "wall-fn-exterior",
            WallFunction::Foundation => "wall-fn-foundation",
            WallFunction::Retaining => "wall-fn-retaining",
        }
    }

    /// Whether walls of this function face the outside of the building.
    pub fn is_external(self) -> bool {
        !matches!(self, WallFunction::Interior)
    }
}

/// Descriptive data shared by all instances of a type.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TypeInfo {
    /// Short code shown in tags and schedules.
    pub type_mark: String,
    pub description: String,
    pub manufacturer: String,
    pub model: String,
    pub url: String,
    pub keynote: String,
    /// Classification or assembly code (e.g. a national construction classification).
    pub classification: String,
    pub fire_rating: String,
    pub comments: String,
    /// Unit cost in the project currency.
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeData {
    Wall {
        layers: Vec<Layer>,
        #[serde(default)]
        function: WallFunction,
    },
    Door {
        width: f64,
        height: f64,
        material: String,
    },
    Window {
        width: f64,
        height: f64,
        default_sill: f64,
        material: String,
    },
    Floor {
        layers: Vec<Layer>,
    },
    Ceiling {
        thickness: f64,
        material: String,
    },
    Roof {
        layers: Vec<Layer>,
    },
    /// Rooms have no type parameters yet; the type groups them in schedules.
    Room {},
    /// Grid line with a bubble of `bubble` paper millimeters.
    Grid {
        bubble: f64,
    },
    /// Linear dimension with text of `text_size` paper millimeters.
    Dimension {
        text_size: f64,
    },
}

/// An element type: the shared parameters of a group of instances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementType {
    pub id: ElementTypeId,
    pub name: String,
    pub data: TypeData,
    #[serde(default)]
    pub info: TypeInfo,
    #[serde(default)]
    pub uid: Uid,
}

impl ElementType {
    pub fn category(&self) -> Category {
        match self.data {
            TypeData::Wall { .. } => Category::Wall,
            TypeData::Door { .. } => Category::Door,
            TypeData::Window { .. } => Category::Window,
            TypeData::Floor { .. } => Category::Floor,
            TypeData::Ceiling { .. } => Category::Ceiling,
            TypeData::Roof { .. } => Category::Roof,
            TypeData::Room {} => Category::Room,
            TypeData::Grid { .. } => Category::Grid,
            TypeData::Dimension { .. } => Category::Dimension,
        }
    }

    pub fn layers(&self) -> &[Layer] {
        match &self.data {
            TypeData::Wall { layers, .. } | TypeData::Floor { layers } | TypeData::Roof { layers } => layers,
            _ => &[],
        }
    }

    /// Total thickness for layered types, 0 for openings.
    pub fn thickness(&self) -> f64 {
        match &self.data {
            TypeData::Ceiling { thickness, .. } => *thickness,
            _ => self.layers().iter().map(|l| l.thickness).sum(),
        }
    }

    /// Wall function, for wall types.
    pub fn wall_function(&self) -> Option<WallFunction> {
        match self.data {
            TypeData::Wall { function, .. } => Some(function),
            _ => None,
        }
    }

    /// Distances from the exterior (first) face to the exterior and interior
    /// faces of the core: the span from the first to the last structure layer,
    /// or the whole thickness when there is none.
    pub fn core_range(&self) -> (f64, f64) {
        let layers = self.layers();
        let first = layers.iter().position(|l| l.function == LayerFunction::Structure);
        let last = layers.iter().rposition(|l| l.function == LayerFunction::Structure);
        match (first, last) {
            (Some(a), Some(b)) => {
                let before: f64 = layers[..a].iter().map(|l| l.thickness).sum();
                let core: f64 = layers[a..=b].iter().map(|l| l.thickness).sum();
                (before, before + core)
            }
            _ => (0.0, self.thickness()),
        }
    }

    /// Width/height of an opening type (doors, windows).
    pub fn opening_size(&self) -> Option<(f64, f64)> {
        match self.data {
            TypeData::Door { width, height, .. } | TypeData::Window { width, height, .. } => Some((width, height)),
            _ => None,
        }
    }

    /// Material used to color the element in shaded views: the exterior finish
    /// layer for walls and roofs, the thickest layer for floors.
    pub fn main_material(&self) -> &str {
        match &self.data {
            TypeData::Door { material, .. }
            | TypeData::Window { material, .. }
            | TypeData::Ceiling { material, .. } => material,
            TypeData::Wall { layers, .. } | TypeData::Roof { layers } => {
                layers.first().map_or("", |l| l.material.as_str())
            }
            TypeData::Room {} | TypeData::Grid { .. } | TypeData::Dimension { .. } => "",
            TypeData::Floor { .. } => self
                .layers()
                .iter()
                .max_by(|a, b| a.thickness.total_cmp(&b.thickness))
                .map(|l| l.material.as_str())
                .unwrap_or(""),
        }
    }
}
