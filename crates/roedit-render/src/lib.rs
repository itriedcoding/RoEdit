//! Rendering: a deterministic multithreaded CPU path tracer for final frames and
//! a wgpu rasteriser for the interactive viewport.

pub mod bvh;
pub mod pathtracer;
pub mod rng;
pub mod viewport;

pub use bvh::{Bvh, Hit};
pub use pathtracer::{
    build_render_scene, render, render_float, to_raster, RenderLight, RenderMaterial,
    RenderRequest, RenderScene, RenderTriangle,
};
pub use rng::Pcg32;

/// Shader sources, exposed so the build can validate them in tests.
pub mod shaders {
    pub const MESH: &str = include_str!("../shaders/mesh.wgsl");
    pub const GRID: &str = include_str!("../shaders/grid.wgsl");
    pub const SKY: &str = include_str!("../shaders/sky.wgsl");
    pub const SHADOW: &str = include_str!("../shaders/shadow.wgsl");
    pub const ALL: [(&str, &str); 4] = [
        ("mesh.wgsl", MESH),
        ("grid.wgsl", GRID),
        ("sky.wgsl", SKY),
        ("shadow.wgsl", SHADOW),
    ];
}
