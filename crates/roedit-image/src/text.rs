//! Text rendering. DejaVu Sans is bundled (see `assets/fonts`) so typography
//! works offline and identically on every machine.

use crate::blend::composite_pixel;
use crate::filters::gaussian_blur;
use crate::to_u8;
use fontdue::{Font, FontSettings};
use roedit_core::gfx::{TextAlign, TextSpec};
use roedit_core::math::Vec2;
use roedit_core::raster::RasterImage;
use std::sync::OnceLock;

const REGULAR: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");
const BOLD: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans-Bold.ttf");
const OBLIQUE: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans-Oblique.ttf");
const BOLD_OBLIQUE: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans-BoldOblique.ttf");

pub struct FontSet {
    pub regular: Font,
    pub bold: Font,
    pub oblique: Font,
    pub bold_oblique: Font,
}

impl FontSet {
    fn parse() -> Option<Self> {
        let settings = FontSettings::default();
        Some(Self {
            regular: Font::from_bytes(REGULAR, settings).ok()?,
            bold: Font::from_bytes(BOLD, settings).ok()?,
            oblique: Font::from_bytes(OBLIQUE, settings).ok()?,
            bold_oblique: Font::from_bytes(BOLD_OBLIQUE, settings).ok()?,
        })
    }

    pub fn pick(&self, bold: bool, italic: bool) -> &Font {
        match (bold, italic) {
            (false, false) => &self.regular,
            (true, false) => &self.bold,
            (false, true) => &self.oblique,
            (true, true) => &self.bold_oblique,
        }
    }
}

static FONTS: OnceLock<Option<FontSet>> = OnceLock::new();

pub fn fonts() -> &'static Option<FontSet> {
    FONTS.get_or_init(FontSet::parse)
}

/// Width and height of the laid-out text block, in pixels.
pub fn measure_text(spec: &TextSpec) -> Vec2 {
    let Some(set) = fonts().as_ref() else {
        return Vec2::ZERO;
    };
    let font = set.pick(spec.bold, spec.italic);
    let px = spec.font_size.max(1.0);
    let line_height = px * spec.line_height.max(0.1);
    let mut width: f32 = 0.0;
    for line in spec.text.split('\n') {
        let mut line_width = 0.0;
        for ch in line.chars() {
            let (metrics, _) = font.rasterize(ch, px);
            line_width += metrics.advance_width + spec.letter_spacing;
        }
        width = width.max(line_width);
    }
    Vec2::new(width, line_height * spec.text.split('\n').count() as f32)
}

fn line_offsets(spec: &TextSpec, font: &Font, line: &str) -> Vec<(char, f32)> {
    let px = spec.font_size.max(1.0);
    let mut out = Vec::with_capacity(line.chars().count());
    let mut pen = 0.0;
    for ch in line.chars() {
        out.push((ch, pen));
        let (metrics, _) = font.rasterize(ch, px);
        pen += metrics.advance_width + spec.letter_spacing;
    }
    out
}

/// Rasterises a text layer into an RGBA image of the given canvas size.
pub fn render_text(spec: &TextSpec, width: u32, height: u32) -> RasterImage {
    let mut out = RasterImage::new(width, height);
    let Some(set) = fonts().as_ref() else {
        return out;
    };
    if spec.text.trim().is_empty() || width == 0 || height == 0 {
        return out;
    }
    let font = set.pick(spec.bold, spec.italic);
    let px = spec.font_size.max(1.0);
    let line_height = px * spec.line_height.max(0.1);
    let lines: Vec<&str> = spec.text.split('\n').collect();
    let block_height = line_height * lines.len() as f32;
    let block_top = spec.position.y - block_height * 0.5;

    let mut coverage = vec![0.0f32; (width as usize) * (height as usize)];
    for (line_index, line) in lines.iter().enumerate() {
        let offsets = line_offsets(spec, font, line);
        let line_width = offsets
            .last()
            .map(|(ch, pen)| {
                let (m, _) = font.rasterize(*ch, px);
                pen + m.advance_width
            })
            .unwrap_or(0.0);
        let start_x = match spec.alignment {
            TextAlign::Left => spec.position.x,
            TextAlign::Center => spec.position.x - line_width * 0.5,
            TextAlign::Right => spec.position.x - line_width,
        };
        let baseline = block_top + line_height * (line_index as f32 + 0.8);
        for (ch, pen_x) in offsets {
            if ch == ' ' {
                continue;
            }
            let (metrics, bitmap) = font.rasterize(ch, px);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            let gx = start_x + pen_x + metrics.xmin as f32;
            let gy = baseline - (metrics.ymin + metrics.height as i32) as f32;
            for row in 0..metrics.height {
                let y = gy as i32 + row as i32;
                if y < 0 || y >= height as i32 {
                    continue;
                }
                for col in 0..metrics.width {
                    let x = gx as i32 + col as i32;
                    if x < 0 || x >= width as i32 {
                        continue;
                    }
                    let cov = bitmap[row * metrics.width + col] as f32 / 255.0;
                    let idx = y as usize * width as usize + x as usize;
                    if cov > coverage[idx] {
                        coverage[idx] = cov;
                    }
                }
            }
        }
    }

    let color = spec.color;
    let opacity = color[3].clamp(0.0, 1.0);
    let coverage_img = RasterImage::from_gray(width, height, &coverage);

    // Shadow first.
    if let Some(shadow) = &spec.shadow {
        let blurred = gaussian_blur(&coverage_img, shadow.blur.max(0.1));
        for y in 0..height {
            for x in 0..width {
                let sx = x as i32 - shadow.offset.x.round() as i32;
                let sy = y as i32 - shadow.offset.y.round() as i32;
                if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                    continue;
                }
                let a = blurred.get_f32(sx as u32, sy as u32)[0] * shadow.color[3] * opacity;
                if a <= 0.0 {
                    continue;
                }
                out.set_f32(x, y, [shadow.color[0], shadow.color[1], shadow.color[2], a]);
            }
        }
    }

    // Stroke (dilated coverage) under the fill.
    let stroke_radius = spec.stroke_width.max(0.0);
    if stroke_radius > 0.05 {
        let r = stroke_radius.ceil() as i32;
        let mut stroke = vec![0.0f32; coverage.len()];
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let idx = y as usize * width as usize + x as usize;
                if coverage[idx] > 0.0 {
                    continue;
                }
                let mut best: f32 = 0.0;
                'outer: for dy in -r..=r {
                    for dx in -r..=r {
                        if (dx * dx + dy * dy) as f32 > stroke_radius * stroke_radius {
                            continue;
                        }
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                            continue;
                        }
                        let c = coverage[ny as usize * width as usize + nx as usize];
                        if c > best {
                            best = c;
                            if best >= 1.0 {
                                break 'outer;
                            }
                        }
                    }
                }
                stroke[idx] = best;
            }
        }
        let stroke_color = spec.stroke_color;
        for (i, s) in stroke.iter().enumerate() {
            if *s <= 0.0 {
                continue;
            }
            let x = (i % width as usize) as u32;
            let y = (i / width as usize) as u32;
            let a = s * stroke_color[3] * opacity;
            let dst = out.get_f32(x, y);
            out.set_f32(
                x,
                y,
                composite_pixel(
                    dst,
                    [stroke_color[0], stroke_color[1], stroke_color[2], a],
                    roedit_core::gfx::BlendMode::Normal,
                ),
            );
        }
    }

    // Fill.
    for (i, cov) in coverage.iter().enumerate() {
        if *cov <= 0.0 {
            continue;
        }
        let x = (i % width as usize) as u32;
        let y = (i / width as usize) as u32;
        let dst = out.get_f32(x, y);
        out.set_f32(
            x,
            y,
            composite_pixel(
                dst,
                [color[0], color[1], color[2], cov * opacity],
                roedit_core::gfx::BlendMode::Normal,
            ),
        );
    }

    if spec.rotation_deg.abs() > 0.01 {
        out = rotate_about(&out, spec.position, spec.rotation_deg);
    }
    out
}

/// Rotates an image around an arbitrary anchor with bilinear sampling.
pub fn rotate_about(img: &RasterImage, anchor: Vec2, angle_deg: f32) -> RasterImage {
    if angle_deg.abs() < 1e-4 || img.is_empty() {
        return img.clone();
    }
    let (sin, cos) = (-angle_deg.to_radians()).sin_cos();
    let mut out = RasterImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - anchor;
            let src = Vec2::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos) + anchor;
            if src.x < 0.0 || src.y < 0.0 || src.x >= img.width as f32 || src.y >= img.height as f32
            {
                continue;
            }
            let c = img.sample_bilinear(src.x - 0.5, src.y - 0.5);
            out.set_f32(x, y, c);
        }
    }
    let _ = to_u8(0.0);
    out
}

/// Converts a `.ttf` file into a font usable by the renderer (used by the
/// "Import Font" tool). Returns an error string on malformed fonts.
pub fn validate_font_bytes(bytes: &[u8]) -> Result<(), String> {
    Font::from_bytes(bytes, FontSettings::default())
        .map(|_| ())
        .map_err(|e| format!("Font could not be parsed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(text: &str, size: f32) -> TextSpec {
        TextSpec {
            text: text.into(),
            font_size: size,
            position: Vec2::new(256.0, 128.0),
            ..Default::default()
        }
    }

    #[test]
    fn bundled_fonts_parse() {
        assert!(fonts().is_some(), "bundled DejaVu fonts must parse");
        assert!(validate_font_bytes(REGULAR).is_ok());
        assert!(validate_font_bytes(b"not a font").is_err());
    }

    #[test]
    fn renders_visible_glyphs() {
        let img = render_text(&spec("Roblox", 96.0), 512, 256);
        let lit = img.data.chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(lit > 500, "expected glyph coverage, got {lit}");
    }

    #[test]
    fn empty_text_renders_nothing() {
        let img = render_text(&spec("   ", 64.0), 256, 128);
        assert!(img.data.iter().all(|v| *v == 0));
    }

    #[test]
    fn measure_matches_layout() {
        let mut s = spec("Hello\nWorld", 40.0);
        s.line_height = 1.25;
        let m = measure_text(&s);
        assert!(m.x > 0.0);
        assert!((m.y - 100.0).abs() < 1.0, "{m:?}");
    }

    #[test]
    fn stroke_and_shadow_add_pixels() {
        let plain = render_text(&spec("GO", 64.0), 256, 128);
        let mut decorated = spec("GO", 64.0);
        decorated.stroke_width = 4.0;
        decorated.stroke_color = [0.0, 0.0, 0.0, 1.0];
        decorated.shadow = Some(roedit_core::gfx::TextShadow {
            offset: Vec2::new(6.0, 6.0),
            blur: 4.0,
            color: [0.0, 0.0, 0.0, 0.9],
        });
        let with_effects = render_text(&decorated, 256, 128);
        let count = |img: &RasterImage| img.data.chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(count(&with_effects) > count(&plain) * 2);
    }

    #[test]
    fn bold_and_italic_are_selectable() {
        let mut s = spec("Bold", 48.0);
        s.bold = true;
        s.italic = true;
        let img = render_text(&s, 256, 128);
        assert!(img.data.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn rotation_keeps_size() {
        let mut s = spec("Tilt", 64.0);
        s.rotation_deg = 30.0;
        let img = render_text(&s, 256, 256);
        assert_eq!(img.width, 256);
        assert!(img.data.chunks_exact(4).any(|p| p[3] > 0));
    }
}
