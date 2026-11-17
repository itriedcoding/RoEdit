//! Real-time wgpu viewport renderer: shadowed PBR meshes, ground grid and a
//! sky-gradient background.
//!
//! This module targets Windows GPUs (DX12/Vulkan). It compiles and its shaders
//! are validated by the test suite, but it cannot be exercised on a machine
//! without a GPU adaptor; the offline path tracer is the verified renderer.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use roedit_core::camera::Camera;
use roedit_core::material::Environment;
use roedit_core::mesh::Mesh;
use wgpu::util::DeviceExt;

pub const OBJECT_SLOT_SIZE: u64 = 256;
pub const MAX_OBJECTS: usize = 1024;
const SHADOW_SIZE: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub light_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub sun_dir: [f32; 4],
    pub sun_color: [f32; 4],
    pub sky_top: [f32; 4],
    pub sky_horizon: [f32; 4],
    pub ground: [f32; 4],
    pub params: [f32; 4],
}

impl CameraUniforms {
    pub fn new(camera: &Camera, env: &Environment, aspect: f32, show_grid: bool) -> Self {
        let view_proj = camera.view_projection(aspect.max(1e-4));
        let center = camera.target;
        let radius = (camera.position - camera.target).length().max(4.0) * 1.6 + 4.0;
        let light_view = Mat4::look_at_rh(
            center + env.sun_direction.normalize_or_zero() * (radius * 2.0),
            center,
            Vec3::Y,
        );
        let light_proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.1, radius * 4.0);
        let sky_top = env.sky_top;
        let sky_horizon = env.sky_horizon;
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            light_view_proj: (light_proj * light_view).to_cols_array_2d(),
            camera_pos: [
                camera.position.x,
                camera.position.y,
                camera.position.z,
                env.exposure.clamp(0.02, 8.0),
            ],
            sun_dir: [
                env.sun_direction.x,
                env.sun_direction.y,
                env.sun_direction.z,
                env.sun_intensity,
            ],
            sun_color: [
                env.sun_color[0],
                env.sun_color[1],
                env.sun_color[2],
                env.sky_intensity,
            ],
            sky_top: [sky_top[0], sky_top[1], sky_top[2], 1.0],
            sky_horizon: [sky_horizon[0], sky_horizon[1], sky_horizon[2], 1.0],
            ground: [
                env.ground_color[0],
                env.ground_color[1],
                env.ground_color[2],
                1.0,
            ],
            params: [env.fog_density, 0.0, if show_grid { 1.0 } else { 0.0 }, 0.0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ObjectUniforms {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
    pub misc: [f32; 4],
}

impl ObjectUniforms {
    pub fn new(
        model: Mat4,
        color: [f32; 4],
        roughness: f32,
        metallic: f32,
        emissive: f32,
        selected: bool,
    ) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            color,
            misc: [
                roughness.clamp(0.03, 1.0),
                metallic.clamp(0.0, 1.0),
                emissive.max(0.0),
                if selected { 1.0 } else { 0.0 },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LightUniforms {
    pub view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GridVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl GridVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GridVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub name: String,
}

impl GpuMesh {
    /// Uploads a triangle mesh (n-gons are fan-triangulated).
    pub fn upload(device: &wgpu::Device, mesh: &Mesh) -> Self {
        let mut vertices: Vec<Vertex> = Vec::with_capacity(mesh.positions.len());
        for (i, p) in mesh.positions.iter().enumerate() {
            let n = mesh.normals.get(i).copied().unwrap_or(Vec3::Y);
            let uv = mesh
                .faces
                .iter()
                .find_map(|f| {
                    f.verts
                        .iter()
                        .position(|v| *v as usize == i)
                        .map(|slot| f.uvs.get(slot).copied().unwrap_or_default())
                })
                .unwrap_or_default();
            vertices.push(Vertex {
                position: p.to_array(),
                normal: n.to_array(),
                uv: [uv.x, uv.y],
            });
        }
        let mut indices: Vec<u32> = Vec::new();
        for f in &mesh.faces {
            if f.verts.len() < 3 {
                continue;
            }
            for k in 1..f.verts.len() - 1 {
                indices.push(f.verts[0]);
                indices.push(f.verts[k]);
                indices.push(f.verts[k + 1]);
            }
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("roedit.viewport.vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("roedit.viewport.indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            name: mesh.name.clone(),
        }
    }

    /// Empty placeholder used while a scene is loading.
    pub fn empty(device: &wgpu::Device) -> Self {
        Self {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("roedit.viewport.empty"),
                contents: bytemuck::cast_slice(&[Vertex {
                    position: [0.0; 3],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0; 2],
                }]),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("roedit.viewport.empty.indices"),
                contents: bytemuck::cast_slice(&[0u32, 0, 0]),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: 0,
            name: "empty".into(),
        }
    }
}

pub struct DrawItem<'a> {
    pub mesh: &'a GpuMesh,
    pub model: Mat4,
    pub color: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
    pub emissive: f32,
    pub selected: bool,
}

pub struct ViewFrame<'a> {
    pub camera: Camera,
    pub env: Environment,
    pub aspect: f32,
    pub items: &'a [DrawItem<'a>],
    pub show_sky: bool,
    pub show_grid: bool,
}

pub struct ViewportRenderer {
    pub format: wgpu::TextureFormat,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    light_buffer: wgpu::Buffer,
    object_buffer: wgpu::Buffer,
    mesh_pipeline: wgpu::RenderPipeline,
    grid_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    mesh_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    shadow_view: wgpu::TextureView,
    grid: GpuMesh,
    size: (u32, u32),
}

fn depth_texture(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("roedit.viewport.depth"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn shadow_texture(device: &wgpu::Device) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("roedit.viewport.shadow"),
        size: wgpu::Extent3d {
            width: SHADOW_SIZE,
            height: SHADOW_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn grid_lines() -> Vec<GridVertex> {
    let mut verts = Vec::new();
    let extent = 24.0f32;
    let mut i = -24i32;
    while i <= 24 {
        let p = i as f32;
        let major = i % 5 == 0;
        let color = if i == 0 {
            [0.32, 0.36, 0.42]
        } else if major {
            [0.24, 0.27, 0.32]
        } else {
            [0.17, 0.19, 0.23]
        };
        verts.push(GridVertex {
            position: [p, 0.0, -extent],
            color,
        });
        verts.push(GridVertex {
            position: [p, 0.0, extent],
            color,
        });
        verts.push(GridVertex {
            position: [-extent, 0.0, p],
            color,
        });
        verts.push(GridVertex {
            position: [extent, 0.0, p],
            color,
        });
        i += 1;
    }
    verts
}

impl ViewportRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("roedit.viewport.camera.layout"),
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
        let object_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("roedit.viewport.object.layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ObjectUniforms>() as u64,
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_object_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("roedit.viewport.shadow.layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: wgpu::BufferSize::new(64),
                        },
                        count: None,
                    },
                ],
            });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("roedit.viewport.camera"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("roedit.viewport.light"),
            size: std::mem::size_of::<LightUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let object_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("roedit.viewport.objects"),
            size: OBJECT_SLOT_SIZE * MAX_OBJECTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("roedit.viewport.camera"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let shadow_view = shadow_texture(device);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("roedit.viewport.shadow.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("roedit.viewport.mesh"),
            layout: &object_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &object_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<ObjectUniforms>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });
        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("roedit.viewport.shadow"),
            layout: &shadow_object_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &object_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(64),
                    }),
                },
            ],
        });

        let sky_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("roedit.viewport.sky"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::SKY.into()),
        });
        let grid_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("roedit.viewport.grid"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::GRID.into()),
        });
        let mesh_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("roedit.viewport.mesh"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::MESH.into()),
        });
        let shadow_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("roedit.viewport.shadow"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::SHADOW.into()),
        });

        let camera_only_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("roedit.viewport.camera.pipeline.layout"),
            bind_group_layouts: &[&camera_layout],
            push_constant_ranges: &[],
        });
        let mesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("roedit.viewport.mesh.pipeline.layout"),
            bind_group_layouts: &[&camera_layout, &object_layout],
            push_constant_ranges: &[],
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("roedit.viewport.shadow.pipeline.layout"),
            bind_group_layouts: &[&camera_layout, &shadow_object_layout],
            push_constant_ranges: &[],
        });

        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("roedit.viewport.sky.pipeline"),
            layout: Some(&camera_only_layout),
            vertex: wgpu::VertexState {
                module: &sky_module,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_module,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("roedit.viewport.grid.pipeline"),
            layout: Some(&camera_only_layout),
            vertex: wgpu::VertexState {
                module: &grid_module,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[GridVertex::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &grid_module,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("roedit.viewport.mesh.pipeline"),
            layout: Some(&mesh_layout),
            vertex: wgpu::VertexState {
                module: &mesh_module,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mesh_module,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 1,
                    slope_scale: 1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("roedit.viewport.shadow.pipeline"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shadow_module,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let grid_verts = grid_lines();
        let grid = GpuMesh {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("roedit.viewport.grid.vertices"),
                contents: bytemuck::cast_slice(&grid_verts),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("roedit.viewport.grid.indices"),
                contents: bytemuck::cast_slice(&[0u32]),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: grid_verts.len() as u32,
            name: "grid".into(),
        };

        Self {
            format,
            camera_buffer,
            camera_bind_group,
            light_buffer,
            object_buffer,
            mesh_pipeline,
            grid_pipeline,
            sky_pipeline,
            shadow_pipeline,
            mesh_bind_group,
            shadow_bind_group,
            depth_view: depth_texture(device, (width, height)),
            shadow_view,
            grid,
            size: (width.max(1), height.max(1)),
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let size = (width.max(1), height.max(1));
        if size == self.size {
            return;
        }
        self.size = size;
        self.depth_view = depth_texture(device, size);
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Draws one frame into `target`.
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame: &ViewFrame<'_>,
    ) {
        let uniforms =
            CameraUniforms::new(&frame.camera, &frame.env, frame.aspect, frame.show_grid);
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniforms));
        let light = LightUniforms {
            view_proj: uniforms.light_view_proj,
        };
        queue.write_buffer(&self.light_buffer, 0, bytemuck::bytes_of(&light));

        let items = &frame.items[..frame.items.len().min(MAX_OBJECTS)];
        for (i, item) in items.iter().enumerate() {
            let obj = ObjectUniforms::new(
                item.model,
                item.color,
                item.roughness,
                item.metallic,
                item.emissive,
                item.selected,
            );
            queue.write_buffer(
                &self.object_buffer,
                i as u64 * OBJECT_SLOT_SIZE,
                bytemuck::bytes_of(&obj),
            );
        }

        // Shadow pass.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("roedit.viewport.shadow.pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            for (i, item) in items.iter().enumerate() {
                if item.mesh.index_count == 0 {
                    continue;
                }
                let offset = i as u32 * OBJECT_SLOT_SIZE as u32;
                pass.set_bind_group(1, &self.shadow_bind_group, &[offset]);
                pass.set_vertex_buffer(0, item.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(item.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..item.mesh.index_count, 0, 0..1);
            }
        }

        // Main pass.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("roedit.viewport.main.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.055,
                            g: 0.06,
                            b: 0.075,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            if frame.show_sky {
                pass.set_pipeline(&self.sky_pipeline);
                pass.draw(0..3, 0..1);
            }
            if frame.show_grid {
                pass.set_pipeline(&self.grid_pipeline);
                pass.set_vertex_buffer(0, self.grid.vertex_buffer.slice(..));
                pass.draw(0..self.grid.index_count, 0..1);
            }
            pass.set_pipeline(&self.mesh_pipeline);
            for (i, item) in items.iter().enumerate() {
                if item.mesh.index_count == 0 {
                    continue;
                }
                let offset = i as u32 * OBJECT_SLOT_SIZE as u32;
                pass.set_bind_group(1, &self.mesh_bind_group, &[offset]);
                pass.set_vertex_buffer(0, item.mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(item.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..item.mesh.index_count, 0, 0..1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::material::Environment;
    use std::mem::size_of;

    #[test]
    fn shaders_are_valid_wgsl() {
        for (name, source) in crate::shaders::ALL {
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{name} failed to parse: {e}"));
            let mut validator = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::default(),
            );
            validator
                .validate(&module)
                .unwrap_or_else(|e| panic!("{name} failed validation: {e:?}"));
        }
    }

    #[test]
    fn uniforms_have_stable_layouts() {
        assert_eq!(size_of::<CameraUniforms>(), 304);
        assert_eq!(size_of::<ObjectUniforms>(), 96);
        assert_eq!(size_of::<LightUniforms>(), 64);
        assert_eq!(size_of::<Vertex>(), 32);
        assert_eq!(size_of::<GridVertex>(), 24);
        assert_eq!(OBJECT_SLOT_SIZE % 16, 0);
        assert!(OBJECT_SLOT_SIZE as usize >= size_of::<ObjectUniforms>());
    }

    #[test]
    fn camera_uniforms_are_finite() {
        let camera = Camera::default();
        let env = Environment::default();
        let u = CameraUniforms::new(&camera, &env, 16.0 / 9.0, true);
        for row in u.view_proj.iter().chain(u.inv_view_proj.iter()) {
            assert!(row.iter().all(|v| v.is_finite()));
        }
        assert_eq!(u.params[2], 1.0);
        assert!(u.sun_dir[3] > 0.0);
    }

    #[test]
    fn object_flags_are_clamped() {
        let o = ObjectUniforms::new(Mat4::IDENTITY, [1.0; 4], 2.0, -1.0, -3.0, true);
        assert!(o.misc[0] <= 1.0);
        assert!(o.misc[1] >= 0.0);
        assert!(o.misc[2] >= 0.0);
        assert_eq!(o.misc[3], 1.0);
    }

    #[test]
    fn grid_has_lines_on_both_axes() {
        let verts = grid_lines();
        assert!(verts.len() >= 4);
        assert_eq!(verts.len() % 2, 0);
        assert!(verts.iter().any(|v| v.color[0] > 0.3));
    }
}
