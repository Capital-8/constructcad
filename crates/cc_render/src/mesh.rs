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
            let color = [srgb_to_linear(base[0]), srgb_to_linear(base[1]), srgb_to_linear(base[2]), 1.0];
            let edge_color = if sel { [0.02, 0.15, 0.6, 1.0] } else { [0.02, 0.02, 0.025, 1.0] };
            for face in &solid.faces {
                let start = mesh.vertices.len() as u32;
                let normal = face.normal.as_vec3().to_array();
                mesh.vertices.extend(face.verts.iter().map(|p| Vertex { pos: p.as_vec3().to_array(), normal, color }));
                for [a, b, c] in face.triangulate() {
                    mesh.indices.extend([start + a as u32, start + b as u32, start + c as u32]);
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
}
