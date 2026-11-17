//! Raster and GFX engine for RoEdit Studio.
//!
//! * `blend` – W3C separable and non-separable blend modes
//! * `composite` – layer stack flattening with groups, masks and clipping
//! * `brush` / `filters` / `adjust` – traditional pixel editing
//! * `text` – glyph rasterisation with stroke/shadow/glow effects
//! * `keying` – chroma key and border-connected background removal
//! * `graph` – deterministic procedural node graph evaluation
//! * `color` – HSL/HSV conversion, curves, levels and `.cube` LUTs

pub mod adjust;
pub mod blend;
pub mod brush;
pub mod color;
pub mod composite;
pub mod filters;
pub mod graph;
pub mod keying;
pub mod text;

pub use blend::{blend_rgb, composite_pixel};
pub use color::{Lut3D, Rgb};
pub use composite::{flatten_doc, render_layer_preview, FlattenOptions};
pub use graph::{evaluate_graph, EvalContext, GraphOutput};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("image has no pixels")]
    Empty,
    #[error("invalid parameters: {0}")]
    InvalidParameter(String),
    #[error("font could not be parsed: {0}")]
    Font(String),
    #[error("node graph is invalid: {0}")]
    Graph(String),
    #[error("image encoding failed: {0}")]
    Encode(String),
}

/// Clamps and converts `0..1` floats into a byte.
#[inline]
pub fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[inline]
pub fn from_u8(v: u8) -> f32 {
    v as f32 / 255.0
}

/// Copies `src` into `dst` at `offset`, clipping at the borders.
pub fn blit(
    dst: &mut roedit_core::raster::RasterImage,
    src: &roedit_core::raster::RasterImage,
    offset: [i32; 2],
) {
    for y in 0..src.height {
        let dy = y as i64 + offset[1] as i64;
        if dy < 0 || dy >= dst.height as i64 {
            continue;
        }
        for x in 0..src.width {
            let dx = x as i64 + offset[0] as i64;
            if dx < 0 || dx >= dst.width as i64 {
                continue;
            }
            dst.set(dx as u32, dy as u32, src.get(x, y));
        }
    }
}
