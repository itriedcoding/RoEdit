//! Layer stack flattening: groups, opacity, masks, clipping masks and blend
//! modes, plus single-layer previews for the UI.

use crate::blend::composite_pixel;
use crate::{blit, text, to_u8};
use roedit_core::gfx::{GfxDoc, Layer};
use roedit_core::ids::LayerId;
use roedit_core::raster::RasterImage;

#[derive(Clone, Copy, Debug)]
pub struct FlattenOptions {
    /// Fill the canvas with the document background before compositing.
    pub include_background: bool,
}

impl Default for FlattenOptions {
    fn default() -> Self {
        Self {
            include_background: true,
        }
    }
}

/// Composites the whole document into a single image.
pub fn flatten_doc(doc: &GfxDoc, opts: &FlattenOptions) -> RasterImage {
    let stack = render_scope(doc, None, doc.width.max(1), doc.height.max(1));
    if !opts.include_background {
        return stack;
    }
    let bg = doc.background;
    let mut base = RasterImage::filled(
        stack.width,
        stack.height,
        [to_u8(bg[0]), to_u8(bg[1]), to_u8(bg[2]), to_u8(bg[3])],
    );
    for y in 0..base.height {
        for x in 0..base.width {
            let d = base.get_f32(x, y);
            let s = stack.get_f32(x, y);
            if s[3] <= 0.0 {
                continue;
            }
            base.set_f32(
                x,
                y,
                composite_pixel(d, s, roedit_core::gfx::BlendMode::Normal),
            );
        }
    }
    base
}

fn render_scope(doc: &GfxDoc, parent: Option<LayerId>, width: u32, height: u32) -> RasterImage {
    let mut out = RasterImage::new(width, height);
    for layer in doc.layers.iter().filter(|l| l.parent == parent) {
        if !layer.visible || layer.opacity <= 0.0 {
            continue;
        }
        let mut img = layer_image(doc, layer, width, height);
        if layer.mask_enabled {
            if let Some(mask) = &layer.mask {
                apply_mask(&mut img, mask);
            }
        }
        let opacity = layer.opacity.clamp(0.0, 1.0);
        for y in 0..height {
            for x in 0..width {
                let mut src = img.get_f32(x, y);
                if src[3] <= 0.0 {
                    continue;
                }
                src[3] *= opacity;
                if layer.clip_to_below {
                    // Clipping masks are revealed only where the stack below is
                    // opaque.
                    src[3] *= out.get_f32(x, y)[3];
                    if src[3] <= 0.0 {
                        continue;
                    }
                }
                let dst = out.get_f32(x, y);
                out.set_f32(x, y, composite_pixel(dst, src, layer.blend));
            }
        }
    }
    out
}

fn layer_image(doc: &GfxDoc, layer: &Layer, width: u32, height: u32) -> RasterImage {
    if layer.is_group {
        return render_scope(doc, Some(layer.id), width, height);
    }
    if let Some(pixels) = &layer.pixels {
        let mut out = RasterImage::new(width, height);
        let source = if pixels.width == width && pixels.height == height {
            pixels.as_ref().clone()
        } else {
            pixels.resized(width, height)
        };
        blit(&mut out, &source, layer.offset);
        return out;
    }
    if let Some(spec) = &layer.text {
        let mut rendered = text::render_text(spec, width, height);
        if layer.offset != [0, 0] {
            let mut shifted = RasterImage::new(width, height);
            blit(&mut shifted, &rendered, layer.offset);
            rendered = shifted;
        }
        return rendered;
    }
    RasterImage::new(width, height)
}

/// Multiplies layer alpha by the mask luminance (alpha channel if present).
pub fn apply_mask(img: &mut RasterImage, mask: &RasterImage) {
    for y in 0..img.height {
        for x in 0..img.width {
            let m = mask.sample_bilinear(x as f32, y as f32);
            let weight = if m[3] > 1e-4 {
                (0.2126 * m[0] + 0.7152 * m[1] + 0.0722 * m[2]) * m[3]
            } else {
                0.0
            };
            let mut px = img.get_f32(x, y);
            px[3] *= weight.clamp(0.0, 1.0);
            img.set_f32(x, y, px);
        }
    }
}

/// Renders a single layer at (at most) `max_size` for the layers panel.
pub fn render_layer_preview(doc: &GfxDoc, layer: &Layer, max_size: u32) -> RasterImage {
    let img = layer_image(doc, layer, doc.width.max(1), doc.height.max(1));
    let largest = img.width.max(img.height).max(1);
    if largest <= max_size {
        return img;
    }
    let scale = max_size as f32 / largest as f32;
    let w = ((img.width as f32 * scale) as u32).max(1);
    let h = ((img.height as f32 * scale) as u32).max(1);
    img.resized(w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::gfx::{BlendMode, CanvasPreset, TextSpec};

    fn doc_with_layers() -> GfxDoc {
        let mut doc = GfxDoc::new(32, 32);
        doc.add_pixel_layer("Bottom", RasterImage::filled(32, 32, [255, 0, 0, 255]));
        doc.add_pixel_layer("Top", RasterImage::filled(32, 32, [0, 0, 255, 255]));
        doc
    }

    #[test]
    fn top_opaque_layer_wins() {
        let doc = doc_with_layers();
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        assert_eq!(flat.get(4, 4), [0, 0, 255, 255]);
    }

    #[test]
    fn opacity_blends() {
        let mut doc = doc_with_layers();
        let top = doc.layers[1].id;
        doc.layer_mut(top).unwrap().opacity = 0.5;
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        let px = flat.get(4, 4);
        assert!(px[0] > 100 && px[2] > 100, "{px:?}");
    }

    #[test]
    fn hidden_layers_are_skipped() {
        let mut doc = doc_with_layers();
        let top = doc.layers[1].id;
        doc.layer_mut(top).unwrap().visible = false;
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        assert_eq!(flat.get(0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn multiply_blend_is_applied() {
        let mut doc = GfxDoc::new(8, 8);
        doc.add_pixel_layer("A", RasterImage::filled(8, 8, [128, 128, 128, 255]));
        let top = doc.add_pixel_layer("B", RasterImage::filled(8, 8, [128, 128, 128, 255]));
        doc.layer_mut(top).unwrap().blend = BlendMode::Multiply;
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        let px = flat.get(1, 1);
        assert!((px[0] as i32 - 64).abs() <= 2, "{px:?}");
    }

    #[test]
    fn group_opacity_applies_to_children() {
        let mut doc = GfxDoc::new(8, 8);
        doc.add_pixel_layer("Base", RasterImage::filled(8, 8, [255, 255, 255, 255]));
        let group = doc.add_group("Group", None);
        let child = doc.add_pixel_layer("Child", RasterImage::filled(8, 8, [0, 0, 0, 255]));
        doc.layer_mut(child).unwrap().parent = Some(group);
        doc.layer_mut(group).unwrap().opacity = 0.5;
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        let px = flat.get(2, 2);
        assert!(px[0] > 100 && px[0] < 160, "{px:?}");
    }

    #[test]
    fn mask_removes_alpha() {
        let mut doc = GfxDoc::new(8, 8);
        doc.add_pixel_layer("Layer", RasterImage::filled(8, 8, [0, 255, 0, 255]));
        let mut mask = RasterImage::new(8, 8);
        for x in 0..8 {
            mask.set(x, 4, [255, 255, 255, 255]);
        }
        let id = doc.layers[0].id;
        doc.layer_mut(id).unwrap().mask = Some(std::sync::Arc::new(mask));
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        assert_eq!(flat.get(0, 0)[3], 0);
        assert!(flat.get(4, 4)[3] > 200);
    }

    #[test]
    fn clipping_mask_uses_backdrop_alpha() {
        let mut doc = GfxDoc::new(8, 8);
        let base = doc.add_pixel_layer("Base", RasterImage::new(8, 8));
        for x in 0..8 {
            doc.layer_mut(base)
                .unwrap()
                .pixel_mut()
                .unwrap()
                .set(x, 0, [255, 255, 255, 255]);
        }
        let clip = doc.add_pixel_layer("Clip", RasterImage::filled(8, 8, [255, 0, 0, 255]));
        doc.layer_mut(clip).unwrap().clip_to_below = true;
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        assert_eq!(flat.get(0, 0)[3], 255);
        assert_eq!(flat.get(0, 4)[3], 0);
    }

    #[test]
    fn background_fill_is_included_when_requested() {
        let mut doc = GfxDoc::new(4, 4);
        doc.background = [0.0, 0.0, 0.0, 1.0];
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: true,
            },
        );
        assert_eq!(flat.get(0, 0), [0, 0, 0, 255]);
        let transparent = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        assert_eq!(transparent.get(0, 0)[3], 0);
    }

    #[test]
    fn text_layers_render() {
        let mut doc = GfxDoc::from_preset(CanvasPreset::GameIcon);
        doc.add_text_layer("Title", TextSpec::default());
        let flat = flatten_doc(
            &doc,
            &FlattenOptions {
                include_background: false,
            },
        );
        let lit = flat.data.chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(lit > 100, "text should cover pixels, got {lit}");
    }

    #[test]
    fn preview_is_scaled_down() {
        let doc = doc_with_layers();
        let preview = render_layer_preview(&doc, &doc.layers[0], 8);
        assert!(preview.width <= 8 && preview.height <= 8);
    }
}
