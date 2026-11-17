//! Non-destructive colour and layer-style adjustments.

use crate::blend::composite_pixel;
use crate::color::{hsl_to_rgb, rgb_to_hsl, saturate, temperature_shift};
use crate::filters::gaussian_blur;
use crate::{to_u8, ImageError};
use roedit_core::gfx::BlendMode;
use roedit_core::math::Vec2;
use roedit_core::raster::RasterImage;

fn map_rgb(img: &RasterImage, f: impl Fn([f32; 3]) -> [f32; 3]) -> RasterImage {
    let mut out = img.clone();
    for y in 0..img.height {
        for x in 0..img.width {
            let mut px = img.get_f32(x, y);
            let rgb = f([px[0], px[1], px[2]]);
            px[0] = rgb[0].clamp(0.0, 1.0);
            px[1] = rgb[1].clamp(0.0, 1.0);
            px[2] = rgb[2].clamp(0.0, 1.0);
            out.set_f32(x, y, px);
        }
    }
    out
}

pub fn levels(
    img: &RasterImage,
    in_black: f32,
    in_white: f32,
    gamma: f32,
    out_black: f32,
    out_white: f32,
) -> RasterImage {
    map_rgb(img, |c| {
        [
            crate::color::levels(c[0], in_black, in_white, gamma, out_black, out_white),
            crate::color::levels(c[1], in_black, in_white, gamma, out_black, out_white),
            crate::color::levels(c[2], in_black, in_white, gamma, out_black, out_white),
        ]
    })
}

/// Applies one curve to RGB and a second curve to the alpha channel.
pub fn curves(
    img: &RasterImage,
    rgb_points: &[(f32, f32)],
    alpha_points: &[(f32, f32)],
) -> RasterImage {
    let lut = crate::color::curves_lut(rgb_points);
    let alpha_lut = crate::color::curves_lut(alpha_points);
    let mut out = img.clone();
    for y in 0..img.height {
        for x in 0..img.width {
            let px = img.get_f32(x, y);
            out.set(
                x,
                y,
                [
                    to_u8(crate::color::apply_lut1d(&lut, px[0])),
                    to_u8(crate::color::apply_lut1d(&lut, px[1])),
                    to_u8(crate::color::apply_lut1d(&lut, px[2])),
                    to_u8(crate::color::apply_lut1d(&alpha_lut, px[3])),
                ],
            );
        }
    }
    out
}

pub fn hue_saturation(
    img: &RasterImage,
    hue_shift_deg: f32,
    saturation: f32,
    lightness: f32,
) -> RasterImage {
    map_rgb(img, |c| {
        let mut hsl = rgb_to_hsl(c);
        hsl[0] = (hsl[0] + hue_shift_deg / 360.0).rem_euclid(1.0);
        hsl[1] = (hsl[1] * saturation).clamp(0.0, 1.0);
        hsl[2] = (hsl[2] + lightness).clamp(0.0, 1.0);
        hsl_to_rgb(hsl)
    })
}

pub fn exposure_contrast(
    img: &RasterImage,
    exposure_ev: f32,
    contrast: f32,
    brightness: f32,
) -> RasterImage {
    let gain = 2f32.powf(exposure_ev);
    map_rgb(img, |c| {
        let mut out = [0.0f32; 3];
        for i in 0..3 {
            let v = c[i] * gain;
            let v = (v - 0.5) * contrast + 0.5 + brightness;
            out[i] = v.clamp(0.0, 1.0);
        }
        out
    })
}

pub fn color_balance(
    img: &RasterImage,
    temperature: f32,
    tint: f32,
    saturation: f32,
) -> RasterImage {
    map_rgb(img, |c| {
        let c = temperature_shift(c, temperature);
        let mut c = [
            (c[0] + tint * 0.06).clamp(0.0, 1.0),
            (c[1] - tint * 0.03).clamp(0.0, 1.0),
            (c[2] - tint * 0.06).clamp(0.0, 1.0),
        ];
        c = saturate(c, saturation);
        c
    })
}

/// Darkens towards the edges. `amount` 0..1, `radius` is where falloff starts
/// (fraction of the half-diagonal), `softness` controls the ramp.
pub fn vignette(img: &RasterImage, amount: f32, radius: f32, softness: f32) -> RasterImage {
    let mut out = img.clone();
    if img.is_empty() {
        return out;
    }
    let cx = img.width as f32 * 0.5;
    let cy = img.height as f32 * 0.5;
    let max_d = (cx * cx + cy * cy).sqrt().max(1.0);
    for y in 0..img.height {
        for x in 0..img.width {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / max_d;
            let t = ((d - radius) / softness.max(1e-3)).clamp(0.0, 1.0);
            let factor = 1.0 - amount.clamp(0.0, 1.0) * (t * t * (3.0 - 2.0 * t));
            let mut px = img.get_f32(x, y);
            for c in 0..3 {
                px[c] *= factor;
            }
            out.set_f32(x, y, px);
        }
    }
    out
}

pub fn gradient_overlay(
    img: &RasterImage,
    color_a: [f32; 4],
    color_b: [f32; 4],
    angle_deg: f32,
    opacity: f32,
) -> RasterImage {
    let mut out = img.clone();
    let dir = Vec2::new(angle_deg.to_radians().cos(), angle_deg.to_radians().sin());
    let w = img.width.max(1) as f32;
    let h = img.height.max(1) as f32;
    let diag = (w * w + h * h).sqrt().max(1.0);
    for y in 0..img.height {
        for x in 0..img.width {
            let p = Vec2::new(x as f32 - w * 0.5, y as f32 - h * 0.5);
            let t = ((p.dot(dir) / diag) + 0.5).clamp(0.0, 1.0);
            let mut color = [0.0f32; 4];
            for c in 0..4 {
                color[c] = color_a[c] + (color_b[c] - color_a[c]) * t;
            }
            // Blend the overlay against the layer's own alpha.
            let dst = img.get_f32(x, y);
            let src = [color[0], color[1], color[2], color[3] * opacity * dst[3]];
            out.set_f32(x, y, composite_pixel(dst, src, BlendMode::Normal));
        }
    }
    out
}

/// Soft drop shadow behind the layer content.
pub fn drop_shadow(img: &RasterImage, offset: Vec2, blur: f32, color: [f32; 4]) -> RasterImage {
    let mut shadow = RasterImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let a = img.get_f32(x, y)[3];
            if a > 0.0 {
                shadow.set_f32(x, y, [1.0, 1.0, 1.0, a]);
            }
        }
    }
    shadow = gaussian_blur(&shadow, blur.max(0.01));
    let mut out = RasterImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let sx = x as i32 - offset.x.round() as i32;
            let sy = y as i32 - offset.y.round() as i32;
            if sx < 0 || sy < 0 || sx >= img.width as i32 || sy >= img.height as i32 {
                continue;
            }
            let a = shadow.get_f32(sx as u32, sy as u32)[3];
            if a <= 0.0 {
                continue;
            }
            out.set_f32(x, y, [color[0], color[1], color[2], color[3] * a]);
        }
    }
    for y in 0..img.height {
        for x in 0..img.width {
            let d = out.get_f32(x, y);
            let s = img.get_f32(x, y);
            if s[3] > 0.0 {
                out.set_f32(x, y, composite_pixel(d, s, BlendMode::Normal));
            }
        }
    }
    out
}

pub fn outer_glow(img: &RasterImage, color: [f32; 4], radius: f32, intensity: f32) -> RasterImage {
    let mut mask = RasterImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let a = img.get_f32(x, y)[3];
            mask.set_f32(x, y, [1.0, 1.0, 1.0, a]);
        }
    }
    let blurred = gaussian_blur(&mask, radius.max(0.1));
    let mut out = RasterImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let a = blurred.get_f32(x, y)[3] * intensity;
            if a <= 0.0 {
                continue;
            }
            out.set_f32(x, y, [color[0], color[1], color[2], color[3] * a]);
        }
    }
    for y in 0..img.height {
        for x in 0..img.width {
            let d = out.get_f32(x, y);
            let s = img.get_f32(x, y);
            if s[3] > 0.0 {
                out.set_f32(x, y, composite_pixel(d, s, BlendMode::Normal));
            }
        }
    }
    out
}

pub fn inner_glow(img: &RasterImage, color: [f32; 4], radius: f32, intensity: f32) -> RasterImage {
    let mut out = img.clone();
    let blurred = gaussian_blur(img, radius.max(0.1));
    for y in 0..img.height {
        for x in 0..img.width {
            let edge = img.get_f32(x, y)[3] - blurred.get_f32(x, y)[3];
            if edge <= 0.0 {
                continue;
            }
            let dst = out.get_f32(x, y);
            let src = [
                color[0],
                color[1],
                color[2],
                color[3] * edge.clamp(0.0, 1.0) * intensity,
            ];
            out.set_f32(x, y, composite_pixel(dst, src, BlendMode::Screen));
        }
    }
    out
}

/// Bevel & emboss driven by the layer alpha as a height map.
pub fn bevel_emboss(
    img: &RasterImage,
    depth: f32,
    angle_deg: f32,
    highlight: f32,
    shadow: f32,
) -> RasterImage {
    let mut out = img.clone();
    let height: Vec<f32> = img.alpha_mask();
    let w = img.width as i32;
    let h = img.height as i32;
    let lx = angle_deg.to_radians().cos();
    let ly = -angle_deg.to_radians().sin();
    let at = |x: i32, y: i32| -> f32 {
        if x < 0 || y < 0 || x >= w || y >= h {
            0.0
        } else {
            height[(y * w + x) as usize]
        }
    };
    for y in 0..h {
        for x in 0..w {
            let hx = at(x + 1, y) - at(x - 1, y);
            let hy = at(x, y + 1) - at(x, y - 1);
            let lit = (hx * lx + hy * ly) * depth;
            let px = out.get_f32(x as u32, y as u32);
            if px[3] <= 0.0 {
                continue;
            }
            let mut rgb = [px[0], px[1], px[2]];
            if lit > 0.0 {
                for c in &mut rgb {
                    *c = (*c + lit * highlight).clamp(0.0, 1.0);
                }
            } else {
                for c in &mut rgb {
                    *c = (*c + lit * shadow).clamp(0.0, 1.0);
                }
            }
            out.set_f32(x as u32, y as u32, [rgb[0], rgb[1], rgb[2], px[3]]);
        }
    }
    out
}

pub fn invert(img: &RasterImage) -> RasterImage {
    map_rgb(img, |c| [1.0 - c[0], 1.0 - c[1], 1.0 - c[2]])
}

pub fn threshold(img: &RasterImage, level: f32) -> RasterImage {
    map_rgb(img, |c| {
        let l = crate::color::luma(c);
        let v = if l >= level { 1.0 } else { 0.0 };
        [v, v, v]
    })
}

pub fn posterize(img: &RasterImage, levels: u32) -> RasterImage {
    let levels = levels.clamp(2, 64) as f32;
    map_rgb(img, |c| {
        [
            (c[0] * levels).floor() / (levels - 1.0),
            (c[1] * levels).floor() / (levels - 1.0),
            (c[2] * levels).floor() / (levels - 1.0),
        ]
    })
}

pub fn sepia(img: &RasterImage, amount: f32) -> RasterImage {
    let a = amount.clamp(0.0, 1.0);
    map_rgb(img, |c| {
        let sepia = [
            c[0] * 0.393 + c[1] * 0.769 + c[2] * 0.189,
            c[0] * 0.349 + c[1] * 0.686 + c[2] * 0.168,
            c[0] * 0.272 + c[1] * 0.534 + c[2] * 0.131,
        ];
        [
            (c[0] + (sepia[0] - c[0]) * a).clamp(0.0, 1.0),
            (c[1] + (sepia[1] - c[1]) * a).clamp(0.0, 1.0),
            (c[2] + (sepia[2] - c[2]) * a).clamp(0.0, 1.0),
        ]
    })
}

pub fn grayscale(img: &RasterImage) -> RasterImage {
    map_rgb(img, |c| {
        let l = crate::color::luma(c);
        [l, l, l]
    })
}

/// Stretches the histogram so the darkest pixel becomes black and the
/// brightest becomes white. Ignores fully transparent pixels.
pub fn auto_levels(img: &RasterImage) -> RasterImage {
    let mut min = 1.0f32;
    let mut max = 0.0f32;
    for px in img.data.chunks_exact(4) {
        if px[3] < 4 {
            continue;
        }
        for c in 0..3 {
            let v = px[c] as f32 / 255.0;
            min = min.min(v);
            max = max.max(v);
        }
    }
    if max - min < 1e-4 {
        return img.clone();
    }
    levels(img, min, max, 1.0, 0.0, 1.0)
}

/// Applies a 3D LUT in both directions (identity when the LUT is identity).
pub fn apply_lut3d(img: &RasterImage, lut: &crate::color::Lut3D) -> RasterImage {
    map_rgb(img, |c| lut.apply(c))
}

/// Validates that an adjust payload is usable.
pub fn ensure_not_empty(img: &RasterImage) -> Result<(), ImageError> {
    if img.is_empty() {
        Err(ImageError::Empty)
    } else {
        Ok(())
    }
}

/// Colour wheel-style lift/gamma/gain grading (also used by the video editor).
pub fn lift_gamma_gain(
    img: &RasterImage,
    lift: [f32; 3],
    gamma: [f32; 3],
    gain: [f32; 3],
    saturation: f32,
    contrast: f32,
) -> RasterImage {
    map_rgb(img, |c| {
        let mut out = [0.0f32; 3];
        for i in 0..3 {
            let g = gamma[i].max(0.01);
            let v = ((c[i] + lift[i] * (1.0 - c[i])).max(0.0) * gain[i]).clamp(0.0, 1.0);
            let v = v.powf(1.0 / g);
            out[i] = ((v - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
        }
        saturate(out, saturation)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient() -> RasterImage {
        let mut img = RasterImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let v = x as f32 / 15.0;
                img.set_f32(x, y, [v, v, v, 1.0]);
            }
        }
        img
    }

    #[test]
    fn levels_identity_keeps_image() {
        let img = gradient();
        let out = levels(&img, 0.0, 1.0, 1.0, 0.0, 1.0);
        assert_eq!(out.get(8, 8), img.get(8, 8));
    }

    #[test]
    fn hue_rotation_turns_red_green() {
        let mut img = RasterImage::filled(4, 4, [255, 0, 0, 255]);
        img = hue_saturation(&img, 120.0, 1.0, 0.0);
        let px = img.get(1, 1);
        assert!(px[1] > 200 && px[0] < 40, "{px:?}");
    }

    #[test]
    fn exposure_brightens() {
        let img = RasterImage::filled(2, 2, [100, 100, 100, 255]);
        let out = exposure_contrast(&img, 1.0, 1.0, 0.0);
        assert!(out.get(0, 0)[0] > 150);
        let darker = exposure_contrast(&img, -1.0, 1.0, 0.0);
        assert!(darker.get(0, 0)[0] < 60);
    }

    #[test]
    fn vignette_darkens_corners() {
        let img = RasterImage::filled(32, 32, [200, 200, 200, 255]);
        let out = vignette(&img, 0.8, 0.2, 0.6);
        assert!(out.get(0, 0)[0] < out.get(16, 16)[0]);
    }

    #[test]
    fn drop_shadow_extends_alpha() {
        let mut img = RasterImage::new(32, 32);
        for y in 12..20 {
            for x in 12..20 {
                img.set(x, y, [255, 255, 255, 255]);
            }
        }
        let out = drop_shadow(&img, Vec2::new(3.0, 3.0), 2.0, [0.0, 0.0, 0.0, 0.8]);
        assert!(
            out.get(23, 23)[3] > 0,
            "shadow should be offset past content"
        );
        assert_eq!(out.get(13, 13)[3], 255);
    }

    #[test]
    fn glows_add_pixels_outside_and_inside() {
        let mut img = RasterImage::new(24, 24);
        for y in 8..16 {
            for x in 8..16 {
                img.set(x, y, [120, 120, 120, 255]);
            }
        }
        let outer = outer_glow(&img, [0.0, 0.5, 1.0, 1.0], 3.0, 1.0);
        assert!(outer.get(5, 12)[3] > 0);
        let inner = inner_glow(&img, [1.0, 0.0, 0.0, 1.0], 2.0, 1.0);
        assert_ne!(inner.get(8, 8), img.get(8, 8));
    }

    #[test]
    fn bevel_shades_edges() {
        // A bevel needs an alpha edge to derive height from.
        let mut img = RasterImage::new(16, 16);
        for y in 4..12 {
            for x in 4..12 {
                img.set(x, y, [128, 128, 128, 255]);
            }
        }
        let out = bevel_emboss(&img, 3.0, 45.0, 0.5, 0.5);
        let changed = (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .filter(|(x, y)| out.get(*x, *y) != img.get(*x, *y))
            .count();
        assert!(changed > 0, "bevel should shade the shape border");
    }

    #[test]
    fn invert_and_threshold() {
        let img = RasterImage::filled(2, 2, [0, 0, 0, 255]);
        assert_eq!(invert(&img).get(0, 0), [255, 255, 255, 255]);
        let mut g = RasterImage::new(2, 2);
        g.set(0, 0, [250, 250, 250, 255]);
        g.set(1, 0, [10, 10, 10, 255]);
        let t = threshold(&g, 0.5);
        assert_eq!(t.get(0, 0)[0], 255);
        assert_eq!(t.get(1, 0)[0], 0);
    }

    #[test]
    fn posterize_limits_values() {
        let out = posterize(&gradient(), 4);
        let mut values: Vec<u8> = out.data.chunks_exact(4).map(|p| p[0]).collect();
        values.sort_unstable();
        values.dedup();
        assert!(values.len() <= 5, "{} distinct values", values.len());
    }

    #[test]
    fn auto_levels_stretches_contrast() {
        let mut img = RasterImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                let v = 0.4 + x as f32 * 0.02;
                img.set_f32(x, y, [v, v, v, 1.0]);
            }
        }
        let out = auto_levels(&img);
        assert!(out.get(0, 0)[0] < 10);
        assert!(out.get(3, 0)[0] > 245);
    }

    #[test]
    fn color_balance_and_grading_stay_in_range() {
        let img = RasterImage::filled(4, 4, [120, 140, 200, 255]);
        let cb = color_balance(&img, 0.6, -0.2, 1.4);
        assert!(cb.data.chunks_exact(4).all(|p| p[3] == 255));
        let graded = lift_gamma_gain(
            &img,
            [0.05, 0.0, -0.02],
            [1.1, 1.0, 0.95],
            [1.2, 1.0, 0.9],
            1.2,
            1.1,
        );
        assert!(graded.get(0, 0)[0] > 0);
    }

    #[test]
    fn gradient_overlay_and_lut() {
        let img = RasterImage::filled(8, 8, [128, 128, 128, 255]);
        let out = gradient_overlay(&img, [1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], 0.0, 0.5);
        assert_ne!(out.get(0, 0), img.get(0, 0));
        let lut = crate::color::Lut3D::identity(2);
        let same = apply_lut3d(&img, &lut);
        assert_eq!(same.get(1, 1), img.get(1, 1));
        assert!(ensure_not_empty(&img).is_ok());
        assert!(ensure_not_empty(&RasterImage::default()).is_err());
        assert_ne!(
            grayscale(&RasterImage::filled(2, 2, [255, 0, 0, 255])).get(0, 0),
            [255, 0, 0, 255]
        );
        assert_ne!(sepia(&img, 1.0).get(0, 0), [128, 128, 128, 255]);
    }
}
