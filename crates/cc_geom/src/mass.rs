//! Conceptual masses: the body of a mass (an extrusion, or a loft between
//! two outlines) and its mass floors, the sections at the levels it lists.

use cc_core::mass::{Mass, MassFloor, area, perimeter};
use cc_core::{Element, Project, SketchPlane};
use glam::{DVec2, DVec3};

/// A mass's body and floors.
#[derive(Debug, Clone, PartialEq)]
pub struct MassGeom {
    /// Faces of the closed body.
    pub faces: Vec<Vec<DVec3>>,
    /// Elevations of the base and the top.
    pub z_range: (f64, f64),
    pub volume: f64,
    /// Mass floors from the lowest up: one per listed level the mass spans
    /// (at or above its base, below its top).
    pub floors: Vec<MassFloor>,
}

impl MassGeom {
    /// Total area of the mass floors (the gross floor area).
    pub fn floor_area(&self) -> f64 {
        self.floors.iter().map(|f| f.area).sum()
    }
}

/// The body and floors of mass `m` (element `e`), or `None` when it is degenerate.
pub fn mass_geom(project: &Project, e: &Element, m: &Mass) -> Option<MassGeom> {
    if !m.is_valid() {
        return None;
    }
    let base = project.level_elevation(e.level) + m.base_offset;
    let top = base + m.height;
    let bottom = m.section(0.0);
    let faces = match &m.top {
        None => {
            let mut s = crate::Solid::new(e.id, e.category(), [0, 0, 0]);
            s.add_prism(&bottom, |_| base, |_| top);
            s.faces.into_iter().map(|f| f.verts).collect()
        }
        Some(_) => cc_core::sweep::blend(SketchPlane::Plan, &bottom, &m.section(1.0), base, m.height).ok()?,
    };
    if faces.is_empty() {
        return None;
    }
    let volume = cc_core::sweep::volume(&faces).abs();
    let mut levels: Vec<_> = m
        .floors
        .iter()
        .filter_map(|id| project.level(*id))
        .filter(|l| l.elevation >= base - 1e-9 && l.elevation < top - 1e-9)
        .collect();
    levels.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
    levels.dedup_by_key(|l| l.id);
    let floors = levels
        .into_iter()
        .map(|l| {
            let outline: Vec<DVec2> = m.section((l.elevation - base) / m.height);
            MassFloor { level: l.id, area: area(&outline), perimeter: perimeter(&outline), outline }
        })
        .collect();
    Some(MassGeom { faces, z_range: (base, top), volume, floors })
}
