//! Render settings shared by the viewport, the offline renderer and the video
//! exporter, plus the render queue entry type.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderQuality {
    Draft,
    Low,
    Medium,
    High,
    Ultra,
}

impl RenderQuality {
    pub const ALL: [RenderQuality; 5] = [
        RenderQuality::Draft,
        RenderQuality::Low,
        RenderQuality::Medium,
        RenderQuality::High,
        RenderQuality::Ultra,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RenderQuality::Draft => "Draft",
            RenderQuality::Low => "Low",
            RenderQuality::Medium => "Medium",
            RenderQuality::High => "High",
            RenderQuality::Ultra => "Ultra",
        }
    }

    /// (samples per pixel, max bounces, resolution scale)
    pub fn preset(self) -> (u32, u32, f32) {
        match self {
            RenderQuality::Draft => (8, 2, 0.5),
            RenderQuality::Low => (32, 3, 0.75),
            RenderQuality::Medium => (96, 4, 1.0),
            RenderQuality::High => (256, 6, 1.0),
            RenderQuality::Ultra => (1024, 10, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderMode {
    PathTrace,
    AmbientOcclusion,
    Albedo,
    Normals,
    Depth,
    ShadowMatte,
}

impl RenderMode {
    pub const ALL: [RenderMode; 6] = [
        RenderMode::PathTrace,
        RenderMode::AmbientOcclusion,
        RenderMode::Albedo,
        RenderMode::Normals,
        RenderMode::Depth,
        RenderMode::ShadowMatte,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RenderMode::PathTrace => "Path Traced",
            RenderMode::AmbientOcclusion => "Ambient Occlusion",
            RenderMode::Albedo => "Albedo",
            RenderMode::Normals => "Normals",
            RenderMode::Depth => "Depth",
            RenderMode::ShadowMatte => "Shadow Matte",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Exr,
}

impl ImageFormat {
    pub const ALL: [ImageFormat; 3] = [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Exr];

    pub fn label(self) -> &'static str {
        match self {
            ImageFormat::Png => "PNG (16-bit capable)",
            ImageFormat::Jpeg => "JPEG",
            ImageFormat::Exr => "OpenEXR",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Exr => "exr",
        }
    }

    pub fn supports_alpha(self) -> bool {
        !matches!(self, ImageFormat::Jpeg)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl RenderRegion {
    pub fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub fn clamp_to(self, width: u32, height: u32) -> Self {
        let x = self.x.min(width.saturating_sub(1));
        let y = self.y.min(height.saturating_sub(1));
        Self {
            x,
            y,
            width: self.width.max(1).min(width - x),
            height: self.height.max(1).min(height - y),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub quality: RenderQuality,
    pub samples: u32,
    pub max_bounces: u32,
    pub mode: RenderMode,
    pub transparent_background: bool,
    pub denoise: bool,
    pub firefly_clamp: f32,
    pub format: ImageFormat,
    pub jpeg_quality: u8,
    pub threads: usize,
    pub region: Option<RenderRegion>,
    pub output_directory: String,
    pub sequence: bool,
    pub seed: u32,
    /// Deprecated-but-saved option: keep the window responsive while rendering.
    pub progressive: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            quality: RenderQuality::Medium,
            samples: 96,
            max_bounces: 4,
            mode: RenderMode::PathTrace,
            transparent_background: false,
            denoise: true,
            firefly_clamp: 20.0,
            format: ImageFormat::Png,
            jpeg_quality: 92,
            threads: 0,
            region: None,
            output_directory: String::new(),
            sequence: false,
            seed: 1,
            progressive: true,
        }
    }
}

impl RenderSettings {
    pub fn apply_quality_preset(&mut self, quality: RenderQuality) {
        let (samples, bounces, scale) = quality.preset();
        self.quality = quality;
        self.samples = samples;
        self.max_bounces = bounces;
        self.width = ((self.width.max(16) as f32 * scale) as u32).max(16);
        self.height = ((self.height.max(16) as f32 * scale) as u32).max(16);
    }

    pub fn effective_threads(&self) -> usize {
        if self.threads == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        } else {
            self.threads.min(256)
        }
    }

    pub fn megapixels(&self) -> f32 {
        (self.width as f32 * self.height as f32) / 1_000_000.0
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.width == 0 || self.height == 0 {
            problems.push("Render resolution must be at least 1x1".into());
        }
        if self.width > 16384 || self.height > 16384 {
            problems.push("Render resolution above 16384 px is not supported".into());
        }
        if self.samples == 0 {
            problems.push("Sample count must be at least 1".into());
        }
        if self.max_bounces == 0 {
            problems.push("Bounce count must be at least 1".into());
        }
        if self.format == ImageFormat::Jpeg && self.transparent_background {
            problems.push("JPEG does not support alpha; disable transparency or pick PNG".into());
        }
        if !(1..=100).contains(&self.jpeg_quality) {
            problems.push("JPEG quality must be between 1 and 100".into());
        }
        if let Some(r) = self.region {
            if r.x >= self.width || r.y >= self.height || r.width == 0 || r.height == 0 {
                problems.push("Render region is outside the canvas".into());
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_presets_scale() {
        let mut s = RenderSettings::default();
        s.apply_quality_preset(RenderQuality::Draft);
        assert_eq!(s.samples, 8);
        assert!(s.width < 1920);
        s.width = 1920;
        s.height = 1080;
        s.apply_quality_preset(RenderQuality::Ultra);
        assert_eq!(s.samples, 1024);
        assert_eq!(s.max_bounces, 10);
    }

    #[test]
    fn validation_catches_bad_settings() {
        let mut s = RenderSettings::default();
        assert!(s.validate().is_empty());
        s.samples = 0;
        s.format = ImageFormat::Jpeg;
        s.transparent_background = true;
        let problems = s.validate();
        assert!(problems.len() >= 2);
    }

    #[test]
    fn region_clamps() {
        let r = RenderRegion {
            x: 100,
            y: 100,
            width: 500,
            height: 500,
        }
        .clamp_to(200, 150);
        assert_eq!(r.width, 100);
        assert_eq!(r.height, 50);
    }

    #[test]
    fn thread_override() {
        let mut s = RenderSettings::default();
        assert!(s.effective_threads() >= 1);
        s.threads = 3;
        assert_eq!(s.effective_threads(), 3);
    }
}
