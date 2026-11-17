//! Editor state that is not part of the saved project.

use glam::{Vec2, Vec3};
use roedit_core::camera::Camera;
use roedit_core::ids::{ClipId, EntityId, ImageId, LayerId, MaterialId, VideoTrackId};
use roedit_core::math::orbit_position;
use roedit_image::brush::{BrushMode, BrushSettings};
use roedit_io::SaveFormat;
use roedit_mesh::sculpt::SculptBrush;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Workspace {
    Model,
    Sculpt,
    Uv,
    Texture,
    Gfx,
    Rig,
    Animate,
    Materials,
    Render,
    Video,
    Assets,
    Plugins,
}

impl Workspace {
    pub const ALL: [Workspace; 12] = [
        Workspace::Model,
        Workspace::Sculpt,
        Workspace::Uv,
        Workspace::Texture,
        Workspace::Gfx,
        Workspace::Rig,
        Workspace::Animate,
        Workspace::Materials,
        Workspace::Render,
        Workspace::Video,
        Workspace::Assets,
        Workspace::Plugins,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Workspace::Model => "3D Modeling",
            Workspace::Sculpt => "Sculpting",
            Workspace::Uv => "UV Editing",
            Workspace::Texture => "Texture Paint",
            Workspace::Gfx => "GFX Studio",
            Workspace::Rig => "Rigging",
            Workspace::Animate => "Animation",
            Workspace::Materials => "Material Graph",
            Workspace::Render => "Rendering",
            Workspace::Video => "Video Editing",
            Workspace::Assets => "Asset Library",
            Workspace::Plugins => "Extensions",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Workspace::Model => "Model",
            Workspace::Sculpt => "Sculpt",
            Workspace::Uv => "UV",
            Workspace::Texture => "Texture",
            Workspace::Gfx => "GFX",
            Workspace::Rig => "Rig",
            Workspace::Animate => "Animate",
            Workspace::Materials => "Materials",
            Workspace::Render => "Render",
            Workspace::Video => "Video",
            Workspace::Assets => "Assets",
            Workspace::Plugins => "Plugins",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Workspace::Model => "Primitives, mesh operators, transforms and object hierarchy.",
            Workspace::Sculpt => {
                "Brushes that displace vertices: draw, inflate, smooth, flatten, pinch, crease."
            }
            Workspace::Uv => "Box, planar, cylindrical and spherical mapping plus shelf packing.",
            Workspace::Texture => {
                "Paint and edit project images with brushes, filters and adjustments."
            }
            Workspace::Gfx => {
                "Layer-based 2D design with blend modes, masks, text and background removal."
            }
            Workspace::Rig => "Roblox R6/R15 rig building, bone inspection and IK targets.",
            Workspace::Animate => {
                "Keyframes on entity transforms with playback and marker navigation."
            }
            Workspace::Materials => "PBR controls and procedural material node graphs.",
            Workspace::Render => {
                "Path traced stills, AOV modes, quality presets and output formats."
            }
            Workspace::Video => {
                "Multitrack timeline, transitions, audio mix and frame-sequence export."
            }
            Workspace::Assets => "Project images, imported meshes and file format support.",
            Workspace::Plugins => "Data-driven extension packs: macros, presets and tips.",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|w| *w == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Move,
    Rotate,
    Scale,
    Sculpt,
    Paint,
    Text,
    Hand,
}

impl Tool {
    pub const ALL: [Tool; 8] = [
        Tool::Select,
        Tool::Move,
        Tool::Rotate,
        Tool::Scale,
        Tool::Sculpt,
        Tool::Paint,
        Tool::Text,
        Tool::Hand,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Move => "Move",
            Tool::Rotate => "Rotate",
            Tool::Scale => "Scale",
            Tool::Sculpt => "Sculpt",
            Tool::Paint => "Paint",
            Tool::Text => "Text",
            Tool::Hand => "Hand",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "Click objects to select; drag in empty space to orbit.",
            Tool::Move => "Drag the object to move it on the ground plane.",
            Tool::Rotate => "Drag horizontally to rotate around Y.",
            Tool::Scale => "Drag vertically to scale uniformly.",
            Tool::Sculpt => "Click the surface to apply the active sculpt brush.",
            Tool::Paint => "Paint on the canvas with the active brush.",
            Tool::Text => "Click the canvas to place a text layer.",
            Tool::Hand => "Drag to pan the view.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Shaded,
    Geometry,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Shaded => "Rendered preview",
            ViewMode::Geometry => "Geometry (wireframe)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_y_deg: f32,
    pub fstop: f32,
    pub focus_distance: f32,
    pub orthographic: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 0.9, 0.0),
            yaw: 0.7,
            pitch: 0.45,
            distance: 6.0,
            fov_y_deg: 45.0,
            fstop: 0.0,
            focus_distance: 6.0,
            orthographic: false,
        }
    }
}

impl OrbitCamera {
    pub fn position(&self) -> Vec3 {
        orbit_position(self.target, self.yaw, self.pitch, self.distance)
    }

    pub fn camera(&self, _aspect: f32) -> Camera {
        Camera {
            position: self.position(),
            target: self.target,
            up: Vec3::Y,
            fov_y_deg: self.fov_y_deg,
            near: 0.05,
            far: 600.0,
            orthographic: self.orthographic,
            ortho_scale: (self.distance * 0.5).max(0.05),
            fstop: self.fstop,
            focus_distance: self.focus_distance,
            ..Default::default()
        }
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.008;
        self.pitch = (self.pitch + delta.y * 0.008).clamp(-1.5, 1.5);
    }

    pub fn pan(&mut self, delta: Vec2) {
        let c = self.camera(1.0);
        let right = c.right();
        let up = c.up_vector();
        let scale = self.distance * 0.0015;
        self.target += (-right * delta.x + up * delta.y) * scale;
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 - delta * 0.0015)).clamp(0.2, 400.0);
        self.focus_distance = self.distance;
    }

    pub fn frame(&mut self, center: Vec3, radius: f32) {
        self.target = center;
        self.distance = (radius * 3.2).clamp(0.6, 300.0);
        self.focus_distance = self.distance;
    }
}

#[derive(Clone, Debug)]
pub struct ExportState {
    pub directory: String,
    pub prefix: String,
    pub format: SaveFormat,
    pub start_frame: u32,
    pub end_frame: u32,
    pub jpeg_quality: u8,
    pub write_audio: bool,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            directory: "renders".into(),
            prefix: "shot".into(),
            format: SaveFormat::Png,
            start_frame: 0,
            end_frame: 89,
            jpeg_quality: 92,
            write_audio: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderState {
    pub output: String,
    pub format: SaveFormat,
    pub jpeg_quality: u8,
    pub use_settings_resolution: bool,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            output: "renders/still.png".into(),
            format: SaveFormat::Png,
            jpeg_quality: 92,
            use_settings_resolution: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PerfStats {
    pub frame_ms: f32,
    pub average_ms: f32,
    pub samples: Vec<f32>,
    pub last_preview_ms: f32,
    pub last_render_ms: f32,
    pub last_frame_pixels: usize,
}

impl PerfStats {
    pub fn push(&mut self, ms: f32) {
        self.frame_ms = ms;
        self.samples.push(ms);
        if self.samples.len() > 120 {
            self.samples.remove(0);
        }
        let sum: f32 = self.samples.iter().sum();
        self.average_ms = sum / self.samples.len().max(1) as f32;
    }

    pub fn fps(&self) -> f32 {
        if self.average_ms <= 0.01 {
            0.0
        } else {
            1000.0 / self.average_ms
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub time: String,
    pub level: &'static str,
    pub message: String,
}

pub struct UiState {
    pub selected: Option<EntityId>,
    pub selected_material: Option<MaterialId>,
    pub selected_image: Option<ImageId>,
    pub selected_layer: Option<LayerId>,
    pub selected_track: Option<VideoTrackId>,
    pub selected_clip: Option<ClipId>,
    pub selected_light: Option<roedit_core::ids::LightId>,
    pub selected_plugin: Option<String>,

    pub tool: Tool,
    pub view_mode: ViewMode,
    pub show_grid: bool,
    pub show_wireframe: bool,
    pub show_stats: bool,
    pub camera: OrbitCamera,

    pub brush: BrushSettings,
    pub sculpt_brush: SculptBrush,
    pub sculpt_radius: f32,
    pub sculpt_strength: f32,
    pub paint_color: [f32; 4],
    pub brush_stroke_active: bool,
    pub last_paint: Option<Vec2>,

    pub gfx_text: String,
    pub gfx_font_size: f32,
    pub gfx_blend: roedit_core::gfx::BlendMode,
    pub gfx_zoom: f32,
    pub gfx_pan: Vec2,

    pub anim_time: f32,
    pub playing: bool,
    pub anim_key_mode: usize,

    pub export: ExportState,
    pub render: RenderState,
    pub io_path: String,
    pub plugin_dir: String,
    pub asset_dir: String,

    pub palette_open: bool,
    pub palette_query: String,
    pub palette_index: usize,
    pub show_settings: bool,
    pub show_shortcuts: bool,
    pub show_about: bool,
    pub show_render_dialog: bool,
    pub show_export_dialog: bool,
    pub show_plugin_dialog: bool,
    pub show_recovery: bool,

    pub hierarchy_filter: String,
    pub asset_filter: String,
    pub palette_filter: String,
    pub log: Vec<LogLine>,
    pub perf: PerfStats,
    pub toast: Option<(String, f64)>,
    pub autosave_status: String,
    pub timeline_zoom: f32,
    pub scene_radius: f32,
    pub collapse: Collapse,
}

#[derive(Clone, Debug, Default)]
pub struct Collapse {
    pub left: bool,
    pub right: bool,
    pub bottom: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected: None,
            selected_material: None,
            selected_image: None,
            selected_layer: None,
            selected_track: None,
            selected_clip: None,
            selected_light: None,
            selected_plugin: None,
            tool: Tool::Select,
            view_mode: ViewMode::Shaded,
            show_grid: true,
            show_wireframe: false,
            show_stats: true,
            camera: OrbitCamera::default(),
            brush: BrushSettings {
                mode: BrushMode::Normal,
                color: [0.92, 0.94, 1.0, 1.0],
                ..Default::default()
            },
            sculpt_brush: SculptBrush::Draw,
            sculpt_radius: 0.35,
            sculpt_strength: 0.35,
            paint_color: [0.92, 0.94, 1.0, 1.0],
            brush_stroke_active: false,
            last_paint: None,
            gfx_text: "ROBLOX".into(),
            gfx_font_size: 96.0,
            gfx_blend: roedit_core::gfx::BlendMode::Normal,
            gfx_zoom: 1.0,
            gfx_pan: Vec2::ZERO,
            anim_time: 0.0,
            playing: false,
            anim_key_mode: 0,
            export: ExportState::default(),
            render: RenderState::default(),
            io_path: String::new(),
            plugin_dir: String::new(),
            asset_dir: String::new(),
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            show_settings: false,
            show_shortcuts: false,
            show_about: false,
            show_render_dialog: false,
            show_export_dialog: false,
            show_plugin_dialog: false,
            show_recovery: false,
            hierarchy_filter: String::new(),
            asset_filter: String::new(),
            palette_filter: String::new(),
            log: Vec::new(),
            perf: PerfStats::default(),
            toast: None,
            autosave_status: "Autosave idle".into(),
            timeline_zoom: 60.0,
            scene_radius: 2.0,
            collapse: Collapse::default(),
        }
    }
}

impl UiState {
    pub fn log(&mut self, level: &'static str, message: impl Into<String>) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let time = format!(
            "{:02}:{:02}:{:02}",
            (secs / 3600) % 24,
            (secs / 60) % 60,
            secs % 60
        );
        self.log.push(LogLine {
            time,
            level,
            message: message.into(),
        });
        if self.log.len() > 500 {
            self.log.remove(0);
        }
        log::info!("{level}: {}", self.log.last().unwrap().message);
    }

    pub fn toast(&mut self, message: impl Into<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        self.toast = Some((message.into(), now + 4.0));
    }

    pub fn active_toast(&self) -> Option<&str> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        self.toast
            .as_ref()
            .filter(|(_, until)| *until > now)
            .map(|(msg, _)| msg.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_workspaces_with_unique_labels() {
        let mut labels = std::collections::BTreeSet::new();
        for w in Workspace::ALL {
            assert!(labels.insert(w.label()));
            assert!(!w.description().is_empty());
            assert_eq!(Workspace::ALL[w.index()], w);
        }
        assert_eq!(Workspace::ALL.len(), 12);
    }

    #[test]
    fn camera_orbit_and_zoom_stay_bounded() {
        let mut cam = OrbitCamera::default();
        cam.orbit(Vec2::new(1000.0, 1000.0));
        assert!(cam.pitch <= 1.5 && cam.pitch >= -1.5);
        cam.zoom(100000.0);
        assert!(cam.distance >= 0.2);
        cam.zoom(-100000.0);
        assert!(cam.distance <= 400.0);
        let c = cam.camera(16.0 / 9.0);
        assert!(c.position.is_finite());
        assert!(c.target.is_finite());
    }

    #[test]
    fn framing_centres_and_fits() {
        let mut cam = OrbitCamera::default();
        cam.frame(Vec3::new(1.0, 2.0, 3.0), 4.0);
        assert_eq!(cam.target, Vec3::new(1.0, 2.0, 3.0));
        assert!(cam.distance > 4.0);
    }

    #[test]
    fn perf_stats_track_average_and_fps() {
        let mut p = PerfStats::default();
        for _ in 0..10 {
            p.push(10.0);
        }
        assert!((p.average_ms - 10.0).abs() < 1e-3);
        assert!((p.fps() - 100.0).abs() < 1e-2);
        for _ in 0..200 {
            p.push(20.0);
        }
        assert!(p.samples.len() <= 120);
    }

    #[test]
    fn tools_have_hints() {
        for t in Tool::ALL {
            assert!(!t.hint().is_empty());
        }
        assert_eq!(Tool::ALL.len(), 8);
    }

    #[test]
    fn log_is_capped() {
        let mut ui = UiState::default();
        for i in 0..600 {
            ui.log("info", format!("line {i}"));
        }
        assert_eq!(ui.log.len(), 500);
        assert!(ui.log.last().unwrap().message.contains("599"));
    }
}
