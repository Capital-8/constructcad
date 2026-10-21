use cc_core::{Category, ElementId};
use cc_geom::ModelGeometry;
use std::collections::BTreeSet;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

/// GPU-ready triangles and edge lines.
#[derive(Debug, Clone, Default)]
pub struct SceneMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Triangles of transparent solids, drawn last with blending.
    pub transparent_indices: Vec<u32>,
    pub edges: Vec<Vertex>,
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

const SELECTED: [u8; 3] = [70, 150, 255];

impl SceneMesh {
    pub fn build(geom: &ModelGeometry, selected: &BTreeSet<ElementId>, hidden: &[Category]) -> Self {
        let mut mesh = Self::default();
        for solid in geom.solids.iter().filter(|s| !hidden.contains(&s.category)) {
            let sel = selected.contains(&solid.element);
            let base = if sel { SELECTED } else { solid.color };
            // Selected solids stay opaque so they read clearly.
            let alpha = if sel { 1.0 } else { (1.0 - solid.transparency as f32).clamp(0.08, 1.0) };
            let color = [srgb_to_linear(base[0]), srgb_to_linear(base[1]), srgb_to_linear(base[2]), alpha];
            let target = if alpha < 0.999 { &mut mesh.transparent_indices } else { &mut mesh.indices };
            let edge_color = if sel { [0.02, 0.15, 0.6, 1.0] } else { [0.02, 0.02, 0.025, 1.0] };
            for face in &solid.faces {
                let start = mesh.vertices.len() as u32;
                let normal = face.normal.as_vec3().to_array();
                mesh.vertices.extend(face.verts.iter().map(|p| Vertex { pos: p.as_vec3().to_array(), normal, color }));
                for [a, b, c] in face.triangulate() {
                    target.extend([start + a as u32, start + b as u32, start + c as u32]);
                }
            }
            for (face, edges) in solid.faces.iter().zip(solid.feature_edges()) {
                let normal = face.normal.as_vec3().to_array();
                for (a, b) in edges {
                    mesh.edges.push(Vertex { pos: a.as_vec3().to_array(), normal, color: edge_color });
                    mesh.edges.push(Vertex { pos: b.as_vec3().to_array(), normal, color: edge_color });
                }
            }
        }
        mesh
    }

    /// The reinforcing bars (those of the selected hosts or sets highlighted)
    /// over the model's solids drawn faint.
    pub fn build_rebar(geom: &ModelGeometry, selected: &BTreeSet<ElementId>) -> Self {
        let mut mesh = Self::faint(geom);
        for set in geom.rebar.values() {
            let sel = selected.contains(&set.element) || selected.contains(&set.host);
            let color = if sel { [0.02, 0.15, 0.6, 1.0] } else { [0.55, 0.06, 0.04, 1.0] };
            for bar in &set.bars {
                let n = bar.points.len();
                let segments = if bar.closed { n } else { n.saturating_sub(1) };
                for i in 0..segments {
                    let (a, b) = (bar.points[i], bar.points[(i + 1) % n]);
                    let normal = [0.0, 0.0, 1.0];
                    mesh.edges.push(Vertex { pos: a.as_vec3().to_array(), normal, color });
                    mesh.edges.push(Vertex { pos: b.as_vec3().to_array(), normal, color });
                }
            }
        }
        mesh
    }

    /// The model's solids drawn faint, with no edges.
    fn faint(geom: &ModelGeometry) -> Self {
        let mut mesh = Self::default();
        for solid in &geom.solids {
            let base = solid.color;
            let color = [srgb_to_linear(base[0]), srgb_to_linear(base[1]), srgb_to_linear(base[2]), 0.12];
            for face in &solid.faces {
                let start = mesh.vertices.len() as u32;
                let normal = face.normal.as_vec3().to_array();
                mesh.vertices.extend(face.verts.iter().map(|p| Vertex { pos: p.as_vec3().to_array(), normal, color }));
                for [a, b, c] in face.triangulate() {
                    mesh.transparent_indices.extend([start + a as u32, start + b as u32, start + c as u32]);
                }
            }
        }
        mesh
    }

    /// The analytical model: member lines (columns blue, beams red, the
    /// selection highlighted) and node crosses (supports green) over the
    /// model's solids drawn faint.
    pub fn build_analytical(
        geom: &ModelGeometry,
        model: &cc_geom::analytical::AnalyticalModel,
        selected: &BTreeSet<ElementId>,
    ) -> Self {
        let mut mesh = Self::faint(geom);
        let mut line = |a: glam::DVec3, b: glam::DVec3, color: [f32; 4]| {
            let normal = [0.0, 0.0, 1.0];
            mesh.edges.push(Vertex { pos: a.as_vec3().to_array(), normal, color });
            mesh.edges.push(Vertex { pos: b.as_vec3().to_array(), normal, color });
        };
        for m in &model.members {
            let Some((a, b)) = model.line(m) else { continue };
            let color = if selected.contains(&m.element) {
                [0.02, 0.15, 0.6, 1.0]
            } else if m.kind == cc_geom::analytical::MemberKind::Column {
                [0.05, 0.25, 0.75, 1.0]
            } else {
                [0.75, 0.08, 0.05, 1.0]
            };
            line(a, b, color);
        }
        for n in &model.nodes {
            let color = if n.support { [0.05, 0.5, 0.1, 1.0] } else { [0.02, 0.02, 0.025, 1.0] };
            let r = 0.08;
            for d in [glam::DVec3::X, glam::DVec3::Y, glam::DVec3::Z] {
                line(n.position - d * r, n.position + d * r, color);
            }
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_materials_go_to_the_blended_pass() {
        let project = cc_core::from_ron(include_str!("../../../examples/casa_simples.ccad")).unwrap();
        let geom = ModelGeometry::build(&project);
        let mesh = SceneMesh::build(&geom, &BTreeSet::new(), &[]);
        // Window glass (60 % transparent in the template) is blended; walls are not.
        assert!(!mesh.transparent_indices.is_empty());
        assert!(!mesh.indices.is_empty());
        let glass: Vec<&Vertex> =
            mesh.transparent_indices.iter().filter_map(|i| mesh.vertices.get(*i as usize)).collect();
        assert!(glass.iter().all(|v| (v.color[3] - 0.4).abs() < 1e-6));
        // A selected window is drawn opaque.
        let window = project.elements.values().find(|e| matches!(e.kind, cc_core::ElementKind::Window(_))).unwrap().id;
        let sel = SceneMesh::build(&geom, &BTreeSet::from([window]), &[]);
        assert!(sel.transparent_indices.len() < mesh.transparent_indices.len());
    }
}
