//! Timeline audio mixing: places WAV clips at their start times with volume,
//! fades and trim, producing one 48 kHz stereo mix.

use crate::VideoError;
use roedit_core::timeline::Timeline;
use roedit_io::AudioBuffer;
use std::collections::HashMap;

pub const MIX_SAMPLE_RATE: u32 = 48_000;
pub const MIX_CHANNELS: u16 = 2;

#[derive(Clone, Debug)]
pub struct AudioMix {
    pub buffer: AudioBuffer,
    /// Clips that contributed at least one sample.
    pub clips: usize,
    /// Clips that could not be loaded (missing files).
    pub missing: Vec<String>,
}

impl AudioMix {
    pub fn duration_secs(&self) -> f32 {
        self.buffer.duration_secs()
    }

    pub fn peak(&self) -> f32 {
        self.buffer.peak()
    }
}

/// Mixes all enabled audio clips into a single stereo buffer.
pub fn mix_timeline_audio(
    timeline: &Timeline,
    cache: &mut HashMap<String, AudioBuffer>,
) -> AudioMix {
    let duration = timeline.content_duration().max(0.1);
    let mut out = AudioBuffer::new(MIX_SAMPLE_RATE, MIX_CHANNELS);
    let frames = (duration * MIX_SAMPLE_RATE as f32).ceil() as usize;
    out.samples = vec![0.0; frames * MIX_CHANNELS as usize];
    let mut clips = 0usize;
    let mut missing = Vec::new();

    for track in &timeline.tracks {
        if !track.kind.is_audio() || !track.enabled || track.muted {
            continue;
        }
        for clip in timeline.clips_on_track(track.id) {
            if !clip.enabled {
                continue;
            }
            let src = match &clip.source {
                roedit_core::timeline::ClipSource::AudioFile(path) => path,
                _ => continue,
            };
            let buffer = match cache.entry(src.clone()) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut().clone(),
                std::collections::hash_map::Entry::Vacant(e) => match std::fs::read(e.key()) {
                    Ok(bytes) => match roedit_io::read_wav(&bytes) {
                        Ok(b) => e.insert(b).clone(),
                        Err(err) => {
                            log::warn!("audio clip {src} could not be decoded: {err}");
                            missing.push(src.clone());
                            continue;
                        }
                    },
                    Err(err) => {
                        log::warn!("audio clip {src} is missing: {err}");
                        missing.push(src.clone());
                        continue;
                    }
                },
            };
            if buffer.samples.is_empty() {
                continue;
            }
            let buffer = if buffer.sample_rate != MIX_SAMPLE_RATE {
                buffer.resample(MIX_SAMPLE_RATE)
            } else {
                buffer
            };
            mix_clip(&mut out, &buffer, clip);
            clips += 1;
        }
    }
    AudioMix {
        buffer: out,
        clips,
        missing,
    }
}

fn mix_clip(out: &mut AudioBuffer, src: &AudioBuffer, clip: &roedit_core::timeline::Clip) {
    let ch = MIX_CHANNELS as usize;
    let src_ch = src.channels.max(1) as usize;
    let clip_frames = (clip.duration * MIX_SAMPLE_RATE as f32).ceil() as usize;
    let start_frame = (clip.start.max(0.0) * MIX_SAMPLE_RATE as f32) as usize;
    let source_offset = (clip.source_in.max(0.0) * MIX_SAMPLE_RATE as f32) as usize;
    let volume = clip.volume.clamp(0.0, 4.0);
    for f in 0..clip_frames {
        let t = f as f32 / MIX_SAMPLE_RATE as f32;
        let mut gain = volume;
        if clip.fade_in > 0.0 && t < clip.fade_in {
            gain *= t / clip.fade_in;
        }
        if clip.fade_out > 0.0 && t > clip.duration - clip.fade_out {
            gain *= ((clip.duration - t) / clip.fade_out).clamp(0.0, 1.0);
        }
        if gain <= 0.0 {
            continue;
        }
        let src_frame = source_offset + f;
        if src_frame >= src.frame_count() {
            break;
        }
        let dst_frame = start_frame + f;
        for c in 0..ch {
            let s = src
                .samples
                .get(src_frame * src_ch + c.min(src_ch - 1))
                .copied()
                .unwrap_or(0.0);
            if let Some(slot) = out.samples.get_mut(dst_frame * ch + c) {
                *slot = (*slot + s * gain).clamp(-1.0, 1.0);
            }
        }
    }
}

/// Utility used by tests and the UI: builds a short sine buffer.
pub fn tone(freq: f32, seconds: f32, sample_rate: u32, amplitude: f32) -> AudioBuffer {
    let frames = (seconds * sample_rate as f32).max(1.0) as usize;
    let mut buf = AudioBuffer::new(sample_rate, MIX_CHANNELS);
    for i in 0..frames {
        let t = i as f32 / sample_rate as f32;
        let v = (t * freq * std::f32::consts::TAU).sin() * amplitude;
        buf.samples.push(v);
        buf.samples.push(v);
    }
    buf
}

/// Ensures an audio file is readable before it is added to the timeline.
pub fn probe_audio(path: &str) -> Result<f32, VideoError> {
    let bytes = std::fs::read(path)?;
    let buffer = roedit_io::read_wav(&bytes)?;
    Ok(buffer.duration_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::ids::VideoTrackId;
    use roedit_core::timeline::{ClipSource, TrackKind, VideoTrack};

    fn setup_timeline(paths: &[(&str, f32, f32)]) -> Timeline {
        let mut tl = Timeline::new(16, 16, 30.0, 4.0);
        tl.tracks
            .push(VideoTrack::new(VideoTrackId(0), "Audio", TrackKind::Audio));
        for (path, start, duration) in paths {
            let id = tl
                .add_clip(
                    VideoTrackId(0),
                    ClipSource::AudioFile((*path).to_string()),
                    *start,
                    *duration,
                )
                .unwrap();
            if let Some(clip) = tl.clip_mut(id) {
                clip.volume = 0.8;
            }
        }
        tl
    }

    #[test]
    fn mixes_two_clips_at_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        std::fs::write(
            &a,
            roedit_io::write_wav_pcm16(&tone(440.0, 1.0, 48_000, 0.5)),
        )
        .unwrap();
        std::fs::write(
            &b,
            roedit_io::write_wav_pcm16(&tone(660.0, 1.0, 48_000, 0.5)),
        )
        .unwrap();
        let tl = setup_timeline(&[
            (a.to_str().unwrap(), 0.0, 1.0),
            (b.to_str().unwrap(), 1.0, 1.0),
        ]);
        let mut cache = HashMap::new();
        let mix = mix_timeline_audio(&tl, &mut cache);
        assert_eq!(mix.clips, 2);
        assert!(mix.missing.is_empty());
        assert!(
            mix.duration_secs() >= 1.9,
            "duration {}",
            mix.duration_secs()
        );
        assert!(mix.peak() > 0.1);
        // The first clip should be silent at 1.5 s, the second active. A sine
        // can land exactly on a zero crossing, so measure a short window.
        let window = |start: f32, end: f32| -> f32 {
            let a = (start * MIX_SAMPLE_RATE as f32) as usize * 2;
            let b = (end * MIX_SAMPLE_RATE as f32) as usize * 2;
            mix.buffer.samples[a..b]
                .iter()
                .fold(0.0f32, |m, v| m.max(v.abs()))
        };
        assert!(window(1.45, 1.55) > 0.05, "second clip should be audible");
        assert!(window(0.45, 0.55) > 0.05, "first clip should be audible");
    }

    #[test]
    fn missing_files_are_reported_not_fatal() {
        let tl = setup_timeline(&[("does-not-exist.wav", 0.0, 1.0)]);
        let mut cache = HashMap::new();
        let mix = mix_timeline_audio(&tl, &mut cache);
        assert_eq!(mix.clips, 0);
        assert_eq!(mix.missing.len(), 1);
    }

    #[test]
    fn fades_attenuate_the_edges() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.wav");
        std::fs::write(
            &p,
            roedit_io::write_wav_pcm16(&tone(220.0, 2.0, 48_000, 0.8)),
        )
        .unwrap();
        let mut tl = setup_timeline(&[(p.to_str().unwrap(), 0.0, 2.0)]);
        if let Some(clip) = tl.clip_mut(roedit_core::ids::ClipId(0)) {
            clip.fade_in = 1.0;
            clip.fade_out = 1.0;
        }
        let mut cache = HashMap::new();
        let mix = mix_timeline_audio(&tl, &mut cache);
        // Compare peak envelopes over windows rather than single samples.
        let window_peak = |start: f32, end: f32| -> f32 {
            let a = (start * MIX_SAMPLE_RATE as f32) as usize;
            let b = (end * MIX_SAMPLE_RATE as f32) as usize;
            mix.buffer.samples[a * 2..b * 2]
                .iter()
                .fold(0.0f32, |m, v| m.max(v.abs()))
        };
        assert!(window_peak(0.0, 0.1) < window_peak(0.9, 1.1));
        assert!(window_peak(1.9, 2.0) < window_peak(0.9, 1.1));
        assert!(window_peak(0.9, 1.1) > 0.0);
    }

    #[test]
    fn probe_reports_duration() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("p.wav");
        std::fs::write(
            &p,
            roedit_io::write_wav_pcm16(&tone(100.0, 0.5, 44_100, 0.3)),
        )
        .unwrap();
        let d = probe_audio(p.to_str().unwrap()).unwrap();
        assert!((d - 0.5).abs() < 0.02);
        assert!(probe_audio("nope.wav").is_err());
    }
}
