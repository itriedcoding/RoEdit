//! Traditional image filters: gaussian/box/motion blur, unsharp mask, median,
//! bilateral denoise. Blurs are separable and parallelised with rayon.

use crate::to_u8;
use rayon::prelude::*;
use roedit_core::math::Vec2;
use roedit_core::raster::RasterImage;

/// Normalised 1D gaussian kernel.
pub fn gaussian_kernel(sigma: f32) -> Vec<f32> {
    let sigma = sigma.max(0.1);
    let radius = (sigma * 3.0).ceil() as i32;
    let mut kernel = Vec::with_capacity((radius * 2 + 1) as usize);
    let mut sum = 0.0;
    for i in -radius..=radius {
        let x = i as f32;
        let v = (-(x * x) / (2.0 * sigma * sigma)).exp();
        kernel.push(v);
        sum += v;
    }
    if sum > 0.0 {
        for v in &mut kernel {
            *v /= sum;
        }
    }
    kernel
}

fn to_float(img: &RasterImage) -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity(img.pixel_count());
    for px in img.data.chunks_exact(4) {
        out.push([
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
            px[3] as f32 / 255.0,
        ]);
    }
    out
}

fn from_float(data: &[[f32; 4]], width: u32, height: u32) -> RasterImage {
    let mut img = RasterImage::new(width, height);
    for (i, px) in data.iter().enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width.max(1);
        img.set(
            x,
            y,
            [to_u8(px[0]), to_u8(px[1]), to_u8(px[2]), to_u8(px[3])],
        );
    }
    img
}

fn sample_clamped(data: &[[f32; 4]], width: u32, height: u32, x: i64, y: i64) -> [f32; 4] {
    let cx = x.clamp(0, width as i64 - 1) as usize;
    let cy = y.clamp(0, height as i64 - 1) as usize;
    data[cy * width as usize + cx]
}

/// Separable gaussian blur with clamped edges.
pub fn gaussian_blur(img: &RasterImage, sigma: f32) -> RasterImage {
    if img.is_empty() || sigma <= 0.05 {
        return img.clone();
    }
    let kernel = gaussian_kernel(sigma);
    let radius = (kernel.len() / 2) as i64;
    let (w, h) = (img.width, img.height);
    let src = to_float(img);
    // Horizontal pass
    let mut tmp = vec![[0.0f32; 4]; src.len()];
    tmp.par_chunks_mut(w as usize)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w as i64 {
                let mut acc = [0.0f32; 4];
                for (k, weight) in kernel.iter().enumerate() {
                    let sx = x + k as i64 - radius;
                    let px = sample_clamped(&src, w, h, sx, y as i64);
                    for c in 0..4 {
                        acc[c] += px[c] * weight;
                    }
                }
                row[x as usize] = acc;
            }
        });
    // Vertical pass
    let mut out = vec![[0.0f32; 4]; src.len()];
    out.par_chunks_mut(w as usize)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w as usize {
                let mut acc = [0.0f32; 4];
                for (k, weight) in kernel.iter().enumerate() {
                    let sy = y as i64 + k as i64 - radius;
                    let px = sample_clamped(&tmp, w, h, x as i64, sy);
                    for c in 0..4 {
                        acc[c] += px[c] * weight;
                    }
                }
                row[x] = acc;
            }
        });
    from_float(&out, w, h)
}

pub fn blur_in_place(img: &mut RasterImage, sigma: f32) {
    let blurred = gaussian_blur(img, sigma);
    *img = blurred;
}

pub fn box_blur(img: &RasterImage, radius: u32) -> RasterImage {
    let radius = radius as i64;
    if img.is_empty() || radius <= 0 {
        return img.clone();
    }
    let (w, h) = (img.width, img.height);
    let src = to_float(img);
    let mut out = vec![[0.0f32; 4]; src.len()];
    out.par_chunks_mut(w as usize)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w as usize {
                let mut acc = [0.0f32; 4];
                let mut n: f32 = 0.0;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let px = sample_clamped(&src, w, h, x as i64 + dx, y as i64 + dy);
                        for c in 0..4 {
                            acc[c] += px[c];
                        }
                        n += 1.0;
                    }
                }
                for c in &mut acc {
                    *c /= n.max(1.0);
                }
                row[x] = acc;
            }
        });
    from_float(&out, w, h)
}

/// Directional motion blur along `angle_deg`, `distance` pixels long.
pub fn motion_blur(img: &RasterImage, distance: f32, angle_deg: f32) -> RasterImage {
    if img.is_empty() || distance < 0.5 {
        return img.clone();
    }
    let (w, h) = (img.width, img.height);
    let src = to_float(img);
    let dir = Vec2::new(angle_deg.to_radians().cos(), angle_deg.to_radians().sin());
    let samples = (distance * 2.0).ceil().clamp(2.0, 128.0) as i64;
    let mut out = vec![[0.0f32; 4]; src.len()];
    out.par_chunks_mut(w as usize)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w as usize {
                let mut acc = [0.0f32; 4];
                for s in 0..samples {
                    let t = (s as f32 / (samples - 1).max(1) as f32 - 0.5) * distance;
                    let px = sample_clamped(
                        &src,
                        w,
                        h,
                        x as i64 + (dir.x * t).round() as i64,
                        y as i64 + (dir.y * t).round() as i64,
                    );
                    for c in 0..4 {
                        acc[c] += px[c];
                    }
                }
                for c in &mut acc {
                    *c /= samples as f32;
                }
                row[x] = acc;
            }
        });
    from_float(&out, w, h)
}

/// Unsharp mask: the standard sharpening filter.
pub fn sharpen(img: &RasterImage, amount: f32, threshold: f32) -> RasterImage {
    if img.is_empty() || amount <= 0.0 {
        return img.clone();
    }
    let blurred = gaussian_blur(img, 1.2);
    let mut out = img.clone();
    for y in 0..img.height {
        for x in 0..img.width {
            let a = img.get_f32(x, y);
            let b = blurred.get_f32(x, y);
            let diff = (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs();
            if diff < threshold {
                continue;
            }
            let mut px = a;
            for c in 0..3 {
                px[c] = (a[c] + (a[c] - b[c]) * amount).clamp(0.0, 1.0);
            }
            out.set_f32(x, y, px);
        }
    }
    out
}

fn median_channel(values: &mut [u8]) -> u8 {
    values.sort_unstable();
    values[values.len() / 2]
}

/// 3x3 (or larger) median filter: removes salt-and-pepper noise.
pub fn median_filter(img: &RasterImage, radius: u32) -> RasterImage {
    let r = radius.max(1) as i64;
    let mut out = img.clone();
    for y in 0..img.height as i64 {
        for x in 0..img.width as i64 {
            let mut channels: [Vec<u8>; 3] = [Vec::new(), Vec::new(), Vec::new()];
            let mut alpha: Vec<u8> = Vec::new();
            for dy in -r..=r {
                for dx in -r..=r {
                    let sx = (x + dx).clamp(0, img.width as i64 - 1) as u32;
                    let sy = (y + dy).clamp(0, img.height as i64 - 1) as u32;
                    let px = img.get(sx, sy);
                    for c in 0..3 {
                        channels[c].push(px[c]);
                    }
                    alpha.push(px[3]);
                }
            }
            let px = [
                median_channel(&mut channels[0]),
                median_channel(&mut channels[1]),
                median_channel(&mut channels[2]),
                median_channel(&mut alpha),
            ];
            out.set(x as u32, y as u32, px);
        }
    }
    out
}

/// Edge-preserving bilateral filter, used for noise reduction.
pub fn bilateral(
    img: &RasterImage,
    radius: u32,
    sigma_color: f32,
    sigma_space: f32,
) -> RasterImage {
    let r = radius.max(1) as i64;
    let sigma_color = sigma_color.max(1e-4);
    let sigma_space = sigma_space.max(1e-4);
    let mut out = img.clone();
    for y in 0..img.height as i64 {
        for x in 0..img.width as i64 {
            let center = img.get_f32(x as u32, y as u32);
            let mut acc = [0.0f32; 4];
            let mut weight_sum = 0.0;
            for dy in -r..=r {
                for dx in -r..=r {
                    let sx = (x + dx).clamp(0, img.width as i64 - 1) as u32;
                    let sy = (y + dy).clamp(0, img.height as i64 - 1) as u32;
                    let px = img.get_f32(sx, sy);
                    let spatial =
                        (-((dx * dx + dy * dy) as f32) / (2.0 * sigma_space * sigma_space)).exp();
                    let color_dist = ((px[0] - center[0]).powi(2)
                        + (px[1] - center[1]).powi(2)
                        + (px[2] - center[2]).powi(2))
                    .sqrt();
                    let range =
                        (-(color_dist * color_dist) / (2.0 * sigma_color * sigma_color)).exp();
                    let w = spatial * range;
                    for c in 0..4 {
                        acc[c] += px[c] * w;
                    }
                    weight_sum += w;
                }
            }
            if weight_sum > 0.0 {
                for c in &mut acc {
                    *c /= weight_sum;
                }
                out.set_f32(x as u32, y as u32, acc);
            }
        }
    }
    out
}

/// Convenience wrapper used by the "Noise Reduction" slider.
pub fn noise_reduce(img: &RasterImage, strength: f32) -> RasterImage {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.01 {
        return img.clone();
    }
    let radius = (1.0 + strength * 2.0) as u32;
    bilateral(img, radius, 0.02 + strength * 0.12, 1.0 + strength * 2.0)
}

/// Rotates an image by an arbitrary angle with bilinear sampling.
pub fn rotate_free(img: &RasterImage, angle_deg: f32, fill: [u8; 4]) -> RasterImage {
    if img.is_empty() {
        return img.clone();
    }
    let (w, h) = (img.width as f32, img.height as f32);
    let center = Vec2::new(w * 0.5, h * 0.5);
    let (sin, cos) = (-angle_deg.to_radians()).sin_cos();
    let mut out = RasterImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - center;
            let src = Vec2::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos) + center;
            if src.x < 0.0 || src.y < 0.0 || src.x >= w || src.y >= h {
                out.set(x, y, fill);
                continue;
            }
            let c = img.sample_bilinear(src.x - 0.5, src.y - 0.5);
            out.set_f32(x, y, c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_is_normalised() {
        let k = gaussian_kernel(2.0);
        let sum: f32 = k.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
        assert!(k.len() % 2 == 1);
    }

    #[test]
    fn blur_preserves_flat_colour() {
        let img = RasterImage::filled(32, 32, [100, 150, 200, 255]);
        let out = gaussian_blur(&img, 3.0);
        assert_eq!(out.get(16, 16), [100, 150, 200, 255]);
        assert_eq!(out.get(0, 0), [100, 150, 200, 255]);
    }

    #[test]
    fn blur_spreads_an_impulse() {
        let mut img = RasterImage::new(31, 31);
        img.set(15, 15, [255, 255, 255, 255]);
        let out = gaussian_blur(&img, 2.0);
        assert!(out.get(16, 15)[3] > 0);
        assert!(out.get(15, 15)[3] < 255);
        let total: u32 = out.data.chunks_exact(4).map(|p| p[3] as u32).sum();
        assert!(total > 100, "energy should be preserved, got {total}");
    }

    #[test]
    fn box_blur_averages() {
        let mut img = RasterImage::new(9, 9);
        img.set(4, 4, [255, 255, 255, 255]);
        let out = box_blur(&img, 1);
        let px = out.get(4, 4);
        assert!(px[0] > 0 && px[0] < 60, "{px:?}");
    }

    #[test]
    fn motion_blur_draws_a_streak() {
        let mut img = RasterImage::new(41, 41);
        img.set(20, 20, [255, 255, 255, 255]);
        let out = motion_blur(&img, 10.0, 0.0);
        assert!(out.get(24, 20)[0] > 0);
        assert_eq!(out.get(20, 30)[0], 0);
    }

    #[test]
    fn sharpen_increases_local_contrast() {
        let mut img = RasterImage::filled(16, 16, [120, 120, 120, 255]);
        for y in 0..16 {
            for x in 8..16 {
                img.set(x, y, [180, 180, 180, 255]);
            }
        }
        let out = sharpen(&img, 1.5, 0.0);
        assert!(out.get(7, 8)[0] <= 121);
        assert!(out.get(8, 8)[0] >= 179);
    }

    #[test]
    fn median_removes_salt_noise() {
        let mut img = RasterImage::filled(9, 9, [100, 100, 100, 255]);
        img.set(4, 4, [255, 255, 255, 255]);
        let out = median_filter(&img, 1);
        assert_eq!(out.get(4, 4), [100, 100, 100, 255]);
    }

    #[test]
    fn bilateral_keeps_edges() {
        let mut img = RasterImage::filled(16, 16, [20, 20, 20, 255]);
        for y in 0..16 {
            for x in 8..16 {
                img.set(x, y, [230, 230, 230, 255]);
            }
        }
        let out = bilateral(&img, 2, 0.05, 2.0);
        assert!(out.get(0, 8)[0] < 60);
        assert!(out.get(15, 8)[0] > 190);
    }

    #[test]
    fn noise_reduce_is_safe_on_empty() {
        let img = RasterImage::default();
        assert!(noise_reduce(&img, 0.5).is_empty());
    }

    #[test]
    fn rotate_maps_corners_to_fill() {
        let img = RasterImage::filled(16, 16, [255, 0, 0, 255]);
        let out = rotate_free(&img, 45.0, [0, 0, 0, 0]);
        assert_eq!(out.get(0, 0), [0, 0, 0, 0]);
        assert!(out.get(8, 8)[0] > 100);
    }
}
