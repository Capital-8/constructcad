use crate::{SceneMesh, Vertex, view_proj};
use cc_core::CameraState;
use wgpu::util::DeviceExt;

// egui samples native textures as gamma-encoded values, so the shader encodes sRGB itself.
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
const BACKGROUND: wgpu::Color = wgpu::Color { r: 0.84, g: 0.88, b: 0.93, a: 1.0 };

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve: wgpu::TextureView,
}

struct Buffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    edges: wgpu::Buffer,
    edge_count: u32,
}

/// Draws a [`SceneMesh`] into an offscreen, multisampled texture.
pub struct Renderer3D {
    faces: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    buffers: Option<Buffers>,
    targets: Option<Targets>,
}

impl Renderer3D {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cc_render shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cc_render uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cc_render layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cc_render uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cc_render uniforms"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });

        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        };
        let pipeline = |label: &str, topology, fs: &str, bias: wgpu::DepthBiasState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(vertex_layout.clone())],
                },
                primitive: wgpu::PrimitiveState { topology, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias,
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // Push faces slightly back so coplanar edges win the depth test.
        let face_bias = wgpu::DepthBiasState { constant: 2, slope_scale: 1.5, clamp: 0.0 };
        let faces = pipeline("cc_render faces", wgpu::PrimitiveTopology::TriangleList, "fs_face", face_bias);
        let edges = pipeline("cc_render edges", wgpu::PrimitiveTopology::LineList, "fs_edge", Default::default());
        Self { faces, edges, uniforms, bind_group, buffers: None, targets: None }
    }

    pub fn upload(&mut self, device: &wgpu::Device, mesh: &SceneMesh) {
        if mesh.indices.is_empty() && mesh.edges.is_empty() {
            self.buffers = None;
            return;
        }
        let init = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
        };
        // Zero-length buffers are invalid; pad with a dummy vertex.
        let pad = [Vertex { pos: [0.0; 3], normal: [0.0; 3], color: [0.0; 4] }];
        let verts: &[Vertex] = if mesh.vertices.is_empty() { &pad } else { &mesh.vertices };
        let edges: &[Vertex] = if mesh.edges.is_empty() { &pad } else { &mesh.edges };
        let indices: &[u32] = if mesh.indices.is_empty() { &[0, 0, 0] } else { &mesh.indices };
        self.buffers = Some(Buffers {
            vertices: init("cc_render vertices", bytemuck::cast_slice(verts), wgpu::BufferUsages::VERTEX),
            indices: init("cc_render indices", bytemuck::cast_slice(indices), wgpu::BufferUsages::INDEX),
            index_count: mesh.indices.len() as u32,
            edges: init("cc_render edges", bytemuck::cast_slice(edges), wgpu::BufferUsages::VERTEX),
            edge_count: mesh.edges.len() as u32,
        });
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) -> bool {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return false;
        }
        let texture = |label, format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        self.targets = Some(Targets {
            size,
            msaa: texture("cc_render msaa", COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT),
            depth: texture("cc_render depth", DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT),
            resolve: texture(
                "cc_render resolve",
                COLOR_FORMAT,
                1,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
        });
        true
    }

    /// Render a frame. Returns the output view and whether it was recreated
    /// (callers must then re-register it with egui).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        camera: &CameraState,
    ) -> (&wgpu::TextureView, bool) {
        let size = [size[0].clamp(1, 8192), size[1].clamp(1, 8192)];
        let recreated = self.ensure_targets(device, size);
        let aspect = size[0] as f32 / size[1] as f32;
        let fwd = camera.forward().as_vec3();
        // Sun from behind the viewer's left shoulder, high up.
        let light = (glam::Vec3::new(-fwd.x + 0.4, -fwd.y + 0.3, 1.6)).normalize();
        let u = Uniforms {
            view_proj: view_proj(camera, aspect).to_cols_array_2d(),
            light_dir: [light.x, light.y, light.z, 0.0],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));

        let targets = self.targets.as_ref().expect("targets created above");
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cc_render") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cc_render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.msaa,
                    depth_slice: None,
                    resolve_target: Some(&targets.resolve),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(BACKGROUND), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(b) = &self.buffers {
                pass.set_bind_group(0, &self.bind_group, &[]);
                if b.index_count > 0 {
                    pass.set_pipeline(&self.faces);
                    pass.set_vertex_buffer(0, b.vertices.slice(..));
                    pass.set_index_buffer(b.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..b.index_count, 0, 0..1);
                }
                if b.edge_count > 0 {
                    pass.set_pipeline(&self.edges);
                    pass.set_vertex_buffer(0, b.edges.slice(..));
                    pass.draw(0..b.edge_count, 0..1);
                }
            }
        }
        queue.submit([encoder.finish()]);
        (&targets.resolve, recreated)
    }
}
