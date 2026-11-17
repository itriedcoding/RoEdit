//! Versioned native project format (`.roedit`).
//!
//! The document is UTF-8 JSON: readable, diff-friendly, and self-contained
//! because every raster asset is embedded as a base64 PNG.

use crate::animation::Animation;
use crate::gfx::GfxDoc;
use crate::material::Material;
use crate::nodegraph::NodeGraph;
use crate::raster::RasterImage;
use crate::render_settings::RenderSettings;
use crate::scene::Scene;
use crate::timeline::Timeline;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub const FORMAT_VERSION: u32 = 1;
pub const PROJECT_EXTENSION: &str = "roedit";

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("project file is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("project format version {found} is newer than this build supports ({supported})")]
    NewerFormat { found: u32, supported: u32 },
    #[error("file is not a RoEdit project (missing format version)")]
    NotProject,
    #[error("project is invalid: {0}")]
    Invalid(String),
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub author: String,
    pub description: String,
    pub created: u64,
    pub modified: u64,
    pub tags: Vec<String>,
    pub notes: String,
    pub thumbnail: Option<Arc<RasterImage>>,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        let now = now_secs();
        Self {
            name: "Untitled Project".into(),
            author: String::new(),
            description: String::new(),
            created: now,
            modified: now,
            tags: Vec::new(),
            notes: String::new(),
            thumbnail: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphKind {
    Material,
    Texture,
    Compositing,
    Geometry,
    Animation,
    Vfx,
}

impl GraphKind {
    pub const ALL: [GraphKind; 6] = [
        GraphKind::Material,
        GraphKind::Texture,
        GraphKind::Compositing,
        GraphKind::Geometry,
        GraphKind::Animation,
        GraphKind::Vfx,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GraphKind::Material => "Material",
            GraphKind::Texture => "Texture",
            GraphKind::Compositing => "Compositing",
            GraphKind::Geometry => "Geometry",
            GraphKind::Animation => "Animation",
            GraphKind::Vfx => "Visual Effects",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamedGraph {
    pub name: String,
    pub kind: GraphKind,
    pub graph: NodeGraph,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub format_version: u32,
    pub app_version: String,
    pub meta: ProjectMeta,
    pub scene: Scene,
    pub animation: Animation,
    pub gfx: Option<GfxDoc>,
    pub timeline: Option<Timeline>,
    pub render: RenderSettings,
    pub node_graphs: Vec<NamedGraph>,
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled Project")
    }
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut scene = Scene::new(format!("{name} Scene"));
        let plastic = Material::new("Roblox Plastic", [0.72, 0.18, 0.16], 0.0, 0.55);
        scene.add_material(plastic);
        scene.add_light(crate::scene::LightKind::Directional);
        let meta = ProjectMeta {
            name: name.clone(),
            ..Default::default()
        };
        Self {
            format_version: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            meta,
            scene,
            animation: Animation::default(),
            gfx: None,
            timeline: None,
            render: RenderSettings::default(),
            node_graphs: Vec::new(),
        }
    }

    pub fn gfx_document(&mut self, width: u32, height: u32) -> &mut GfxDoc {
        if self.gfx.is_none() {
            self.gfx = Some(GfxDoc::new(width, height));
        }
        self.gfx.as_mut().unwrap()
    }

    pub fn timeline_document(&mut self) -> &mut Timeline {
        if self.timeline.is_none() {
            self.timeline = Some(Timeline::default());
        }
        self.timeline.as_mut().unwrap()
    }

    pub fn touch(&mut self) {
        self.meta.modified = now_secs();
    }

    pub fn snapshot(&self) -> Project {
        self.clone()
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, ProjectError> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn to_json_string_pretty(&self) -> Result<String, ProjectError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Project, ProjectError> {
        let probe: serde_json::Value = serde_json::from_slice(bytes)?;
        let version = probe
            .get("format_version")
            .and_then(|v| v.as_u64())
            .ok_or(ProjectError::NotProject)? as u32;
        if version > FORMAT_VERSION {
            return Err(ProjectError::NewerFormat {
                found: version,
                supported: FORMAT_VERSION,
            });
        }
        let mut project: Project = serde_json::from_slice(bytes)?;
        let loaded_version = project.format_version;
        if loaded_version < FORMAT_VERSION {
            migrate(&mut project, loaded_version)?;
        }
        project.repair();
        Ok(project)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<u64, ProjectError> {
        let bytes = self.to_json_bytes()?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let tmp = path.with_extension(format!("{PROJECT_EXTENSION}.tmp"));
        std::fs::write(&tmp, &bytes)?;
        // Windows cannot rename over an existing file.
        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(bytes.len() as u64)
    }

    pub fn load_from_path(path: &Path) -> Result<Project, ProjectError> {
        let bytes = std::fs::read(path)?;
        Project::from_json_bytes(&bytes)
    }

    pub fn repair(&mut self) {
        self.scene.repair_counters();
        self.animation.repair_counters();
        if let Some(g) = &mut self.gfx {
            g.repair_counters();
        }
        if let Some(t) = &mut self.timeline {
            t.repair_counters();
        }
        for g in &mut self.node_graphs {
            g.graph.repair_counters();
        }
        if self.format_version == 0 {
            self.format_version = FORMAT_VERSION;
        }
        self.app_version = env!("CARGO_PKG_VERSION").to_string();
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.format_version > FORMAT_VERSION {
            problems.push(format!(
                "Project format version {} is newer than this build supports",
                self.format_version
            ));
        }
        problems.extend(self.scene.validate());
        if let Some(g) = &self.gfx {
            problems.extend(g.validate());
        }
        if let Some(t) = &self.timeline {
            problems.extend(t.validate());
        }
        for g in &self.node_graphs {
            for p in g.graph.validate() {
                problems.push(format!("[{}] {}", g.name, p));
            }
        }
        if self.meta.name.trim().is_empty() {
            problems.push("Project name is empty".into());
        }
        // Dangling animation targets.
        for track in &self.animation.tracks {
            use crate::animation::Target::*;
            match &track.target {
                EntityTranslation(e) | EntityRotation(e) | EntityScale(e) => {
                    if !self.scene.entities.contains_key(e) {
                        problems.push(format!("Track '{}' targets a deleted entity", track.name));
                    }
                }
                Material(m, _) => {
                    if !self.scene.materials.contains_key(m) {
                        problems.push(format!("Track '{}' targets a deleted material", track.name));
                    }
                }
                LightIntensity(l) | LightColor(l) => {
                    if !self.scene.lights.contains_key(l) {
                        problems.push(format!("Track '{}' targets a deleted light", track.name));
                    }
                }
                ClipOpacity(c) | ClipVolume(c) | ClipSpeed(c) => {
                    let exists = self
                        .timeline
                        .as_ref()
                        .map(|t| t.clips.iter().any(|cl| cl.id == *c))
                        .unwrap_or(false);
                    if !exists {
                        problems.push(format!("Track '{}' targets a deleted clip", track.name));
                    }
                }
                _ => {}
            }
        }
        problems.extend(self.render.validate());
        problems
    }

    /// Where the autosave file for a project lives.
    pub fn autosave_path(path: &Path) -> PathBuf {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".into());
        let dir = path.parent().unwrap_or(Path::new("."));
        dir.join(format!(".{name}.{PROJECT_EXTENSION}.autosave"))
    }

    pub fn estimated_json_size(&self) -> usize {
        self.to_json_bytes().map(|b| b.len()).unwrap_or(0)
    }
}

fn migrate(project: &mut Project, from: u32) -> Result<(), ProjectError> {
    // Only version 1 exists so far; keep the door open for additive migrations.
    let mut version = from;
    while version < FORMAT_VERSION {
        match version {
            0 => {
                project.meta.tags.clear();
                version = 1;
            }
            _ => {
                return Err(ProjectError::Invalid(format!(
                    "no migration path from v{version}"
                )))
            }
        }
    }
    project.format_version = FORMAT_VERSION;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::GfxDoc;
    use crate::timeline::Timeline;

    fn sample_project() -> Project {
        let mut p = Project::new("Sample");
        let mesh = crate::mesh::Mesh::new("Plane");
        let mid = p.scene.add_mesh(mesh);
        let e = p.scene.add_entity("Plane", None);
        p.scene.entity_mut(e).unwrap().mesh = Some(mid);
        p.animation
            .add_track(crate::animation::Target::EntityTranslation(e));
        p.gfx = Some(GfxDoc::new(320, 180));
        p.gfx.as_mut().unwrap().add_empty_layer("Paint");
        p.timeline = Some(Timeline::new(1920, 1080, 30.0, 5.0));
        p
    }

    #[test]
    fn json_roundtrip_preserves_document() {
        let p = sample_project();
        let bytes = p.to_json_bytes().unwrap();
        let back = Project::from_json_bytes(&bytes).unwrap();
        assert_eq!(back.meta.name, "Sample");
        assert_eq!(back.scene.entities.len(), 1);
        assert_eq!(back.scene.meshes.len(), 1);
        assert_eq!(back.animation.tracks.len(), 1);
        assert_eq!(back.gfx.as_ref().unwrap().layers.len(), 1);
        assert_eq!(back.timeline.as_ref().unwrap().width, 1920);
        assert!(back.validate().is_empty(), "{:?}", back.validate());
    }

    #[test]
    fn embedded_images_survive_roundtrip() {
        let mut p = Project::new("Img");
        let img = RasterImage::filled(16, 16, [10, 200, 30, 255]);
        let id = p.scene.add_image(img);
        let m = p.scene.add_material(Material {
            albedo_map: Some(id),
            ..Default::default()
        });
        let e = p.scene.add_entity("Mapped", None);
        p.scene.entity_mut(e).unwrap().material = Some(m);
        let bytes = p.to_json_bytes().unwrap();
        let back = Project::from_json_bytes(&bytes).unwrap();
        assert_eq!(back.scene.image(id).unwrap().get(3, 3), [10, 200, 30, 255]);
    }

    #[test]
    fn save_and_load_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.roedit");
        let p = sample_project();
        let size = p.save_to_path(&path).unwrap();
        assert!(size > 0);
        assert!(path.exists());
        let back = Project::load_from_path(&path).unwrap();
        assert_eq!(back.scene.name, "Sample Scene");
        // overwriting an existing project works on every platform
        p.save_to_path(&path).unwrap();
    }

    #[test]
    fn newer_format_is_rejected() {
        let p = sample_project();
        let mut v: serde_json::Value = serde_json::from_slice(&p.to_json_bytes().unwrap()).unwrap();
        v["format_version"] = serde_json::json!(999);
        let err = Project::from_json_bytes(&serde_json::to_vec(&v).unwrap()).unwrap_err();
        assert!(matches!(err, ProjectError::NewerFormat { .. }));
    }

    #[test]
    fn non_project_file_is_rejected() {
        let err = Project::from_json_bytes(br#"{"hello": true}"#).unwrap_err();
        assert!(matches!(err, ProjectError::NotProject));
    }

    #[test]
    fn dangling_targets_are_reported() {
        let mut p = Project::new("X");
        p.animation
            .add_track(crate::animation::Target::EntityTranslation(
                crate::ids::EntityId(999),
            ));
        let problems = p.validate();
        assert!(problems.iter().any(|p| p.contains("deleted entity")));
    }

    #[test]
    fn autosave_path_is_hidden_sibling() {
        let p = Project::autosave_path(Path::new("/tmp/demo.roedit"));
        assert!(p.to_string_lossy().contains(".demo.roedit.autosave"));
    }
}
