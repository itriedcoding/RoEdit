//! Raster image loading and saving (PNG, JPEG, TIFF, BMP, WebP, OpenEXR).

use crate::ImportError;
use image::{DynamicImage, ExtendedColorType, ImageEncoder};
use roedit_core::raster::RasterImage;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFormat {
    Png,
    Jpeg,
    Tiff,
    Exr,
}

impl SaveFormat {
    pub fn extension(self) -> &'static str {
        match self {
            SaveFormat::Png => "png",
            SaveFormat::Jpeg => "jpg",
            SaveFormat::Tiff => "tif",
            SaveFormat::Exr => "exr",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SaveFormat::Png => "PNG (lossless, alpha)",
            SaveFormat::Jpeg => "JPEG (lossy, no alpha)",
            SaveFormat::Tiff => "TIFF (lossless, alpha)",
            SaveFormat::Exr => "OpenEXR (linear float)",
        }
    }

    pub fn from_path(path: &Path) -> Option<SaveFormat> {
        match path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .as_deref()
        {
            Some("png") => Some(SaveFormat::Png),
            Some("jpg") | Some("jpeg") => Some(SaveFormat::Jpeg),
            Some("tif") | Some("tiff") => Some(SaveFormat::Tiff),
            Some("exr") => Some(SaveFormat::Exr),
            _ => None,
        }
    }

    pub fn all() -> [SaveFormat; 4] {
        [
            SaveFormat::Png,
            SaveFormat::Jpeg,
            SaveFormat::Tiff,
            SaveFormat::Exr,
        ]
    }
}

fn srgb_to_linear(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

fn from_dynamic(img: DynamicImage) -> RasterImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    RasterImage {
        width: w,
        height: h,
        data: rgba.into_raw(),
    }
}

/// Loads an image from memory. EXR needs a path because of the exr API.
pub fn load_image_bytes(bytes: &[u8]) -> Result<RasterImage, ImportError> {
    if bytes.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
        return Err(ImportError::Unsupported(
            "OpenEXR import needs a file path; use load_image()".into(),
        ));
    }
    let img = image::load_from_memory(bytes)
        .map_err(|e| ImportError::Image(format!("could not decode image: {e}")))?;
    Ok(from_dynamic(img))
}

/// Loads an image from disk, including OpenEXR.
pub fn load_image(path: &Path) -> Result<RasterImage, ImportError> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
        return load_exr(path);
    }
    load_image_bytes(&bytes)
}

fn load_exr(path: &Path) -> Result<RasterImage, ImportError> {
    use exr::prelude::*;
    let image = read_first_rgba_layer_from_file(
        path,
        |resolution, _channels| {
            let w = resolution.width();
            let h = resolution.height();
            vec![vec![[0.0f32; 4]; w]; h]
        },
        |pixels: &mut Vec<Vec<[f32; 4]>>,
         position: Vec2<usize>,
         (r, g, b, a): (f32, f32, f32, f32)| {
            if let Some(row) = pixels.get_mut(position.y()) {
                if let Some(px) = row.get_mut(position.x()) {
                    *px = [r, g, b, a];
                }
            }
        },
    )
    .map_err(|e| ImportError::Image(format!("could not decode EXR: {e}")))?;
    let rows = image.layer_data.channel_data.pixels;
    let height = rows.len() as u32;
    let width = rows.first().map(|r| r.len()).unwrap_or(0) as u32;
    let mut out = RasterImage::new(width, height);
    for (y, row) in rows.iter().enumerate() {
        for (x, px) in row.iter().enumerate() {
            let alpha = px[3].clamp(0.0, 1.0);
            out.set(
                x as u32,
                y as u32,
                [
                    linear_to_srgb(px[0]),
                    linear_to_srgb(px[1]),
                    linear_to_srgb(px[2]),
                    (alpha * 255.0).round() as u8,
                ],
            );
        }
    }
    Ok(out)
}

/// Encodes an image as PNG bytes.
pub fn encode_png(img: &RasterImage) -> Result<Vec<u8>, ImportError> {
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    encoder
        .write_image(&img.data, img.width, img.height, ExtendedColorType::Rgba8)
        .map_err(|e| ImportError::Image(format!("PNG encode failed: {e}")))?;
    Ok(out)
}

fn to_rgb_over_white(img: &RasterImage) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.width * img.height * 3) as usize);
    for px in img.data.chunks_exact(4) {
        let a = px[3] as f32 / 255.0;
        for c in 0..3 {
            let v = px[c] as f32 * a + 255.0 * (1.0 - a);
            out.push(v.round().clamp(0.0, 255.0) as u8);
        }
    }
    out
}

/// Saves an image in the requested format.
pub fn save_image(
    img: &RasterImage,
    path: &Path,
    format: SaveFormat,
    jpeg_quality: u8,
) -> Result<(), ImportError> {
    if img.width == 0 || img.height == 0 {
        return Err(ImportError::Image("cannot save an empty image".into()));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    match format {
        SaveFormat::Png => {
            let bytes = encode_png(img)?;
            std::fs::write(path, bytes)?;
        }
        SaveFormat::Jpeg => {
            let rgb = to_rgb_over_white(img);
            let file = std::fs::File::create(path)?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::io::BufWriter::new(file),
                jpeg_quality.clamp(1, 100),
            );
            encoder
                .encode(&rgb, img.width, img.height, ExtendedColorType::Rgb8)
                .map_err(|e| ImportError::Image(format!("JPEG encode failed: {e}")))?;
        }
        SaveFormat::Tiff => {
            let file = std::fs::File::create(path)?;
            let encoder = image::codecs::tiff::TiffEncoder::new(std::io::BufWriter::new(file));
            encoder
                .write_image(&img.data, img.width, img.height, ExtendedColorType::Rgba8)
                .map_err(|e| ImportError::Image(format!("TIFF encode failed: {e}")))?;
        }
        SaveFormat::Exr => {
            use exr::prelude::write_rgba_file;
            write_rgba_file(path, img.width as usize, img.height as usize, |x, y| {
                let p = img.get(x as u32, y as u32);
                (
                    srgb_to_linear(p[0]),
                    srgb_to_linear(p[1]),
                    srgb_to_linear(p[2]),
                    p[3] as f32 / 255.0,
                )
            })
            .map_err(|e| ImportError::Image(format!("EXR encode failed: {e}")))?;
        }
    }
    Ok(())
}

/// Loads every frame of an image sequence (sorted by file name).
pub fn load_sequence(
    dir: &Path,
    prefix: &str,
    extension: &str,
) -> Result<Vec<RasterImage>, ImportError> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with(prefix) && n.ends_with(extension)
                })
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(ImportError::Invalid(format!(
            "no {extension} files starting with '{prefix}' in {}",
            dir.display()
        )));
    }
    paths.iter().map(|p| load_image(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> RasterImage {
        let mut img = RasterImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.set(
                    x,
                    y,
                    [
                        (x * 255 / w.max(1)) as u8,
                        (y * 255 / h.max(1)) as u8,
                        ((x + y) % 256) as u8,
                        255,
                    ],
                );
            }
        }
        img
    }

    #[test]
    fn png_roundtrip_is_lossless() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.png");
        let img = gradient(16, 12);
        save_image(&img, &path, SaveFormat::Png, 90).unwrap();
        let back = load_image(&path).unwrap();
        assert_eq!((back.width, back.height), (16, 12));
        assert_eq!(back.data, img.data);
    }

    #[test]
    fn tiff_roundtrip_is_lossless() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.tif");
        let img = gradient(8, 8);
        save_image(&img, &path, SaveFormat::Tiff, 90).unwrap();
        let back = load_image(&path).unwrap();
        assert_eq!(back.data, img.data);
    }

    #[test]
    fn jpeg_writes_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        let img = RasterImage::filled(32, 32, [200, 40, 40, 255]);
        save_image(&img, &path, SaveFormat::Jpeg, 95).unwrap();
        let back = load_image(&path).unwrap();
        assert_eq!((back.width, back.height), (32, 32));
        let px = back.get(16, 16);
        assert!((px[0] as i32 - 200).abs() < 30, "got {px:?}");
        assert_eq!(px[3], 255);
    }

    #[test]
    fn exr_roundtrip_survives_linear_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.exr");
        let img = RasterImage::filled(4, 4, [128, 64, 200, 255]);
        save_image(&img, &path, SaveFormat::Exr, 90).unwrap();
        let back = load_image(&path).unwrap();
        assert_eq!((back.width, back.height), (4, 4));
        let px = back.get(1, 1);
        assert!((px[0] as i32 - 128).abs() <= 2, "got {px:?}");
        assert!((px[1] as i32 - 64).abs() <= 2, "got {px:?}");
        assert!((px[2] as i32 - 200).abs() <= 2, "got {px:?}");
    }

    #[test]
    fn rejects_unknown_bytes() {
        assert!(load_image_bytes(b"not an image at all").is_err());
    }

    #[test]
    fn jpeg_flattens_alpha_onto_white() {
        let img = RasterImage::filled(2, 2, [0, 0, 0, 0]);
        let rgb = to_rgb_over_white(&img);
        assert_eq!(
            rgb,
            vec![255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]
        );
    }
}
