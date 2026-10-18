use crate::ElementTypeId;
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
}

impl Category {
    pub const ALL: [Category; 6] =
        [Category::Wall, Category::Door, Category::Window, Category::Floor, Category::Ceiling, Category::Roof];

    /// Translation key for the category name.
    pub fn key(self) -> &'static str {
        match self {
            Category::Wall => "category-wall",
            Category::Door => "category-door",
            Category::Window => "category-window",
            Category::Floor => "category-floor",
            Category::Ceiling => "category-ceiling",
            Category::Roof => "category-roof",
        }
    }
}

/// One material layer of a compound wall/floor/roof type, listed exterior first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub material: String,
    /// Thickness in meters.
    pub thickness: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeData {
    Wall { layers: Vec<Layer> },
    Door { width: f64, height: f64, material: String },
    Window { width: f64, height: f64, default_sill: f64, material: String },
    Floor { layers: Vec<Layer> },
    Ceiling { thickness: f64, material: String },
    Roof { layers: Vec<Layer> },
}

/// A family type: the shared parameters of a group of instances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementType {
    pub id: ElementTypeId,
    pub name: String,
    pub data: TypeData,
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
        }
    }

    pub fn layers(&self) -> &[Layer] {
        match &self.data {
            TypeData::Wall { layers } | TypeData::Floor { layers } | TypeData::Roof { layers } => layers,
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
            TypeData::Wall { layers } | TypeData::Roof { layers } => layers.first().map_or("", |l| l.material.as_str()),
            TypeData::Floor { .. } => self
                .layers()
                .iter()
                .max_by(|a, b| a.thickness.total_cmp(&b.thickness))
                .map(|l| l.material.as_str())
                .unwrap_or(""),
        }
    }
}
