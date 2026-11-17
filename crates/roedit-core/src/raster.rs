//! RGBA8 raster image container, shared by the texture editor, the GFX designer,
//! the compositor and the offline renderer.
//!
//! Images are stored as straight (non-premultiplied) sRGB RGBA8. Serialisation
//! uses base64-encoded PNG so project files stay reasonably small.

use crate::math::clamp01;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8, straight alpha.
    pub data: Vec<u8>,
}

impl RasterImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width as usize * height as usize) * 4],
        }
    }

    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut img = Self::new(width, height);
        for px in img.data.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
        img
    }

    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        if data.len() != width as usize * height as usize * 4 {
            return None;
        }
        Some(Self {
            width,
            height,
            data,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn byte_len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            None
        } else {
            Some((y as usize * self.width as usize + x as usize) * 4)
        }
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        match self.index(x, y) {
            Some(i) => [
                self.data[i],
                self.data[i + 1],
                self.data[i + 2],
                self.data[i + 3],
            ],
            None => [0, 0, 0, 0],
        }
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if let Some(i) = self.index(x, y) {
            self.data[i..i + 4].copy_from_slice(&rgba);
        }
    }

    /// Straight-alpha source-over composite of one pixel.
    #[inline]
    pub fn blend_over(&mut self, x: u32, y: u32, src: [u8; 4]) {
        let Some(i) = self.index(x, y) else { return };
        let sa = src[3] as f32 / 255.0;
        if sa <= 0.0 {
            return;
        }
        let da = self.data[i + 3] as f32 / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a <= 0.0 {
            self.data[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
            return;
        }
        for c in 0..3 {
            let s = src[c] as f32 / 255.0;
            let d = self.data[i + c] as f32 / 255.0;
            let out = (s * sa + d * da * (1.0 - sa)) / out_a;
            self.data[i + c] = (clamp01(out) * 255.0).round() as u8;
        }
        self.data[i + 3] = (clamp01(out_a) * 255.0).round() as u8;
    }

    pub fn get_f32(&self, x: u32, y: u32) -> [f32; 4] {
        let p = self.get(x, y);
        [
            p[0] as f32 / 255.0,
            p[1] as f32 / 255.0,
            p[2] as f32 / 255.0,
            p[3] as f32 / 255.0,
        ]
    }

    pub fn set_f32(&mut self, x: u32, y: u32, rgba: [f32; 4]) {
        self.set(
            x,
            y,
            [
                (clamp01(rgba[0]) * 255.0).round() as u8,
                (clamp01(rgba[1]) * 255.0).round() as u8,
                (clamp01(rgba[2]) * 255.0).round() as u8,
                (clamp01(rgba[3]) * 255.0).round() as u8,
            ],
        );
    }

    /// Bilinear sample in pixel coordinates.
    pub fn sample_bilinear(&self, x: f32, y: f32) -> [f32; 4] {
        if self.is_empty() {
            return [0.0; 4];
        }
        let fx = x.clamp(0.0, self.width as f32 - 1.0);
        let fy = y.clamp(0.0, self.height as f32 - 1.0);
        let x0 = fx.floor() as u32;
        let y0 = fy.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let p00 = self.get_f32(x0, y0);
        let p10 = self.get_f32(x1, y0);
        let p01 = self.get_f32(x0, y1);
        let p11 = self.get_f32(x1, y1);
        let mut out = [0.0f32; 4];
        for c in 0..4 {
            let a = p00[c] + (p10[c] - p00[c]) * tx;
            let b = p01[c] + (p11[c] - p01[c]) * tx;
            out[c] = a + (b - a) * ty;
        }
        out
    }

    /// Bilinear sample with wrap addressing (seamless texture preview).
    pub fn sample_wrapped(&self, x: f32, y: f32) -> [f32; 4] {
        if self.is_empty() {
            return [0.0; 4];
        }
        let w = self.width as f32;
        let h = self.height as f32;
        let x = x.rem_euclid(w);
        let y = y.rem_euclid(h);
        let x0 = x.floor() as u32 % self.width;
        let y0 = y.floor() as u32 % self.height;
        let x1 = (x0 + 1) % self.width;
        let y1 = (y0 + 1) % self.height;
        let tx = x - x.floor();
        let ty = y - y.floor();
        let p00 = self.get_f32(x0, y0);
        let p10 = self.get_f32(x1, y0);
        let p01 = self.get_f32(x0, y1);
        let p11 = self.get_f32(x1, y1);
        let mut out = [0.0f32; 4];
        for c in 0..4 {
            let a = p00[c] + (p10[c] - p00[c]) * tx;
            let b = p01[c] + (p11[c] - p01[c]) * tx;
            out[c] = a + (b - a) * ty;
        }
        out
    }

    pub fn resized(&self, width: u32, height: u32) -> Self {
        let mut out = Self::new(width.max(1), height.max(1));
        if self.is_empty() {
            return out;
        }
        let sx = self.width as f32 / out.width as f32;
        let sy = self.height as f32 / out.height as f32;
        for y in 0..out.height {
            for x in 0..out.width {
                let px =
                    self.sample_bilinear((x as f32 + 0.5) * sx - 0.5, (y as f32 + 0.5) * sy - 0.5);
                out.set_f32(x, y, px);
            }
        }
        out
    }

    pub fn cropped(&self, x: i32, y: i32, width: u32, height: u32) -> Self {
        let mut out = Self::new(width.max(1), height.max(1));
        for oy in 0..out.height {
            for ox in 0..out.width {
                let sx = x + ox as i32;
                let sy = y + oy as i32;
                if sx >= 0 && sy >= 0 && (sx as u32) < self.width && (sy as u32) < self.height {
                    out.set(ox, oy, self.get(sx as u32, sy as u32));
                }
            }
        }
        out
    }

    pub fn flipped_horizontal(&self) -> Self {
        let mut out = Self::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                out.set(self.width - 1 - x, y, self.get(x, y));
            }
        }
        out
    }

    pub fn rotated_90(&self) -> Self {
        let mut out = Self::new(self.height, self.width);
        for y in 0..self.height {
            for x in 0..self.width {
                out.set(self.height - 1 - y, x, self.get(x, y));
            }
        }
        out
    }

    /// Alpha-only mask as a grayscale image (used for masks/height maps).
    pub fn alpha_mask(&self) -> Vec<f32> {
        self.data
            .chunks_exact(4)
            .map(|p| p[3] as f32 / 255.0)
            .collect()
    }

    /// Fill from a grayscale buffer.
    pub fn from_gray(width: u32, height: u32, values: &[f32]) -> Self {
        let mut img = Self::new(width, height);
        for (i, v) in values.iter().enumerate().take(img.pixel_count()) {
            let g = (clamp01(*v) * 255.0).round() as u8;
            img.data[i * 4..i * 4 + 4].copy_from_slice(&[g, g, g, 255]);
        }
        img
    }

    /// Grayscale luminance buffer (0..1).
    pub fn to_gray(&self) -> Vec<f32> {
        self.data
            .chunks_exact(4)
            .map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0)
            .collect()
    }

    pub fn png_bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        PngEncoder::new_with_quality(&mut out, CompressionType::Fast, PngFilter::Adaptive)
            .write_image(
                &self.data,
                self.width.max(1),
                self.height.max(1),
                ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("PNG encode failed: {e}"))?;
        Ok(out)
    }

    pub fn png_preview_data_uri(&self) -> String {
        match self.png_bytes() {
            Ok(bytes) => format!("data:image/png;base64,{}", b64::encode(&bytes)),
            Err(_) => String::new(),
        }
    }
}

/// Minimal zero-dependency base64 (standard alphabet, padded).
pub mod b64 {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = *chunk.get(1).unwrap_or(&0) as u32;
            let b2 = *chunk.get(2).unwrap_or(&0) as u32;
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(ALPHABET[(n >> 18) as usize & 63] as char);
            out.push(ALPHABET[(n >> 12) as usize & 63] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[(n >> 6) as usize & 63] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[n as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    pub fn decode(s: &str) -> Option<Vec<u8>> {
        let mut table = [255u8; 256];
        for (i, &c) in ALPHABET.iter().enumerate() {
            table[c as usize] = i as u8;
        }
        let bytes: Vec<u8> = s
            .bytes()
            .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
            .collect();
        if bytes.len() % 4 == 1 {
            return None;
        }
        let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
        let mut acc: u32 = 0;
        let mut bits = 0;
        for b in bytes {
            let v = table[b as usize];
            if v == 255 {
                return None;
            }
            acc = (acc << 6) | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        Some(out)
    }
}

/// Custom serde: RasterImage <-> base64 PNG string. Keeps JSON compact.
mod png_serde {
    use super::*;

    pub fn serialize<S: Serializer>(img: &RasterImage, s: S) -> Result<S::Ok, S::Error> {
        if img.is_empty() {
            return s.serialize_str("");
        }
        let bytes = img.png_bytes().map_err(serde::ser::Error::custom)?;
        s.serialize_str(&b64::encode(&bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<RasterImage, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() {
            return Ok(RasterImage::default());
        }
        let bytes =
            b64::decode(&s).ok_or_else(|| serde::de::Error::custom("invalid base64 image"))?;
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .map_err(serde::de::Error::custom)?;
        let rgba = decoded.to_rgba8();
        Ok(RasterImage {
            width: rgba.width(),
            height: rgba.height(),
            data: rgba.into_raw(),
        })
    }
}

impl Serialize for RasterImage {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        png_serde::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for RasterImage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        png_serde::deserialize(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        for sample in [
            &b""[..],
            &b"f"[..],
            &b"fo"[..],
            &b"foo"[..],
            &b"foobar"[..],
            &[0u8, 255, 128, 7, 9][..],
        ] {
            let e = b64::encode(sample);
            assert_eq!(b64::decode(&e).unwrap(), sample);
        }
    }

    #[test]
    fn blend_and_sample() {
        let mut img = RasterImage::filled(4, 4, [0, 0, 0, 255]);
        img.blend_over(0, 0, [255, 0, 0, 255]);
        assert_eq!(img.get(0, 0), [255, 0, 0, 255]);
        img.blend_over(0, 0, [0, 0, 255, 128]);
        let p = img.get(0, 0);
        assert!(p[2] > 100 && p[0] > 50);
        let s = img.sample_bilinear(1.5, 1.5);
        assert!(s[3] > 0.9);
    }

    #[test]
    fn resize_and_rotate() {
        let img = RasterImage::filled(8, 4, [10, 20, 30, 255]);
        let r = img.resized(16, 8);
        assert_eq!((r.width, r.height), (16, 8));
        let rot = img.rotated_90();
        assert_eq!((rot.width, rot.height), (4, 8));
        assert_eq!(rot.get(0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn png_serde_roundtrip() {
        let mut img = RasterImage::filled(3, 2, [1, 2, 3, 255]);
        img.set(1, 1, [200, 100, 50, 128]);
        let json = serde_json::to_string(&img).unwrap();
        let back: RasterImage = serde_json::from_str(&json).unwrap();
        assert_eq!(img, back);
    }

    #[test]
    fn crop_pads_out_of_bounds() {
        let img = RasterImage::filled(2, 2, [255, 0, 0, 255]);
        let c = img.cropped(-1, -1, 2, 2);
        assert_eq!(c.get(0, 0), [0, 0, 0, 0]);
        assert_eq!(c.get(1, 1), [255, 0, 0, 255]);
    }
}
