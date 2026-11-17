//! Layer blend modes and alpha compositing (W3C compositing model).

use crate::color::Rgb;
use roedit_core::gfx::BlendMode;

fn soft_light_channel(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
    } else {
        let d = if cb <= 0.25 {
            ((16.0 * cb - 12.0) * cb + 4.0) * cb
        } else {
            cb.sqrt()
        };
        cb + (2.0 * cs - 1.0) * (d - cb)
    }
}

fn screen(cb: f32, cs: f32) -> f32 {
    cb + cs - cb * cs
}

fn hard_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb * 2.0 * cs
    } else {
        screen(cb, 2.0 * cs - 1.0)
    }
}

fn color_dodge(cb: f32, cs: f32) -> f32 {
    if cb <= 1e-5 {
        0.0
    } else if cs >= 1.0 - 1e-5 {
        1.0
    } else {
        (cb / (1.0 - cs)).min(1.0)
    }
}

fn color_burn(cb: f32, cs: f32) -> f32 {
    if cb >= 1.0 - 1e-5 {
        1.0
    } else if cs <= 1e-5 {
        0.0
    } else {
        1.0 - ((1.0 - cb) / cs).min(1.0)
    }
}

fn lum(c: Rgb) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn sat(c: Rgb) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn clip_color(c: Rgb) -> Rgb {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        for ch in &mut out {
            *ch = l + (*ch - l) * l / (l - n).max(1e-6);
        }
    }
    if x > 1.0 {
        for ch in &mut out {
            *ch = l + (*ch - l) * (1.0 - l) / (x - l).max(1e-6);
        }
    }
    out
}

fn set_lum(c: Rgb, l: f32) -> Rgb {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

fn set_sat(c: Rgb, s: f32) -> Rgb {
    let mut v = [(0usize, c[0]), (1usize, c[1]), (2usize, c[2])];
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (min_i, min) = v[0];
    let (mid_i, mid) = v[1];
    let (max_i, max) = v[2];
    let mut out = [0.0f32; 3];
    if max - min > 1e-6 {
        out[mid_i] = (mid - min) * s / (max - min);
        out[max_i] = s;
    }
    out[min_i] = 0.0;
    out
}

/// Blends one backdrop/source channel pair.
pub fn blend_channel(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    let (cb, cs) = (cb.clamp(0.0, 1.0), cs.clamp(0.0, 1.0));
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => cb * cs,
        BlendMode::Screen => screen(cb, cs),
        BlendMode::Overlay => hard_light(cs, cb),
        BlendMode::SoftLight => soft_light_channel(cb, cs),
        BlendMode::HardLight => hard_light(cb, cs),
        BlendMode::ColorDodge => color_dodge(cb, cs),
        BlendMode::ColorBurn => color_burn(cb, cs),
        BlendMode::LinearBurn => (cb + cs - 1.0).clamp(0.0, 1.0),
        BlendMode::Difference => (cb - cs).abs(),
        BlendMode::Exclusion => cb + cs - 2.0 * cb * cs,
        BlendMode::Add => (cb + cs).clamp(0.0, 1.0),
        BlendMode::Subtract => (cb - cs).max(0.0),
        BlendMode::Darken => cb.min(cs),
        BlendMode::Lighten => cb.max(cs),
        // Non-separable modes are handled in `blend_rgb`.
        BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => cb,
    }
}

pub fn blend_rgb(mode: BlendMode, cb: Rgb, cs: Rgb) -> Rgb {
    match mode {
        BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        BlendMode::Color => set_lum(cs, lum(cb)),
        BlendMode::Luminosity => set_lum(cb, lum(cs)),
        _ => [
            blend_channel(mode, cb[0], cs[0]),
            blend_channel(mode, cb[1], cs[1]),
            blend_channel(mode, cb[2], cs[2]),
        ],
    }
}

/// Straight-alpha source-over compositing with a blend mode.
pub fn composite_pixel(dst: [f32; 4], src: [f32; 4], mode: BlendMode) -> [f32; 4] {
    let sa = src[3].clamp(0.0, 1.0);
    let da = dst[3].clamp(0.0, 1.0);
    if sa <= 0.0 {
        return dst;
    }
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-6 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let blended = blend_rgb(mode, [dst[0], dst[1], dst[2]], [src[0], src[1], src[2]]);
    let mut out = [0.0f32; 4];
    for c in 0..3 {
        out[c] = (((1.0 - da) * sa * src[c]) + (da * sa * blended[c]) + ((1.0 - sa) * da * dst[c]))
            / out_a;
    }
    out[3] = out_a;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_opaque_replaces() {
        let out = composite_pixel(
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            BlendMode::Normal,
        );
        assert!((out[2] - 1.0).abs() < 1e-5);
        assert!((out[0]).abs() < 1e-5);
        assert!((out[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn half_alpha_mixes() {
        let out = composite_pixel(
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 0.5],
            BlendMode::Normal,
        );
        assert!((out[0] - 0.5).abs() < 1e-5);
        assert!((out[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn multiply_darkens() {
        let out = composite_pixel(
            [0.5, 0.5, 0.5, 1.0],
            [0.5, 0.5, 0.5, 1.0],
            BlendMode::Multiply,
        );
        assert!((out[0] - 0.25).abs() < 1e-5);
    }

    #[test]
    fn screen_and_add_lighten() {
        let s = composite_pixel(
            [0.4, 0.4, 0.4, 1.0],
            [0.3, 0.3, 0.3, 1.0],
            BlendMode::Screen,
        );
        assert!(s[0] > 0.4);
        let a = composite_pixel([0.4, 0.4, 0.4, 1.0], [0.3, 0.3, 0.3, 1.0], BlendMode::Add);
        assert!((a[0] - 0.7).abs() < 1e-5);
    }

    #[test]
    fn transparent_source_is_noop() {
        let dst = [0.2, 0.3, 0.4, 0.8];
        let out = composite_pixel(dst, [1.0, 1.0, 1.0, 0.0], BlendMode::Normal);
        assert_eq!(out, dst);
    }

    #[test]
    fn non_separable_modes_keep_luminosity() {
        let cb = [0.2, 0.6, 0.4];
        let out = blend_rgb(BlendMode::Luminosity, cb, [0.9, 0.1, 0.1]);
        assert!((lum(out) - 0.9 * 0.3 + 0.1 * 0.59 + 0.1 * 0.11).abs() < 0.2);
        let hue = blend_rgb(BlendMode::Hue, cb, [0.9, 0.2, 0.1]);
        assert!(hue.iter().all(|v| (-0.01..=1.01).contains(v)));
    }

    #[test]
    fn blending_stays_in_gamut() {
        for mode in BlendMode::ALL {
            let out = blend_rgb(mode, [0.9, 0.1, 0.5], [0.2, 0.8, 0.05]);
            assert!(out.iter().all(|v| (-0.001..=1.001).contains(v)), "{mode:?}");
        }
    }
}
