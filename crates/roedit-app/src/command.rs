//! Command registry: every user-facing action, its default shortcut and the
//! category it appears under in the palette and menus.
//!
//! The registry is generated from the engine's own enumerations, so a new
//! primitive, mesh operator or blend mode automatically appears in the palette.

use crate::state::{Tool, Workspace};
use roedit_core::gfx::{BlendMode, TextSpec};
use roedit_core::material::Material;
use roedit_core::math::Vec3;
use roedit_core::mesh::Mesh;
use roedit_core::render_settings::{RenderMode, RenderQuality};
use roedit_core::scene::LightKind;
use roedit_mesh::primitives as prim;
use roedit_mesh::roblox::RigType;
use roedit_mesh::sculpt::SculptBrush;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveKind {
    Cube,
    Sphere,
    Cylinder,
    Cone,
    Torus,
    Capsule,
    Disc,
    Grid,
    Wedge,
    CornerWedge,
    RobloxBlock,
}

impl PrimitiveKind {
    pub const ALL: [PrimitiveKind; 11] = [
        PrimitiveKind::Cube,
        PrimitiveKind::Sphere,
        PrimitiveKind::Cylinder,
        PrimitiveKind::Cone,
        PrimitiveKind::Torus,
        PrimitiveKind::Capsule,
        PrimitiveKind::Disc,
        PrimitiveKind::Grid,
        PrimitiveKind::Wedge,
        PrimitiveKind::CornerWedge,
        PrimitiveKind::RobloxBlock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PrimitiveKind::Cube => "Cube",
            PrimitiveKind::Sphere => "Sphere",
            PrimitiveKind::Cylinder => "Cylinder",
            PrimitiveKind::Cone => "Cone",
            PrimitiveKind::Torus => "Torus",
            PrimitiveKind::Capsule => "Capsule",
            PrimitiveKind::Disc => "Disc",
            PrimitiveKind::Grid => "Grid Plane",
            PrimitiveKind::Wedge => "Wedge",
            PrimitiveKind::CornerWedge => "Corner Wedge",
            PrimitiveKind::RobloxBlock => "Roblox Part",
        }
    }

    pub fn build(self, size: Vec3) -> Mesh {
        let s = Vec3::new(size.x.max(0.01), size.y.max(0.01), size.z.max(0.01));
        let mut mesh = match self {
            PrimitiveKind::Cube => prim::cube(s),
            PrimitiveKind::Sphere => {
                prim::uv_sphere((s.x.min(s.y).min(s.z) * 0.5).max(0.01), 32, 16)
            }
            PrimitiveKind::Cylinder => prim::cylinder(s.x * 0.5, s.y, 32, true),
            PrimitiveKind::Cone => prim::cone(s.x * 0.5, s.y, 32),
            PrimitiveKind::Torus => prim::torus(s.x * 0.5, (s.y * 0.18).max(0.01), 40, 18),
            PrimitiveKind::Capsule => prim::capsule(s.x * 0.5, s.y, 24, 12),
            PrimitiveKind::Disc => prim::disc(s.x * 0.5, 40),
            PrimitiveKind::Grid => prim::grid_plane(s.x, 8),
            PrimitiveKind::Wedge => prim::wedge(s),
            PrimitiveKind::CornerWedge => prim::corner_wedge(s),
            PrimitiveKind::RobloxBlock => prim::roblox_part(prim::PartShape::Block, s, 8),
        };
        mesh.name = self.label().to_string();
        mesh
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshOpKind {
    Subdivide,
    Bevel,
    Extrude,
    Inset,
    Smooth,
    Decimate,
    Solidify,
    Triangulate,
    RecalcNormals,
    Weld,
    MirrorX,
    MirrorY,
    MirrorZ,
    Array,
    FlipNormals,
}

impl MeshOpKind {
    pub const ALL: [MeshOpKind; 15] = [
        MeshOpKind::Subdivide,
        MeshOpKind::Bevel,
        MeshOpKind::Extrude,
        MeshOpKind::Inset,
        MeshOpKind::Smooth,
        MeshOpKind::Decimate,
        MeshOpKind::Solidify,
        MeshOpKind::Triangulate,
        MeshOpKind::RecalcNormals,
        MeshOpKind::Weld,
        MeshOpKind::MirrorX,
        MeshOpKind::MirrorY,
        MeshOpKind::MirrorZ,
        MeshOpKind::Array,
        MeshOpKind::FlipNormals,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MeshOpKind::Subdivide => "Subdivide",
            MeshOpKind::Bevel => "Bevel",
            MeshOpKind::Extrude => "Extrude",
            MeshOpKind::Inset => "Inset",
            MeshOpKind::Smooth => "Smooth",
            MeshOpKind::Decimate => "Decimate",
            MeshOpKind::Solidify => "Solidify",
            MeshOpKind::Triangulate => "Triangulate",
            MeshOpKind::RecalcNormals => "Recalculate Normals",
            MeshOpKind::Weld => "Merge Vertices",
            MeshOpKind::MirrorX => "Mirror X",
            MeshOpKind::MirrorY => "Mirror Y",
            MeshOpKind::MirrorZ => "Mirror Z",
            MeshOpKind::Array => "Array (3)",
            MeshOpKind::FlipNormals => "Flip Normals",
        }
    }

    pub fn default_amount(self) -> f32 {
        match self {
            MeshOpKind::Subdivide => 1.0,
            MeshOpKind::Bevel => 0.05,
            MeshOpKind::Extrude => 0.1,
            MeshOpKind::Inset => 0.1,
            MeshOpKind::Smooth => 3.0,
            MeshOpKind::Decimate => 0.5,
            MeshOpKind::Solidify => 0.02,
            MeshOpKind::Weld => 0.001,
            MeshOpKind::Array => 0.5,
            _ => 0.0,
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            MeshOpKind::Subdivide => "cuts",
            MeshOpKind::Bevel => "m",
            MeshOpKind::Extrude => "m",
            MeshOpKind::Inset => "m",
            MeshOpKind::Smooth => "iterations",
            MeshOpKind::Decimate => "ratio",
            MeshOpKind::Solidify => "m",
            MeshOpKind::Weld => "m",
            MeshOpKind::Array => "spacing",
            _ => "",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UvKind {
    Box,
    Planar,
    Cylindrical,
    Spherical,
    Unwrap,
}

impl UvKind {
    pub const ALL: [UvKind; 5] = [
        UvKind::Box,
        UvKind::Planar,
        UvKind::Cylindrical,
        UvKind::Spherical,
        UvKind::Unwrap,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UvKind::Box => "Box Projection",
            UvKind::Planar => "Planar Projection",
            UvKind::Cylindrical => "Cylindrical Projection",
            UvKind::Spherical => "Spherical Projection",
            UvKind::Unwrap => "Unwrap and Pack",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOpKind {
    Exposure,
    Invert,
    Threshold,
    Posterize,
    Sepia,
    Vignette,
    HueShift,
    Saturate,
    Contrast,
    Blur,
    Sharpen,
    Median,
    ChromaKey,
}

impl ImageOpKind {
    pub const ALL: [ImageOpKind; 13] = [
        ImageOpKind::Exposure,
        ImageOpKind::Invert,
        ImageOpKind::Threshold,
        ImageOpKind::Posterize,
        ImageOpKind::Sepia,
        ImageOpKind::Vignette,
        ImageOpKind::HueShift,
        ImageOpKind::Saturate,
        ImageOpKind::Contrast,
        ImageOpKind::Blur,
        ImageOpKind::Sharpen,
        ImageOpKind::Median,
        ImageOpKind::ChromaKey,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ImageOpKind::Exposure => "Exposure",
            ImageOpKind::Invert => "Invert",
            ImageOpKind::Threshold => "Threshold",
            ImageOpKind::Posterize => "Posterize",
            ImageOpKind::Sepia => "Sepia",
            ImageOpKind::Vignette => "Vignette",
            ImageOpKind::HueShift => "Hue Shift",
            ImageOpKind::Saturate => "Saturation +20%",
            ImageOpKind::Contrast => "Contrast +10%",
            ImageOpKind::Blur => "Gaussian Blur",
            ImageOpKind::Sharpen => "Sharpen",
            ImageOpKind::Median => "Median 3×3",
            ImageOpKind::ChromaKey => "Chroma Key Green",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewProject,
    OpenProject,
    Save,
    SaveAs,
    ShowImport,
    ShowExport,
    ShowSettings,
    ShowShortcuts,
    ShowAbout,
    TogglePalette,
    Quit,

    Undo,
    Redo,
    DeleteSelected,
    DuplicateSelected,
    FrameSelection,
    ResetCamera,
    ToggleGrid,
    ToggleWireframe,
    ToggleStats,

    Workspace(Workspace),
    Tool(Tool),

    AddPrimitive(PrimitiveKind),
    BuildRig(RigType),
    MeshOp(MeshOpKind),
    ApplyUv(UvKind),
    Sculpt(SculptBrush),
    ImageOp(ImageOpKind),
    GfxAddText,
    GfxAddGroup,
    GfxSetBlend(BlendMode),
    GfxFlatten,

    SetRenderMode(RenderMode),
    SetQuality(RenderQuality),
    RenderStill,
    QuickPreview,

    TimelineAddColor,
    TimelineAddText,
    TimelineSplit,
    TimelineDeleteClip,
    TimelineMarker,
    PlayPause,
    ExportSequence,

    AddKeyframe,
    DeleteKeyframe,

    AddLight(LightKind),
    AddPreset(usize),
    ReloadPlugins,
    ClearLog,
}

#[derive(Clone, Debug)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub category: &'static str,
    pub default_key: Option<&'static str>,
    pub action: Action,
    pub detail: String,
}

impl Command {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        category: &'static str,
        default_key: Option<&'static str>,
        action: Action,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            category,
            default_key,
            action,
            detail: String::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }
}

fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '.' })
        .collect::<String>()
        .split('.')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(".")
}

/// The full command list. Built once per session.
pub fn all_commands() -> Vec<Command> {
    let mut out: Vec<Command> = Vec::new();
    let push = |out: &mut Vec<Command>,
                id: &str,
                label: &str,
                category: &'static str,
                key: Option<&'static str>,
                action: Action| {
        out.push(Command::new(id, label, category, key, action));
    };

    // File
    push(
        &mut out,
        "file.new",
        "New Project",
        "File",
        Some("Ctrl+N"),
        Action::NewProject,
    );
    push(
        &mut out,
        "file.open",
        "Open Project…",
        "File",
        Some("Ctrl+O"),
        Action::OpenProject,
    );
    push(
        &mut out,
        "file.save",
        "Save",
        "File",
        Some("Ctrl+S"),
        Action::Save,
    );
    push(
        &mut out,
        "file.save_as",
        "Save As…",
        "File",
        Some("Ctrl+Shift+S"),
        Action::SaveAs,
    );
    push(
        &mut out,
        "file.import",
        "Import File…",
        "File",
        Some("Ctrl+I"),
        Action::ShowImport,
    );
    push(
        &mut out,
        "file.export",
        "Export…",
        "File",
        Some("Ctrl+E"),
        Action::ShowExport,
    );
    push(
        &mut out,
        "file.settings",
        "Settings",
        "File",
        Some("Ctrl+,"),
        Action::ShowSettings,
    );
    push(
        &mut out,
        "help.shortcuts",
        "Keyboard Shortcuts",
        "Help",
        Some("F1"),
        Action::ShowShortcuts,
    );
    push(
        &mut out,
        "help.about",
        "About RoEdit Studio",
        "Help",
        None,
        Action::ShowAbout,
    );
    push(
        &mut out,
        "file.quit",
        "Quit",
        "File",
        Some("Ctrl+Q"),
        Action::Quit,
    );

    // Edit
    push(
        &mut out,
        "edit.undo",
        "Undo",
        "Edit",
        Some("Ctrl+Z"),
        Action::Undo,
    );
    push(
        &mut out,
        "edit.redo",
        "Redo",
        "Edit",
        Some("Ctrl+Y"),
        Action::Redo,
    );
    push(
        &mut out,
        "edit.delete",
        "Delete Selected",
        "Edit",
        Some("Delete"),
        Action::DeleteSelected,
    );
    push(
        &mut out,
        "edit.duplicate",
        "Duplicate Selected",
        "Edit",
        Some("Ctrl+D"),
        Action::DuplicateSelected,
    );
    push(
        &mut out,
        "view.frame",
        "Frame Selection",
        "View",
        Some("F"),
        Action::FrameSelection,
    );
    push(
        &mut out,
        "view.reset_camera",
        "Reset Camera",
        "View",
        Some("Home"),
        Action::ResetCamera,
    );
    push(
        &mut out,
        "view.grid",
        "Toggle Grid",
        "View",
        Some("Shift+G"),
        Action::ToggleGrid,
    );
    push(
        &mut out,
        "view.wireframe",
        "Toggle Wireframe",
        "View",
        Some("Z"),
        Action::ToggleWireframe,
    );
    push(
        &mut out,
        "view.stats",
        "Toggle Stats Overlay",
        "View",
        None,
        Action::ToggleStats,
    );
    push(
        &mut out,
        "workspace.palette",
        "Command Palette",
        "Workspace",
        Some("Ctrl+K"),
        Action::TogglePalette,
    );

    // Workspaces
    for (i, w) in Workspace::ALL.iter().enumerate() {
        // Twelve workspaces but only ten digit keys: 1-9 then 0 for Video.
        // Assets and Plugins are reachable by clicking or the palette.
        let key = match i {
            0..=8 => Some(["1", "2", "3", "4", "5", "6", "7", "8", "9"][i]),
            9 => Some("0"),
            _ => None,
        };
        out.push(
            Command::new(
                format!("workspace.{}", slug(w.label())),
                format!("Go to {}", w.label()),
                "Workspaces",
                key,
                Action::Workspace(*w),
            )
            .with_detail(w.description()),
        );
    }

    // Tools
    for (i, t) in Tool::ALL.iter().enumerate() {
        let key = match i {
            0 => Some("Q"),
            1 => Some("W"),
            2 => Some("E"),
            3 => Some("R"),
            4 => Some("T"),
            5 => Some("B"),
            6 => Some("X"),
            _ => Some("H"),
        };
        out.push(
            Command::new(
                format!("tool.{}", slug(t.label())),
                format!("{} Tool", t.label()),
                "Tools",
                key,
                Action::Tool(*t),
            )
            .with_detail(t.hint()),
        );
    }

    // Primitives
    for p in PrimitiveKind::ALL {
        out.push(Command::new(
            format!("mesh.add.{}", slug(p.label())),
            format!("Add {}", p.label()),
            "Create",
            None,
            Action::AddPrimitive(p),
        ));
    }
    for rig in RigType::ALL {
        out.push(Command::new(
            format!("rig.build.{}", slug(rig.label())),
            format!("Build Roblox {} Rig", rig.label()),
            "Create",
            None,
            Action::BuildRig(rig),
        ));
    }

    // Mesh operators
    for op in MeshOpKind::ALL {
        out.push(
            Command::new(
                format!("mesh.op.{}", slug(op.label())),
                format!("{} Selected Mesh", op.label()),
                "Mesh",
                None,
                Action::MeshOp(op),
            )
            .with_detail(format!("Default: {} {}", op.default_amount(), op.unit())),
        );
    }

    // UV
    for uv in UvKind::ALL {
        out.push(Command::new(
            format!("uv.{}", slug(uv.label())),
            uv.label(),
            "UV",
            None,
            Action::ApplyUv(uv),
        ));
    }

    // Sculpt brushes
    for brush in SculptBrush::ALL {
        out.push(Command::new(
            format!("sculpt.{}", slug(brush.label())),
            format!("{} Brush", brush.label()),
            "Sculpt",
            None,
            Action::Sculpt(brush),
        ));
    }

    // Image operations
    for op in ImageOpKind::ALL {
        out.push(Command::new(
            format!("image.{}", slug(op.label())),
            format!("Apply {}", op.label()),
            "Image",
            None,
            Action::ImageOp(op),
        ));
    }

    // GFX
    push(
        &mut out,
        "gfx.text",
        "Add Text Layer",
        "GFX",
        Some("Ctrl+T"),
        Action::GfxAddText,
    );
    push(
        &mut out,
        "gfx.group",
        "Add Layer Group",
        "GFX",
        Some("Ctrl+G"),
        Action::GfxAddGroup,
    );
    push(
        &mut out,
        "gfx.flatten",
        "Flatten and Export…",
        "GFX",
        None,
        Action::GfxFlatten,
    );
    for mode in BlendMode::ALL {
        out.push(Command::new(
            format!("gfx.blend.{}", slug(mode.label())),
            format!("Blend Mode: {}", mode.label()),
            "GFX",
            None,
            Action::GfxSetBlend(mode),
        ));
    }

    // Render
    push(
        &mut out,
        "render.still",
        "Render Still",
        "Render",
        Some("F12"),
        Action::RenderStill,
    );
    push(
        &mut out,
        "render.preview",
        "Refresh Preview",
        "Render",
        Some("F5"),
        Action::QuickPreview,
    );
    for mode in RenderMode::ALL {
        out.push(Command::new(
            format!("render.mode.{}", slug(mode.label())),
            format!("Render Mode: {}", mode.label()),
            "Render",
            None,
            Action::SetRenderMode(mode),
        ));
    }
    for q in RenderQuality::ALL {
        out.push(Command::new(
            format!("render.quality.{}", slug(q.label())),
            format!("Quality: {}", q.label()),
            "Render",
            None,
            Action::SetQuality(q),
        ));
    }

    // Video
    push(
        &mut out,
        "video.add_color",
        "Add Colour Clip",
        "Video",
        None,
        Action::TimelineAddColor,
    );
    push(
        &mut out,
        "video.add_text",
        "Add Text Clip",
        "Video",
        None,
        Action::TimelineAddText,
    );
    push(
        &mut out,
        "video.split",
        "Split Clip at Playhead",
        "Video",
        Some("Ctrl+B"),
        Action::TimelineSplit,
    );
    push(
        &mut out,
        "video.delete",
        "Delete Selected Clip",
        "Video",
        None,
        Action::TimelineDeleteClip,
    );
    push(
        &mut out,
        "video.marker",
        "Add Marker",
        "Video",
        Some("M"),
        Action::TimelineMarker,
    );
    push(
        &mut out,
        "video.play",
        "Play / Pause",
        "Video",
        Some("Space"),
        Action::PlayPause,
    );
    push(
        &mut out,
        "video.export",
        "Export Frame Sequence…",
        "Video",
        None,
        Action::ExportSequence,
    );

    // Animation
    push(
        &mut out,
        "anim.key",
        "Keyframe Selected Transform",
        "Animation",
        Some("K"),
        Action::AddKeyframe,
    );
    push(
        &mut out,
        "anim.key_delete",
        "Delete Keyframe at Playhead",
        "Animation",
        Some("Shift+K"),
        Action::DeleteKeyframe,
    );

    // Lights
    for kind in [LightKind::Point, LightKind::Spot, LightKind::Directional] {
        out.push(Command::new(
            format!("light.add.{}", slug(kind.label())),
            format!("Add {} Light", kind.label()),
            "Lighting",
            None,
            Action::AddLight(kind),
        ));
    }

    // Material presets
    for (i, (name, _)) in Material::presets().into_iter().enumerate() {
        out.push(Command::new(
            format!("material.preset.{}", slug(name)),
            format!("Material: {name}"),
            "Materials",
            None,
            Action::AddPreset(i),
        ));
    }

    push(
        &mut out,
        "plugins.reload",
        "Reload Extension Packs",
        "Extensions",
        None,
        Action::ReloadPlugins,
    );
    push(
        &mut out,
        "log.clear",
        "Clear Log",
        "Diagnostics",
        None,
        Action::ClearLog,
    );

    out
}

/// Looks up the binding for a command: a settings override keyed by command id
/// wins over the built-in default.
pub fn binding_for<'a>(
    command: &'a Command,
    settings: &'a roedit_core::settings::AppSettings,
) -> Option<&'a str> {
    settings
        .shortcuts
        .get(&command.id)
        .map(|s| s.as_str())
        .or(command.default_key)
}

fn parse_key(name: &str) -> Option<egui::Key> {
    use egui::Key;
    let upper = name.to_uppercase();
    let key = match upper.as_str() {
        "A" => Key::A,
        "B" => Key::B,
        "C" => Key::C,
        "D" => Key::D,
        "E" => Key::E,
        "F" => Key::F,
        "G" => Key::G,
        "H" => Key::H,
        "I" => Key::I,
        "J" => Key::J,
        "K" => Key::K,
        "L" => Key::L,
        "M" => Key::M,
        "N" => Key::N,
        "O" => Key::O,
        "P" => Key::P,
        "Q" => Key::Q,
        "R" => Key::R,
        "S" => Key::S,
        "T" => Key::T,
        "U" => Key::U,
        "V" => Key::V,
        "W" => Key::W,
        "X" => Key::X,
        "Y" => Key::Y,
        "Z" => Key::Z,
        "0" => Key::Num0,
        "1" => Key::Num1,
        "2" => Key::Num2,
        "3" => Key::Num3,
        "4" => Key::Num4,
        "5" => Key::Num5,
        "6" => Key::Num6,
        "7" => Key::Num7,
        "8" => Key::Num8,
        "9" => Key::Num9,
        "SPACE" => Key::Space,
        "DELETE" => Key::Delete,
        "BACKSPACE" => Key::Backspace,
        "ESCAPE" | "ESC" => Key::Escape,
        "TAB" => Key::Tab,
        "ENTER" | "RETURN" => Key::Enter,
        "HOME" => Key::Home,
        "END" => Key::End,
        "[" => Key::OpenBracket,
        "]" => Key::CloseBracket,
        "," => Key::Comma,
        "." => Key::Period,
        "-" => Key::Minus,
        "=" => Key::Equals,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        _ => return None,
    };
    Some(key)
}

/// Parsed shortcut description used for matching and for the UI.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Binding {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
}

impl Binding {
    pub fn parse(text: &str) -> Option<Binding> {
        let mut binding = Binding::default();
        let parts: Vec<&str> = text
            .split('+')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        if parts.is_empty() {
            return None;
        }
        let (key_part, modifiers) = parts.split_last().unwrap();
        for m in modifiers {
            match m.to_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" => binding.ctrl = true,
                "shift" => binding.shift = true,
                "alt" | "option" => binding.alt = true,
                _ => return None,
            }
        }
        parse_key(key_part)?;
        binding.key = key_part.to_uppercase();
        Some(binding)
    }

    pub fn matches(&self, ctx: &egui::Context) -> bool {
        let Some(key) = parse_key(&self.key) else {
            return false;
        };
        ctx.input(|i| {
            let modifier_ok = i.modifiers.command == self.ctrl
                && i.modifiers.shift == self.shift
                && i.modifiers.alt == self.alt;
            modifier_ok && i.key_pressed(key)
        })
    }

    pub fn display(&self) -> String {
        let mut out = String::new();
        if self.ctrl {
            out.push_str("Ctrl+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        out.push_str(&self.key);
        out
    }
}

/// Filters commands for the palette: every whitespace-separated term must match
/// the label, category or id (case-insensitive).
pub fn search_commands<'a>(commands: &'a [Command], query: &str) -> Vec<&'a Command> {
    let terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if terms.is_empty() {
        return commands.iter().collect();
    }
    commands
        .iter()
        .filter(|c| {
            let haystack = format!(
                "{} {} {} {}",
                c.label.to_lowercase(),
                c.category.to_lowercase(),
                c.id.to_lowercase(),
                c.detail.to_lowercase()
            );
            terms.iter().all(|t| haystack.contains(t))
        })
        .collect()
}

/// Default text card used by the GFX and video workspaces.
pub fn default_text_spec(text: &str, font_size: f32) -> TextSpec {
    TextSpec {
        text: text.to_string(),
        font_size,
        color: [1.0, 1.0, 1.0, 1.0],
        position: roedit_core::math::Vec2::new(640.0, 360.0),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_large_and_unique() {
        let commands = all_commands();
        assert!(commands.len() >= 150, "only {} commands", commands.len());
        let mut ids = std::collections::BTreeSet::new();
        for c in &commands {
            assert!(ids.insert(c.id.clone()), "duplicate id {}", c.id);
            assert!(!c.label.is_empty());
            assert!(!c.category.is_empty());
        }
    }

    #[test]
    fn every_primitive_and_operator_is_present() {
        let commands = all_commands();
        for p in PrimitiveKind::ALL {
            let id = format!("mesh.add.{}", slug(p.label()));
            assert!(commands.iter().any(|c| c.id == id), "missing {id}");
        }
        for op in MeshOpKind::ALL {
            let id = format!("mesh.op.{}", slug(op.label()));
            assert!(commands.iter().any(|c| c.id == id), "missing {id}");
        }
        for mode in BlendMode::ALL {
            let id = format!("gfx.blend.{}", slug(mode.label()));
            assert!(commands.iter().any(|c| c.id == id), "missing {id}");
        }
        for m in RenderMode::ALL {
            assert!(commands
                .iter()
                .any(|c| c.id == format!("render.mode.{}", slug(m.label()))));
        }
    }

    #[test]
    fn primitives_build_real_geometry() {
        for p in PrimitiveKind::ALL {
            let mesh = p.build(Vec3::new(1.0, 2.0, 1.0));
            assert!(!mesh.faces.is_empty(), "{} has no faces", p.label());
            assert!(!mesh.positions.is_empty());
        }
    }

    #[test]
    fn search_matches_label_and_category() {
        let commands = all_commands();
        let results = search_commands(&commands, "bevel");
        assert!(!results.is_empty());
        assert!(results
            .iter()
            .all(|c| c.label.to_lowercase().contains("bevel")));
        let render = search_commands(&commands, "render quality");
        assert!(!render.is_empty());
        assert!(search_commands(&commands, "").len() == commands.len());
        assert!(search_commands(&commands, "zzzz").is_empty());
    }

    #[test]
    fn bindings_parse_modifiers() {
        let b = Binding::parse("Ctrl+Shift+S").unwrap();
        assert!(b.ctrl && b.shift && !b.alt);
        assert_eq!(b.key, "S");
        assert_eq!(b.display(), "Ctrl+Shift+S");
        assert!(Binding::parse("Space").is_some());
        assert!(Binding::parse("Ctrl+NotAKey").is_none());
        // A modifier with no key is rejected instead of matching stray presses.
        assert!(Binding::parse("Ctrl+").is_none());
        assert!(Binding::parse("Hyper+Q").is_none());
    }

    #[test]
    fn settings_override_wins() {
        let commands = all_commands();
        let cmd = commands.iter().find(|c| c.id == "file.save").unwrap();
        let mut settings = roedit_core::settings::AppSettings::default();
        assert_eq!(binding_for(cmd, &settings), Some("Ctrl+S"));
        settings.set_binding("file.save", "Ctrl+Shift+E");
        assert_eq!(binding_for(cmd, &settings), Some("Ctrl+Shift+E"));
    }

    #[test]
    fn text_spec_defaults_are_sane() {
        let spec = default_text_spec("Hello", 48.0);
        assert_eq!(spec.text, "Hello");
        assert_eq!(spec.font_size, 48.0);
    }
}
