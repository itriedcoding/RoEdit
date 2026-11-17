//! Brush engine: soft round stamps, stroke interpolation, erase, clone, blur,
//! multiply and add modes.

use crate::composite_pixel;
use roedit_core::gfx::BlendMode;
use roedit_core::math::{smoothstep, Vec2};
use roedit_core::raster::RasterImage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrushMode {
    Normal,
    Erase,
    Multiply,
    Add,
    Blur,
    Clone,
}

impl BrushMode {
    pub const ALL: [BrushMode; 6] = [
        BrushMode::Normal,
        BrushMode::Erase,
        BrushMode::Multiply,
        BrushMode::Add,
        BrushMode::Blur,
        BrushMode::Clone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BrushMode::Normal => "Paint",
            BrushMode::Erase => "Erase",
            BrushMode::Multiply => "Multiply",
            BrushMode::Add => "Add",
            BrushMode::Blur => "Blur",
            BrushMode::Clone => "Clone Stamp",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrushSettings {
    /// Diameter in pixels.
    pub size: f32,
    /// 0 = very soft, 1 = hard edge.
    pub hardness: f32,
    pub opacity: f32,
    pub flow: f32,
    /// Stamp spacing as a fraction of the diameter.
    pub spacing: f32,
    pub mode: BrushMode,
    pub color: [f32; 4],
    /// Clone source offset: source pixel = target pixel + offset.
    pub clone_offset: [i32; 2],
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            size: 48.0,
            hardness: 0.7,
            opacity: 1.0,
            flow: 0.6,
            spacing: 0.12,
            mode: BrushMode::Normal,
            color: [1.0, 1.0, 1.0, 1.0],
            clone_offset: [0, 0],
        }
    }
}

impl BrushSettings {
    pub fn radius(&self) -> f32 {
        (self.size.max(1.0)) * 0.5
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
}

fn coverage(distance: f32, radius: f32, hardness: f32) -> f32 {
    if distance >= radius {
        return 0.0;
    }
    let t = 1.0 - distance / radius;
    let hardness = hardness.clamp(0.0, 1.0);
    // A hard brush keeps almost the full disc opaque, a soft brush ramps in.
    smoothstep(0.0, (1.0 - hardness).max(0.02), t)
}

/// Applies one brush dab centred on `center` in pixel coordinates.
/// Returns the number of pixels touched.
pub fn stamp(img: &mut RasterImage, center: Vec2, s: &BrushSettings) -> usize {
    if img.is_empty() {
        return 0;
    }
    let radius = s.radius();
    if radius < 0.5 {
        return 0;
    }
    let source = if s.mode == BrushMode::Clone {
        Some(img.clone())
    } else {
        None
    };
    let flow = s.flow.clamp(0.0, 1.0) * s.opacity.clamp(0.0, 1.0);
    let x0 = (center.x - radius).floor().max(0.0) as u32;
    let x1 = ((center.x + radius).ceil() as i64).clamp(0, img.width as i64) as u32;
    let y0 = (center.y - radius).floor().max(0.0) as u32;
    let y1 = ((center.y + radius).ceil() as i64).clamp(0, img.height as i64) as u32;
    let mut touched = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let d = Vec2::new(x as f32 + 0.5, y as f32 + 0.5).distance(center);
            let cov = coverage(d, radius, s.hardness);
            if cov <= 0.0 {
                continue;
            }
            let alpha = (cov * flow).clamp(0.0, 1.0);
            let dst = img.get_f32(x, y);
            match s.mode {
                BrushMode::Normal => {
                    let src = [s.color[0], s.color[1], s.color[2], s.color[3] * alpha];
                    img.set_f32(x, y, composite_pixel(dst, src, BlendMode::Normal));
                }
                BrushMode::Erase => {
                    let mut out = dst;
                    out[3] = (dst[3] - alpha).max(0.0);
                    img.set_f32(x, y, out);
                }
                BrushMode::Multiply => {
                    let src = [
                        s.color[0] * s.color[3] * alpha,
                        s.color[1] * s.color[3] * alpha,
                        s.color[2] * s.color[3] * alpha,
                        alpha,
                    ];
                    img.set_f32(x, y, composite_pixel(dst, src, BlendMode::Multiply));
                }
                BrushMode::Add => {
                    let src = [s.color[0], s.color[1], s.color[2], alpha];
                    img.set_f32(x, y, composite_pixel(dst, src, BlendMode::Add));
                }
                BrushMode::Blur => {
                    let mut avg = [0.0f32; 4];
                    let mut n = 0.0;
                    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= img.width as i32 || ny >= img.height as i32 {
                            continue;
                        }
                        let c = img.get_f32(nx as u32, ny as u32);
                        for i in 0..4 {
                            avg[i] += c[i];
                        }
                        n += 1.0;
                    }
                    if n > 0.0 {
                        let mut out = [0.0f32; 4];
                        for (i, a) in avg.iter().enumerate() {
                            out[i] = dst[i] + ((a / n) - dst[i]) * alpha;
                        }
                        img.set_f32(x, y, out);
                    }
                }
                BrushMode::Clone => {
                    if let Some(src_img) = &source {
                        let sx = x as i32 + s.clone_offset[0];
                        let sy = y as i32 + s.clone_offset[1];
                        if sx >= 0 && sy >= 0 && sx < img.width as i32 && sy < img.height as i32 {
                            let c = src_img.get_f32(sx as u32, sy as u32);
                            let src = [c[0], c[1], c[2], c[3] * alpha];
                            img.set_f32(x, y, composite_pixel(dst, src, BlendMode::Normal));
                        }
                    }
                }
            }
            touched += 1;
        }
    }
    touched
}

/// Stateful stroke that interpolates dabs so fast pointer movement leaves no
/// gaps.
#[derive(Clone, Debug, Default)]
pub struct Stroke {
    last: Option<Vec2>,
    residual: f32,
}

impl Stroke {
    pub fn begin(&mut self, point: Vec2) {
        self.last = Some(point);
        self.residual = 0.0;
    }

    pub fn end(&mut self) {
        self.last = None;
        self.residual = 0.0;
    }

    pub fn is_active(&self) -> bool {
        self.last.is_some()
    }

    pub fn last(&self) -> Option<Vec2> {
        self.last
    }

    /// Adds a segment from the previous point to `point`, stamping along it.
    pub fn to(&mut self, img: &mut RasterImage, point: Vec2, s: &BrushSettings) -> usize {
        let spacing = (s.size * s.spacing.clamp(0.02, 1.0)).max(0.6);
        let Some(last) = self.last else {
            self.last = Some(point);
            return stamp(img, point, s);
        };
        let delta = point - last;
        let length = delta.length();
        if length < 1e-4 {
            return 0;
        }
        let dir = delta / length;
        let mut travelled = self.residual;
        let mut stamps = 0;
        while travelled + spacing <= length {
            travelled += spacing;
            stamp(img, last + dir * travelled, s);
            stamps += 1;
        }
        self.residual = travelled - length + spacing;
        self.last = Some(point);
        stamps
    }
}

/// Creates a filled elliptical brush stamp image for custom brush thumbnails.
pub fn brush_preview(settings: &BrushSettings, size: u32) -> RasterImage {
    let mut img = RasterImage::new(size, size);
    let center = Vec2::splat(size as f32 * 0.5);
    let scale = size as f32 / settings.size.max(1.0);
    let local = BrushSettings {
        size: settings.size * scale,
        ..settings.clone()
    };
    stamp(&mut img, center, &local);
    img
}

/// A named brush preset from the library.
pub struct BrushPreset {
    pub name: &'static str,
    pub settings: BrushSettings,
}

pub fn presets() -> Vec<BrushPreset> {
    let mut soft = BrushSettings::default();
    soft.hardness = 0.0;
    soft.flow = 0.35;
    let mut hard = BrushSettings::default();
    hard.hardness = 0.98;
    hard.flow = 1.0;
    hard.size = 12.0;
    let mut eraser = BrushSettings::default();
    eraser.mode = BrushMode::Erase;
    eraser.hardness = 0.5;
    let mut airbrush = BrushSettings::default();
    airbrush.hardness = 0.15;
    airbrush.flow = 0.08;
    airbrush.size = 120.0;
    let mut glow = BrushSettings::default();
    glow.hardness = 0.05;
    glow.flow = 0.2;
    glow.mode = BrushMode::Add;
    glow.color = [0.4, 0.8, 1.0, 1.0];
    vec![
        BrushPreset {
            name: "Soft Round",
            settings: soft,
        },
        BrushPreset {
            name: "Hard Round",
            settings: hard,
        },
        BrushPreset {
            name: "Eraser",
            settings: eraser,
        },
        BrushPreset {
            name: "Airbrush",
            settings: airbrush,
        },
        BrushPreset {
            name: "Glow",
            settings: glow,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_paints_center() {
        let mut img = RasterImage::new(64, 64);
        let s = BrushSettings::default();
        let touched = stamp(&mut img, Vec2::splat(32.0), &s);
        assert!(touched > 100);
        let px = img.get(32, 32);
        assert!(px[3] > 100, "{px:?}");
        assert_eq!(img.get(0, 0)[3], 0);
    }

    #[test]
    fn stamp_outside_canvas_does_nothing() {
        let mut img = RasterImage::new(16, 16);
        let s = BrushSettings::default();
        assert_eq!(stamp(&mut img, Vec2::new(-50.0, -50.0), &s), 0);
        assert!(img.data.iter().all(|v| *v == 0));
    }

    #[test]
    fn erase_clears_alpha() {
        let mut img = RasterImage::filled(32, 32, [255, 0, 0, 255]);
        let s = BrushSettings {
            mode: BrushMode::Erase,
            hardness: 1.0,
            flow: 1.0,
            ..Default::default()
        };
        stamp(&mut img, Vec2::splat(16.0), &s);
        assert_eq!(img.get(16, 16)[3], 0);
    }

    #[test]
    fn stroke_leaves_no_gaps() {
        let mut img = RasterImage::new(128, 32);
        let s = BrushSettings {
            size: 6.0,
            hardness: 1.0,
            flow: 1.0,
            spacing: 0.25,
            ..Default::default()
        };
        let mut stroke = Stroke::default();
        stroke.begin(Vec2::new(4.0, 16.0));
        stroke.to(&mut img, Vec2::new(120.0, 16.0), &s);
        for x in (6..118).step_by(4) {
            assert!(img.get(x, 16)[3] > 0, "gap at x={x}");
        }
    }

    #[test]
    fn clone_copies_from_offset() {
        let mut img = RasterImage::new(32, 32);
        img.set(4, 4, [10, 200, 30, 255]);
        let s = BrushSettings {
            mode: BrushMode::Clone,
            size: 6.0,
            hardness: 1.0,
            flow: 1.0,
            clone_offset: [-20, -20],
            ..Default::default()
        };
        stamp(&mut img, Vec2::new(24.0, 24.0), &s);
        let px = img.get(24, 24);
        assert!(px[1] > 150, "{px:?}");
    }

    #[test]
    fn blur_softens_edges() {
        let mut img = RasterImage::filled(32, 32, [255, 255, 255, 255]);
        for y in 0..32 {
            img.set(16, y, [0, 0, 0, 255]);
        }
        let s = BrushSettings {
            mode: BrushMode::Blur,
            size: 9.0,
            hardness: 1.0,
            flow: 0.8,
            ..Default::default()
        };
        stamp(&mut img, Vec2::new(16.0, 16.0), &s);
        let px = img.get(15, 16);
        assert!(px[0] > 0 && px[0] < 255);
    }

    #[test]
    fn previews_and_presets_exist() {
        for p in presets() {
            assert!(!p.name.is_empty());
            let preview = brush_preview(&p.settings, 32);
            assert_eq!(preview.width, 32);
        }
    }
}
