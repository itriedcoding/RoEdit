//! Timeline compositor: evaluates a `Timeline` at a time and produces a frame.
//!
//! Clips composite bottom-to-top in track order (the timeline stores tracks with
//! the background last). Effects are applied per clip: transform, crop, blur,
//! chroma key, colour grade, opacity, speed and transitions.

use crate::transitions::apply_transition;
use roedit_core::animation::Ease;
use roedit_core::gfx::TextSpec;
use roedit_core::ids::ImageId;
use roedit_core::raster::RasterImage;
use roedit_core::timeline::{Clip, ClipEffect, ClipSource, Timeline, TransitionKind};
use roedit_image::filters::gaussian_blur;
use roedit_image::keying::{chroma_key, ChromaKeySettings};
use roedit_image::text::render_text;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Still images available to the timeline, keyed by the project image ids.
#[derive(Clone, Default)]
pub struct MediaLibrary {
    pub images: HashMap<ImageId, Arc<RasterImage>>,
}

impl MediaLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: ImageId, image: RasterImage) {
        self.images.insert(id, Arc::new(image));
    }

    pub fn get(&self, id: ImageId) -> Option<&Arc<RasterImage>> {
        self.images.get(&id)
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

pub struct Compositor {
    pub timeline: Timeline,
    pub media: MediaLibrary,
    /// Cache for disk-backed frames (image sequences / render output).
    frame_cache: HashMap<String, Arc<RasterImage>>,
    pub background: [u8; 4],
}

impl Compositor {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            media: MediaLibrary::new(),
            frame_cache: HashMap::new(),
            background: [0, 0, 0, 255],
        }
    }

    pub fn with_media(timeline: Timeline, media: MediaLibrary) -> Self {
        Self {
            timeline,
            media,
            frame_cache: HashMap::new(),
            background: [0, 0, 0, 255],
        }
    }

    pub fn clear_cache(&mut self) {
        self.frame_cache.clear();
    }

    pub fn frame_at(&mut self, time: f32) -> RasterImage {
        let width = self.timeline.width.max(1);
        let height = self.timeline.height.max(1);
        let mut canvas = RasterImage::filled(width, height, self.background);
        let clips: Vec<Clip> = self
            .timeline
            .active_clips(time)
            .into_iter()
            .filter(|c| {
                self.timeline
                    .track(c.track)
                    .map(|t| !t.kind.is_audio())
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        for clip in &clips {
            let Some(layer) = self.layer_for(clip, time) else {
                continue;
            };
            let mut opacity = clip.opacity.clamp(0.0, 1.0);
            if clip.fade_in > 0.0 {
                let k = ((time - clip.start) / clip.fade_in).clamp(0.0, 1.0);
                opacity *= k;
            }
            if clip.fade_out > 0.0 {
                let k = ((clip.end() - time) / clip.fade_out).clamp(0.0, 1.0);
                opacity *= k;
            }
            if let Some(ClipEffect::Transition {
                kind,
                duration,
                ease,
            }) = clip
                .effects
                .iter()
                .find(|e| matches!(e, ClipEffect::Transition { .. }))
            {
                let d = duration.max(0.0);
                if d > 0.0 && time < clip.start + d {
                    let raw = ((time - clip.start) / d).clamp(0.0, 1.0);
                    let t = ease.apply(raw);
                    let transitioned =
                        apply_transition(&canvas, &with_alpha(&layer, opacity), *kind, t);
                    canvas = transitioned;
                    continue;
                }
            }
            composite_over(&mut canvas, &layer, opacity);
        }
        canvas
    }

    /// Builds a layer for a clip, applying its effects in order.
    fn layer_for(&mut self, clip: &Clip, time: f32) -> Option<RasterImage> {
        let mut local = (time - clip.start).max(0.0) + clip.source_in.max(0.0);
        for effect in &clip.effects {
            if let ClipEffect::Speed { factor, reverse } = effect {
                let f = factor.clamp(0.05, 20.0);
                local = (time - clip.start).max(0.0) * f + clip.source_in.max(0.0);
                if *reverse {
                    local = (clip.duration * f - local).max(clip.source_in.max(0.0));
                }
            }
        }
        let width = self.timeline.width;
        let height = self.timeline.height;
        let mut image = match &clip.source {
            ClipSource::Image(id) => match self.media.get(*id) {
                Some(img) => (**img).clone(),
                None => RasterImage::new(width, height),
            },
            ClipSource::Color(color) => RasterImage::filled(
                width,
                height,
                [
                    (color[0].clamp(0.0, 1.0) * 255.0) as u8,
                    (color[1].clamp(0.0, 1.0) * 255.0) as u8,
                    (color[2].clamp(0.0, 1.0) * 255.0) as u8,
                    255,
                ],
            ),
            ClipSource::Text(spec) => render_text(spec, width, height),
            ClipSource::ImageSequence {
                directory,
                pattern,
                frame_count,
                fps,
            } => {
                let index = (local * fps.max(1.0)).floor().max(0.0) as u32;
                let index = index.min(frame_count.saturating_sub(1));
                let path = sequence_path(directory, pattern, index);
                self.load_cached(&path)
                    .unwrap_or_else(|| RasterImage::new(width, height))
            }
            ClipSource::RenderOutput(path) => {
                let p = PathBuf::from(path);
                if p.is_dir() {
                    let index = (local * self.timeline.fps.max(1.0)).floor().max(0.0) as u32;
                    let candidate = p.join(format!("frame_{index:04}.png"));
                    self.load_cached(&candidate)
                        .unwrap_or_else(|| RasterImage::new(width, height))
                } else {
                    self.load_cached(&p)
                        .unwrap_or_else(|| RasterImage::new(width, height))
                }
            }
            ClipSource::AudioFile(_) => return None,
        };

        if image.width != width || image.height != height {
            image = resize_bilinear(&image, width.max(1), height.max(1));
        }

        for effect in &clip.effects {
            match effect {
                ClipEffect::GaussianBlur { radius } => {
                    image = gaussian_blur(&image, radius.max(0.0));
                }
                ClipEffect::ChromaKey {
                    key_color,
                    tolerance,
                    softness,
                    spill_suppression,
                } => {
                    chroma_key(
                        &mut image,
                        &ChromaKeySettings {
                            key_color: *key_color,
                            tolerance: *tolerance,
                            softness: *softness,
                            spill_suppression: *spill_suppression,
                        },
                    );
                }
                ClipEffect::ColorGrade {
                    lift,
                    gamma,
                    gain,
                    temperature,
                    saturation,
                    contrast,
                } => {
                    color_grade(
                        &mut image,
                        *lift,
                        *gamma,
                        *gain,
                        *temperature,
                        *saturation,
                        *contrast,
                    );
                }
                ClipEffect::Transform {
                    scale,
                    offset,
                    rotation_deg,
                } => {
                    image = transform_image(&image, *scale, *offset, *rotation_deg);
                }
                ClipEffect::Crop { rect } => {
                    crop_alpha(&mut image, *rect);
                }
                ClipEffect::Opacity { value } => {
                    let v = value.clamp(0.0, 1.0);
                    for px in image.data.chunks_exact_mut(4) {
                        px[3] = (px[3] as f32 * v).round() as u8;
                    }
                }
                ClipEffect::Speed { .. } | ClipEffect::Transition { .. } => {}
            }
        }
        Some(image)
    }

    fn load_cached(&mut self, path: &Path) -> Option<RasterImage> {
        let key = path.to_string_lossy().to_string();
        if let Some(cached) = self.frame_cache.get(&key) {
            return Some((**cached).clone());
        }
        match roedit_io::load_image(path) {
            Ok(img) => {
                self.frame_cache.insert(key, Arc::new(img.clone()));
                if self.frame_cache.len() > 240 {
                    // Keep memory bounded during long exports.
                    let first = self.frame_cache.keys().next().cloned();
                    if let Some(k) = first {
                        self.frame_cache.remove(&k);
                    }
                }
                Some(img)
            }
            Err(err) => {
                log::warn!("could not load timeline frame {}: {err}", path.display());
                None
            }
        }
    }
}

fn sequence_path(directory: &str, pattern: &str, index: u32) -> PathBuf {
    let dir = Path::new(directory);
    if pattern.contains("%0") || pattern.contains("{frame}") {
        let name = pattern
            .replace("{frame}", &format!("{index:04}"))
            .replace("%04d", &format!("{index:04}"))
            .replace("%d", &index.to_string());
        dir.join(name)
    } else if pattern.is_empty() {
        dir.join(format!("{index:04}.png"))
    } else {
        dir.join(pattern.replace("{index}", &format!("{index:04}")))
    }
}

fn with_alpha(img: &RasterImage, opacity: f32) -> RasterImage {
    let mut out = img.clone();
    if opacity < 0.999 {
        for px in out.data.chunks_exact_mut(4) {
            px[3] = (px[3] as f32 * opacity.clamp(0.0, 1.0)).round() as u8;
        }
    }
    out
}

/// Alpha-composites `layer` over `canvas` with an extra opacity multiplier.
pub fn composite_over(canvas: &mut RasterImage, layer: &RasterImage, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let lx = x.min(layer.width.saturating_sub(1));
            let ly = y.min(layer.height.saturating_sub(1));
            let src = layer.get(lx, ly);
            let sa = (src[3] as f32 / 255.0) * opacity;
            if sa <= 0.001 {
                continue;
            }
            let dst = canvas.get(x, y);
            let da = dst[3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 1e-6 {
                canvas.set(x, y, [0, 0, 0, 0]);
                continue;
            }
            let mut px = [0u8; 4];
            for c in 0..3 {
                let v = (src[c] as f32 * sa + dst[c] as f32 * da * (1.0 - sa)) / out_a;
                px[c] = v.round().clamp(0.0, 255.0) as u8;
            }
            px[3] = (out_a * 255.0).round() as u8;
            canvas.set(x, y, px);
        }
    }
}

fn resize_bilinear(img: &RasterImage, width: u32, height: u32) -> RasterImage {
    let mut out = RasterImage::new(width, height);
    if img.width == 0 || img.height == 0 {
        return out;
    }
    let sx = img.width as f32 / width as f32;
    let sy = img.height as f32 / height as f32;
    for y in 0..height {
        for x in 0..width {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, img.width as f32 - 1.0);
            let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, img.height as f32 - 1.0);
            let x0 = fx.floor() as u32;
            let y0 = fy.floor() as u32;
            let x1 = (x0 + 1).min(img.width - 1);
            let y1 = (y0 + 1).min(img.height - 1);
            let tx = fx - x0 as f32;
            let ty = fy - y0 as f32;
            let mut px = [0u8; 4];
            for c in 0..4 {
                let a = img.get(x0, y0)[c] as f32;
                let b = img.get(x1, y0)[c] as f32;
                let cc = img.get(x0, y1)[c] as f32;
                let d = img.get(x1, y1)[c] as f32;
                let top = a + (b - a) * tx;
                let bottom = cc + (d - cc) * tx;
                px[c] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
            }
            out.set(x, y, px);
        }
    }
    out
}

fn transform_image(
    img: &RasterImage,
    scale: f32,
    offset: glam::Vec2,
    rotation_deg: f32,
) -> RasterImage {
    let s = scale.clamp(0.01, 100.0);
    let angle = -rotation_deg.to_radians();
    let (sin, cos) = angle.sin_cos();
    let inv = 1.0 / s;
    let cx = img.width as f32 * 0.5;
    let cy = img.height as f32 * 0.5;
    let ox = offset.x * img.width as f32;
    let oy = offset.y * img.height as f32;
    let mut out = RasterImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let dx = x as f32 + 0.5 - cx - ox;
            let dy = y as f32 + 0.5 - cy - oy;
            let rx = (dx * cos - dy * sin) * inv + cx;
            let ry = (dx * sin + dy * cos) * inv + cy;
            if rx < 0.0 || ry < 0.0 || rx >= img.width as f32 || ry >= img.height as f32 {
                continue;
            }
            let px = img.get(rx as u32, ry as u32);
            out.set(x, y, px);
        }
    }
    out
}

fn crop_alpha(img: &mut RasterImage, rect: [f32; 4]) {
    let x0 = rect[0].clamp(0.0, 1.0);
    let y0 = rect[1].clamp(0.0, 1.0);
    let x1 = rect[2].clamp(0.0, 1.0);
    let y1 = rect[3].clamp(0.0, 1.0);
    let (pw, ph) = (img.width.max(1), img.height.max(1));
    for y in 0..img.height {
        for x in 0..img.width {
            let nx = (x as f32 + 0.5) / pw as f32;
            let ny = (y as f32 + 0.5) / ph as f32;
            if nx < x0 || nx > x1 || ny < y0 || ny > y1 {
                let mut px = img.get(x, y);
                px[3] = 0;
                img.set(x, y, px);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn color_grade(
    img: &mut RasterImage,
    lift: [f32; 3],
    gamma: [f32; 3],
    gain: [f32; 3],
    temperature: f32,
    saturation: f32,
    contrast: f32,
) {
    let temp = temperature.clamp(-1.0, 1.0);
    for px in img.data.chunks_exact_mut(4) {
        let mut c = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ];
        for (i, channel) in c.iter_mut().enumerate() {
            let g = gamma[i].max(0.05);
            let v = (*channel + lift[i]).max(0.0) * gain[i];
            *channel = v.max(0.0).powf(1.0 / g);
        }
        // Temperature: warm pushes red, cool pushes blue.
        c[0] = (c[0] + temp * 0.12).clamp(0.0, 1.0);
        c[2] = (c[2] - temp * 0.12).clamp(0.0, 1.0);
        let luma = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        for channel in &mut c {
            *channel = (luma + (*channel - luma) * saturation.max(0.0)).clamp(0.0, 1.0);
            *channel = ((*channel - 0.5) * contrast.max(0.0) + 0.5).clamp(0.0, 1.0);
        }
        px[0] = (c[0] * 255.0).round() as u8;
        px[1] = (c[1] * 255.0).round() as u8;
        px[2] = (c[2] * 255.0).round() as u8;
    }
}

/// Easing helper exposed for the app's transition controls.
pub fn eased(ease: Ease, t: f32) -> f32 {
    ease.apply(t.clamp(0.0, 1.0))
}

/// Convenience used by the UI to list transition kinds.
pub fn transition_kinds() -> [TransitionKind; 7] {
    TransitionKind::ALL
}

/// Text cards use the image library's text renderer.
pub fn text_card(spec: &TextSpec, width: u32, height: u32) -> RasterImage {
    render_text(spec, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::ids::VideoTrackId;
    use roedit_core::timeline::{TrackKind, VideoTrack};

    fn timeline_with_tracks(kinds: &[TrackKind]) -> (Timeline, Vec<VideoTrackId>) {
        let mut tl = Timeline::new(64, 36, 30.0, 10.0);
        let mut ids = Vec::new();
        for (i, kind) in kinds.iter().enumerate() {
            let id = VideoTrackId(i as u32);
            tl.tracks
                .push(VideoTrack::new(id, format!("Track {i}"), *kind));
            ids.push(id);
        }
        (tl, ids)
    }

    fn solid_clip(color: [f32; 3], start: f32, duration: f32) -> ClipSource {
        let _ = (start, duration);
        ClipSource::Color(color)
    }

    #[test]
    fn top_track_wins() {
        let (mut tl, tracks) = timeline_with_tracks(&[TrackKind::Image, TrackKind::Image]);
        tl.add_clip(tracks[0], solid_clip([1.0, 0.0, 0.0], 0.0, 2.0), 0.0, 2.0);
        tl.add_clip(tracks[1], solid_clip([0.0, 1.0, 0.0], 0.0, 2.0), 0.0, 2.0);
        let mut comp = Compositor::new(tl);
        let frame = comp.frame_at(1.0);
        let px = frame.get(10, 10);
        assert!(px[1] > 240 && px[0] < 10, "{px:?}");
    }

    #[test]
    fn fade_in_darkens_the_start() {
        let (mut tl, tracks) = timeline_with_tracks(&[TrackKind::Image]);
        let id = tl
            .add_clip(tracks[0], solid_clip([1.0, 1.0, 1.0], 0.0, 2.0), 0.0, 2.0)
            .unwrap();
        if let Some(clip) = tl.clip_mut(id) {
            clip.fade_in = 1.0;
        }
        let mut comp = Compositor::new(tl);
        let early = comp.frame_at(0.1).get(10, 10);
        let late = comp.frame_at(1.8).get(10, 10);
        assert!(late[0] > early[0], "fade in: {early:?} -> {late:?}");
    }

    #[test]
    fn opacity_effect_blends_with_background() {
        let (mut tl, tracks) = timeline_with_tracks(&[TrackKind::Image]);
        let mut comp = Compositor::new(tl.clone());
        comp.background = [0, 0, 0, 255];
        let id = tl
            .add_clip(tracks[0], solid_clip([1.0, 1.0, 1.0], 0.0, 2.0), 0.0, 2.0)
            .unwrap();
        if let Some(clip) = tl.clip_mut(id) {
            clip.effects.push(ClipEffect::Opacity { value: 0.5 });
        }
        let mut comp = Compositor::new(tl);
        comp.background = [0, 0, 0, 255];
        let px = comp.frame_at(1.0).get(8, 8);
        assert!((px[0] as i32 - 128).abs() < 6, "{px:?}");
    }

    #[test]
    fn color_grade_and_blur_run() {
        let (mut tl, tracks) = timeline_with_tracks(&[TrackKind::Image]);
        let id = tl
            .add_clip(tracks[0], solid_clip([0.5, 0.5, 0.5], 0.0, 2.0), 0.0, 2.0)
            .unwrap();
        if let Some(clip) = tl.clip_mut(id) {
            clip.effects.push(ClipEffect::ColorGrade {
                lift: [0.1, 0.0, 0.0],
                gamma: [1.0, 1.0, 1.0],
                gain: [1.0, 1.0, 1.0],
                temperature: 0.5,
                saturation: 1.4,
                contrast: 1.1,
            });
            clip.effects.push(ClipEffect::GaussianBlur { radius: 1.0 });
        }
        let mut comp = Compositor::new(tl);
        let px = comp.frame_at(1.0).get(8, 8);
        assert!(px[0] > px[2], "warm grade should raise red: {px:?}");
    }

    #[test]
    fn chroma_key_removes_green() {
        let (mut tl, tracks) = timeline_with_tracks(&[TrackKind::Image]);
        let id = tl
            .add_clip(tracks[0], solid_clip([0.0, 1.0, 0.0], 0.0, 2.0), 0.0, 2.0)
            .unwrap();
        if let Some(clip) = tl.clip_mut(id) {
            clip.effects.push(ClipEffect::ChromaKey {
                key_color: [0.0, 1.0, 0.0],
                tolerance: 0.2,
                softness: 0.05,
                spill_suppression: 0.5,
            });
        }
        let mut comp = Compositor::new(tl);
        comp.background = [0, 0, 0, 255];
        let px = comp.frame_at(1.0).get(8, 8);
        assert!(px[1] < 20, "green should be keyed out: {px:?}");
    }

    #[test]
    fn transition_at_clip_start_blends() {
        let (mut tl, tracks) = timeline_with_tracks(&[TrackKind::Image, TrackKind::Image]);
        tl.add_clip(tracks[0], solid_clip([0.0, 0.0, 1.0], 0.0, 4.0), 0.0, 4.0);
        let id = tl
            .add_clip(tracks[1], solid_clip([1.0, 1.0, 1.0], 0.0, 4.0), 1.0, 3.0)
            .unwrap();
        if let Some(clip) = tl.clip_mut(id) {
            clip.effects.push(ClipEffect::Transition {
                kind: TransitionKind::CrossDissolve,
                duration: 1.0,
                ease: Ease::Linear,
            });
        }
        let mut comp = Compositor::new(tl);
        let mid = comp.frame_at(1.5).get(8, 8);
        let after = comp.frame_at(3.0).get(8, 8);
        assert!(mid[0] > 0 && mid[0] < 250, "transition should mix: {mid:?}");
        assert!(
            after[0] > 240,
            "after the transition the white clip shows: {after:?}"
        );
    }

    #[test]
    fn audio_clips_do_not_render_pixels() {
        let (mut tl, tracks) = timeline_with_tracks(&[TrackKind::Audio]);
        tl.add_clip(
            tracks[0],
            ClipSource::AudioFile("missing.wav".into()),
            0.0,
            2.0,
        );
        let mut comp = Compositor::new(tl);
        let frame = comp.frame_at(0.5);
        let px = frame.get(1, 1);
        assert_eq!(px[3], 255);
        assert_eq!(px[0], 0);
    }

    #[test]
    fn empty_timeline_returns_background() {
        let tl = Timeline::new(8, 8, 30.0, 1.0);
        let mut comp = Compositor::new(tl);
        comp.background = [10, 20, 30, 255];
        let frame = comp.frame_at(0.0);
        assert_eq!(frame.get(0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn transform_scales_a_layer() {
        let mut img = RasterImage::new(32, 32);
        for y in 10..22 {
            for x in 10..22 {
                img.set(x, y, [255, 255, 255, 255]);
            }
        }
        let small = transform_image(&img, 2.0, glam::Vec2::ZERO, 0.0);
        // Scaling up 2x should grow the opaque block.
        let opaque = small.data.chunks_exact(4).filter(|p| p[3] > 0).count();
        let original = img.data.chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(opaque > original, "{opaque} vs {original}");
    }

    #[test]
    fn resize_matches_requested_size() {
        let img = RasterImage::filled(10, 5, [255, 0, 0, 255]);
        let out = resize_bilinear(&img, 40, 20);
        assert_eq!((out.width, out.height), (40, 20));
        assert_eq!(out.get(39, 19), [255, 0, 0, 255]);
    }
}
