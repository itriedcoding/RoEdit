//! Frame-sequence export with optional mixed WAV audio.

use crate::audio::mix_timeline_audio;
use crate::{Compositor, VideoError};
use roedit_io::{save_image, SaveFormat};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub directory: PathBuf,
    pub prefix: String,
    pub format: SaveFormat,
    /// Inclusive frame range in timeline frames.
    pub start_frame: u32,
    pub end_frame: u32,
    pub jpeg_quality: u8,
    /// 0 uses the timeline frame rate.
    pub fps: f32,
    pub overwrite: bool,
    /// Write the mixed audio track next to the frames.
    pub write_audio: bool,
}

impl ExportOptions {
    pub fn new(directory: impl Into<PathBuf>, prefix: impl Into<String>) -> Self {
        Self {
            directory: directory.into(),
            prefix: prefix.into(),
            format: SaveFormat::Png,
            start_frame: 0,
            end_frame: 89,
            jpeg_quality: 92,
            fps: 0.0,
            overwrite: true,
            write_audio: true,
        }
    }

    pub fn frame_count(&self) -> u32 {
        if self.end_frame < self.start_frame {
            0
        } else {
            self.end_frame - self.start_frame + 1
        }
    }

    pub fn effective_fps(&self, timeline_fps: f32) -> f32 {
        if self.fps > 0.0 {
            self.fps
        } else {
            timeline_fps.max(1.0)
        }
    }

    pub fn duration(&self, timeline_fps: f32) -> f32 {
        self.frame_count() as f32 / self.effective_fps(timeline_fps)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExportReport {
    pub frames: u32,
    pub directory: PathBuf,
    pub first_frame: Option<PathBuf>,
    pub last_frame: Option<PathBuf>,
    pub audio: Option<PathBuf>,
    pub duration_secs: f32,
    pub fps: f32,
}

impl ExportReport {
    pub fn summary(&self) -> String {
        let first = self
            .first_frame
            .as_ref()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|| "-".into());
        let last = self
            .last_frame
            .as_ref()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|| "-".into());
        format!(
            "{} frames at {:.2} fps ({:.2}s): {} .. {}{}",
            self.frames,
            self.fps,
            self.duration_secs,
            first,
            last,
            if self.audio.is_some() {
                " + mix.wav"
            } else {
                ""
            }
        )
    }
}

/// Renders every frame in the range and writes it to disk.
///
/// Frames are written as `prefix_00000.png`-style names so they sort correctly
/// and can be imported into an external encoder.
pub fn export_sequence(
    compositor: &mut Compositor,
    opts: &ExportOptions,
    progress: &mut dyn FnMut(f32),
) -> Result<ExportReport, VideoError> {
    if opts.frame_count() == 0 {
        return Err(VideoError::Range(
            "end frame is before the start frame".into(),
        ));
    }
    if opts.prefix.trim().is_empty() {
        return Err(VideoError::Range("export prefix is empty".into()));
    }
    let fps = opts.effective_fps(compositor.timeline.fps);
    std::fs::create_dir_all(&opts.directory)?;
    let total = opts.frame_count();
    let mut report = ExportReport {
        directory: opts.directory.clone(),
        fps,
        duration_secs: total as f32 / fps,
        ..Default::default()
    };
    let mut wrote_any = false;
    for (i, frame) in (opts.start_frame..=opts.end_frame).enumerate() {
        let time = frame as f32 / fps;
        let image = compositor.frame_at(time);
        let name = format!("{}_{frame:05}.{}", opts.prefix, opts.format.extension());
        let path = opts.directory.join(name);
        if path.exists() && !opts.overwrite {
            return Err(VideoError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{} already exists", path.display()),
            )));
        }
        if image.data.iter().all(|b| *b == 0) {
            log::warn!("frame {frame} at {time:.3}s is empty");
        }
        save_image(&image, &path, opts.format, opts.jpeg_quality)?;
        if !wrote_any {
            report.first_frame = Some(path.clone());
            wrote_any = true;
        }
        report.last_frame = Some(path.clone());
        report.frames += 1;
        progress((i + 1) as f32 / total as f32);
    }
    if opts.write_audio {
        let mut cache: HashMap<String, roedit_io::AudioBuffer> = HashMap::new();
        let mix = mix_timeline_audio(&compositor.timeline, &mut cache);
        if mix.clips > 0 {
            let path = opts.directory.join("mix.wav");
            std::fs::write(&path, roedit_io::write_wav_pcm16(&mix.buffer))?;
            report.audio = Some(path);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::ids::VideoTrackId;
    use roedit_core::timeline::{ClipSource, Timeline, TrackKind, VideoTrack};

    fn comp() -> Compositor {
        let mut tl = Timeline::new(32, 18, 24.0, 4.0);
        tl.tracks
            .push(VideoTrack::new(VideoTrackId(0), "Main", TrackKind::Image));
        tl.add_clip(
            VideoTrackId(0),
            ClipSource::Color([0.2, 0.4, 0.8]),
            0.0,
            4.0,
        );
        Compositor::new(tl)
    }

    #[test]
    fn exports_numbered_frames() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = comp();
        let opts = ExportOptions {
            directory: dir.path().to_path_buf(),
            prefix: "shot".into(),
            start_frame: 0,
            end_frame: 4,
            write_audio: false,
            ..ExportOptions::new(dir.path(), "shot")
        };
        let mut seen = Vec::new();
        let report = export_sequence(&mut c, &opts, &mut |p| seen.push(p)).unwrap();
        assert_eq!(report.frames, 5);
        assert!(seen.len() == 5);
        assert!((seen.last().unwrap() - 1.0).abs() < 1e-6);
        let first = report.first_frame.clone().unwrap();
        assert!(first.exists());
        assert!(first
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("shot_00000"));
        let img = roedit_io::load_image(&first).unwrap();
        assert_eq!((img.width, img.height), (32, 18));
        assert!(report.summary().contains("5 frames"));
    }

    #[test]
    fn rejects_inverted_ranges() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = comp();
        let opts = ExportOptions {
            start_frame: 5,
            end_frame: 2,
            ..ExportOptions::new(dir.path(), "x")
        };
        assert!(export_sequence(&mut c, &opts, &mut |_| {}).is_err());
    }

    #[test]
    fn respects_existing_files_when_not_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a_00000.png"), b"x").unwrap();
        let mut c = comp();
        let opts = ExportOptions {
            overwrite: false,
            end_frame: 0,
            write_audio: false,
            ..ExportOptions::new(dir.path(), "a")
        };
        assert!(export_sequence(&mut c, &opts, &mut |_| {}).is_err());
    }

    #[test]
    fn option_maths() {
        let mut o = ExportOptions::new("out", "p");
        assert_eq!(o.frame_count(), 90);
        o.start_frame = 10;
        o.end_frame = 19;
        assert_eq!(o.frame_count(), 10);
        assert!((o.duration(30.0) - 10.0 / 30.0).abs() < 1e-6);
        o.fps = 10.0;
        assert!((o.duration(30.0) - 1.0).abs() < 1e-6);
    }
}
