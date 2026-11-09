//! Offscreen wgpu renderer for the 3D view.
//!
//! The app builds a [`SceneMesh`] from the model geometry, uploads it with
//! [`Renderer3D::upload`] and calls [`Renderer3D::render`] each frame; the
//! resulting texture is shown as an egui image.

mod camera;
mod mesh;
mod renderer;
mod select;
pub mod textures;

pub use camera::{pick_ray, view_proj};
pub use mesh::{SceneMesh, SolidStyle, Tessellation, Vertex, plane_axes};
pub use renderer::Renderer3D;
pub use select::elements_in_rect;
pub use textures::TextureSet;
