//! Schedule definitions: which fields a schedule shows, how rows are sorted
//! and filtered, and whether totals are shown. The values are computed by
//! `cc_views::schedule`.

use crate::Category;
use serde::{Deserialize, Serialize};

/// A column a schedule can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScheduleField {
    Mark,
    Type,
    Level,
    Comments,
    TypeMark,
    Description,
    Manufacturer,
    Model,
    FireRating,
    Cost,
    Length,
    Height,
    Width,
    Thickness,
    Area,
    Perimeter,
    Volume,
    Sill,
    Host,
    Function,
    Structural,
    Number,
    Name,
    Risers,
    RiserHeight,
    Run,
    Depth,
    ComponentName,
    ComponentKind,
}

impl ScheduleField {
    /// Translation key of the column header.
    pub fn key(self) -> &'static str {
        match self {
            ScheduleField::Mark => "col-mark",
            ScheduleField::Type => "col-type",
            ScheduleField::Level => "col-level",
            ScheduleField::Comments => "col-comments",
            ScheduleField::TypeMark => "info-type-mark",
            ScheduleField::Description => "info-description",
            ScheduleField::Manufacturer => "info-manufacturer",
            ScheduleField::Model => "info-model",
            ScheduleField::FireRating => "info-fire-rating",
            ScheduleField::Cost => "info-cost",
            ScheduleField::Length => "col-length",
            ScheduleField::Height => "col-height",
            ScheduleField::Width => "col-width",
            ScheduleField::Thickness => "col-thickness",
            ScheduleField::Area => "col-area",
            ScheduleField::Perimeter => "col-perimeter",
            ScheduleField::Volume => "col-volume",
            ScheduleField::Sill => "col-sill",
            ScheduleField::Host => "col-host",
            ScheduleField::Function => "prop-function",
            ScheduleField::Structural => "prop-structural",
            ScheduleField::Number => "col-number",
            ScheduleField::Name => "col-name",
            ScheduleField::Risers => "col-risers",
            ScheduleField::RiserHeight => "col-riser-height",
            ScheduleField::Run => "col-run",
            ScheduleField::Depth => "col-depth",
            ScheduleField::ComponentName => "col-component",
            ScheduleField::ComponentKind => "col-component-kind",
        }
    }

    /// Fields that make sense for `category`, in a sensible order.
    pub fn available(category: Category) -> Vec<ScheduleField> {
        use ScheduleField as F;
        let common = [F::Mark, F::Type, F::Level, F::Comments];
        let type_info = [F::TypeMark, F::Description, F::Manufacturer, F::Model, F::FireRating, F::Cost];
        let specific: &[F] = match category {
            Category::Wall => &[F::Length, F::Height, F::Thickness, F::Area, F::Volume, F::Function, F::Structural],
            Category::Door => &[F::Width, F::Height, F::Host],
            Category::Window => &[F::Width, F::Height, F::Sill, F::Host],
            Category::Floor | Category::Ceiling => &[F::Thickness, F::Area, F::Perimeter, F::Volume],
            Category::Roof => &[F::Thickness, F::Area, F::Perimeter, F::Volume],
            Category::Column => &[F::Height, F::Volume],
            Category::Beam => &[F::Length, F::Volume],
            Category::Stair => &[F::Width, F::Risers, F::RiserHeight, F::Run, F::Volume],
            Category::Railing => &[F::Length, F::Height],
            Category::Component => &[F::ComponentName, F::ComponentKind, F::Width, F::Depth, F::Height, F::Volume],
            Category::Room => {
                return vec![F::Number, F::Name, F::Level, F::Area, F::Perimeter, F::Height, F::Volume, F::Comments];
            }
            Category::Grid | Category::Dimension => return Vec::new(),
        };
        common.iter().chain(specific).chain(&type_info).copied().collect()
    }

    /// Fields shown by a new schedule of `category`.
    pub fn defaults(category: Category) -> Vec<ScheduleField> {
        use ScheduleField as F;
        match category {
            Category::Wall => vec![F::Mark, F::Type, F::Level, F::Length, F::Height, F::Thickness, F::Area, F::Volume],
            Category::Door => vec![F::Mark, F::Type, F::Width, F::Height, F::Level, F::Host, F::Comments],
            Category::Window => vec![F::Mark, F::Type, F::Width, F::Height, F::Sill, F::Level, F::Host, F::Comments],
            Category::Floor | Category::Ceiling | Category::Roof => {
                vec![F::Mark, F::Type, F::Level, F::Area, F::Perimeter, F::Volume]
            }
            Category::Column => vec![F::Mark, F::Type, F::Level, F::Height, F::Volume],
            Category::Beam => vec![F::Mark, F::Type, F::Level, F::Length, F::Volume],
            Category::Stair => vec![F::Mark, F::Type, F::Level, F::Risers, F::RiserHeight, F::Run, F::Volume],
            Category::Railing => vec![F::Mark, F::Type, F::Level, F::Length],
            Category::Component => vec![F::Mark, F::ComponentName, F::Type, F::Level, F::Width, F::Depth, F::Height],
            Category::Room => vec![F::Number, F::Name, F::Level, F::Area, F::Perimeter],
            Category::Grid | Category::Dimension => Vec::new(),
        }
    }
}

/// What a schedule shows. Empty `fields` means the category defaults (files
/// written before schedules were configurable).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScheduleDef {
    pub fields: Vec<ScheduleField>,
    /// Sort rows by this field (the default order is by mark or number).
    pub sort_by: Option<ScheduleField>,
    pub descending: bool,
    /// Keep only rows whose value of the field contains the text (case-insensitive).
    pub filter: Option<(ScheduleField, String)>,
    /// Hide the totals row.
    pub hide_totals: bool,
}

impl ScheduleDef {
    /// The fields to show for `category`.
    pub fn fields_for(&self, category: Category) -> Vec<ScheduleField> {
        if self.fields.is_empty() { ScheduleField::defaults(category) } else { self.fields.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_available() {
        for c in Category::SCHEDULABLE {
            let available = ScheduleField::available(c);
            for f in ScheduleField::defaults(c) {
                assert!(available.contains(&f), "{c:?} default {f:?} not available");
            }
        }
    }
}
