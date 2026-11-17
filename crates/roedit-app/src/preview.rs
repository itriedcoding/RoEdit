//! Background jobs: viewport preview, video frame preview, final still renders
//! and frame-sequence exports. All run on worker threads and report through
//! channels so the UI never blocks on a render.

use roedit_core::camera::Camera;
use roedit_core::project::Project;
use roedit_core::raster::RasterImage;
use roedit_core::render_settings::{RenderMode, RenderSettings};
use roedit_io::SaveFormat;
use roedit_render::{build_render_scene, render_float, to_raster, RenderRequest};
use roedit_video::{Compositor, ExportOptions, MediaLibrary};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct PreviewResult {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub millis: f32,
}

fn clamp_preview(width: u32, height: u32) -> (u32, u32) {
    let w = width.clamp(64, 1280);
    let h = height.clamp(36, 720);
    (w, h)
}

#[derive(Default)]
pub struct PreviewEngine {
    rx: Option<Receiver<PreviewResult>>,
    pub in_flight: bool,
    pub last_ms: f32,
    pub generation: u64,
    pub error: Option<String>,
}

impl PreviewEngine {
    pub fn busy(&self) -> bool {
        self.in_flight
    }

    /// Starts a low-sample path traced preview of the 3D scene.
    pub fn request(
        &mut self,
        project: Project,
        camera: Camera,
        width: u32,
        height: u32,
        samples: u32,
        mode: RenderMode,
    ) {
        if self.in_flight {
            return;
        }
        let (w, h) = clamp_preview(width, height);
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.in_flight = true;
        self.generation += 1;
        let env = project.scene.environment.clone();
        let lights: Vec<roedit_core::scene::Light> =
            project.scene.lights.values().cloned().collect();
        let build_begin = Instant::now();
        std::thread::Builder::new()
            .name("roedit-preview".into())
            .spawn(move || {
                let scene = build_render_scene(&project.scene, &env, &camera, &lights);
                let req = RenderRequest {
                    width: w,
                    height: h,
                    samples: samples.max(1),
                    max_bounces: 2,
                    seed: 7,
                    mode,
                    exposure: env.exposure,
                    tonemap: env.tonemap,
                    transparent_background: false,
                    firefly_clamp: 24.0,
                    denoise: true,
                    region: None,
                };
                let started = Instant::now();
                let linear = render_float(&scene, &req, &|_| {});
                let image = to_raster(&linear, &req);
                let _ = tx.send(PreviewResult {
                    width: image.width,
                    height: image.height,
                    rgba: image.data,
                    millis: started.elapsed().as_secs_f32() * 1000.0
                        + build_begin.elapsed().as_secs_f32() * 0.0,
                });
            })
            .ok();
    }

    /// Renders one video-composite frame at the playhead.
    pub fn request_video(&mut self, project: Project, time: f32, width: u32, height: u32) {
        if self.in_flight {
            return;
        }
        let Some(timeline) = project.timeline.clone() else {
            self.error = Some("this project has no timeline yet".into());
            return;
        };
        let (w, h) = clamp_preview(width, height);
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.in_flight = true;
        self.generation += 1;
        let mut media = MediaLibrary::new();
        for (id, image) in &project.scene.images {
            media.images.insert(*id, image.clone());
        }
        std::thread::Builder::new()
            .name("roedit-video-preview".into())
            .spawn(move || {
                let mut comp = Compositor::with_media(timeline, media);
                let started = Instant::now();
                let frame = comp.frame_at(time);
                let scaled = if frame.width == w && frame.height == h {
                    frame
                } else {
                    RasterImage {
                        width: w,
                        height: h,
                        data: {
                            let mut data = vec![0u8; (w * h * 4) as usize];
                            for y in 0..h {
                                for x in 0..w {
                                    let src = frame.get(x * frame.width / w, y * frame.height / h);
                                    let o = ((y * w + x) * 4) as usize;
                                    data[o..o + 4].copy_from_slice(&src);
                                }
                            }
                            data
                        },
                    }
                };
                let _ = tx.send(PreviewResult {
                    width: scaled.width,
                    height: scaled.height,
                    rgba: scaled.data,
                    millis: started.elapsed().as_secs_f32() * 1000.0,
                });
            })
            .ok();
    }

    pub fn poll(&mut self) -> Option<PreviewResult> {
        let result = self.rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if result.is_some() {
            self.in_flight = false;
            if let Some(r) = &result {
                self.last_ms = r.millis;
                self.error = None;
            }
        }
        result
    }
}

#[derive(Clone, Debug)]
pub enum JobOutcome {
    Render {
        path: PathBuf,
        millis: f32,
        width: u32,
        height: u32,
        samples: u32,
    },
    Sequence {
        frames: u32,
        directory: PathBuf,
        audio: Option<PathBuf>,
        millis: f32,
    },
    Flattened {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    Failed(String),
}

pub struct BackgroundJob {
    rx: Receiver<JobOutcome>,
    pub progress: Arc<AtomicU32>,
    pub started: Instant,
    pub label: &'static str,
}

impl BackgroundJob {
    pub fn poll(&mut self) -> Option<JobOutcome> {
        self.rx.try_recv().ok()
    }

    pub fn progress_fraction(&self) -> f32 {
        self.progress.load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub fn elapsed_secs(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }
}

/// Starts a final still render and writes it to `path`.
pub fn start_render(
    project: Project,
    camera: Camera,
    settings: RenderSettings,
    path: PathBuf,
    format: SaveFormat,
    jpeg_quality: u8,
) -> BackgroundJob {
    let (tx, rx) = channel();
    let progress = Arc::new(AtomicU32::new(0));
    let progress_thread = progress.clone();
    let started = Instant::now();
    std::thread::Builder::new()
        .name("roedit-render".into())
        .spawn(move || {
            let env = project.scene.environment.clone();
            let lights: Vec<roedit_core::scene::Light> =
                project.scene.lights.values().cloned().collect();
            let scene = build_render_scene(&project.scene, &env, &camera, &lights);
            let req = RenderRequest::from_settings(&settings, &env);
            let linear = render_float(&scene, &req, &|p| {
                progress_thread.store((p.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
            });
            let image = to_raster(&linear, &req);
            let outcome = match roedit_io::save_image(&image, &path, format, jpeg_quality) {
                Ok(()) => JobOutcome::Render {
                    path,
                    millis: started.elapsed().as_secs_f32() * 1000.0,
                    width: image.width,
                    height: image.height,
                    samples: req.samples,
                },
                Err(err) => JobOutcome::Failed(err.to_string()),
            };
            let _ = tx.send(outcome);
        })
        .ok();
    BackgroundJob {
        rx,
        progress,
        started,
        label: "Rendering still",
    }
}

/// Starts a timeline frame-sequence export.
pub fn start_sequence_export(project: Project, opts: ExportOptions) -> BackgroundJob {
    let (tx, rx) = channel();
    let progress = Arc::new(AtomicU32::new(0));
    let progress_thread = progress.clone();
    let started = Instant::now();
    std::thread::Builder::new()
        .name("roedit-sequence".into())
        .spawn(move || {
            let Some(timeline) = project.timeline.clone() else {
                let _ = tx.send(JobOutcome::Failed(
                    "this project has no timeline to export".into(),
                ));
                return;
            };
            let mut media = MediaLibrary::new();
            for (id, image) in &project.scene.images {
                media.images.insert(*id, image.clone());
            }
            let mut comp = Compositor::with_media(timeline, media);
            let outcome = match roedit_video::export_sequence(&mut comp, &opts, &mut |p| {
                progress_thread.store((p.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
            }) {
                Ok(report) => JobOutcome::Sequence {
                    frames: report.frames,
                    directory: report.directory,
                    audio: report.audio,
                    millis: started.elapsed().as_secs_f32() * 1000.0,
                },
                Err(err) => JobOutcome::Failed(err.to_string()),
            };
            let _ = tx.send(outcome);
        })
        .ok();
    BackgroundJob {
        rx,
        progress,
        started,
        label: "Exporting sequence",
    }
}

/// Starts flattening the GFX document to a PNG.
pub fn start_flatten(project: Project, path: PathBuf) -> BackgroundJob {
    let (tx, rx) = channel();
    let progress = Arc::new(AtomicU32::new(1000));
    let started = Instant::now();
    std::thread::Builder::new()
        .name("roedit-flatten".into())
        .spawn(move || {
            let Some(gfx) = project.gfx.clone() else {
                let _ = tx.send(JobOutcome::Failed("there is no GFX document".into()));
                return;
            };
            let opts = roedit_image::composite::FlattenOptions::default();
            let image = roedit_image::composite::flatten_doc(&gfx, &opts);
            let outcome = match roedit_io::save_image(&image, &path, SaveFormat::Png, 92) {
                Ok(()) => JobOutcome::Flattened {
                    path,
                    width: image.width,
                    height: image.height,
                },
                Err(err) => JobOutcome::Failed(err.to_string()),
            };
            let _ = tx.send(outcome);
        })
        .ok();
    BackgroundJob {
        rx,
        progress,
        started,
        label: "Flattening GFX",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_sizes_are_bounded() {
        assert_eq!(clamp_preview(0, 0), (64, 36));
        assert_eq!(clamp_preview(4000, 4000), (1280, 720));
        assert_eq!(clamp_preview(320, 180), (320, 180));
    }

    #[test]
    fn preview_engine_reports_busy_until_polled() {
        let mut engine = PreviewEngine::default();
        assert!(!engine.busy());
        let project = Project::new("Preview");
        let camera = Camera::default();
        engine.request(project, camera, 128, 72, 1, RenderMode::Albedo);
        // The worker may finish immediately; either state is valid, but polling
        // must clear the busy flag once a result arrives.
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let mut got = None;
        while Instant::now() < deadline && got.is_none() {
            got = engine.poll();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let result = got.expect("preview should complete");
        assert_eq!((result.width, result.height), (128, 72));
        assert_eq!(result.rgba.len(), 128 * 72 * 4);
        assert!(!engine.busy());
    }
}
