//! Chroma keying and traditional background removal (border-connected flood
//! fill + feathered matte). No machine learning involved.

use crate::filters::gaussian_blur;
use roedit_core::math::smoothstep;
use roedit_core::raster::RasterImage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChromaKeySettings {
    pub key_color: [f32; 3],
    /// Chroma distance below which pixels are fully keyed out.
    pub tolerance: f32,
    /// Extra chroma distance used to feather the matte.
    pub softness: f32,
    /// How strongly to remove colour spill from the subject.
    pub spill_suppression: f32,
}

impl Default for ChromaKeySettings {
    fn default() -> Self {
        Self {
            key_color: [0.1, 0.75, 0.2],
            tolerance: 0.18,
            softness: 0.12,
            spill_suppression: 0.6,
        }
    }
}

fn chroma(c: [f32; 3]) -> [f32; 2] {
    let y = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    [c[2] - y, c[0] - y]
}

/// Keys out the configured colour, suppressing spill. Returns the number of
/// pixels whose alpha changed.
pub fn chroma_key(img: &mut RasterImage, settings: &ChromaKeySettings) -> usize {
    let key = chroma(settings.key_color);
    let tolerance = settings.tolerance.max(0.0);
    let softness = settings.softness.max(1e-4);
    let mut changed = 0;
    // Dominant channel of the key colour drives spill suppression.
    let key_dominant = if settings.key_color[1] >= settings.key_color[0]
        && settings.key_color[1] >= settings.key_color[2]
    {
        1
    } else if settings.key_color[0] >= settings.key_color[2] {
        0
    } else {
        2
    };
    for y in 0..img.height {
        for x in 0..img.width {
            let mut px = img.get_f32(x, y);
            if px[3] <= 0.0 {
                continue;
            }
            let c = chroma([px[0], px[1], px[2]]);
            let d = ((c[0] - key[0]).powi(2) + (c[1] - key[1]).powi(2)).sqrt();
            let matte = smoothstep(tolerance, tolerance + softness, d);
            if matte < 0.999 {
                px[3] *= matte;
                // De-spill: pull the dominant channel towards the average of
                // the other two, proportionally to the matte.
                let others = [0usize, 1, 2]
                    .iter()
                    .copied()
                    .filter(|i| *i != key_dominant)
                    .collect::<Vec<_>>();
                let avg = (px[others[0]] + px[others[1]]) * 0.5;
                if px[key_dominant] > avg {
                    let amount = (1.0 - matte) * settings.spill_suppression.clamp(0.0, 1.0);
                    px[key_dominant] -= (px[key_dominant] - avg) * amount;
                }
                img.set_f32(x, y, px);
                changed += 1;
            }
        }
    }
    changed
}

/// Average colour of the one-pixel border, useful as a key colour guess.
pub fn sample_border_color(img: &RasterImage) -> [f32; 3] {
    if img.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let mut sum = [0.0f32; 3];
    let mut n = 0.0;
    let sample = |x: u32, y: u32, sum: &mut [f32; 3], n: &mut f32| {
        let p = img.get(x, y);
        if p[3] < 8 {
            return;
        }
        for c in 0..3 {
            sum[c] += p[c] as f32 / 255.0;
        }
        *n += 1.0;
    };
    for x in 0..img.width {
        sample(x, 0, &mut sum, &mut n);
        sample(x, img.height - 1, &mut sum, &mut n);
    }
    for y in 0..img.height {
        sample(0, y, &mut sum, &mut n);
        sample(img.width - 1, y, &mut sum, &mut n);
    }
    if n <= 0.0 {
        return [0.0, 0.0, 0.0];
    }
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

fn color_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Removes a border-connected background colour. Only pixels reachable from the
/// image border are removed, so similar colours inside the subject survive.
pub fn remove_background(img: &mut RasterImage, tolerance: f32, feather: f32) -> usize {
    if img.is_empty() {
        return 0;
    }
    let reference = sample_border_color(img);
    let tolerance = tolerance.clamp(0.0, 1.5);
    let w = img.width as usize;
    let h = img.height as usize;
    let mut mask = vec![0.0f32; w * h];
    let mut visited = vec![false; w * h];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let idx = y * w + x;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        let px = img.get_f32(x as u32, y as u32);
        let d = color_distance([px[0], px[1], px[2]], reference);
        if d > tolerance || px[3] <= 0.005 {
            continue;
        }
        mask[idx] = 1.0;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    let removed = mask.iter().filter(|m| **m > 0.0).count();
    if removed == 0 {
        return 0;
    }
    if feather > 0.05 {
        let mask_img = RasterImage::from_gray(img.width, img.height, &mask);
        let blurred = gaussian_blur(&mask_img, feather);
        for y in 0..h {
            for x in 0..w {
                let m = blurred.get_f32(x as u32, y as u32)[0].clamp(0.0, 1.0);
                let mut px = img.get_f32(x as u32, y as u32);
                px[3] *= 1.0 - m;
                img.set_f32(x as u32, y as u32, px);
            }
        }
    } else {
        for y in 0..h {
            for x in 0..w {
                if mask[y * w + x] > 0.0 {
                    let mut px = img.get_f32(x as u32, y as u32);
                    px[3] = 0.0;
                    img.set_f32(x as u32, y as u32, px);
                }
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn green_screen_is_keyed() {
        let mut img = RasterImage::filled(32, 32, [30, 200, 60, 255]);
        // subject block
        for y in 10..20 {
            for x in 10..20 {
                img.set(x, y, [220, 170, 140, 255]);
            }
        }
        let changed = chroma_key(&mut img, &ChromaKeySettings::default());
        assert!(changed > 500);
        assert_eq!(img.get(0, 0)[3], 0);
        assert!(img.get(15, 15)[3] > 200, "subject must survive");
    }

    #[test]
    fn border_color_sampling() {
        let mut img = RasterImage::filled(8, 8, [255, 255, 255, 255]);
        img.set(4, 4, [0, 0, 0, 255]);
        let c = sample_border_color(&img);
        assert!((c[0] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn background_removal_keeps_enclosed_similar_colour() {
        let mut img = RasterImage::filled(32, 32, [250, 250, 250, 255]);
        // white-ish square fully enclosed by a dark ring
        for y in 8..24 {
            for x in 8..24 {
                img.set(x, y, [20, 20, 20, 255]);
            }
        }
        img.set(16, 16, [250, 250, 250, 255]);
        let removed = remove_background(&mut img, 0.15, 0.0);
        assert!(removed > 200);
        assert_eq!(img.get(0, 0)[3], 0);
        assert_eq!(img.get(16, 16)[3], 255, "enclosed pixel must stay");
        assert_eq!(img.get(12, 12)[3], 255, "subject must stay");
    }

    #[test]
    fn feathered_removal_produces_partial_alpha() {
        let mut img = RasterImage::filled(32, 32, [0, 0, 0, 255]);
        for y in 12..20 {
            for x in 12..20 {
                img.set(x, y, [255, 0, 0, 255]);
            }
        }
        remove_background(&mut img, 0.1, 2.0);
        let partial = img
            .data
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[3] < 255)
            .count();
        assert!(partial > 0, "feather should create soft edges");
    }

    #[test]
    fn uniform_image_is_fully_removed() {
        let mut img = RasterImage::filled(8, 8, [10, 20, 30, 255]);
        let removed = remove_background(&mut img, 0.5, 0.0);
        assert_eq!(removed, 64);
        assert!(img.data.chunks_exact(4).all(|p| p[3] == 0));
    }
}
