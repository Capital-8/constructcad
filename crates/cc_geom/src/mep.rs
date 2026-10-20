//! Pipes and conduits: each segment is a cylinder of its type's diameter
//! along its center line, colored by its system.

use crate::Solid;
use cc_core::{Element, ElementId, Project};
use glam::DVec3;

/// Sides of the polygon approximating a segment's section.
const SIDES: usize = 12;

/// A pipe or conduit run in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentGeom {
    pub element: ElementId,
    /// Center line ends.
    pub a: DVec3,
    pub b: DVec3,
    /// Outside diameter.
    pub diameter: f64,
}

impl SegmentGeom {
    pub fn length(&self) -> f64 {
        self.a.distance(self.b)
    }
}

/// The run of a pipe or conduit element (`None` for other elements or a degenerate run).
pub fn segment_geom(project: &Project, e: &Element) -> Option<SegmentGeom> {
    let s = e.as_segment()?;
    let diameter = project.element_type(e.type_id)?.diameter()?;
    let (a, b) = s.points(project.level_elevation(e.level));
    (a.distance(b) > 1e-6 && a.is_finite() && b.is_finite()).then_some(SegmentGeom { element: e.id, a, b, diameter })
}

/// The faces of a closed cylinder of `radius` from `a` to `b`.
pub fn cylinder(a: DVec3, b: DVec3, radius: f64) -> Vec<Vec<DVec3>> {
    let Some(axis) = (b - a).try_normalize() else { return Vec::new() };
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    let ring = |c: DVec3| -> Vec<DVec3> {
        (0..SIDES)
            .map(|i| {
                let t = i as f64 / SIDES as f64 * std::f64::consts::TAU;
                c + (u * t.cos() + v * t.sin()) * radius
            })
            .collect()
    };
    let (bottom, top) = (ring(a), ring(b));
    let mut faces = Vec::with_capacity(SIDES + 2);
    // Caps wound to face outwards: the bottom against the axis, the top along it.
    faces.push(bottom.iter().rev().copied().collect());
    faces.push(top.clone());
    for i in 0..SIDES {
        let j = (i + 1) % SIDES;
        faces.push(vec![bottom[i], bottom[j], top[j], top[i]]);
    }
    faces
}

impl crate::ModelGeometry {
    pub(crate) fn build_mep(&mut self, project: &Project) {
        for e in project.elements.values() {
            let Some(g) = segment_geom(project, e) else { continue };
            let mut solid = Solid::new(e.id, e.category(), project.system_color(e));
            solid.add_polygons(cylinder(g.a, g.b, g.diameter * 0.5));
            self.solids.push(solid);
            self.segments.insert(e.id, g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylinder_is_closed_with_outward_faces() {
        let (a, b) = (DVec3::ZERO, DVec3::new(2.0, 0.0, 1.0));
        let mut s = Solid::new(ElementId(1), cc_core::Category::Pipe, [0, 0, 0]);
        s.add_polygons(cylinder(a, b, 0.05));
        assert_eq!(s.faces.len(), SIDES + 2);
        // The polygon's area times the length.
        let n = SIDES as f64;
        let area = 0.5 * n * 0.05 * 0.05 * (std::f64::consts::TAU / n).sin();
        assert!((s.volume() - area * a.distance(b)).abs() < 1e-9, "{}", s.volume());
    }
}
