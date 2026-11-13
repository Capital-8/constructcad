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
    /// Section size of a column or beam.
    Section,
    /// Name of the profile of a column or beam type, whatever its material (tags show it only for steel and
    /// cold-formed profiles).
    Profile,
    /// Mass per meter of a column or beam, kg/m.
    LinearMass,
    /// Length of a beam's body after its joins.
    CutLength,
    /// Main material of the type.
    Material,
    /// Bar diameter of reinforcement.
    Diameter,
    /// Number of bars in a reinforcement set.
    BarCount,
    /// Length of one bar.
    BarLength,
    /// Steel weight, kilograms.
    Weight,
    /// The part of the structure a reinforcement set belongs to.
    Partition,
    /// Plumbing system or electrical circuits.
    System,
    /// Fall of a pipe per unit of length.
    Slope,
    /// Apparent power of an electrical device, VA.
    Power,
    /// Drainage fixture units of a plumbing fixture.
    DrainUnits,
    /// The phase the element is built in.
    PhaseCreated,
    /// The phase the element is demolished in.
    PhaseDemolished,
    /// Gross floor area of a mass: the sum of its mass floors.
    FloorArea,
    /// Number of tiles on a roof.
    TileCount,
    /// Total length of a roof's framing members.
    FramingLength,
    /// Volume of a roof's framing members.
    FramingVolume,
    /// How many mass floors a mass has.
    FloorCount,
    /// Number of panels of a curtain wall.
    PanelCount,
    /// Area of a curtain wall's glass panels.
    GlazedArea,
    /// Total length of a curtain wall's mullions.
    MullionLength,
    /// Elevation of a building pad above its level.
    Elevation,
    /// Volume of earth cut by a pad (a terrain: by its pads).
    CutVolume,
    /// Volume of earth filled by a pad (a terrain: by its pads).
    FillVolume,
    /// Estimated length of the horizontal cables from a data outlet to its rack.
    CableLength,
    /// Kind of a connection (base plate, end plate…).
    ConnectionKind,
    /// Steel mass of a connection's plates, kilograms.
    PlateMass,
    /// Number of bolts of a connection.
    BoltCount,
    /// Bolt diameter of a connection.
    BoltDiameter,
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
            ScheduleField::Section => "col-section",
            ScheduleField::Profile => "col-profile",
            ScheduleField::LinearMass => "col-linear-mass",
            ScheduleField::CutLength => "col-cut-length",
            ScheduleField::Material => "col-material",
            ScheduleField::Diameter => "col-diameter",
            ScheduleField::BarCount => "col-bar-count",
            ScheduleField::BarLength => "col-bar-length",
            ScheduleField::Weight => "col-weight",
            ScheduleField::Partition => "col-partition",
            ScheduleField::System => "col-system",
            ScheduleField::Slope => "col-slope",
            ScheduleField::Power => "col-power",
            ScheduleField::DrainUnits => "col-drain-units",
            ScheduleField::PhaseCreated => "col-phase-created",
            ScheduleField::PhaseDemolished => "col-phase-demolished",
            ScheduleField::FloorArea => "col-floor-area",
            ScheduleField::FloorCount => "col-floor-count",
            ScheduleField::TileCount => "col-tile-count",
            ScheduleField::FramingLength => "col-framing-length",
            ScheduleField::FramingVolume => "col-framing-volume",
            ScheduleField::PanelCount => "col-panel-count",
            ScheduleField::GlazedArea => "col-glazed-area",
            ScheduleField::MullionLength => "col-mullion-length",
            ScheduleField::Elevation => "col-elevation",
            ScheduleField::CutVolume => "col-cut-volume",
            ScheduleField::FillVolume => "col-fill-volume",
            ScheduleField::CableLength => "col-cable-length",
            ScheduleField::ConnectionKind => "col-connection-kind",
            ScheduleField::PlateMass => "col-plate-mass",
            ScheduleField::BoltCount => "col-bolt-count",
            ScheduleField::BoltDiameter => "col-bolt-diameter",
        }
    }

    /// Translation key of the column header in a project of `system`. The
    /// metric headers of lengths, areas and volumes name their unit
    /// (`Length (m)`); imperial values carry their own marks and symbols
    /// (`12'-6"`, `100.00 ft²`), so those headers name none.
    pub fn key_in(self, system: crate::MeasurementSystem) -> &'static str {
        if system == crate::MeasurementSystem::Metric {
            return self.key();
        }
        match self {
            ScheduleField::Length => "col-imp-length",
            ScheduleField::Thickness => "col-imp-thickness",
            ScheduleField::Area => "col-imp-area",
            ScheduleField::Perimeter => "col-imp-perimeter",
            ScheduleField::Volume => "col-imp-volume",
            _ => self.key(),
        }
    }

    /// Fields that make sense for `category`, in a sensible order.
    pub fn available(category: Category) -> Vec<ScheduleField> {
        use ScheduleField as F;
        let common = [F::Mark, F::Type, F::Level, F::Comments];
        let type_info = [F::TypeMark, F::Description, F::Manufacturer, F::Model, F::FireRating, F::Cost];
        let phases = [F::PhaseCreated, F::PhaseDemolished];
        let specific: &[F] = match category {
            Category::Wall => &[
                F::Length,
                F::Height,
                F::Thickness,
                F::Area,
                F::Volume,
                F::Function,
                F::Structural,
                F::PanelCount,
                F::GlazedArea,
                F::MullionLength,
            ],
            Category::Door => &[F::Width, F::Height, F::Sill, F::Host, F::ComponentName],
            Category::Window => &[F::Width, F::Height, F::Sill, F::Host, F::ComponentName],
            Category::Floor | Category::Ceiling => &[F::Thickness, F::Area, F::Perimeter, F::Volume],
            Category::Roof => &[
                F::Thickness,
                F::Area,
                F::Perimeter,
                F::Volume,
                F::Slope,
                F::TileCount,
                F::Weight,
                F::FramingLength,
                F::FramingVolume,
            ],
            Category::Column => {
                &[F::Section, F::Profile, F::LinearMass, F::Height, F::Volume, F::Material, F::Structural]
            }
            Category::Beam => &[F::Section, F::Profile, F::LinearMass, F::Length, F::CutLength, F::Volume, F::Material],
            Category::Foundation => &[F::Section, F::Length, F::Area, F::Thickness, F::Volume, F::Material],
            Category::Rebar => {
                &[F::Host, F::Partition, F::Diameter, F::BarCount, F::BarLength, F::Length, F::Weight, F::Material]
            }
            Category::Stair => &[F::Width, F::Risers, F::RiserHeight, F::Run, F::Volume],
            Category::Ramp => &[F::Width, F::Slope, F::Run, F::Volume],
            Category::Railing => &[F::Length, F::Height],
            Category::Sweep => &[F::Length, F::Volume],
            Category::Shaft => &[F::Area, F::Height],
            Category::SteelBuilding => &[],
            Category::Truss => &[],
            Category::Bracing => &[],
            Category::WallFraming => &[],
            Category::Connection => {
                &[F::ConnectionKind, F::Host, F::PlateMass, F::BoltCount, F::BoltDiameter, F::Volume]
            }
            Category::Topography => &[F::Area, F::Volume, F::Material, F::CutVolume, F::FillVolume],
            Category::Pad => &[F::Elevation, F::Area, F::CutVolume, F::FillVolume],
            Category::Pipe => &[F::Diameter, F::Length, F::System, F::Slope, F::Material],
            Category::Conduit => &[F::Diameter, F::Length, F::System, F::Material],
            Category::CableTray => &[F::Width, F::Height, F::Length, F::System, F::Material],
            Category::Valve => &[F::Host, F::Diameter, F::System, F::Material],
            Category::LightningConductor => &[F::Function, F::Diameter, F::Length, F::System, F::Material],
            Category::Component => &[
                F::ComponentName,
                F::ComponentKind,
                F::Width,
                F::Depth,
                F::Height,
                F::Volume,
                F::System,
                F::Power,
                F::DrainUnits,
                F::CableLength,
            ],
            Category::Mass => &[F::Height, F::FloorCount, F::FloorArea, F::Volume],
            Category::Room => {
                return vec![
                    F::Number,
                    F::Name,
                    F::Level,
                    F::Area,
                    F::Perimeter,
                    F::Height,
                    F::Volume,
                    F::Comments,
                    F::PhaseCreated,
                    F::PhaseDemolished,
                ];
            }
            Category::Grid
            | Category::Dimension
            | Category::Text
            | Category::DetailLine
            | Category::FilledRegion
            | Category::MaskingRegion
            | Category::DetailComponent
            | Category::LevelMark
            | Category::CadImport
            | Category::TitleBlock => {
                return Vec::new();
            }
        };
        common.iter().chain(specific).chain(&type_info).chain(&phases).copied().collect()
    }

    /// Whether elements of `category` can have a value for this field: the
    /// fields [`available`](Self::available) to its schedules, plus the type,
    /// level, main material and volume, which most elements have. A filter
    /// rule on a field that applies to none of its categories never matches.
    pub fn applies_to(self, category: Category) -> bool {
        use ScheduleField as F;
        matches!(self, F::Type | F::Level | F::Material | F::Volume) || Self::available(category).contains(&self)
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
            Category::Column => vec![F::Mark, F::Type, F::Level, F::Section, F::Height, F::Volume, F::Material],
            Category::Beam => {
                vec![F::Mark, F::Type, F::Level, F::Section, F::Length, F::CutLength, F::Volume, F::Material]
            }
            Category::Foundation => vec![F::Mark, F::Type, F::Level, F::Section, F::Thickness, F::Volume, F::Material],
            Category::Rebar => vec![F::Host, F::Type, F::Diameter, F::BarCount, F::BarLength, F::Length, F::Weight],
            Category::Stair => vec![F::Mark, F::Type, F::Level, F::Risers, F::RiserHeight, F::Run, F::Volume],
            Category::Ramp => vec![F::Mark, F::Type, F::Level, F::Slope, F::Run, F::Volume],
            Category::Railing => vec![F::Mark, F::Type, F::Level, F::Length],
            Category::Sweep => vec![F::Mark, F::Type, F::Level, F::Length, F::Volume],
            Category::Pipe => vec![F::Type, F::System, F::Level, F::Diameter, F::Length, F::Slope],
            Category::Conduit => vec![F::Type, F::System, F::Level, F::Diameter, F::Length],
            Category::CableTray => vec![F::Type, F::System, F::Level, F::Width, F::Height, F::Length],
            Category::Valve => vec![F::Mark, F::Type, F::Diameter, F::System, F::Level],
            Category::LightningConductor => vec![F::Type, F::System, F::Level, F::Function, F::Diameter, F::Length],
            Category::Component => vec![F::Mark, F::ComponentName, F::Type, F::Level, F::Width, F::Depth, F::Height],
            Category::Room => vec![F::Number, F::Name, F::Level, F::Area, F::Perimeter],
            Category::Mass => vec![F::Mark, F::Type, F::Level, F::Height, F::FloorCount, F::FloorArea, F::Volume],
            Category::Shaft => vec![F::Mark, F::Type, F::Level, F::Area, F::Height],
            Category::SteelBuilding => Vec::new(),
            Category::Truss => Vec::new(),
            Category::Bracing => Vec::new(),
            Category::WallFraming => Vec::new(),
            Category::Connection => {
                vec![F::Mark, F::ConnectionKind, F::PlateMass, F::BoltCount, F::BoltDiameter, F::Comments]
            }
            Category::Topography => vec![F::Mark, F::Type, F::Level, F::Area, F::Volume],
            Category::Pad => vec![F::Mark, F::Type, F::Level, F::Elevation, F::Area, F::CutVolume, F::FillVolume],
            Category::Grid
            | Category::Dimension
            | Category::Text
            | Category::DetailLine
            | Category::FilledRegion
            | Category::MaskingRegion
            | Category::DetailComponent
            | Category::LevelMark
            | Category::CadImport
            | Category::TitleBlock => Vec::new(),
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
    /// A reinforcement schedule as a bending schedule: one row per bar mark
    /// instead of one per set (the fields, sort and filter do not apply).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bending: Option<BendingOptions>,
}

/// Most partitions a bending schedule is limited to.
pub const MAX_BENDING_PARTITIONS: usize = 100;
/// Largest waste a bending schedule adds, percent.
pub const MAX_WASTE_PERCENT: f64 = 100.0;

/// What a bending schedule shows: one row per bar mark (mark, diameter,
/// grade, shape sketch with its dimensions, length of one bar, count, total
/// length, weight; rows of bars longer than the stock length noted), then a
/// summary per diameter and grade (total length and weight, with the waste
/// added when given).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BendingOptions {
    /// Only the sets of these partitions (none: every set; an empty text
    /// stands for the sets without one).
    pub partitions: Vec<String>,
    /// Rows under a heading per partition (each partition's marks counted
    /// on their own).
    pub group_by_partition: bool,
    /// Waste added in the summary, percent (none: no waste columns).
    pub waste_percent: Option<f64>,
}

impl BendingOptions {
    /// Why the options cannot be kept (`None`: they can), as a message.
    pub fn check(&self) -> Option<String> {
        if self.partitions.len() > MAX_BENDING_PARTITIONS {
            return Some(format!("at most {MAX_BENDING_PARTITIONS} partitions"));
        }
        if self.partitions.iter().any(|p| p.chars().count() > crate::bar_marks::MAX_MARK_TEXT) {
            return Some(format!("partitions have at most {} characters", crate::bar_marks::MAX_MARK_TEXT));
        }
        if self.waste_percent.is_some_and(|w| !(w.is_finite() && (0.0..=MAX_WASTE_PERCENT).contains(&w))) {
            return Some(format!("waste_percent must be between 0 and {MAX_WASTE_PERCENT}"));
        }
        None
    }

    /// Bring the options into range; returns how many changed.
    pub fn repair(&mut self) -> usize {
        let mut n = 0;
        if self.partitions.len() > MAX_BENDING_PARTITIONS {
            self.partitions.truncate(MAX_BENDING_PARTITIONS);
            n += 1;
        }
        let before = self.partitions.len();
        self.partitions.retain(|p| p.chars().count() <= crate::bar_marks::MAX_MARK_TEXT);
        n += before - self.partitions.len();
        if self.waste_percent.is_some_and(|w| !(w.is_finite() && (0.0..=MAX_WASTE_PERCENT).contains(&w))) {
            self.waste_percent = None;
            n += 1;
        }
        n
    }

    /// Whether a set of `partition` is shown.
    pub fn shows(&self, partition: &str) -> bool {
        self.partitions.is_empty() || self.partitions.iter().any(|p| p.trim().eq_ignore_ascii_case(partition.trim()))
    }
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
