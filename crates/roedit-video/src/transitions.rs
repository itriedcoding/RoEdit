//! Transition masks and frame blending.
//!
//! `transition_mask` returns 0 where the outgoing frame shows and 1 where the
//! incoming frame shows; `apply_transition` composites accordingly with a soft
//! edge on wipes so they do not alias.

use roedit_core::raster::RasterImage;
use roedit_core::timeline::TransitionKind;

/// 0 = outgoing/below, 1 = incoming/above.
pub fn transition_mask(kind: TransitionKind, t: f32, x: f32, y: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match kind {
        TransitionKind::CrossDissolve => t,
        TransitionKind::DipToBlack => {
            // Midpoint is black; the caller composites over a black canvas.
            if t < 0.5 {
                0.0
            } else {
                1.0
            }
        }
        TransitionKind::WipeLeft => smooth(t, x),
        TransitionKind::WipeRight => smooth(t, 1.0 - x),
        TransitionKind::WipeUp => smooth(t, 1.0 - y),
        TransitionKind::WipeDown => smooth(t, y),
        TransitionKind::Circle => {
            let dx = x - 0.5;
            let dy = y - 0.5;
            let d = (dx * dx + dy * dy).sqrt() / std::f32::consts::FRAC_1_SQRT_2;
            smooth(t, d)
        }
    }
}

fn smooth(t: f32, value: f32) -> f32 {
    let edge = 0.06;
    let p = (t - value) / edge + 0.5;
    p.clamp(0.0, 1.0)
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// Composites `above` over `below` using the transition mask. `DipToBlack`
/// fades through black instead of dissolving.
pub fn apply_transition(
    below: &RasterImage,
    above: &RasterImage,
    kind: TransitionKind,
    t: f32,
) -> RasterImage {
    let width = below.width.max(above.width);
    let height = below.height.max(below.height);
    let mut out = RasterImage::filled(width, height, [0, 0, 0, 255]);
    let t = t.clamp(0.0, 1.0);
    for y in 0..height {
        for x in 0..width {
            let nx = (x as f32 + 0.5) / width as f32;
            let ny = (y as f32 + 0.5) / height as f32;
            let b = below.get(
                x.min(below.width.saturating_sub(1)),
                y.min(below.height.saturating_sub(1)),
            );
            let a = above.get(
                x.min(above.width.saturating_sub(1)),
                y.min(above.height.saturating_sub(1)),
            );
            let px = match kind {
                TransitionKind::DipToBlack => {
                    if t < 0.5 {
                        let k = 1.0 - t * 2.0;
                        [
                            (b[0] as f32 * k) as u8,
                            (b[1] as f32 * k) as u8,
                            (b[2] as f32 * k) as u8,
                            255,
                        ]
                    } else {
                        let k = (t - 0.5) * 2.0;
                        [
                            (a[0] as f32 * k) as u8,
                            (a[1] as f32 * k) as u8,
                            (a[2] as f32 * k) as u8,
                            255,
                        ]
                    }
                }
                _ => {
                    let m = transition_mask(kind, t, nx, ny);
                    [
                        lerp_u8(b[0], a[0], m),
                        lerp_u8(b[1], a[1], m),
                        lerp_u8(b[2], a[2], m),
                        lerp_u8(b[3], a[3], m),
                    ]
                }
            };
            out.set(x, y, px);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RasterImage {
        RasterImage::filled(w, h, color)
    }

    #[test]
    fn edges_are_exact_for_every_kind() {
        let a = solid(16, 16, [255, 0, 0, 255]);
        let b = solid(16, 16, [0, 0, 255, 255]);
        for kind in TransitionKind::ALL {
            let start = apply_transition(&a, &b, kind, 0.0);
            let end = apply_transition(&a, &b, kind, 1.0);
            if kind == TransitionKind::DipToBlack {
                assert_eq!(start.get(8, 8), [255, 0, 0, 255]);
                assert_eq!(end.get(8, 8), [0, 0, 255, 255]);
                let mid = apply_transition(&a, &b, kind, 0.5);
                assert_eq!(mid.get(8, 8), [0, 0, 0, 255]);
            } else {
                assert_eq!(start.get(8, 8), [255, 0, 0, 255], "{kind:?}");
                assert_eq!(end.get(8, 8), [0, 0, 255, 255], "{kind:?}");
            }
        }
    }

    #[test]
    fn cross_dissolve_midpoint_mixes() {
        let a = solid(8, 8, [0, 0, 0, 255]);
        let b = solid(8, 8, [200, 100, 50, 255]);
        let mid = apply_transition(&a, &b, TransitionKind::CrossDissolve, 0.5);
        let px = mid.get(4, 4);
        assert!((px[0] as i32 - 100).abs() < 3, "{px:?}");
        assert!((px[1] as i32 - 50).abs() < 3, "{px:?}");
    }

    #[test]
    fn wipes_move_across_the_frame() {
        let a = solid(32, 32, [0, 0, 0, 255]);
        let b = solid(32, 32, [255, 255, 255, 255]);
        let mid = apply_transition(&a, &b, TransitionKind::WipeLeft, 0.5);
        let left = mid.get(2, 16);
        let right = mid.get(29, 16);
        assert!(
            left[0] > 230,
            "left edge should already be incoming: {left:?}"
        );
        assert!(
            right[0] < 20,
            "right edge should still be outgoing: {right:?}"
        );

        let vertical = apply_transition(&a, &b, TransitionKind::WipeDown, 0.5);
        assert!(vertical.get(16, 2)[0] > 230);
        assert!(vertical.get(16, 29)[0] < 20);
    }

    #[test]
    fn circle_reveals_from_the_centre() {
        let a = solid(32, 32, [0, 0, 0, 255]);
        let b = solid(32, 32, [255, 255, 255, 255]);
        let mid = apply_transition(&a, &b, TransitionKind::Circle, 0.35);
        assert!(mid.get(16, 16)[0] > 200);
        assert!(mid.get(0, 0)[0] < 50);
    }

    #[test]
    fn mask_values_stay_in_range() {
        for kind in TransitionKind::ALL {
            for t in [0.0, 0.25, 0.5, 1.0] {
                for (x, y) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)] {
                    let m = transition_mask(kind, t, x, y);
                    assert!((0.0..=1.0).contains(&m), "{kind:?} {t} -> {m}");
                }
            }
        }
    }
}
