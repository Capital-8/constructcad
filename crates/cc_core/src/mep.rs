//! Building services (MEP): plumbing and electrical systems, the pipe and
//! conduit segments that run them, and the data that makes a component a
//! plumbing fixture or an electrical device.
//!
//! A system (a water supply, a drain, an electrical circuit) is a project
//! setting; pipes, conduits and components refer to it by [`SystemId`].
//! Defaults follow common Brazilian practice (fixture units and supply
//! weights as in the public tables of the drainage and water supply
//! standards; outlet mounting heights as usual in electrical plans).

use crate::{ElementId, SystemId};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// What a system carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SystemKind {
    #[default]
    ColdWater,
    HotWater,
    /// Sanitary drainage (soil and waste).
    Sanitary,
    /// Drainage vents.
    Vent,
    Rainwater,
    /// An electrical circuit feeding outlets and equipment.
    Power,
    /// An electrical circuit feeding lights.
    Lighting,
}

impl SystemKind {
    pub const ALL: [SystemKind; 7] = [
        SystemKind::ColdWater,
        SystemKind::HotWater,
        SystemKind::Sanitary,
        SystemKind::Vent,
        SystemKind::Rainwater,
        SystemKind::Power,
        SystemKind::Lighting,
    ];

    pub fn key(self) -> &'static str {
        match self {
            SystemKind::ColdWater => "system-cold-water",
            SystemKind::HotWater => "system-hot-water",
            SystemKind::Sanitary => "system-sanitary",
            SystemKind::Vent => "system-vent",
            SystemKind::Rainwater => "system-rainwater",
            SystemKind::Power => "system-power",
            SystemKind::Lighting => "system-lighting",
        }
    }

    /// Name in commands and files exchanged with other programs (`cold_water`…).
    pub fn name(self) -> &'static str {
        match self {
            SystemKind::ColdWater => "cold_water",
            SystemKind::HotWater => "hot_water",
            SystemKind::Sanitary => "sanitary",
            SystemKind::Vent => "vent",
            SystemKind::Rainwater => "rainwater",
            SystemKind::Power => "power",
            SystemKind::Lighting => "lighting",
        }
    }

    /// An electrical circuit (carried by conduits) rather than a piping system.
    pub fn is_electrical(self) -> bool {
        matches!(self, SystemKind::Power | SystemKind::Lighting)
    }

    /// Gravity drainage: its pipes slope down along the flow.
    pub fn is_drainage(self) -> bool {
        matches!(self, SystemKind::Sanitary | SystemKind::Rainwater)
    }

    /// Color of the system's pipes and conduits in plans and 3D.
    pub fn default_color(self) -> [u8; 3] {
        match self {
            SystemKind::ColdWater => [40, 110, 200],
            SystemKind::HotWater => [210, 50, 40],
            SystemKind::Sanitary => [130, 85, 45],
            SystemKind::Vent => [120, 160, 60],
            SystemKind::Rainwater => [20, 160, 170],
            SystemKind::Power => [220, 130, 20],
            SystemKind::Lighting => [190, 160, 0],
        }
    }
}

fn volts() -> f64 {
    127.0
}

/// A plumbing system or an electrical circuit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MepSystem {
    pub name: String,
    /// Short code shown in plan tags (e.g. a system abbreviation or a circuit number).
    #[serde(default)]
    pub code: String,
    pub kind: SystemKind,
    /// Display color; `None` uses the kind's color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Circuits: the distribution board (a panel component) feeding it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<ElementId>,
    /// Circuits: nominal voltage, volts.
    #[serde(default = "volts")]
    pub voltage: f64,
}

impl MepSystem {
    pub fn new(name: impl Into<String>, code: impl Into<String>, kind: SystemKind) -> Self {
        Self { name: name.into(), code: code.into(), kind, color: None, panel: None, voltage: volts() }
    }

    pub fn color(&self) -> [u8; 3] {
        self.color.unwrap_or_else(|| self.kind.default_color())
    }

    /// The code, or the name when it has none.
    pub fn label(&self) -> &str {
        if self.code.is_empty() { &self.name } else { &self.code }
    }
}

/// A straight pipe or conduit run between two points, each at its own
/// elevation above the level: horizontal, sloped, or vertical (a riser,
/// when both ends share the plan position).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MepSegment {
    pub start: DVec2,
    pub end: DVec2,
    /// Elevation of the center line above the level at the start.
    pub start_offset: f64,
    /// Elevation of the center line above the level at the end.
    pub end_offset: f64,
    /// Systems it belongs to: one for a pipe, the circuits a conduit carries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub systems: Vec<SystemId>,
}

impl MepSegment {
    pub fn new(start: DVec2, end: DVec2, offset: f64) -> Self {
        Self { start, end, start_offset: offset, end_offset: offset, systems: Vec::new() }
    }

    /// Length in plan.
    pub fn plan_length(&self) -> f64 {
        self.start.distance(self.end)
    }

    /// Rise from the start to the end.
    pub fn rise(&self) -> f64 {
        self.end_offset - self.start_offset
    }

    /// True length along the run.
    pub fn length(&self) -> f64 {
        self.plan_length().hypot(self.rise())
    }

    /// A vertical run (less than 1 mm long in plan).
    pub fn is_vertical(&self) -> bool {
        self.plan_length() < 1e-3
    }

    /// Fall per unit of plan length (positive when the end is lower); 0 for risers.
    pub fn slope(&self) -> f64 {
        let run = self.plan_length();
        if run < 1e-3 { 0.0 } else { -self.rise() / run }
    }

    /// The end points in the world, on a level at `level_z`.
    pub fn points(&self, level_z: f64) -> (DVec3, DVec3) {
        (self.start.extend(level_z + self.start_offset), self.end.extend(level_z + self.end_offset))
    }

    /// The first system, for pipes.
    pub fn system(&self) -> Option<SystemId> {
        self.systems.first().copied()
    }
}

/// What an electrical device is; it picks the plan symbol and the IFC class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DeviceKind {
    /// General-purpose outlet.
    #[default]
    Outlet,
    /// Outlet dedicated to one appliance.
    DedicatedOutlet,
    Switch,
    /// Light point on the ceiling.
    CeilingLight,
    /// Light point on a wall.
    WallLight,
    /// Distribution board.
    Panel,
}

impl DeviceKind {
    pub const ALL: [DeviceKind; 6] = [
        DeviceKind::Outlet,
        DeviceKind::DedicatedOutlet,
        DeviceKind::Switch,
        DeviceKind::CeilingLight,
        DeviceKind::WallLight,
        DeviceKind::Panel,
    ];

    pub fn key(self) -> &'static str {
        match self {
            DeviceKind::Outlet => "device-outlet",
            DeviceKind::DedicatedOutlet => "device-dedicated-outlet",
            DeviceKind::Switch => "device-switch",
            DeviceKind::CeilingLight => "device-ceiling-light",
            DeviceKind::WallLight => "device-wall-light",
            DeviceKind::Panel => "device-panel",
        }
    }

    /// A light point (on a lighting circuit).
    pub fn is_light(self) -> bool {
        matches!(self, DeviceKind::CeilingLight | DeviceKind::WallLight)
    }

    /// The kind of circuit that feeds it (`None` for boards and switches).
    pub fn circuit_kind(self) -> Option<SystemKind> {
        match self {
            DeviceKind::Outlet | DeviceKind::DedicatedOutlet => Some(SystemKind::Power),
            DeviceKind::CeilingLight | DeviceKind::WallLight => Some(SystemKind::Lighting),
            DeviceKind::Switch | DeviceKind::Panel => None,
        }
    }
}

/// Building-services data of a component definition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MepData {
    /// A plumbing fixture: its drainage fixture units, its relative weight
    /// on the water supply and whether it takes hot water.
    Fixture { drain_units: f64, supply_weight: f64, hot_water: bool },
    /// An electrical device with its apparent power in volt-amperes.
    Device { device: DeviceKind, power: f64 },
}

impl MepData {
    pub fn device(&self) -> Option<DeviceKind> {
        match *self {
            MepData::Device { device, .. } => Some(device),
            MepData::Fixture { .. } => None,
        }
    }

    /// Apparent power of a device, VA (0 for fixtures).
    pub fn power(&self) -> f64 {
        match *self {
            MepData::Device { power, .. } => power,
            MepData::Fixture { .. } => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_measures_slope_and_length() {
        let mut s = MepSegment::new(DVec2::ZERO, DVec2::new(4.0, 0.0), -0.3);
        s.end_offset = -0.34;
        assert!((s.slope() - 0.01).abs() < 1e-12);
        assert!((s.length() - 4.0f64.hypot(0.04)).abs() < 1e-12);
        let riser = MepSegment { end: DVec2::ZERO, end_offset: 2.8, ..s };
        assert!(riser.is_vertical());
        assert_eq!(riser.slope(), 0.0);
        assert!((riser.length() - 3.1).abs() < 1e-12);
    }
}
