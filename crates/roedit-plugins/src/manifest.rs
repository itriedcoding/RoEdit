//! Extension pack manifest format.
//!
//! Packs are plain JSON with no executable code: a manifest declares macros
//! (ordered steps the host executes through its own operator code), material
//! presets and tips. This keeps loading a pack safe — there is no dynamic
//! library loading and no scripting runtime.

use serde::{Deserialize, Serialize};

pub const API_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Adding primitives and mesh operators.
    Mesh,
    /// Adding materials and assigning them.
    Material,
    /// Adding lights.
    Light,
    /// Changing sky/sun/exposure.
    Environment,
    /// Changing render settings.
    Render,
    /// Adding timeline tracks and clips.
    Timeline,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Capability::Mesh => "Mesh",
            Capability::Material => "Material",
            Capability::Light => "Light",
            Capability::Environment => "Environment",
            Capability::Render => "Render",
            Capability::Timeline => "Timeline",
        }
    }
}

fn default_api() -> u32 {
    API_VERSION
}

fn default_size() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn default_roughness() -> f32 {
    0.45
}

fn default_intensity() -> f32 {
    10.0
}

fn default_white() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MaterialPreset {
    pub name: String,
    pub color: [f32; 3],
    #[serde(default)]
    pub metallic: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub emission: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MacroStep {
    AddPrimitive {
        primitive: String,
        #[serde(default)]
        position: [f32; 3],
        #[serde(default = "default_size")]
        size: [f32; 3],
        #[serde(default)]
        material: Option<String>,
        #[serde(default)]
        name: Option<String>,
    },
    AddMaterial {
        name: String,
        color: [f32; 3],
        #[serde(default)]
        metallic: f32,
        #[serde(default = "default_roughness")]
        roughness: f32,
        #[serde(default)]
        emission: f32,
    },
    AddLight {
        kind: String,
        #[serde(default)]
        position: [f32; 3],
        #[serde(default = "default_intensity")]
        intensity: f32,
        #[serde(default = "default_white")]
        color: [f32; 3],
    },
    ApplyMeshOp {
        /// Mesh operator name; see [`MESH_OPS`].
        operator: String,
        #[serde(default)]
        amount: f32,
    },
    SetEnvironment {
        #[serde(default)]
        sun_intensity: Option<f32>,
        #[serde(default)]
        sun_angle_deg: Option<f32>,
        #[serde(default)]
        sky_top: Option<[f32; 3]>,
        #[serde(default)]
        sky_horizon: Option<[f32; 3]>,
        #[serde(default)]
        exposure: Option<f32>,
    },
    SetRender {
        #[serde(default)]
        quality: Option<String>,
        #[serde(default)]
        samples: Option<u32>,
        #[serde(default)]
        max_bounces: Option<u32>,
        #[serde(default)]
        mode: Option<String>,
    },
    AddTimelineTrack {
        name: String,
        kind: String,
    },
    AddTimelineClip {
        track: String,
        color: [f32; 3],
        start: f32,
        duration: f32,
    },
}

impl MacroStep {
    pub fn label(&self) -> &'static str {
        match self {
            MacroStep::AddPrimitive { .. } => "Add primitive",
            MacroStep::AddMaterial { .. } => "Add material",
            MacroStep::AddLight { .. } => "Add light",
            MacroStep::ApplyMeshOp { .. } => "Apply mesh operator",
            MacroStep::SetEnvironment { .. } => "Set environment",
            MacroStep::SetRender { .. } => "Set render options",
            MacroStep::AddTimelineTrack { .. } => "Add timeline track",
            MacroStep::AddTimelineClip { .. } => "Add timeline clip",
        }
    }

    pub fn required_capability(&self) -> Capability {
        match self {
            MacroStep::AddPrimitive { .. } | MacroStep::ApplyMeshOp { .. } => Capability::Mesh,
            MacroStep::AddMaterial { .. } => Capability::Material,
            MacroStep::AddLight { .. } => Capability::Light,
            MacroStep::SetEnvironment { .. } => Capability::Environment,
            MacroStep::SetRender { .. } => Capability::Render,
            MacroStep::AddTimelineTrack { .. } | MacroStep::AddTimelineClip { .. } => {
                Capability::Timeline
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandSpec {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub steps: Vec<MacroStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_api")]
    pub api_version: u32,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub commands: Vec<CommandSpec>,
    #[serde(default)]
    pub material_presets: Vec<MaterialPreset>,
    #[serde(default)]
    pub tips: Vec<String>,
}

fn default_version() -> String {
    "0.1.0".into()
}

/// Primitive names a macro may request.
pub const PRIMITIVES: [&str; 11] = [
    "cube",
    "sphere",
    "cylinder",
    "cone",
    "torus",
    "capsule",
    "disc",
    "grid",
    "wedge",
    "corner_wedge",
    "roblox_part",
];

/// Mesh operators a macro may request, with their parameter semantics.
pub const MESH_OPS: [(&str, &str); 8] = [
    ("subdivide", "amount = cuts per edge (1..4)"),
    ("bevel", "amount = bevel width in metres"),
    ("extrude", "amount = extrusion distance in metres"),
    ("smooth", "amount = smoothing iterations (1..20)"),
    ("decimate", "amount = target ratio (0.05..0.95)"),
    ("solidify", "amount = shell thickness in metres"),
    ("triangulate", "amount is ignored"),
    ("recalc_normals", "amount is ignored"),
];

impl PluginManifest {
    /// Checks structure, API version, capabilities and step parameters.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.id.trim().is_empty() {
            problems.push("manifest id is empty".into());
        } else if !self.id.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-' || c == '_'
        }) {
            problems.push("manifest id must use lowercase letters, digits, '.', '-' or '_'".into());
        }
        if self.name.trim().is_empty() {
            problems.push("manifest name is empty".into());
        }
        if self.api_version != API_VERSION {
            problems.push(format!(
                "api_version {} is not supported (host API is {API_VERSION})",
                self.api_version
            ));
        }
        let mut seen_commands = std::collections::BTreeSet::new();
        for cmd in &self.commands {
            if cmd.id.trim().is_empty() {
                problems.push("a command has an empty id".into());
            }
            if !seen_commands.insert(cmd.id.clone()) {
                problems.push(format!("duplicate command id '{}'", cmd.id));
            }
            if cmd.steps.is_empty() {
                problems.push(format!("command '{}' has no steps", cmd.id));
            }
            for (i, step) in cmd.steps.iter().enumerate() {
                let needed = step.required_capability();
                if !self.capabilities.contains(&needed) {
                    problems.push(format!(
                        "command '{}' step {} ({}) needs the '{}' capability",
                        cmd.id,
                        i + 1,
                        step.label(),
                        needed.label().to_lowercase()
                    ));
                }
                problems.extend(validate_step(cmd, i, step));
            }
        }
        for preset in &self.material_presets {
            for c in preset.color.iter().chain(std::iter::once(&preset.metallic)) {
                if !(0.0..=1.0).contains(c) {
                    problems.push(format!(
                        "material preset '{}' has a value outside 0..1",
                        preset.name
                    ));
                    break;
                }
            }
        }
        problems
    }
}

fn validate_step(cmd: &CommandSpec, index: usize, step: &MacroStep) -> Vec<String> {
    let mut problems = Vec::new();
    let where_ = |msg: String| format!("command '{}' step {}: {msg}", cmd.id, index + 1);
    match step {
        MacroStep::AddPrimitive {
            primitive, size, ..
        } => {
            if !PRIMITIVES.contains(&primitive.as_str()) {
                problems.push(where_(format!(
                    "unknown primitive '{primitive}' (known: {})",
                    PRIMITIVES.join(", ")
                )));
            }
            if size.iter().any(|s| !s.is_finite() || *s <= 0.0) {
                problems.push(where_("size must be positive and finite".into()));
            }
        }
        MacroStep::ApplyMeshOp { operator, amount } => {
            match MESH_OPS.iter().find(|(name, _)| name == operator) {
                None => problems.push(where_(format!(
                    "unknown mesh op '{operator}' (known: {})",
                    MESH_OPS
                        .iter()
                        .map(|(n, _)| *n)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))),
                Some((name, _)) => match *name {
                    "subdivide" => {
                        if !(1.0..=4.0).contains(amount) {
                            problems
                                .push(where_("subdivide amount must be between 1 and 4".into()));
                        }
                    }
                    "bevel" | "extrude" | "solidify" => {
                        if *amount <= 0.0 || !amount.is_finite() {
                            problems.push(where_("amount must be positive".into()));
                        }
                    }
                    "smooth" => {
                        if !(1.0..=20.0).contains(amount) {
                            problems.push(where_("smooth amount must be between 1 and 20".into()));
                        }
                    }
                    "decimate" if !(0.05..=0.95).contains(amount) => {
                        problems.push(where_(
                            "decimate amount must be between 0.05 and 0.95".into(),
                        ));
                    }
                    _ => {}
                },
            }
        }
        MacroStep::AddLight {
            kind, intensity, ..
        } => {
            if !["point", "directional", "spot"].contains(&kind.to_lowercase().as_str()) {
                problems.push(where_(format!("unknown light kind '{kind}'")));
            }
            if *intensity < 0.0 {
                problems.push(where_("light intensity cannot be negative".into()));
            }
        }
        MacroStep::SetRender { quality, mode, .. } => {
            if let Some(q) = quality {
                if !["draft", "low", "medium", "high", "ultra"].contains(&q.to_lowercase().as_str())
                {
                    problems.push(where_(format!("unknown render quality '{q}'")));
                }
            }
            if let Some(m) = mode {
                if ![
                    "path_trace",
                    "ambient_occlusion",
                    "albedo",
                    "normals",
                    "depth",
                    "shadow_matte",
                ]
                .contains(&m.to_lowercase().as_str())
                {
                    problems.push(where_(format!("unknown render mode '{m}'")));
                }
            }
        }
        MacroStep::AddTimelineTrack { kind, .. } => {
            if !["video", "image", "text", "audio", "adjustment"]
                .contains(&kind.to_lowercase().as_str())
            {
                problems.push(where_(format!("unknown track kind '{kind}'")));
            }
        }
        MacroStep::AddTimelineClip {
            duration, start, ..
        } => {
            if *duration <= 0.0 || *start < 0.0 {
                problems.push(where_("clip timing must be positive".into()));
            }
        }
        MacroStep::AddMaterial { color, .. } => {
            if color.iter().any(|c| !(0.0..=1.0).contains(c)) {
                problems.push(where_("material colour must be within 0..1".into()));
            }
        }
        MacroStep::SetEnvironment { .. } => {}
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "id": "studio.lighting",
        "name": "Studio Lighting",
        "version": "1.0.0",
        "api_version": 1,
        "capabilities": ["mesh", "material", "light", "environment", "render", "timeline"],
        "commands": [{
            "id": "studio.lighting.three_point",
            "label": "Three Point Lighting",
            "steps": [
                {"op": "add_light", "kind": "point", "position": [3.0, 4.0, 2.0], "intensity": 30.0},
                {"op": "set_environment", "sun_intensity": 1.0, "exposure": 1.1}
            ]
        }],
        "material_presets": [{"name": "Studio Red", "color": [0.8, 0.1, 0.1], "roughness": 0.4}]
    }"#;

    #[test]
    fn sample_pack_parses_and_validates() {
        let m: PluginManifest = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(m.id, "studio.lighting");
        assert!(m.validate().is_empty(), "{:?}", m.validate());
        assert_eq!(m.commands.len(), 1);
        assert_eq!(m.commands[0].steps.len(), 2);
    }

    #[test]
    fn missing_capability_is_reported() {
        let mut m: PluginManifest = serde_json::from_str(SAMPLE).unwrap();
        m.capabilities.retain(|c| *c != Capability::Light);
        let problems = m.validate();
        assert!(
            problems.iter().any(|p| p.contains("'light' capability")),
            "{problems:?}"
        );
    }

    #[test]
    fn unknown_primitive_and_op_are_reported() {
        let mut m: PluginManifest = serde_json::from_str(SAMPLE).unwrap();
        m.commands[0].steps = vec![
            MacroStep::AddPrimitive {
                primitive: "dodecahedron".into(),
                position: [0.0; 3],
                size: [1.0; 3],
                material: None,
                name: None,
            },
            MacroStep::ApplyMeshOp {
                operator: "warp".into(),
                amount: 1.0,
            },
        ];
        let problems = m.validate();
        assert!(problems.iter().any(|p| p.contains("unknown primitive")));
        assert!(problems.iter().any(|p| p.contains("unknown mesh op")));
    }

    #[test]
    fn api_version_mismatch_is_rejected() {
        let mut m: PluginManifest = serde_json::from_str(SAMPLE).unwrap();
        m.api_version = 99;
        assert!(m.validate().iter().any(|p| p.contains("api_version")));
    }

    #[test]
    fn ids_are_validated() {
        let mut m: PluginManifest = serde_json::from_str(SAMPLE).unwrap();
        m.id = "Bad ID!".into();
        assert!(m.validate().iter().any(|p| p.contains("lowercase")));
    }
}
