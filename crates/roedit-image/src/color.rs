//! Colour utilities: HSL/HSV conversion, monotone curve LUTs, levels and 3D
//! `.cube` LUT parsing/application.

use crate::ImageError;

pub type Rgb = [f32; 3];

pub fn luma(c: Rgb) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

pub fn rgb_to_hsl(c: Rgb) -> Rgb {
    let r = c[0];
    let g = c[1];
    let b = c[2];
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    let d = max - min;
    if d.abs() < 1e-6 {
        return [0.0, 0.0, l];
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min).max(1e-6)
    };
    let h = if (max - r).abs() < 1e-6 {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < 1e-6 {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    [h, s, l]
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 0.5 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

pub fn hsl_to_rgb(c: Rgb) -> Rgb {
    let (h, s, l) = (
        c[0].rem_euclid(1.0),
        c[1].clamp(0.0, 1.0),
        c[2].clamp(0.0, 1.0),
    );
    if s <= 1e-6 {
        return [l, l, l];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    [
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    ]
}

pub fn rgb_to_hsv(c: Rgb) -> Rgb {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    let d = max - min;
    let v = max;
    let s = if max <= 1e-6 { 0.0 } else { d / max };
    let h = if d <= 1e-6 {
        0.0
    } else if (max - c[0]).abs() < 1e-6 {
        ((c[1] - c[2]) / d).rem_euclid(6.0) / 6.0
    } else if (max - c[1]).abs() < 1e-6 {
        ((c[2] - c[0]) / d + 2.0) / 6.0
    } else {
        ((c[0] - c[1]) / d + 4.0) / 6.0
    };
    [h, s, v]
}

pub fn hsv_to_rgb(c: Rgb) -> Rgb {
    let (h, s, v) = (
        c[0].rem_euclid(1.0),
        c[1].clamp(0.0, 1.0),
        c[2].clamp(0.0, 1.0),
    );
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match (i as i32).rem_euclid(6) {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// Adjusts saturation by mixing towards luminance.
pub fn saturate(c: Rgb, amount: f32) -> Rgb {
    let l = luma(c);
    [
        (l + (c[0] - l) * amount).clamp(0.0, 1.0),
        (l + (c[1] - l) * amount).clamp(0.0, 1.0),
        (l + (c[2] - l) * amount).clamp(0.0, 1.0),
    ]
}

pub fn temperature_shift(c: Rgb, amount: f32) -> Rgb {
    // Positive amount warms (more red, less blue).
    [
        (c[0] + amount * 0.12).clamp(0.0, 1.0),
        (c[1] + amount * 0.02).clamp(0.0, 1.0),
        (c[2] - amount * 0.12).clamp(0.0, 1.0),
    ]
}

/// Applies a Photoshop-style levels operation to a single channel.
pub fn levels(
    v: f32,
    in_black: f32,
    in_white: f32,
    gamma: f32,
    out_black: f32,
    out_white: f32,
) -> f32 {
    let in_white = in_white.max(in_black + 1e-4);
    let t = ((v - in_black) / (in_white - in_black)).clamp(0.0, 1.0);
    let g = t.powf(1.0 / gamma.max(0.01));
    out_black + g * (out_white - out_black)
}

/// Monotone cubic (Fritsch-Carlson) interpolation through 0..1 control points.
/// Returns a 256-entry lookup table with no overshoot.
pub fn curves_lut(points: &[(f32, f32)]) -> [f32; 256] {
    let mut pts: Vec<(f32, f32)> = points
        .iter()
        .map(|(x, y)| (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)))
        .collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-5);
    if pts.len() < 2 {
        let mut lut = [0.0f32; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = i as f32 / 255.0;
        }
        return lut;
    }
    // Ensure the curve is defined at both ends.
    if pts.first().unwrap().0 > 0.0 {
        pts.insert(0, (0.0, pts.first().unwrap().1));
    }
    if pts.last().unwrap().0 < 1.0 {
        pts.push((1.0, pts.last().unwrap().1));
    }
    let n = pts.len();
    let mut slopes = vec![0.0f32; n - 1];
    for i in 0..n - 1 {
        let dx = (pts[i + 1].0 - pts[i].0).max(1e-6);
        slopes[i] = (pts[i + 1].1 - pts[i].1) / dx;
    }
    let mut tangents = vec![0.0f32; n];
    tangents[0] = slopes[0];
    tangents[n - 1] = slopes[n - 2];
    for i in 1..n - 1 {
        if slopes[i - 1] * slopes[i] <= 0.0 {
            tangents[i] = 0.0;
        } else {
            tangents[i] = (slopes[i - 1] + slopes[i]) * 0.5;
        }
    }
    // Limit tangents so segments stay monotone.
    for i in 0..n - 1 {
        if slopes[i].abs() < 1e-8 {
            tangents[i] = 0.0;
            tangents[i + 1] = 0.0;
            continue;
        }
        let a = tangents[i] / slopes[i];
        let b = tangents[i + 1] / slopes[i];
        let s = a * a + b * b;
        if s > 9.0 {
            let t = 3.0 / s.sqrt();
            tangents[i] = t * a * slopes[i];
            tangents[i + 1] = t * b * slopes[i];
        }
    }
    let mut lut = [0.0f32; 256];
    for (i, out) in lut.iter_mut().enumerate() {
        let x = i as f32 / 255.0;
        let mut seg = n - 2;
        for k in 0..n - 1 {
            if x >= pts[k].0 && x <= pts[k + 1].0 {
                seg = k;
                break;
            }
        }
        let (x0, y0) = pts[seg];
        let (x1, y1) = pts[seg + 1];
        let h = (x1 - x0).max(1e-6);
        let t = ((x - x0) / h).clamp(0.0, 1.0);
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        *out = (h00 * y0 + h10 * h * tangents[seg] + h01 * y1 + h11 * h * tangents[seg + 1])
            .clamp(0.0, 1.0);
    }
    lut
}

pub fn apply_lut1d(lut: &[f32; 256], v: f32) -> f32 {
    let x = (v.clamp(0.0, 1.0) * 255.0) as usize;
    lut[x.min(255)]
}

/// A 3D colour lookup table loaded from an Adobe `.cube` file.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut3D {
    pub size: usize,
    pub title: String,
    /// Red varies fastest, then green, then blue (`.cube` convention).
    pub data: Vec<Rgb>,
}

impl Lut3D {
    pub fn identity(size: usize) -> Self {
        let size = size.max(2);
        let mut data = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.push([
                        r as f32 / (size - 1) as f32,
                        g as f32 / (size - 1) as f32,
                        b as f32 / (size - 1) as f32,
                    ]);
                }
            }
        }
        Self {
            size,
            title: "Identity".into(),
            data,
        }
    }

    pub fn parse(text: &str) -> Result<Lut3D, ImageError> {
        let mut size: Option<usize> = None;
        let mut title = String::from("LUT");
        let mut data: Vec<Rgb> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let upper = line.to_ascii_uppercase();
            if let Some(rest) = upper.strip_prefix("TITLE") {
                let _ = rest;
                if let Some(idx) = line.find('"') {
                    title = line[idx + 1..].trim_end_matches('"').to_string();
                }
                continue;
            }
            if let Some(rest) = upper.strip_prefix("LUT_3D_SIZE") {
                let _ = rest;
                size = line.split_whitespace().nth(1).and_then(|s| s.parse().ok());
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 3 {
                let rgb: Option<Rgb> = (|| {
                    Some([
                        parts[0].parse().ok()?,
                        parts[1].parse().ok()?,
                        parts[2].parse().ok()?,
                    ])
                })();
                if let Some(c) = rgb {
                    data.push(c);
                }
            }
        }
        let size =
            size.ok_or_else(|| ImageError::InvalidParameter("LUT has no LUT_3D_SIZE".into()))?;
        if data.len() != size * size * size {
            return Err(ImageError::InvalidParameter(format!(
                "LUT expects {} entries but the file has {}",
                size * size * size,
                data.len()
            )));
        }
        Ok(Lut3D { size, title, data })
    }

    /// Trilinear lookup of a linear-space colour.
    pub fn apply(&self, c: Rgb) -> Rgb {
        let s = self.size;
        let f = |v: f32| v.clamp(0.0, 1.0) * (s - 1) as f32;
        let (rf, gf, bf) = (f(c[0]), f(c[1]), f(c[2]));
        let r0 = rf.floor() as usize;
        let g0 = gf.floor() as usize;
        let b0 = bf.floor() as usize;
        let r1 = (r0 + 1).min(s - 1);
        let g1 = (g0 + 1).min(s - 1);
        let b1 = (b0 + 1).min(s - 1);
        let (tr, tg, tb) = (rf - r0 as f32, gf - g0 as f32, bf - b0 as f32);
        let at = |r: usize, g: usize, b: usize| self.data[(b * s + g) * s + r];
        let lerp3 = |a: Rgb, b: Rgb, t: f32| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };
        let c00 = lerp3(at(r0, g0, b0), at(r1, g0, b0), tr);
        let c10 = lerp3(at(r0, g1, b0), at(r1, g1, b0), tr);
        let c01 = lerp3(at(r0, g0, b1), at(r1, g0, b1), tr);
        let c11 = lerp3(at(r0, g1, b1), at(r1, g1, b1), tr);
        let c0 = lerp3(c00, c10, tg);
        let c1 = lerp3(c01, c11, tg);
        lerp3(c0, c1, tb)
    }
}

pub struct Palette {
    pub name: &'static str,
    pub colors: Vec<[u8; 3]>,
}

/// Curated palettes for GFX and material work.
pub fn palettes() -> Vec<Palette> {
    vec![
        Palette {
            name: "Roblox Classic",
            colors: vec![
                [196, 40, 28],
                [13, 105, 172],
                [245, 205, 48],
                [40, 127, 71],
                [75, 151, 75],
                [105, 64, 40],
                [180, 210, 228],
                [163, 162, 165],
                [236, 139, 70],
                [255, 255, 255],
                [0, 0, 0],
                [27, 42, 53],
            ],
        },
        Palette {
            name: "Neon Studio",
            colors: vec![
                [255, 46, 136],
                [0, 255, 214],
                [255, 214, 0],
                [142, 68, 255],
                [0, 168, 255],
                [255, 92, 38],
                [18, 18, 24],
                [247, 247, 255],
            ],
        },
        Palette {
            name: "Cinematic Teal & Orange",
            colors: vec![
                [12, 32, 45],
                [27, 74, 94],
                [232, 150, 74],
                [255, 196, 120],
                [120, 178, 190],
                [246, 240, 226],
                [58, 58, 66],
                [8, 8, 10],
            ],
        },
        Palette {
            name: "Pastel GFX",
            colors: vec![
                [255, 209, 220],
                [255, 240, 190],
                [200, 240, 220],
                [200, 220, 255],
                [230, 210, 255],
                [255, 255, 255],
                [120, 110, 140],
                [60, 55, 75],
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_roundtrip() {
        for c in [
            [0.8, 0.2, 0.1],
            [0.1, 0.9, 0.3],
            [0.2, 0.2, 0.8],
            [0.5, 0.5, 0.5],
            [0.0, 0.0, 0.0],
        ] {
            let back = hsl_to_rgb(rgb_to_hsl(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 1e-3, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn hsv_roundtrip() {
        for c in [[0.9, 0.4, 0.2], [0.0, 0.5, 0.5], [1.0, 1.0, 1.0]] {
            let back = hsv_to_rgb(rgb_to_hsv(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn identity_curve_is_linear() {
        let lut = curves_lut(&[(0.0, 0.0), (1.0, 1.0)]);
        for i in 0..256 {
            assert!((lut[i] - i as f32 / 255.0).abs() < 1e-3);
        }
    }

    #[test]
    fn curve_lifts_midtones_without_overshoot() {
        let lut = curves_lut(&[(0.0, 0.0), (0.5, 0.75), (1.0, 1.0)]);
        assert!(lut[128] > 0.5);
        assert!(lut[128] < 0.8);
        for w in lut.windows(2) {
            assert!(w[1] >= w[0] - 1e-5, "curve must stay monotone");
        }
    }

    #[test]
    fn levels_maps_black_and_white() {
        assert!((levels(0.0, 0.1, 0.9, 1.0, 0.0, 1.0) - 0.0).abs() < 1e-5);
        assert!((levels(1.0, 0.1, 0.9, 1.0, 0.0, 1.0) - 1.0).abs() < 1e-5);
        let mid = levels(0.5, 0.0, 1.0, 2.0, 0.0, 1.0);
        assert!((mid - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-2);
    }

    #[test]
    fn cube_lut_parses_and_applies() {
        let text = "TITLE \"Test\"\nLUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
        let lut = Lut3D::parse(text).unwrap();
        assert_eq!(lut.size, 2);
        assert_eq!(lut.title, "Test");
        let c = lut.apply([1.0, 0.0, 0.0]);
        assert!((c[0] - 1.0).abs() < 1e-5);
        let mid = lut.apply([0.5, 0.5, 0.5]);
        assert!((mid[0] - 0.5).abs() < 1e-4);
        assert!(Lut3D::parse("LUT_3D_SIZE 2\n").is_err());
    }

    #[test]
    fn palettes_are_named() {
        let p = palettes();
        assert!(p.len() >= 3);
        assert!(p
            .iter()
            .all(|pal| !pal.name.is_empty() && !pal.colors.is_empty()));
    }

    #[test]
    fn saturation_and_temperature() {
        let gray = saturate([0.5, 0.5, 0.5], 2.0);
        assert!((gray[0] - 0.5).abs() < 1e-5);
        let warm = temperature_shift([0.5, 0.5, 0.5], 1.0);
        assert!(warm[0] > warm[2]);
    }
}
