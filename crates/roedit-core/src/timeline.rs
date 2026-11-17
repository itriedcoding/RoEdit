//! Multitrack video timeline model and non-destructive edit operations.
//! Rendering of frames lives in `roedit-video`; this module owns the data and
//! the trimming/splitting/ripple logic.

use crate::animation::Ease;
use crate::gfx::TextSpec;
use crate::ids::{ClipId, Counter, ImageId, VideoTrackId};
use crate::math::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    Image,
    Text,
    Audio,
    Adjustment,
}

impl TrackKind {
    pub const ALL: [TrackKind; 5] = [
        TrackKind::Video,
        TrackKind::Image,
        TrackKind::Text,
        TrackKind::Audio,
        TrackKind::Adjustment,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TrackKind::Video => "Video",
            TrackKind::Image => "Image",
            TrackKind::Text => "Text",
            TrackKind::Audio => "Audio",
            TrackKind::Adjustment => "Adjustment",
        }
    }

    pub fn is_audio(self) -> bool {
        self == TrackKind::Audio
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoTrack {
    pub id: VideoTrackId,
    pub name: String,
    pub kind: TrackKind,
    pub enabled: bool,
    pub muted: bool,
    pub locked: bool,
    pub height: f32,
}

impl VideoTrack {
    pub fn new(id: VideoTrackId, name: impl Into<String>, kind: TrackKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            enabled: true,
            muted: false,
            locked: false,
            height: if kind.is_audio() { 56.0 } else { 72.0 },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClipSource {
    /// A still image from the project image library.
    Image(ImageId),
    /// A numbered image sequence loaded from disk (frames are read lazily).
    ImageSequence {
        directory: String,
        pattern: String,
        frame_count: u32,
        fps: f32,
    },
    /// A flat colour matte, useful for backgrounds and transitions.
    Color([f32; 3]),
    /// A text card.
    Text(TextSpec),
    /// A PCM WAV file on disk, decoded when the timeline is composited.
    AudioFile(String),
    /// A rendered sequence produced by this application (renders folder).
    RenderOutput(String),
}

impl ClipSource {
    pub fn label(&self) -> &'static str {
        match self {
            ClipSource::Image(_) => "Image",
            ClipSource::ImageSequence { .. } => "Image Sequence",
            ClipSource::Color(_) => "Solid Colour",
            ClipSource::Text(_) => "Text",
            ClipSource::AudioFile(_) => "Audio (WAV)",
            ClipSource::RenderOutput(_) => "Render Output",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    CrossDissolve,
    DipToBlack,
    WipeLeft,
    WipeRight,
    WipeUp,
    WipeDown,
    Circle,
}

impl TransitionKind {
    pub const ALL: [TransitionKind; 7] = [
        TransitionKind::CrossDissolve,
        TransitionKind::DipToBlack,
        TransitionKind::WipeLeft,
        TransitionKind::WipeRight,
        TransitionKind::WipeUp,
        TransitionKind::WipeDown,
        TransitionKind::Circle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TransitionKind::CrossDissolve => "Cross Dissolve",
            TransitionKind::DipToBlack => "Dip to Black",
            TransitionKind::WipeLeft => "Wipe Left",
            TransitionKind::WipeRight => "Wipe Right",
            TransitionKind::WipeUp => "Wipe Up",
            TransitionKind::WipeDown => "Wipe Down",
            TransitionKind::Circle => "Circle Wipe",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipEffect {
    /// Lift/gamma/gain grading with temperature and saturation.
    ColorGrade {
        lift: [f32; 3],
        gamma: [f32; 3],
        gain: [f32; 3],
        temperature: f32,
        saturation: f32,
        contrast: f32,
    },
    ChromaKey {
        key_color: [f32; 3],
        tolerance: f32,
        softness: f32,
        spill_suppression: f32,
    },
    Transform {
        scale: f32,
        offset: Vec2,
        rotation_deg: f32,
    },
    GaussianBlur {
        radius: f32,
    },
    /// Timing-based effect; speed is applied by the compositor.
    Speed {
        factor: f32,
        reverse: bool,
    },
    /// Transition at the start of the clip, blending with what is underneath.
    Transition {
        kind: TransitionKind,
        duration: f32,
        ease: Ease,
    },
    Opacity {
        value: f32,
    },
    Crop {
        rect: [f32; 4],
    },
}

impl ClipEffect {
    pub fn label(&self) -> &'static str {
        match self {
            ClipEffect::ColorGrade { .. } => "Colour Grade",
            ClipEffect::ChromaKey { .. } => "Chroma Key",
            ClipEffect::Transform { .. } => "Transform",
            ClipEffect::GaussianBlur { .. } => "Gaussian Blur",
            ClipEffect::Speed { .. } => "Speed",
            ClipEffect::Transition { .. } => "Transition",
            ClipEffect::Opacity { .. } => "Opacity",
            ClipEffect::Crop { .. } => "Crop",
        }
    }

    pub fn defaults() -> Vec<ClipEffect> {
        vec![
            ClipEffect::ColorGrade {
                lift: [0.0, 0.0, 0.0],
                gamma: [1.0, 1.0, 1.0],
                gain: [1.0, 1.0, 1.0],
                temperature: 0.0,
                saturation: 1.0,
                contrast: 1.0,
            },
            ClipEffect::ChromaKey {
                key_color: [0.0, 1.0, 0.0],
                tolerance: 0.25,
                softness: 0.1,
                spill_suppression: 0.5,
            },
            ClipEffect::Transform {
                scale: 1.0,
                offset: Vec2::ZERO,
                rotation_deg: 0.0,
            },
            ClipEffect::GaussianBlur { radius: 4.0 },
            ClipEffect::Speed {
                factor: 1.0,
                reverse: false,
            },
            ClipEffect::Transition {
                kind: TransitionKind::CrossDissolve,
                duration: 0.5,
                ease: Ease::InOut,
            },
            ClipEffect::Opacity { value: 1.0 },
            ClipEffect::Crop {
                rect: [0.0, 0.0, 1.0, 1.0],
            },
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub track: VideoTrackId,
    pub name: String,
    pub start: f32,
    pub duration: f32,
    pub source: ClipSource,
    pub effects: Vec<ClipEffect>,
    pub enabled: bool,
    pub locked: bool,
    /// Trim offset into the source, in seconds.
    pub source_in: f32,
    pub opacity: f32,
    pub volume: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub color: [u8; 3],
}

impl Clip {
    pub fn end(&self) -> f32 {
        self.start + self.duration
    }

    pub fn contains(&self, time: f32) -> bool {
        self.enabled && time >= self.start - 1e-4 && time < self.end() - 1e-4
    }

    pub fn overlap(&self, a: f32, b: f32) -> bool {
        self.start < b && self.end() > a
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeline {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub duration: f32,
    pub playhead: f32,
    pub tracks: Vec<VideoTrack>,
    pub clips: Vec<Clip>,
    pub markers: Vec<crate::animation::Marker>,
    next_clip: Counter,
    next_track: Counter,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new(1920, 1080, 30.0, 20.0)
    }
}

impl Timeline {
    pub fn new(width: u32, height: u32, fps: f32, duration: f32) -> Self {
        Self {
            name: "Edit".into(),
            width: width.max(1),
            height: height.max(1),
            fps: fps.max(1.0),
            duration: duration.max(0.1),
            playhead: 0.0,
            tracks: Vec::new(),
            clips: Vec::new(),
            markers: Vec::new(),
            next_clip: Counter::default(),
            next_track: Counter::default(),
        }
    }

    pub fn add_track(&mut self, name: impl Into<String>, kind: TrackKind) -> VideoTrackId {
        let id = VideoTrackId(self.next_track.next() as u32);
        let n = self.tracks.iter().filter(|t| t.kind == kind).count() + 1;
        self.tracks
            .push(VideoTrack::new(id, format!("{} {}", kind.label(), n), kind));
        let _ = name;
        id
    }

    pub fn track(&self, id: VideoTrackId) -> Option<&VideoTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: VideoTrackId) -> Option<&mut VideoTrack> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn add_clip(
        &mut self,
        track: VideoTrackId,
        source: ClipSource,
        start: f32,
        duration: f32,
    ) -> Option<ClipId> {
        self.track(track)?;
        let id = ClipId(self.next_clip.next() as u32);
        let name = source.label().to_string();
        let color = match self.track(track).map(|t| t.kind) {
            Some(TrackKind::Audio) => [90, 180, 120],
            Some(TrackKind::Video) => [110, 150, 230],
            Some(TrackKind::Text) => [230, 180, 90],
            Some(TrackKind::Image) => [160, 130, 220],
            _ => [140, 140, 150],
        };
        self.clips.push(Clip {
            id,
            track,
            name,
            start: start.max(0.0),
            duration: duration.max(0.05),
            source,
            effects: Vec::new(),
            enabled: true,
            locked: false,
            source_in: 0.0,
            opacity: 1.0,
            volume: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            color,
        });
        self.sort_clips();
        self.clamp_duration();
        Some(id)
    }

    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }

    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }

    pub fn remove_clip(&mut self, id: ClipId) -> bool {
        let before = self.clips.len();
        self.clips.retain(|c| c.id != id);
        self.clips.len() != before
    }

    pub fn sort_clips(&mut self) {
        self.clips.sort_by(|a, b| {
            a.track
                .0
                .cmp(&b.track.0)
                .then(a.start.total_cmp(&b.start))
                .then(a.id.0.cmp(&b.id.0))
        });
    }

    /// Splits the clip at an absolute timeline time. Both halves keep effects.
    pub fn split_clip(&mut self, id: ClipId, time: f32) -> Option<(ClipId, ClipId)> {
        let clip = self.clip(id)?.clone();
        if time <= clip.start + 0.01 || time >= clip.end() - 0.01 {
            return None;
        }
        let left_dur = time - clip.start;
        let right_dur = clip.end() - time;
        let right_id = ClipId(self.next_clip.next() as u32);
        {
            let left = self.clip_mut(id)?;
            left.duration = left_dur;
        }
        let mut right = clip.clone();
        right.id = right_id;
        right.start = time;
        right.duration = right_dur;
        right.source_in = clip.source_in + left_dur;
        right.name = format!("{} B", clip.name.trim_end_matches(" B"));
        self.clips.push(right);
        self.sort_clips();
        Some((id, right_id))
    }

    pub fn trim_clip(&mut self, id: ClipId, new_start: f32, new_duration: f32) -> bool {
        let Some(c) = self.clip_mut(id) else {
            return false;
        };
        c.start = new_start.max(0.0);
        c.duration = new_duration.max(0.05);
        self.sort_clips();
        self.clamp_duration();
        true
    }

    /// Deletes a clip and closes the gap on its track.
    pub fn ripple_delete(&mut self, id: ClipId) -> bool {
        let Some(clip) = self.clip(id).cloned() else {
            return false;
        };
        self.remove_clip(id);
        for c in &mut self.clips {
            if c.track == clip.track && c.start >= clip.end() - 1e-4 {
                c.start = (c.start - clip.duration).max(0.0);
            }
        }
        self.sort_clips();
        true
    }

    pub fn move_clip(
        &mut self,
        id: ClipId,
        new_track: Option<VideoTrackId>,
        new_start: f32,
    ) -> bool {
        if let Some(t) = new_track {
            if self.tracks.iter().all(|tr| tr.id != t) {
                return false;
            }
        }
        let Some(c) = self.clip_mut(id) else {
            return false;
        };
        if let Some(t) = new_track {
            c.track = t;
        }
        c.start = new_start.max(0.0);
        self.sort_clips();
        self.clamp_duration();
        true
    }

    pub fn clips_on_track(&self, track: VideoTrackId) -> Vec<&Clip> {
        self.clips.iter().filter(|c| c.track == track).collect()
    }

    pub fn clip_at(&self, track: VideoTrackId, time: f32) -> Option<&Clip> {
        self.clips
            .iter()
            .find(|c| c.track == track && c.contains(time))
    }

    /// Visible clips at `time`, bottom track first (video compositing order).
    pub fn active_clips(&self, time: f32) -> Vec<&Clip> {
        let mut out: Vec<&Clip> = self
            .clips
            .iter()
            .filter(|c| c.contains(time))
            .filter(|c| {
                self.track(c.track)
                    .map(|t| t.enabled && !t.muted)
                    .unwrap_or(false)
            })
            .collect();
        out.sort_by_key(|c| c.track.0);
        out
    }

    pub fn active_audio_clips(&self, time: f32) -> Vec<&Clip> {
        self.active_clips(time)
            .into_iter()
            .filter(|c| {
                self.track(c.track)
                    .map(|t| t.kind.is_audio())
                    .unwrap_or(false)
            })
            .collect()
    }

    pub fn content_duration(&self) -> f32 {
        self.clips
            .iter()
            .map(|c| c.end())
            .fold(0.0_f32, f32::max)
            .max(0.1)
    }

    pub fn clamp_duration(&mut self) {
        self.duration = self.duration.max(self.content_duration());
    }

    pub fn add_marker(&mut self, time: f32, name: impl Into<String>) {
        self.markers
            .push(crate::animation::Marker::new(time.max(0.0), name));
        self.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
    }

    pub fn time_to_frame(&self, t: f32) -> u32 {
        (t.max(0.0) * self.fps).round() as u32
    }

    pub fn frame_to_time(&self, f: u32) -> f32 {
        f as f32 / self.fps
    }

    pub fn frame_count(&self) -> u32 {
        (self.duration * self.fps).ceil().max(1.0) as u32
    }

    /// Applies keyframed clip parameters from an animation at `time`.
    pub fn apply_animation(&mut self, anim: &crate::animation::Animation, time: f32) -> usize {
        use crate::animation::Target;
        let mut applied = 0;
        for track in &anim.tracks {
            if !track.enabled || track.muted {
                continue;
            }
            if let (Target::ClipOpacity(id), Some(v)) = (&track.target, track.sample(time)) {
                if let Some(c) = self.clip_mut(*id) {
                    c.opacity = v.as_scalar().clamp(0.0, 1.0);
                    applied += 1;
                }
            }
            if let (Target::ClipVolume(id), Some(v)) = (&track.target, track.sample(time)) {
                if let Some(c) = self.clip_mut(*id) {
                    c.volume = v.as_scalar().clamp(0.0, 4.0);
                    applied += 1;
                }
            }
            if let (Target::ClipSpeed(id), Some(v)) = (&track.target, track.sample(time)) {
                if let Some(c) = self.clip_mut(*id) {
                    for e in &mut c.effects {
                        if let ClipEffect::Speed { factor, .. } = e {
                            *factor = v.as_scalar().clamp(0.05, 8.0);
                            applied += 1;
                        }
                    }
                }
            }
        }
        applied
    }

    pub fn repair_counters(&mut self) {
        let c = self
            .clips
            .iter()
            .map(|c| c.id.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_clip.bump_to(c.saturating_sub(1));
        let t = self
            .tracks
            .iter()
            .map(|t| t.id.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_track.bump_to(t.saturating_sub(1));
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for c in &self.clips {
            if self.track(c.track).is_none() {
                problems.push(format!("Clip '{}' references a missing track", c.name));
            }
            if c.duration <= 0.0 {
                problems.push(format!("Clip '{}' has a non-positive duration", c.name));
            }
            if c.start < 0.0 {
                problems.push(format!("Clip '{}' starts before zero", c.name));
            }
            if c.volume < 0.0 || c.opacity < 0.0 {
                problems.push(format!("Clip '{}' has a negative volume/opacity", c.name));
            }
        }
        if self.fps <= 0.0 {
            problems.push("Timeline FPS must be positive".into());
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline() -> (Timeline, VideoTrackId) {
        let mut tl = Timeline::new(1920, 1080, 30.0, 10.0);
        let track = tl.add_track("Video 1", TrackKind::Video);
        (tl, track)
    }

    #[test]
    fn add_split_and_trim() {
        let (mut tl, track) = timeline();
        let id = tl
            .add_clip(track, ClipSource::Color([1.0, 0.0, 0.0]), 1.0, 4.0)
            .unwrap();
        let (left, right) = tl.split_clip(id, 3.0).unwrap();
        assert!((tl.clip(left).unwrap().duration - 2.0).abs() < 1e-4);
        assert!((tl.clip(right).unwrap().start - 3.0).abs() < 1e-4);
        assert!((tl.clip(right).unwrap().source_in - 2.0).abs() < 1e-4);
        assert!(tl.split_clip(right, 3.0).is_none());
        assert!(tl.trim_clip(left, 0.5, 1.5));
        assert!((tl.clip(left).unwrap().end() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn ripple_delete_closes_gap() {
        let (mut tl, track) = timeline();
        let a = tl
            .add_clip(track, ClipSource::Color([0.0; 3]), 0.0, 2.0)
            .unwrap();
        let b = tl
            .add_clip(track, ClipSource::Color([1.0; 3]), 2.0, 3.0)
            .unwrap();
        assert!(tl.ripple_delete(a));
        assert!((tl.clip(b).unwrap().start - 0.0).abs() < 1e-4);
    }

    #[test]
    fn active_clips_respects_time_and_mute() {
        let (mut tl, track) = timeline();
        tl.add_clip(track, ClipSource::Color([0.0; 3]), 0.0, 2.0)
            .unwrap();
        assert_eq!(tl.active_clips(1.0).len(), 1);
        assert_eq!(tl.active_clips(2.5).len(), 0);
        tl.track_mut(track).unwrap().muted = true;
        assert_eq!(tl.active_clips(1.0).len(), 0);
    }

    #[test]
    fn clip_keyframes_apply() {
        let (mut tl, track) = timeline();
        let id = tl
            .add_clip(track, ClipSource::Color([0.0; 3]), 0.0, 3.0)
            .unwrap();
        let mut anim = crate::animation::Animation::default();
        let ti = anim.add_track(crate::animation::Target::ClipOpacity(id));
        anim.tracks[ti].insert_key(0.0, crate::animation::AnimValue::Scalar(1.0));
        anim.tracks[ti].insert_key(1.0, crate::animation::AnimValue::Scalar(0.0));
        assert_eq!(tl.apply_animation(&anim, 0.5), 1);
        assert!((tl.clip(id).unwrap().opacity - 0.5).abs() < 1e-3);
    }

    #[test]
    fn frame_math() {
        let (mut tl, _) = timeline();
        tl.duration = 10.0;
        assert_eq!(tl.time_to_frame(1.0), 30);
        assert!((tl.frame_to_time(30) - 1.0).abs() < 1e-5);
        assert_eq!(tl.frame_count(), 300);
    }

    #[test]
    fn validate_flags_bad_clip() {
        let (mut tl, track) = timeline();
        let id = tl
            .add_clip(track, ClipSource::Color([0.0; 3]), 1.0, 1.0)
            .unwrap();
        tl.clip_mut(id).unwrap().duration = -1.0;
        assert!(!tl.validate().is_empty());
    }
}
