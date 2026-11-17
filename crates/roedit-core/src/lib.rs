//! RoEdit Studio core: the document model shared by every subsystem.
//!
//! This crate intentionally contains *data and pure logic only* (plus the
//! project file format). All editing operators live in the sibling crates so
//! that the desktop UI stays replaceable and the core stays testable.

pub mod animation;
pub mod camera;
pub mod gfx;
pub mod history;
pub mod ids;
pub mod logbuf;
pub mod material;
pub mod math;
pub mod mesh;
pub mod nodegraph;
pub mod project;
pub mod raster;
pub mod render_settings;
pub mod scene;
pub mod settings;
pub mod timeline;

pub use animation::{AnimValue, Animation, Ease, Key, Marker, Target, Track};
pub use camera::Camera;
pub use gfx::{BlendMode, CanvasPreset, GfxDoc, Layer, TextSpec};
pub use ids::*;
pub use material::{Environment, Material, SkyMode, Tonemap};
pub use math::{Aabb, Mat4, Quat, Vec2, Vec3, Vec4};
pub use mesh::{Face, Mesh};
pub use nodegraph::{NodeGraph, NodeKind, ParamValue};
pub use project::{Project, ProjectError, FORMAT_VERSION, PROJECT_EXTENSION};
pub use raster::RasterImage;
pub use render_settings::{ImageFormat, RenderMode, RenderQuality, RenderSettings};
pub use scene::{Entity, Light, LightKind, Scene, SceneHit, Transform};
pub use settings::AppSettings;
pub use timeline::{Clip, ClipEffect, ClipSource, Timeline, TrackKind, TransitionKind};

pub const APP_NAME: &str = "RoEdit Studio";
pub const APP_ID: &str = "roedit-studio";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_TAGLINE: &str = "Rust creative suite for Roblox 3D, GFX, rendering and cinematics";

/// Workspaces are declared once here so the UI, shortcuts and docs agree.
pub const WORKSPACES: &[&str] = &[
    "3D Modeling",
    "Roblox Studio",
    "GFX Design",
    "Texture Painting",
    "Materials",
    "Rendering",
    "Animation",
    "Cinematic Editing",
    "Video Editing",
    "Compositing",
    "Node Editor",
    "Diagnostics",
];

/// Export presets offered by the export dialog.
pub struct ExportPreset {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub kind: &'static str,
}

pub const EXPORT_PRESETS: &[ExportPreset] = &[
    ExportPreset {
        name: "Roblox Game Icon",
        width: 512,
        height: 512,
        kind: "image",
    },
    ExportPreset {
        name: "YouTube Thumbnail",
        width: 1280,
        height: 720,
        kind: "image",
    },
    ExportPreset {
        name: "Game Banner",
        width: 1920,
        height: 480,
        kind: "image",
    },
    ExportPreset {
        name: "Avatar Showcase",
        width: 1080,
        height: 1080,
        kind: "image",
    },
    ExportPreset {
        name: "Full HD Render",
        width: 1920,
        height: 1080,
        kind: "render",
    },
    ExportPreset {
        name: "QHD Render",
        width: 2560,
        height: 1440,
        kind: "render",
    },
    ExportPreset {
        name: "4K Render",
        width: 3840,
        height: 2160,
        kind: "render",
    },
];

/// Roblox conversion constants. Roblox uses studs; the modern avatar scale is
/// 0.28 m per stud, which is what the modeling tools convert with.
pub const STUD_TO_METER: f32 = 0.28;

pub fn studs_to_meters(studs: f32) -> f32 {
    studs * STUD_TO_METER
}

pub fn meters_to_studs(meters: f32) -> f32 {
    meters / STUD_TO_METER
}

/// Human readable byte size used across panels and logs.
pub fn format_bytes(bytes: usize) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Formats seconds as `H:MM:SS.mmm` for timelines.
pub fn format_timecode(seconds: f32, fps: f32) -> String {
    let total = seconds.max(0.0);
    let h = (total / 3600.0).floor() as u32;
    let m = ((total / 60.0) % 60.0).floor() as u32;
    let s = (total % 60.0).floor() as u32;
    let f = ((total - total.floor()) * fps.max(1.0)).floor() as u32;
    format!("{h}:{m:02}:{s:02}.{f:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_formatting() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn stud_conversion_roundtrips() {
        let m = studs_to_meters(10.0);
        assert!((meters_to_studs(m) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn timecode_formatting() {
        assert_eq!(format_timecode(0.0, 30.0), "0:00:00.00");
        assert_eq!(format_timecode(3661.5, 30.0), "1:01:01.15");
    }

    #[test]
    fn export_presets_are_sane() {
        for p in EXPORT_PRESETS {
            assert!(p.width > 0 && p.height > 0);
            assert!(!p.name.is_empty());
        }
        assert_eq!(WORKSPACES.len(), 12);
    }
}
