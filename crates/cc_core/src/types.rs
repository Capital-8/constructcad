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
    Column,
    Beam,
    Stair,
    Railing,
    Component,
}

impl Category {
    pub const ALL: [Category; 14] = [
        Category::Wall,
        Category::Door,
        Category::Window,
        Category::Floor,
        Category::Ceiling,
        Category::Roof,
        Category::Column,
        Category::Beam,
        Category::Stair,
        Category::Railing,
        Category::Component,
        Category::Room,
        Category::Grid,
        Category::Dimension,
    ];

    /// Categories with physical elements (shown in legends and 3D).
    pub const MODEL: [Category; 11] = [
        Category::Wall,
        Category::Door,
        Category::Window,
        Category::Floor,
        Category::Ceiling,
        Category::Roof,
        Category::Column,
        Category::Beam,
        Category::Stair,
        Category::Railing,
        Category::Component,
    ];

    /// Categories that can be listed in a schedule.
    pub const SCHEDULABLE: [Category; 12] = [
        Category::Wall,
        Category::Door,
        Category::Window,
        Category::Floor,
        Category::Ceiling,
        Category::Roof,
        Category::Column,
        Category::Beam,
        Category::Stair,
        Category::Railing,
        Category::Component,
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
            Category::Column => "category-column",
            Category::Beam => "category-beam",
            Category::Stair => "category-stair",
            Category::Railing => "category-railing",
            Category::Component => "category-component",
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
    Column {
        section: ColumnSection,
        material: String,
    },
    /// Beam section `width` x `height`.
    Beam {
        width: f64,
        height: f64,
        material: String,
    },
    /// Straight stair: risers at most `max_riser` high, treads `tread` deep.
    Stair {
        width: f64,
        max_riser: f64,
        tread: f64,
        material: String,
    },
    /// Railing with a top rail at `height` and posts every `post_spacing`.
    Railing {
        height: f64,
        post_spacing: f64,
        material: String,
    },
    /// Parametric component: `component` names the definition shared by its
    /// types; the parameter values in `def` are this type's.
    Component {
        component: String,
        def: crate::component::ComponentDef,
    },
}

impl TypeData {
    /// Material names referenced by this type.
    pub fn materials(&self) -> Vec<&str> {
        match self {
            TypeData::Wall { layers, .. } | TypeData::Floor { layers } | TypeData::Roof { layers } => {
                layers.iter().map(|l| l.material.as_str()).collect()
            }
            TypeData::Door { material, .. }
            | TypeData::Window { material, .. }
            | TypeData::Ceiling { material, .. }
            | TypeData::Column { material, .. }
            | TypeData::Beam { material, .. }
            | TypeData::Stair { material, .. }
            | TypeData::Railing { material, .. } => vec![material.as_str()],
            TypeData::Component { def, .. } => def.items.iter().map(|i| i.material.as_str()).collect(),
            TypeData::Room {} | TypeData::Grid { .. } | TypeData::Dimension { .. } => Vec::new(),
        }
    }

    pub fn uses_material(&self, name: &str) -> bool {
        self.materials().contains(&name)
    }

    /// Point references to material `old` at `new`.
    pub fn rename_material(&mut self, old: &str, new: &str) {
        let fix = |m: &mut String| {
            if m == old {
                *m = new.to_string();
            }
        };
        match self {
            TypeData::Wall { layers, .. } | TypeData::Floor { layers } | TypeData::Roof { layers } => {
                layers.iter_mut().for_each(|l| fix(&mut l.material))
            }
            TypeData::Door { material, .. }
            | TypeData::Window { material, .. }
            | TypeData::Ceiling { material, .. }
            | TypeData::Column { material, .. }
            | TypeData::Beam { material, .. }
            | TypeData::Stair { material, .. }
            | TypeData::Railing { material, .. } => fix(material),
            TypeData::Component { def, .. } => def.items.iter_mut().for_each(|i| fix(&mut i.material)),
            TypeData::Room {} | TypeData::Grid { .. } | TypeData::Dimension { .. } => {}
        }
    }
}

/// Cross-section of a column type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ColumnSection {
    Rectangular { width: f64, depth: f64 },
    Round { diameter: f64 },
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
            TypeData::Column { .. } => Category::Column,
            TypeData::Beam { .. } => Category::Beam,
            TypeData::Stair { .. } => Category::Stair,
            TypeData::Railing { .. } => Category::Railing,
            TypeData::Component { .. } => Category::Component,
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
            TypeData::Column { material, .. }
            | TypeData::Beam { material, .. }
            | TypeData::Stair { material, .. }
            | TypeData::Railing { material, .. } => material,
            TypeData::Component { def, .. } => def.main_material(),
            TypeData::Floor { .. } => self
                .layers()
                .iter()
                .max_by(|a, b| a.thickness.total_cmp(&b.thickness))
                .map(|l| l.material.as_str())
                .unwrap_or(""),
        }
    }
}

/// Riser count, riser height and run length of a straight stair of `height`
/// with at most `max_riser` per riser and `tread`-deep treads (one tread fewer
/// than risers: the last riser lands on the upper floor).
pub fn stair_layout(height: f64, max_riser: f64, tread: f64) -> (usize, f64, f64) {
    let max_riser = if max_riser.is_finite() && max_riser > 0.01 { max_riser } else { 0.18 };
    let risers = ((height / max_riser).ceil().max(1.0) as usize).min(200);
    let riser = height / risers as f64;
    (risers, riser, tread.max(0.0) * (risers.saturating_sub(1)) as f64)
}
