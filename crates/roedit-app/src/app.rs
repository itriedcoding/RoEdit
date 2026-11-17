//! Application shell: state, shortcuts, action dispatch and background jobs.

use crate::command::{
    all_commands, binding_for, default_text_spec, Action, Command, ImageOpKind, MeshOpKind,
    PrimitiveKind, UvKind,
};
use crate::document::Document;
use crate::preview::{BackgroundJob, JobOutcome, PreviewEngine};
use crate::state::{Tool, UiState, ViewMode, Workspace};
use eframe::egui;
use glam::{Vec2, Vec3};
use roedit_core::animation::{AnimValue, Target};
use roedit_core::ids::EntityId;
use roedit_core::ids::VideoTrackId;
use roedit_core::raster::RasterImage;
use roedit_core::render_settings::RenderMode;
use roedit_core::scene::LightKind;
use roedit_core::settings::AppSettings;
use roedit_core::timeline::{ClipSource, Timeline, TrackKind, VideoTrack};
use roedit_image::filters::gaussian_blur;
use roedit_image::keying::{chroma_key, ChromaKeySettings};
use roedit_mesh::primitives as prim;
use roedit_mesh::roblox::{r15_parts, r6_parts, RigType, STUD_TO_METER};
use roedit_mesh::Axis;
use roedit_plugins::PluginRegistry;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct RoEditApp {
    pub(crate) doc: Document,
    pub(crate) settings: AppSettings,
    pub(crate) ui: UiState,
    pub(crate) workspace: Workspace,
    pub(crate) commands: Vec<Command>,
    pub(crate) plugins: PluginRegistry,
    pub(crate) preview: PreviewEngine,
    pub(crate) preview_texture: Option<egui::TextureHandle>,
    pub(crate) video_texture: Option<egui::TextureHandle>,
    pub(crate) gfx_texture: Option<egui::TextureHandle>,
    pub(crate) gfx_dirty: bool,
    pub(crate) canvas_texture: Option<egui::TextureHandle>,
    pub(crate) canvas_dirty: bool,
    pub(crate) job: Option<BackgroundJob>,
    pub(crate) last_autosave: Instant,
    pub(crate) last_preview_request: Instant,
    pub(crate) preview_dirty: bool,
    pub(crate) theme_applied: bool,
    pub(crate) scratch: Scratch,
}

#[derive(Default)]
pub struct Scratch {
    pub status: String,
    pub rename_buffer: String,
    pub rename_target: Option<EntityId>,
    pub show_import_dialog: bool,
    pub import_kind: Option<&'static str>,
    pub video_preview_time: f32,
    pub last_video_request: Option<Instant>,
    pub palette_scroll_to: usize,
    pub bottom_tab: usize,
    pub canvas_key: String,
    pub import_buffer: String,
    pub shortcut_buffers: std::collections::BTreeMap<String, String>,
}

impl RoEditApp {
    pub fn bootstrap(settings: AppSettings) -> Self {
        let mut plugins = PluginRegistry::new();
        let plugin_dir = Document::config_dir().join("plugins");
        if plugin_dir.is_dir() {
            let errors = plugins.load_dir(&plugin_dir);
            for (path, err) in &errors {
                log::warn!("plugin {}: {err}", path.display());
            }
        }
        let mut ui = UiState::default();
        ui.plugin_dir = plugin_dir.to_string_lossy().to_string();
        ui.show_recovery = Document::autosave_exists();
        ui.log("info", "RoEdit Studio started");
        Self {
            doc: Document::untitled(),
            settings,
            ui,
            workspace: Workspace::Model,
            commands: all_commands(),
            plugins,
            preview: PreviewEngine::default(),
            preview_texture: None,
            video_texture: None,
            gfx_texture: None,
            gfx_dirty: true,
            canvas_texture: None,
            canvas_dirty: true,
            job: None,
            last_autosave: Instant::now(),
            last_preview_request: Instant::now() - Duration::from_secs(10),
            preview_dirty: true,
            theme_applied: false,
            scratch: Scratch::default(),
        }
    }

    pub fn attach(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        crate::ui::theme::apply(&cc.egui_ctx, &self.settings);
        self.apply_ui_scale(&cc.egui_ctx);
        self.theme_applied = true;
        self
    }

    fn apply_ui_scale(&self, ctx: &egui::Context) {
        ctx.set_pixels_per_point(self.settings.ui_scale.clamp(0.75, 2.0));
    }

    pub fn set_workspace(&mut self, workspace: Workspace) {
        self.workspace = workspace;
        self.ui.tool = match workspace {
            Workspace::Sculpt => Tool::Sculpt,
            Workspace::Gfx | Workspace::Texture => Tool::Paint,
            Workspace::Uv => Tool::Select,
            _ => self.ui.tool,
        };
        self.preview_dirty = true;
        self.ui
            .log("info", format!("Workspace: {}", workspace.label()));
    }

    // ------------------------------------------------------------- background

    fn poll_background(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.preview.poll() {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [result.width as usize, result.height as usize],
                &result.rgba,
            );
            let texture = ctx.load_texture("roedit-preview", image, egui::TextureOptions::LINEAR);
            if self.workspace == Workspace::Video {
                self.video_texture = Some(texture);
            } else {
                self.preview_texture = Some(texture);
            }
            self.ui.perf.last_preview_ms = result.millis;
            self.ui.perf.last_frame_pixels = result.rgba.len() / 4;
        }
        if let Some(job) = &mut self.job {
            if let Some(outcome) = job.poll() {
                self.job = None;
                self.handle_job_outcome(outcome);
            }
        }
    }

    fn handle_job_outcome(&mut self, outcome: JobOutcome) {
        match outcome {
            JobOutcome::Render {
                path,
                millis,
                width,
                height,
                samples,
            } => {
                self.ui.perf.last_render_ms = millis;
                self.ui.log(
                    "success",
                    format!(
                        "Rendered {width}x{height} @ {samples} spp in {:.2}s → {}",
                        millis / 1000.0,
                        path.display()
                    ),
                );
                self.ui.toast(format!("Render saved to {}", path.display()));
            }
            JobOutcome::Sequence {
                frames,
                directory,
                audio,
                millis,
            } => {
                self.ui.log(
                    "success",
                    format!(
                        "Exported {frames} frames to {} in {:.1}s{}",
                        directory.display(),
                        millis / 1000.0,
                        if audio.is_some() { " with mix.wav" } else { "" }
                    ),
                );
                self.ui.toast(format!("{frames} frames exported"));
            }
            JobOutcome::Flattened {
                path,
                width,
                height,
            } => {
                self.ui.log(
                    "success",
                    format!("Flattened GFX {width}x{height} → {}", path.display()),
                );
                self.ui.toast("GFX exported");
            }
            JobOutcome::Failed(message) => {
                self.ui.log("error", message.clone());
                self.ui.toast(format!("Failed: {message}"));
            }
        }
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn autosave_tick(&mut self) {
        if !self.settings.autosave_enabled || !self.doc.dirty || self.busy() {
            return;
        }
        let interval = Duration::from_secs(self.settings.autosave_interval_secs.max(15));
        if self.last_autosave.elapsed() < interval {
            return;
        }
        self.last_autosave = Instant::now();
        match self.doc.write_autosave() {
            Ok((path, bytes)) => {
                self.ui.autosave_status = format!(
                    "Autosaved {:.1} KB at {}",
                    bytes as f32 / 1024.0,
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            Err(err) => {
                self.ui.autosave_status = format!("Autosave failed: {err}");
                self.ui.log("error", format!("Autosave failed: {err}"));
            }
        }
    }

    // -------------------------------------------------------------- shortcuts

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.ui.palette_open {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.ui.palette_open = false;
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.ui.show_settings = false;
            self.ui.show_about = false;
            self.ui.show_shortcuts = false;
            self.ui.show_render_dialog = false;
            self.ui.show_export_dialog = false;
            self.scratch.show_import_dialog = false;
        }
        let typing = ctx.wants_keyboard_input();
        let mut triggered: Option<Action> = None;
        for command in &self.commands {
            let Some(binding_text) = binding_for(command, &self.settings) else {
                continue;
            };
            let Some(binding) = crate::command::Binding::parse(binding_text) else {
                continue;
            };
            if typing && !binding.ctrl && !binding.alt {
                continue;
            }
            if binding.matches(ctx) {
                triggered = Some(command.action.clone());
                break;
            }
        }
        if let Some(action) = triggered {
            self.dispatch(action);
        }
    }

    // ------------------------------------------------------------------ files

    fn default_project_path(&self) -> PathBuf {
        if self.ui.io_path.trim().is_empty() {
            PathBuf::from(format!("{}.roedit", self.doc.project.meta.name))
        } else {
            PathBuf::from(self.ui.io_path.trim())
        }
    }

    #[cfg(windows)]
    fn pick_save(&self, default_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("RoEdit project", &["roedit"])
            .set_file_name(default_name)
            .save_file()
    }

    #[cfg(not(windows))]
    fn pick_save(&self, _default_name: &str) -> Option<PathBuf> {
        None
    }

    #[cfg(windows)]
    fn pick_open(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("RoEdit project", &["roedit"])
            .add_filter("All files", &["*"])
            .pick_file()
    }

    #[cfg(not(windows))]
    fn pick_open(&self) -> Option<PathBuf> {
        None
    }

    fn action_new(&mut self) {
        self.doc = Document::untitled();
        self.ui.selected = None;
        self.ui.selected_image = None;
        self.ui.selected_material = None;
        self.ui.log("info", "New project");
        self.preview_dirty = true;
    }

    fn action_open(&mut self) {
        let path = self
            .pick_open()
            .unwrap_or_else(|| self.default_project_path());
        match Document::load(&path) {
            Ok(doc) => {
                self.doc = doc;
                self.settings.push_recent_project(&path.to_string_lossy());
                self.ui.log("success", format!("Opened {}", path.display()));
                self.ui.selected = None;
                self.preview_dirty = true;
                self.fit_scene_to_view();
            }
            Err(err) => {
                self.ui.log("error", err.clone());
                self.ui.toast(err);
            }
        }
    }

    fn action_save(&mut self) {
        let result = if self.doc.path.is_some() {
            self.doc
                .save()
                .map(|bytes| (self.doc.path.clone().unwrap(), bytes))
        } else {
            let default = format!("{}.roedit", self.doc.project.meta.name);
            let path = self
                .pick_save(&default)
                .unwrap_or_else(|| self.default_project_path());
            self.doc.save_to(&path).map(|bytes| (path, bytes))
        };
        match result {
            Ok((path, bytes)) => {
                Document::clear_autosave();
                self.settings.push_recent_project(&path.to_string_lossy());
                self.ui.log(
                    "success",
                    format!("Saved {} ({:.1} KB)", path.display(), bytes as f32 / 1024.0),
                );
                self.ui.toast("Project saved");
            }
            Err(err) => {
                self.ui.log("error", err.clone());
                self.ui.toast(err);
            }
        }
    }

    fn action_save_as(&mut self) {
        let default = format!("{}.roedit", self.doc.project.meta.name);
        let path = self
            .pick_save(&default)
            .unwrap_or_else(|| self.default_project_path());
        match self.doc.save_to(&path) {
            Ok(bytes) => {
                Document::clear_autosave();
                self.settings.push_recent_project(&path.to_string_lossy());
                self.ui.log(
                    "success",
                    format!("Saved {} ({:.1} KB)", path.display(), bytes as f32 / 1024.0),
                );
            }
            Err(err) => {
                self.ui.log("error", err.clone());
                self.ui.toast(err);
            }
        }
    }

    fn action_import(&mut self) {
        let path = PathBuf::from(self.ui.io_path.trim());
        if self.ui.io_path.trim().is_empty() {
            self.scratch.show_import_dialog = true;
            self.scratch.import_buffer = self.ui.io_path.clone();
            return;
        }
        let kind = roedit_io::detect_kind(Some(&path), &std::fs::read(&path).unwrap_or_default());
        match roedit_io::check_supported(kind) {
            support if !support.supported => {
                self.ui
                    .log("error", format!("{}: {}", kind.label(), support.message));
                self.ui.toast("Unsupported format — see Assets workspace");
                return;
            }
            _ => {}
        }
        match kind {
            roedit_io::FileKind::Png
            | roedit_io::FileKind::Jpeg
            | roedit_io::FileKind::Tiff
            | roedit_io::FileKind::Bmp
            | roedit_io::FileKind::Webp
            | roedit_io::FileKind::Exr => match roedit_io::load_image(&path) {
                Ok(image) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Image".into());
                    let mut id = None;
                    self.doc.edit(format!("Import {name}"), |p| {
                        id = Some(p.scene.add_image(image));
                    });
                    self.ui.selected_image = id;
                    self.workspace = Workspace::Assets;
                    self.ui.log("success", format!("Imported image {name}"));
                }
                Err(err) => self.ui.log("error", format!("{err}")),
            },
            roedit_io::FileKind::Obj => match std::fs::read_to_string(&path) {
                Ok(text) => match roedit_io::import_obj_multi(&text) {
                    Ok(meshes) => {
                        let count = meshes.len();
                        self.doc.edit("Import OBJ", |p| {
                            for (name, mesh) in meshes {
                                let mid = p.scene.add_mesh(mesh);
                                let eid = p.scene.add_entity(name, None);
                                if let Some(e) = p.scene.entity_mut(eid) {
                                    e.mesh = Some(mid);
                                }
                            }
                        });
                        self.ui
                            .log("success", format!("Imported {count} OBJ object(s)"));
                        self.fit_scene_to_view();
                        self.preview_dirty = true;
                    }
                    Err(err) => self.ui.log("error", format!("{err}")),
                },
                Err(err) => self.ui.log("error", format!("{err}")),
            },
            roedit_io::FileKind::Stl => match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| roedit_io::import_stl(&bytes).map_err(|e| e.to_string()))
            {
                Ok(mesh) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "STL".into());
                    self.doc.edit("Import STL", |p| {
                        let mid = p.scene.add_mesh(mesh);
                        let eid = p.scene.add_entity(name, None);
                        if let Some(e) = p.scene.entity_mut(eid) {
                            e.mesh = Some(mid);
                        }
                    });
                    self.ui.log("success", "Imported STL");
                    self.fit_scene_to_view();
                    self.preview_dirty = true;
                }
                Err(err) => self.ui.log("error", err),
            },
            roedit_io::FileKind::Glb => match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| roedit_io::import_glb(&bytes).map_err(|e| e.to_string()))
            {
                Ok(meshes) => {
                    let count = meshes.len();
                    self.doc.edit("Import GLB", |p| {
                        for (name, mesh) in meshes {
                            let mid = p.scene.add_mesh(mesh);
                            let eid = p.scene.add_entity(name, None);
                            if let Some(e) = p.scene.entity_mut(eid) {
                                e.mesh = Some(mid);
                            }
                        }
                    });
                    self.ui
                        .log("success", format!("Imported {count} glTF mesh(es)"));
                    self.fit_scene_to_view();
                    self.preview_dirty = true;
                }
                Err(err) => self.ui.log("error", err),
            },
            other => {
                let support = roedit_io::check_supported(other);
                self.ui
                    .log("error", format!("{}: {}", other.label(), support.message));
            }
        }
    }

    pub(crate) fn action_export_mesh(&mut self) {
        let Some(entity) = self.ui.selected else {
            self.ui.toast("Select an object to export");
            return;
        };
        let Some(mesh_id) = self.doc.project.scene.entity(entity).and_then(|e| e.mesh) else {
            self.ui.toast("The selected object has no mesh");
            return;
        };
        let mesh = match self.doc.project.scene.mesh(mesh_id) {
            Some(m) => m.clone(),
            None => return,
        };
        let mut target = PathBuf::from(self.ui.io_path.trim());
        if target.as_os_str().is_empty() {
            target = self.default_project_path();
        }
        let stem = target
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "export".into());
        let dir = target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let ext = target
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let result: Result<PathBuf, String> = match ext.as_str() {
            "stl" => {
                let bytes = roedit_io::export_stl_binary(&mesh);
                let path = target.clone();
                std::fs::write(&path, bytes)
                    .map(|_| path)
                    .map_err(|e| e.to_string())
            }
            "glb" => match roedit_io::export_glb(
                &[(mesh.name.clone(), mesh.clone())],
                [0.8, 0.8, 0.82, 1.0],
            ) {
                Ok(bytes) => {
                    let path = target.clone();
                    std::fs::write(&path, bytes)
                        .map(|_| path)
                        .map_err(|e| e.to_string())
                }
                Err(err) => Err(err.to_string()),
            },
            _ => {
                let materials: Vec<String> = self
                    .doc
                    .project
                    .scene
                    .materials
                    .values()
                    .map(|m| m.name.clone())
                    .collect();
                let (obj, mtl) = roedit_io::export_obj(&mesh, &stem, &materials);
                let obj_path = if ext == "obj" {
                    target.clone()
                } else {
                    dir.join(format!("{stem}.obj"))
                };
                let mtl_path = obj_path.with_extension("mtl");
                std::fs::write(&obj_path, obj)
                    .and_then(|_| std::fs::write(&mtl_path, mtl))
                    .map(|_| obj_path)
                    .map_err(|e| e.to_string())
            }
        };
        match result {
            Ok(path) => {
                self.ui
                    .log("success", format!("Exported mesh → {}", path.display()));
                self.ui.toast("Mesh exported");
            }
            Err(err) => self.ui.log("error", format!("Export failed: {err}")),
        }
    }

    // ------------------------------------------------------------- modelling

    fn add_primitive(&mut self, kind: PrimitiveKind) {
        let size = Vec3::new(1.0, 1.0, 1.0);
        let mesh = kind.build(size);
        let name = kind.label().to_string();
        let mut id = None;
        self.doc.edit(format!("Add {name}"), |p| {
            let mid = p.scene.add_mesh(mesh);
            let eid = p.scene.add_entity(name.clone(), None);
            let plastic = p
                .scene
                .materials
                .iter()
                .find(|(_, m)| m.name == "Roblox Plastic")
                .map(|(id, _)| *id)
                .unwrap_or_else(|| {
                    p.scene.add_material(roedit_core::material::Material::new(
                        "Roblox Plastic",
                        [0.72, 0.18, 0.16],
                        0.0,
                        0.55,
                    ))
                });
            if let Some(e) = p.scene.entity_mut(eid) {
                e.mesh = Some(mid);
                e.material = Some(plastic);
                e.transform.translation = Vec3::new(0.0, 0.5, 0.0);
            }
            id = Some(eid);
        });
        self.ui.selected = id;
        self.ui.log("info", format!("Added {name}"));
        self.preview_dirty = true;
    }

    fn add_rig(&mut self, rig: RigType) {
        let parts = match rig {
            RigType::R6 => r6_parts(),
            RigType::R15 => r15_parts(),
        };
        let count = parts.len();
        let root_name = format!("Roblox {} Rig", rig.label());
        self.doc.edit(format!("Build {} rig", rig.label()), |p| {
            let root = p.scene.add_entity(root_name.clone(), None);
            let plastic = p
                .scene
                .materials
                .iter()
                .find(|(_, m)| m.name == "Roblox Plastic")
                .map(|(id, _)| *id)
                .unwrap_or_else(|| {
                    p.scene.add_material(roedit_core::material::Material::new(
                        "Roblox Plastic",
                        [0.72, 0.18, 0.16],
                        0.0,
                        0.55,
                    ))
                });
            for part in parts {
                let mut mesh = prim::roblox_part(part.shape, part.size * STUD_TO_METER, 10);
                mesh.name = part.name.to_string();
                let mid = p.scene.add_mesh(mesh);
                let eid = p.scene.add_entity(part.name, Some(root));
                if let Some(e) = p.scene.entity_mut(eid) {
                    e.mesh = Some(mid);
                    e.material = Some(plastic);
                    e.transform.translation = part.center * STUD_TO_METER;
                }
            }
            if let Some(e) = p.scene.entity_mut(root) {
                e.visible = true;
            }
        });
        self.ui
            .log("info", format!("Built {root_name} ({count} parts)"));
        self.ui
            .toast(format!("{} rig with {count} parts", rig.label()));
        self.fit_scene_to_view();
        self.preview_dirty = true;
    }

    fn selected_mesh_id(&self) -> Option<roedit_core::ids::MeshId> {
        let entity = self.ui.selected?;
        self.doc.project.scene.entity(entity).and_then(|e| e.mesh)
    }

    fn apply_mesh_op(&mut self, op: MeshOpKind, amount: f32) {
        let Some(mesh_id) = self.selected_mesh_id() else {
            self.ui.toast("Select an object with a mesh first");
            return;
        };
        let label = op.label().to_string();
        let mut message = String::new();
        let mut failed = false;
        self.doc.edit(format!("{label} mesh"), |p| {
            let Some(mesh) = p.scene.mesh_mut(mesh_id) else {
                failed = true;
                return;
            };
            let all: Vec<usize> = (0..mesh.faces.len()).collect();
            let result: Result<String, String> = match op {
                MeshOpKind::Subdivide => roedit_mesh::edit::subdivide_faces(
                    mesh,
                    &all,
                    amount.round().clamp(1.0, 5.0) as u32,
                )
                .map(|f| format!("{} faces", f.len()))
                .map_err(|e| e.to_string()),
                MeshOpKind::Bevel => roedit_mesh::edit::bevel_faces(mesh, &all, amount, 2)
                    .map(|n| format!("{n} bevel faces"))
                    .map_err(|e| e.to_string()),
                MeshOpKind::Extrude => roedit_mesh::edit::extrude_faces(mesh, &all, amount)
                    .map(|f| format!("{} faces", f.len()))
                    .map_err(|e| e.to_string()),
                MeshOpKind::Inset => roedit_mesh::edit::inset_faces(mesh, &all, amount, 0.0)
                    .map(|f| format!("{} faces", f.len()))
                    .map_err(|e| e.to_string()),
                MeshOpKind::Smooth => Ok(format!(
                    "{} vertices moved",
                    roedit_mesh::edit::smooth_laplacian(
                        mesh,
                        None,
                        amount.clamp(1.0, 20.0) as u32,
                        0.5
                    )
                )),
                MeshOpKind::Decimate => roedit_mesh::edit::decimate(mesh, amount)
                    .map(|r| format!("{:.0}% kept", r * 100.0))
                    .map_err(|e| e.to_string()),
                MeshOpKind::Solidify => roedit_mesh::edit::solidify(mesh, amount)
                    .map(|n| format!("{n} shell faces"))
                    .map_err(|e| e.to_string()),
                MeshOpKind::Triangulate => Ok(format!(
                    "{} faces triangulated",
                    roedit_mesh::edit::triangulate(mesh)
                )),
                MeshOpKind::RecalcNormals => {
                    Ok(if roedit_mesh::edit::recalc_normals_outside(mesh) {
                        "flipped outward".into()
                    } else {
                        "already outward".into()
                    })
                }
                MeshOpKind::Weld => Ok(format!(
                    "{} vertices merged",
                    roedit_mesh::edit::merge_by_distance(mesh, amount.max(1e-5))
                )),
                MeshOpKind::MirrorX => roedit_mesh::edit::mirror_modifier(mesh, Axis::X, 1e-4)
                    .map(|n| format!("{n} mirrored faces"))
                    .map_err(|e| e.to_string()),
                MeshOpKind::MirrorY => roedit_mesh::edit::mirror_modifier(mesh, Axis::Y, 1e-4)
                    .map(|n| format!("{n} mirrored faces"))
                    .map_err(|e| e.to_string()),
                MeshOpKind::MirrorZ => roedit_mesh::edit::mirror_modifier(mesh, Axis::Z, 1e-4)
                    .map(|n| format!("{n} mirrored faces"))
                    .map_err(|e| e.to_string()),
                MeshOpKind::Array => roedit_mesh::edit::array_modifier(
                    mesh,
                    3,
                    Vec3::new(amount.max(0.05), 0.0, 0.0),
                    0.0,
                )
                .map(|n| format!("{n} array copies"))
                .map_err(|e| e.to_string()),
                MeshOpKind::FlipNormals => {
                    mesh.flip();
                    Ok("normals flipped".into())
                }
            };
            match result {
                Ok(m) => message = m,
                Err(e) => {
                    failed = true;
                    message = e;
                }
            }
        });
        if failed {
            self.ui.log("error", format!("{label}: {message}"));
            self.ui.toast(format!("{label} failed"));
        } else {
            self.ui.log("success", format!("{label}: {message}"));
            self.preview_dirty = true;
        }
    }

    fn apply_uv(&mut self, kind: UvKind) {
        let Some(mesh_id) = self.selected_mesh_id() else {
            self.ui.toast("Select an object with a mesh first");
            return;
        };
        let mut detail = String::new();
        self.doc.edit(format!("UV {}", kind.label()), |p| {
            if let Some(mesh) = p.scene.mesh_mut(mesh_id) {
                match kind {
                    UvKind::Box => roedit_mesh::uv::box_uvs(mesh, 1.0),
                    UvKind::Planar => roedit_mesh::uv::planar_uvs(mesh, Axis::Y, 1.0, Vec2::ZERO),
                    UvKind::Cylindrical => roedit_mesh::uv::cylindrical_uvs(mesh),
                    UvKind::Spherical => roedit_mesh::uv::spherical_uvs(mesh),
                    UvKind::Unwrap => {
                        let faces: Vec<usize> = (0..mesh.faces.len()).collect();
                        let report = roedit_mesh::uv::unwrap_faces(mesh, &faces, 0.02);
                        detail = format!(
                            "{} islands, {} skipped",
                            report.islands, report.skipped_faces
                        );
                    }
                }
                if detail.is_empty() {
                    detail = format!("{} faces mapped", mesh.faces.len());
                }
            }
        });
        self.ui
            .log("success", format!("{}: {detail}", kind.label()));
        self.ui.toast(format!("{} applied", kind.label()));
    }

    fn duplicate_selected(&mut self) {
        let Some(entity_id) = self.ui.selected else {
            return;
        };
        let Some(source) = self.doc.project.scene.entity(entity_id).cloned() else {
            return;
        };
        let mesh = source
            .mesh
            .and_then(|id| self.doc.project.scene.mesh(id).cloned());
        let mut new_id = None;
        self.doc.edit("Duplicate object", |p| {
            let new_mesh = mesh.map(|m| p.scene.add_mesh(m));
            let eid = p
                .scene
                .add_entity(format!("{} Copy", source.name), source.parent);
            if let Some(e) = p.scene.entity_mut(eid) {
                e.mesh = new_mesh;
                e.material = source.material;
                e.transform = source.transform;
                e.transform.translation += Vec3::new(0.4, 0.0, 0.4);
            }
            new_id = Some(eid);
        });
        self.ui.selected = new_id;
        self.preview_dirty = true;
        self.ui.log("info", "Duplicated object");
    }

    fn delete_selected(&mut self) {
        let Some(entity_id) = self.ui.selected else {
            return;
        };
        let name = self
            .doc
            .project
            .scene
            .entity(entity_id)
            .map(|e| e.name.clone())
            .unwrap_or_default();
        let mut removed = 0;
        self.doc.edit(format!("Delete {name}"), |p| {
            removed = p.scene.delete_entity(entity_id).len();
        });
        self.ui.selected = None;
        self.preview_dirty = true;
        self.ui
            .log("info", format!("Deleted {name} ({removed} object(s))"));
    }

    pub(crate) fn selected_entity_bounds(&self) -> Option<(Vec3, f32)> {
        let entity = self.ui.selected?;
        let mesh_id = self.doc.project.scene.entity(entity)?.mesh?;
        let mesh = self.doc.project.scene.mesh(mesh_id)?;
        let world = self.doc.project.scene.world_matrix(entity);
        let bounds = mesh.bounds().transformed(world);
        let size = bounds.size();
        Some((bounds.center(), size.length() * 0.5))
    }

    fn fit_scene_to_view(&mut self) {
        let scene = &self.doc.project.scene;
        let bounds = scene.bounds();
        let size = bounds.size();
        if size.length() > 1e-4 {
            self.ui
                .camera
                .frame(bounds.center(), (size.length() * 0.5).max(0.5));
            self.ui.scene_radius = (size.length() * 0.5).max(0.5);
        }
    }

    // ---------------------------------------------------------------- images

    fn apply_image_op(&mut self, op: ImageOpKind) {
        let Some(image_id) = self.ui.selected_image else {
            self.ui.toast("Select an image in the Asset Library first");
            return;
        };
        let label = op.label().to_string();
        self.doc.edit(format!("Image {label}"), |p| {
            if let Some(image) = p.scene.images.get(&image_id) {
                let result = process_image(image, op);
                p.scene.images.insert(image_id, Arc::new(result));
            }
        });
        self.ui.log("success", format!("Applied {label}"));
        self.ui.toast(format!("{label} applied"));
    }

    // ------------------------------------------------------------------- GFX

    fn gfx_add_text(&mut self) {
        let text = if self.ui.gfx_text.trim().is_empty() {
            "TEXT".to_string()
        } else {
            self.ui.gfx_text.clone()
        };
        let size = self.ui.gfx_font_size;
        self.doc.edit("Add text layer", |p| {
            let doc = p.gfx_document(1920, 1080);
            let mut spec = default_text_spec(&text, size);
            spec.position = Vec2::new(doc.width as f32 * 0.5, doc.height as f32 * 0.55);
            let id = doc.add_text_layer(text.clone(), spec);
            doc.active = Some(id);
        });
        self.ui.log("info", format!("Added text layer '{text}'"));
    }

    fn gfx_add_group(&mut self) {
        self.doc.edit("Add layer group", |p| {
            let doc = p.gfx_document(1920, 1080);
            doc.add_group("Group", None);
        });
        self.ui.log("info", "Added layer group");
    }

    fn gfx_set_blend(&mut self, mode: roedit_core::gfx::BlendMode) {
        self.doc.edit(format!("Blend {}", mode.label()), |p| {
            if let Some(doc) = p.gfx.as_mut() {
                if let Some(active) = doc.active {
                    if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == active) {
                        layer.blend = mode;
                    }
                }
            }
        });
        self.ui.log("info", format!("Blend mode: {}", mode.label()));
    }

    fn gfx_flatten(&mut self) {
        if self.doc.project.gfx.is_none() {
            self.ui
                .toast("There is no GFX document yet; add a text layer first");
            return;
        }
        let path = PathBuf::from("renders/gfx_flatten.png");
        self.job = Some(crate::preview::start_flatten(
            self.doc.project.clone(),
            path,
        ));
    }

    // ---------------------------------------------------------------- render

    fn render_still(&mut self) {
        if self.busy() {
            self.ui.toast("A render is already running");
            return;
        }
        let mut settings = self.doc.project.render.clone();
        if !self.ui.render.use_settings_resolution {
            settings.width = 1280;
            settings.height = 720;
        }
        let path = PathBuf::from(self.ui.render.output.trim());
        let camera = self
            .ui
            .camera
            .camera(settings.width as f32 / settings.height.max(1) as f32);
        let format = self.ui.render.format;
        let quality = self.ui.render.jpeg_quality;
        self.job = Some(crate::preview::start_render(
            self.doc.project.clone(),
            camera,
            settings.clone(),
            path.clone(),
            format,
            quality,
        ));
        self.ui.log(
            "info",
            format!(
                "Rendering {:.0}x{:.0} with {} samples → {}",
                settings.width,
                settings.height,
                settings.samples,
                path.display()
            ),
        );
    }

    fn quick_preview(&mut self) {
        let width = 480;
        let height = 270;
        let camera = self.ui.camera.camera(width as f32 / height as f32);
        let mode = if self.ui.view_mode == ViewMode::Geometry {
            RenderMode::Albedo
        } else {
            RenderMode::PathTrace
        };
        self.preview
            .request(self.doc.project.clone(), camera, width, height, 4, mode);
        self.last_preview_request = Instant::now();
        self.preview_dirty = false;
    }

    fn request_preview_if_needed(&mut self) {
        let debounce = if self.settings.reduce_motion {
            Duration::from_millis(900)
        } else {
            Duration::from_millis(350)
        };
        if self.preview_dirty
            && !self.preview.busy()
            && self.last_preview_request.elapsed() >= debounce
            && !self.busy()
        {
            self.quick_preview();
        }
    }

    // ----------------------------------------------------------------- video

    fn ensure_timeline(&mut self) -> &mut Timeline {
        if self.doc.project.timeline.is_none() {
            self.doc.project.timeline = Some(Timeline::new(1920, 1080, 30.0, 20.0));
        }
        let timeline = self.doc.project.timeline.as_mut().unwrap();
        if !timeline.tracks.iter().any(|t| !t.kind.is_audio()) {
            timeline
                .tracks
                .push(VideoTrack::new(VideoTrackId(0), "Main", TrackKind::Image));
        }
        timeline
    }

    fn timeline_add_color(&mut self) {
        let playhead = self.ensure_timeline().playhead;
        self.doc.edit("Add colour clip", |p| {
            let timeline = p.timeline.as_mut().unwrap();
            if let Some(track) = timeline.tracks.iter().find(|t| !t.kind.is_audio()) {
                timeline.add_clip(
                    track.id,
                    ClipSource::Color([0.12, 0.45, 0.85]),
                    playhead,
                    3.0,
                );
            }
        });
        self.ui.toast("Colour clip added");
    }

    fn timeline_add_text(&mut self) {
        let playhead = self.ensure_timeline().playhead;
        let text = self.ui.gfx_text.clone();
        let size = self.ui.gfx_font_size;
        self.doc.edit("Add text clip", |p| {
            let timeline = p.timeline.as_mut().unwrap();
            let track_id = timeline
                .tracks
                .iter()
                .find(|t| t.kind == TrackKind::Text)
                .map(|t| t.id)
                .unwrap_or_else(|| {
                    let next = timeline
                        .tracks
                        .iter()
                        .map(|t| t.id.0 + 1)
                        .max()
                        .unwrap_or(0);
                    let id = VideoTrackId(next);
                    timeline
                        .tracks
                        .push(VideoTrack::new(id, "Text", TrackKind::Text));
                    id
                });
            timeline.add_clip(
                track_id,
                ClipSource::Text(default_text_spec(&text, size)),
                playhead,
                3.0,
            );
        });
        self.ui.toast("Text clip added");
    }

    fn timeline_split(&mut self) {
        let playhead = self.ensure_timeline().playhead;
        let mut split = false;
        self.doc.edit("Split clip", |p| {
            let timeline = p.timeline.as_mut().unwrap();
            let target = timeline
                .clips
                .iter()
                .find(|c| c.contains(playhead))
                .map(|c| (c.id, c.track, c.source.clone(), c.end()));
            if let Some((id, track, source, end)) = target {
                let remaining = end - playhead;
                if let Some(clip) = timeline.clip_mut(id) {
                    clip.duration = (playhead - clip.start).max(0.01);
                }
                timeline.add_clip(track, source, playhead, remaining.max(0.01));
                split = true;
            }
        });
        if split {
            self.ui.toast("Clip split");
        } else {
            self.ui.toast("No clip under the playhead");
        }
    }

    fn timeline_delete_clip(&mut self) {
        let playhead = self
            .doc
            .project
            .timeline
            .as_ref()
            .map(|t| t.playhead)
            .unwrap_or(0.0);
        let target = self.ui.selected_clip.or_else(|| {
            self.doc
                .project
                .timeline
                .as_ref()
                .and_then(|t| t.clips.iter().find(|c| c.contains(playhead)).map(|c| c.id))
        });
        if let Some(id) = target {
            self.doc.edit("Delete clip", |p| {
                if let Some(t) = p.timeline.as_mut() {
                    t.remove_clip(id);
                }
            });
            self.ui.selected_clip = None;
            self.ui.toast("Clip deleted");
        }
    }

    fn export_sequence(&mut self) {
        if self.doc.project.timeline.is_none() {
            self.ui.toast("Add clips to the timeline before exporting");
            return;
        }
        if self.busy() {
            self.ui.toast("A job is already running");
            return;
        }
        let fps = self
            .doc
            .project
            .timeline
            .as_ref()
            .map(|t| t.fps)
            .unwrap_or(30.0);
        let opts = roedit_video::ExportOptions {
            directory: PathBuf::from(self.ui.export.directory.trim()),
            prefix: if self.ui.export.prefix.trim().is_empty() {
                "shot".into()
            } else {
                self.ui.export.prefix.clone()
            },
            format: self.ui.export.format,
            start_frame: self.ui.export.start_frame,
            end_frame: self.ui.export.end_frame,
            jpeg_quality: self.ui.export.jpeg_quality,
            fps,
            overwrite: true,
            write_audio: self.ui.export.write_audio,
        };
        self.job = Some(crate::preview::start_sequence_export(
            self.doc.project.clone(),
            opts,
        ));
        self.ui.log("info", "Sequence export started");
    }

    // ------------------------------------------------------------- animation

    fn add_keyframe(&mut self) {
        let Some(entity) = self.ui.selected else {
            self.ui.toast("Select an object to keyframe");
            return;
        };
        let Some(transform) = self
            .doc
            .project
            .scene
            .entity(entity)
            .map(|e| e.transform.clone())
        else {
            return;
        };
        let time = self.ui.anim_time;
        let mode = self.ui.anim_key_mode;
        self.doc.edit("Insert keyframe", |p| {
            let target = match mode {
                1 => Target::EntityRotation(entity),
                2 => Target::EntityScale(entity),
                _ => Target::EntityTranslation(entity),
            };
            let value = match mode {
                1 => AnimValue::Vector(transform.rotation_euler_deg()),
                2 => AnimValue::Vector(transform.scale),
                _ => AnimValue::Vector(transform.translation),
            };
            let index = p
                .animation
                .tracks
                .iter()
                .position(|t| t.target == target)
                .unwrap_or_else(|| p.animation.add_track(target));
            if let Some(track) = p.animation.tracks.get_mut(index) {
                track.insert_key(time, value);
            }
        });
        self.ui.log(
            "info",
            format!(
                "Keyframe at {time:.2}s on {}",
                match mode {
                    1 => "rotation",
                    2 => "scale",
                    _ => "translation",
                }
            ),
        );
        self.ui.toast("Keyframe inserted");
    }

    fn delete_keyframe(&mut self) {
        let time = self.ui.anim_time;
        let mut removed = false;
        self.doc.edit("Delete keyframe", |p| {
            for track in &mut p.animation.tracks {
                if track.remove_key_at(time) {
                    removed = true;
                }
            }
        });
        self.ui.toast(if removed {
            "Keyframe removed"
        } else {
            "No keyframe at the playhead"
        });
    }

    pub(crate) fn apply_animation_at(&mut self, time: f32) {
        let animation = self.doc.project.animation.clone();
        if animation.tracks.is_empty() {
            return;
        }
        let mut scene = std::mem::take(&mut self.doc.project.scene);
        for track in &animation.tracks {
            if !track.enabled || track.muted {
                continue;
            }
            let Some(value) = track.sample(time) else {
                continue;
            };
            if let AnimValue::Vector(v) = value {
                match track.target {
                    Target::EntityTranslation(id) => {
                        if let Some(e) = scene.entity_mut(id) {
                            e.transform.translation = v;
                        }
                    }
                    Target::EntityRotation(id) => {
                        if let Some(e) = scene.entity_mut(id) {
                            e.transform.set_rotation_euler_deg(v);
                        }
                    }
                    Target::EntityScale(id) => {
                        if let Some(e) = scene.entity_mut(id) {
                            e.transform.scale = v;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.doc.project.scene = scene;
    }

    // ----------------------------------------------------------------- lights

    fn add_light(&mut self, kind: LightKind) {
        let mut id = None;
        self.doc.edit(format!("Add {} light", kind.label()), |p| {
            let light_id = p.scene.add_light(kind);
            if let Some(light) = p.scene.light_mut(light_id) {
                light.position = Vec3::new(2.5, 3.5, 2.5);
                light.direction = (Vec3::ZERO - light.position).normalize_or_zero();
            }
            id = Some(light_id);
        });
        self.ui.selected_light = id;
        self.ui.log("info", format!("Added {} light", kind.label()));
        self.preview_dirty = true;
    }

    fn add_material_preset(&mut self, index: usize) {
        let presets = roedit_core::material::Material::presets();
        let Some((name, material)) = presets.get(index).cloned() else {
            return;
        };
        let mut id = None;
        self.doc.edit(format!("Add material {name}"), |p| {
            id = Some(p.scene.add_material(material));
        });
        if let Some(entity) = self.ui.selected {
            if let Some(mat) = id {
                self.doc.silent_edit(|p| {
                    if let Some(e) = p.scene.entity_mut(entity) {
                        e.material = Some(mat);
                    }
                });
            }
        }
        self.ui.selected_material = id;
        self.ui
            .log("info", format!("Material preset '{name}' added"));
    }

    // ---------------------------------------------------------------- plugins

    fn reload_plugins(&mut self) {
        self.plugins = PluginRegistry::new();
        let dir = PathBuf::from(self.ui.plugin_dir.trim());
        if dir.as_os_str().is_empty() {
            self.ui
                .toast("Set an extension folder in the Extensions workspace");
            return;
        }
        let errors = self.plugins.load_dir(&dir);
        for (path, err) in &errors {
            self.ui.log("error", format!("{}: {err}", path.display()));
        }
        self.ui.log(
            "info",
            format!(
                "Loaded {} extension pack(s), {} error(s)",
                self.plugins.packs().len(),
                errors.len()
            ),
        );
    }

    pub(crate) fn run_plugin_command(&mut self, command_id: &str) {
        let mut report = None;
        let mut error = None;
        self.doc.edit("Extension macro", |p| {
            match self.plugins.execute(command_id, p) {
                Ok(r) => report = Some(r),
                Err(e) => error = Some(e.to_string()),
            }
        });
        if let Some(r) = report {
            self.ui
                .log("success", format!("Extension: {}", r.summary()));
            self.ui.toast(r.label.clone());
            self.preview_dirty = true;
            self.fit_scene_to_view();
        }
        if let Some(e) = error {
            self.ui.log("error", format!("Extension failed: {e}"));
            self.ui.toast(format!("Extension failed: {e}"));
        }
    }

    // --------------------------------------------------------------- dispatch

    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::NewProject => self.action_new(),
            Action::OpenProject => self.action_open(),
            Action::Save => self.action_save(),
            Action::SaveAs => self.action_save_as(),
            Action::ShowImport => self.action_import(),
            Action::ShowExport => self.ui.show_export_dialog = true,
            Action::ShowSettings => self.ui.show_settings = true,
            Action::ShowShortcuts => self.ui.show_shortcuts = true,
            Action::ShowAbout => self.ui.show_about = true,
            Action::TogglePalette => {
                self.ui.palette_open = !self.ui.palette_open;
                self.ui.palette_query.clear();
                self.ui.palette_index = 0;
            }
            Action::Quit => {
                if self.doc.dirty {
                    if let Err(err) = self.doc.write_autosave() {
                        log::warn!("autosave on quit failed: {err}");
                    }
                }
                std::process::exit(0);
            }
            Action::Undo => {
                if let Some(label) = self.doc.undo() {
                    self.ui.log("info", format!("Undo {label}"));
                    self.preview_dirty = true;
                }
            }
            Action::Redo => {
                if let Some(label) = self.doc.redo() {
                    self.ui.log("info", format!("Redo {label}"));
                    self.preview_dirty = true;
                }
            }
            Action::DeleteSelected => self.delete_selected(),
            Action::DuplicateSelected => self.duplicate_selected(),
            Action::FrameSelection => {
                if let Some((center, radius)) = self.selected_entity_bounds() {
                    self.ui.camera.frame(center, radius);
                } else {
                    self.fit_scene_to_view();
                }
                self.preview_dirty = true;
            }
            Action::ResetCamera => {
                self.ui.camera = crate::state::OrbitCamera::default();
                self.preview_dirty = true;
            }
            Action::ToggleGrid => self.ui.show_grid = !self.ui.show_grid,
            Action::ToggleWireframe => {
                self.ui.show_wireframe = !self.ui.show_wireframe;
                self.ui.view_mode = if self.ui.show_wireframe {
                    ViewMode::Geometry
                } else {
                    ViewMode::Shaded
                };
                self.preview_dirty = true;
            }
            Action::ToggleStats => self.ui.show_stats = !self.ui.show_stats,
            Action::Workspace(w) => self.set_workspace(w),
            Action::Tool(t) => {
                self.ui.tool = t;
                self.ui.log("info", format!("Tool: {}", t.label()));
            }
            Action::AddPrimitive(kind) => self.add_primitive(kind),
            Action::BuildRig(rig) => self.add_rig(rig),
            Action::MeshOp(op) => self.apply_mesh_op(op, op.default_amount()),
            Action::ApplyUv(kind) => self.apply_uv(kind),
            Action::Sculpt(brush) => {
                self.ui.sculpt_brush = brush;
                self.ui.tool = Tool::Sculpt;
                self.workspace = Workspace::Sculpt;
            }
            Action::ImageOp(op) => self.apply_image_op(op),
            Action::GfxAddText => self.gfx_add_text(),
            Action::GfxAddGroup => self.gfx_add_group(),
            Action::GfxSetBlend(mode) => self.gfx_set_blend(mode),
            Action::GfxFlatten => self.gfx_flatten(),
            Action::SetRenderMode(mode) => {
                self.doc.edit("Render mode", |p| p.render.mode = mode);
                self.ui
                    .log("info", format!("Render mode: {}", mode.label()));
                self.preview_dirty = true;
            }
            Action::SetQuality(q) => {
                self.doc
                    .edit("Render quality", |p| p.render.apply_quality_preset(q));
                self.ui
                    .log("info", format!("Quality preset: {}", q.label()));
            }
            Action::RenderStill => self.render_still(),
            Action::QuickPreview => self.quick_preview(),
            Action::TimelineAddColor => self.timeline_add_color(),
            Action::TimelineAddText => self.timeline_add_text(),
            Action::TimelineSplit => self.timeline_split(),
            Action::TimelineDeleteClip => self.timeline_delete_clip(),
            Action::TimelineMarker => {
                let playhead = self
                    .doc
                    .project
                    .timeline
                    .as_ref()
                    .map(|t| t.playhead)
                    .unwrap_or(0.0);
                self.doc.edit("Add marker", |p| {
                    if let Some(t) = p.timeline.as_mut() {
                        t.add_marker(playhead, format!("Marker {:.1}s", playhead));
                    }
                });
                self.ui.toast("Marker added");
            }
            Action::PlayPause => {
                self.ui.playing = !self.ui.playing;
                self.ui.log(
                    "info",
                    if self.ui.playing {
                        "Playback started"
                    } else {
                        "Playback paused"
                    },
                );
            }
            Action::ExportSequence => self.export_sequence(),
            Action::AddKeyframe => self.add_keyframe(),
            Action::DeleteKeyframe => self.delete_keyframe(),
            Action::AddLight(kind) => self.add_light(kind),
            Action::AddPreset(index) => self.add_material_preset(index),
            Action::ReloadPlugins => self.reload_plugins(),
            Action::ClearLog => {
                self.ui.log.clear();
                self.ui.log("info", "Log cleared");
            }
        }
    }

    pub(crate) fn export_selected_image(&mut self) {
        let Some(id) = self.ui.selected_image else {
            self.ui.toast("Select an image first");
            return;
        };
        let Some(image) = self.doc.project.scene.images.get(&id).cloned() else {
            return;
        };
        let mut path = PathBuf::from(self.ui.io_path.trim());
        if path.as_os_str().is_empty() {
            path = PathBuf::from(format!("renders/image_{}.png", id.0));
        }
        if path.extension().is_none() {
            path.set_extension("png");
        }
        match roedit_io::save_image(&image, &path, roedit_io::SaveFormat::Png, 92) {
            Ok(()) => {
                self.ui.log(
                    "success",
                    format!(
                        "Wrote {}x{} image → {}",
                        image.width,
                        image.height,
                        path.display()
                    ),
                );
                self.ui.toast("Image exported");
            }
            Err(err) => {
                self.ui.log("error", format!("Image export failed: {err}"));
                self.ui.toast("Image export failed");
            }
        }
    }

    pub(crate) fn apply_pending_import(&mut self, path: &str) {
        self.ui.io_path = path.to_string();
        self.action_import();
    }

    pub(crate) fn accept_recovery(&mut self) {
        if let Some(doc) = Document::recover_autosave() {
            self.doc = doc;
            self.ui.show_recovery = false;
            self.ui.log("success", "Recovered autosaved project");
            self.fit_scene_to_view();
            self.preview_dirty = true;
        } else {
            self.ui.show_recovery = false;
        }
    }

    pub(crate) fn discard_recovery(&mut self) {
        Document::clear_autosave();
        self.ui.show_recovery = false;
        self.ui.toast("Autosave discarded");
    }
}

/// Applies an image operation to a raster image. Kept free of UI state so it can
/// be unit tested.
pub fn process_image(image: &RasterImage, op: ImageOpKind) -> RasterImage {
    match op {
        ImageOpKind::Exposure => map_rgb(image, |c| c.map(|v| (v * 1.6).clamp(0.0, 1.0))),
        ImageOpKind::Invert => map_rgb(image, |c| c.map(|v| 1.0 - v)),
        ImageOpKind::Threshold => {
            let mut out = image.clone();
            for px in out.data.chunks_exact_mut(4) {
                let l =
                    (0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32) / 255.0;
                let v = if l > 0.5 { 255 } else { 0 };
                px[0] = v;
                px[1] = v;
                px[2] = v;
            }
            out
        }
        ImageOpKind::Posterize => {
            let mut out = image.clone();
            for px in out.data.chunks_exact_mut(4) {
                for c in 0..3 {
                    px[c] = ((px[c] as u32 / 32) * 32) as u8;
                }
            }
            out
        }
        ImageOpKind::Sepia => map_rgb(image, |c| {
            [
                (0.393 * c[0] + 0.769 * c[1] + 0.189 * c[2]).min(1.0),
                (0.349 * c[0] + 0.686 * c[1] + 0.168 * c[2]).min(1.0),
                (0.272 * c[0] + 0.534 * c[1] + 0.131 * c[2]).min(1.0),
            ]
        }),
        ImageOpKind::Vignette => {
            let mut out = image.clone();
            let (w, h) = (image.width.max(1) as f32, image.height.max(1) as f32);
            for y in 0..image.height {
                for x in 0..image.width {
                    let dx = (x as f32 / w - 0.5) * 2.0;
                    let dy = (y as f32 / h - 0.5) * 2.0;
                    let d = (dx * dx + dy * dy).sqrt() / 1.414;
                    let f = (1.0 - d * 0.8).clamp(0.0, 1.0);
                    let mut px = out.get(x, y);
                    for c in 0..3 {
                        px[c] = (px[c] as f32 * f) as u8;
                    }
                    out.set(x, y, px);
                }
            }
            out
        }
        ImageOpKind::HueShift => roedit_image::adjust::hue_saturation(image, 18.0, 1.0, 0.0),
        ImageOpKind::Saturate => roedit_image::adjust::hue_saturation(image, 0.0, 1.2, 0.0),
        ImageOpKind::Contrast => map_rgb(image, |c| {
            c.map(|v| ((v - 0.5) * 1.1 + 0.5).clamp(0.0, 1.0))
        }),
        ImageOpKind::Blur => gaussian_blur(image, 2.5),
        ImageOpKind::Sharpen => {
            let blurred = gaussian_blur(image, 1.2);
            let mut out = image.clone();
            for (i, px) in out.data.chunks_exact_mut(4).enumerate() {
                for c in 0..3 {
                    let b = blurred.data[i * 4 + c] as f32;
                    let o = px[c] as f32;
                    px[c] = (o + (o - b) * 0.8).clamp(0.0, 255.0) as u8;
                }
            }
            out
        }
        ImageOpKind::Median => {
            let mut out = image.clone();
            for y in 0..image.height {
                for x in 0..image.width {
                    let mut px = image.get(x, y);
                    for c in 0..3 {
                        let mut values = [0u8; 9];
                        let mut n = 0;
                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                let sx = (x as i32 + dx).clamp(0, image.width as i32 - 1) as u32;
                                let sy = (y as i32 + dy).clamp(0, image.height as i32 - 1) as u32;
                                values[n] = image.get(sx, sy)[c];
                                n += 1;
                            }
                        }
                        values.sort_unstable();
                        px[c] = values[4];
                    }
                    out.set(x, y, px);
                }
            }
            out
        }
        ImageOpKind::ChromaKey => {
            let mut out = image.clone();
            chroma_key(
                &mut out,
                &ChromaKeySettings {
                    key_color: [0.0, 1.0, 0.0],
                    tolerance: 0.25,
                    softness: 0.1,
                    spill_suppression: 0.5,
                },
            );
            out
        }
    }
}

fn map_rgb<F: Fn([f32; 3]) -> [f32; 3]>(image: &RasterImage, f: F) -> RasterImage {
    let mut out = image.clone();
    for px in out.data.chunks_exact_mut(4) {
        let c = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ];
        let mapped = f(c);
        for i in 0..3 {
            px[i] = (mapped[i].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    out
}

impl eframe::App for RoEditApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        if !self.theme_applied {
            crate::ui::theme::apply(ctx, &self.settings);
            self.apply_ui_scale(ctx);
            self.theme_applied = true;
        }

        self.handle_shortcuts(ctx);
        self.poll_background(ctx);
        self.request_preview_if_needed();

        if self.ui.playing {
            let dt = ctx.input(|i| i.stable_dt).min(0.1);
            if let Some(timeline) = self.doc.project.timeline.as_mut() {
                timeline.playhead += dt;
                if timeline.playhead > timeline.duration.max(1.0) {
                    timeline.playhead = 0.0;
                }
                self.ui.anim_time = timeline.playhead;
                let playhead = timeline.playhead;
                if !self.preview.busy()
                    && self
                        .scratch
                        .last_video_request
                        .map(|t| t.elapsed() > Duration::from_millis(200))
                        .unwrap_or(true)
                {
                    self.scratch.last_video_request = Some(Instant::now());
                    self.scratch.video_preview_time = playhead;
                    self.preview
                        .request_video(self.doc.project.clone(), playhead, 640, 360);
                }
            }
            ctx.request_repaint();
        }

        crate::ui::top_bar(self, ctx);
        crate::ui::left_panel(self, ctx);
        crate::ui::right_panel(self, ctx);
        crate::ui::bottom_panel(self, ctx);
        crate::ui::central_panel(self, ctx);
        crate::ui::dialogs(self, ctx);
        crate::ui::palette(self, ctx);

        self.autosave_tick();
        self.ui
            .perf
            .push(frame_start.elapsed().as_secs_f32() * 1000.0);
        if self.preview.busy() || self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }

    fn on_exit(&mut self) {
        if self.doc.dirty {
            if let Err(err) = self.doc.write_autosave() {
                log::warn!("autosave on exit failed: {err}");
            }
        }
        if let Err(err) = self.settings.save() {
            log::warn!("could not save settings: {err:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_and_contrast_change_pixels() {
        let img = RasterImage::filled(4, 4, [100, 120, 140, 255]);
        let brighter = process_image(&img, ImageOpKind::Exposure);
        assert!(brighter.get(1, 1)[0] > 100);
        let contrast = process_image(&img, ImageOpKind::Contrast);
        // Values below mid-grey are pushed down by a +10% contrast curve.
        assert!(contrast.get(1, 1)[1] < 120);
        assert!(contrast.get(1, 1)[2] > 140);
    }

    #[test]
    fn invert_is_its_own_inverse() {
        let img = RasterImage::filled(2, 2, [10, 200, 77, 255]);
        let twice = process_image(
            &process_image(&img, ImageOpKind::Invert),
            ImageOpKind::Invert,
        );
        assert_eq!(twice.get(0, 0), [10, 200, 77, 255]);
    }

    #[test]
    fn sepia_shifts_towards_warm() {
        let img = RasterImage::filled(2, 2, [128, 128, 128, 255]);
        let out = process_image(&img, ImageOpKind::Sepia);
        let px = out.get(0, 0);
        assert!(px[0] > px[2], "{px:?}");
    }

    #[test]
    fn threshold_and_posterize_quantise() {
        let img = RasterImage::filled(2, 2, [30, 30, 30, 255]);
        let t = process_image(&img, ImageOpKind::Threshold);
        assert_eq!(t.get(0, 0)[0], 0);
        let p = process_image(
            &RasterImage::filled(2, 2, [70, 70, 70, 255]),
            ImageOpKind::Posterize,
        );
        assert_eq!(p.get(0, 0)[0] % 32, 0);
    }

    #[test]
    fn blur_smooths_an_edge() {
        let mut img = RasterImage::filled(8, 8, [0, 0, 0, 255]);
        for y in 0..8 {
            for x in 4..8 {
                img.set(x, y, [255, 255, 255, 255]);
            }
        }
        let out = process_image(&img, ImageOpKind::Blur);
        let edge = out.get(4, 4)[0];
        assert!(edge > 10 && edge < 245, "edge {edge} should be blended");
    }

    #[test]
    fn median_removes_a_single_hot_pixel() {
        let mut img = RasterImage::filled(5, 5, [20, 20, 20, 255]);
        img.set(2, 2, [255, 255, 255, 255]);
        let out = process_image(&img, ImageOpKind::Median);
        assert!(out.get(2, 2)[0] < 60);
    }

    #[test]
    fn chroma_key_punch_out_green() {
        let img = RasterImage::filled(4, 4, [0, 255, 0, 255]);
        let out = process_image(&img, ImageOpKind::ChromaKey);
        assert!(out.get(1, 1)[3] < 40);
    }

    #[test]
    fn vignette_darkens_corners() {
        let img = RasterImage::filled(16, 16, [200, 200, 200, 255]);
        let out = process_image(&img, ImageOpKind::Vignette);
        assert!(out.get(0, 0)[0] < out.get(8, 8)[0]);
    }

    #[test]
    fn dispatch_of_documented_actions_does_not_panic() {
        // UI-free actions that only touch in-memory state.
        let mut app = RoEditApp::bootstrap(AppSettings::default());
        app.dispatch(Action::AddPrimitive(PrimitiveKind::Cube));
        assert_eq!(app.doc.project.scene.entities.len(), 1);
        app.dispatch(Action::MeshOp(MeshOpKind::Subdivide));
        assert!(app.doc.can_undo());
        app.dispatch(Action::Undo);
        assert_eq!(app.doc.project.scene.entities.len(), 1);
        app.dispatch(Action::Workspace(Workspace::Gfx));
        assert_eq!(app.workspace, Workspace::Gfx);
        app.dispatch(Action::Tool(Tool::Paint));
        assert_eq!(app.ui.tool, Tool::Paint);
    }

    #[test]
    fn rig_building_creates_all_parts() {
        let mut app = RoEditApp::bootstrap(AppSettings::default());
        app.dispatch(Action::BuildRig(RigType::R6));
        // 6 parts + 1 rig root.
        assert_eq!(app.doc.project.scene.entities.len(), 7);
        assert_eq!(app.doc.project.scene.meshes.len(), 6);
        app.dispatch(Action::BuildRig(RigType::R15));
        assert_eq!(app.doc.project.scene.meshes.len(), 6 + 15);
    }

    #[test]
    fn timeline_actions_create_tracks_and_clips() {
        let mut app = RoEditApp::bootstrap(AppSettings::default());
        app.dispatch(Action::TimelineAddColor);
        let timeline = app.doc.project.timeline.as_ref().unwrap();
        assert_eq!(timeline.clips.len(), 1);
        assert!(timeline.tracks.iter().any(|t| !t.kind.is_audio()));
    }

    #[test]
    fn keyframes_are_recorded_and_applied() {
        let mut app = RoEditApp::bootstrap(AppSettings::default());
        app.dispatch(Action::AddPrimitive(PrimitiveKind::Cube));
        let entity = app.ui.selected.unwrap();
        app.add_keyframe();
        assert_eq!(app.doc.project.animation.tracks.len(), 1);
        app.doc.silent_edit(|p| {
            if let Some(e) = p.scene.entity_mut(entity) {
                e.transform.translation = Vec3::new(5.0, 0.0, 0.0);
            }
        });
        app.apply_animation_at(0.0);
        let pos = app
            .doc
            .project
            .scene
            .entity(entity)
            .unwrap()
            .transform
            .translation;
        assert!((pos - Vec3::new(0.0, 0.5, 0.0)).length() < 1e-3, "{pos:?}");
    }

    #[test]
    fn image_ops_require_a_selection() {
        let mut app = RoEditApp::bootstrap(AppSettings::default());
        // No image selected: should not panic and should not modify the project.
        app.dispatch(Action::ImageOp(ImageOpKind::Invert));
        assert!(app.ui.toast.is_some());
    }

    #[test]
    fn save_and_open_roundtrip_through_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = RoEditApp::bootstrap(AppSettings::default());
        app.dispatch(Action::AddPrimitive(PrimitiveKind::Sphere));
        let path = dir.path().join("roundtrip.roedit");
        app.doc.save_to(&path).unwrap();
        app.dispatch(Action::NewProject);
        assert_eq!(app.doc.project.scene.entities.len(), 0);
        app.ui.io_path = path.to_string_lossy().to_string();
        app.dispatch(Action::OpenProject);
        assert_eq!(app.doc.project.scene.entities.len(), 1);
    }
}
