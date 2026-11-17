//! Data-driven extension packs for RoEdit Studio.
//!
//! A pack is a JSON manifest plus optional assets. Packs cannot run native code:
//! they declare macros built from the host's operators, material presets and
//! tips. See `docs/PLUGIN_DEVELOPMENT.md` for the format.

pub mod execute;
pub mod manifest;

pub use execute::{execute_command, MacroReport};
pub use manifest::{
    Capability, CommandSpec, MacroStep, MaterialPreset, PluginManifest, API_VERSION, MESH_OPS,
    PRIMITIVES,
};

use roedit_core::project::Project;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid pack: {}", .0.join("; "))]
    Invalid(Vec<String>),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("capability: {0}")]
    Capability(String),
    #[error("step {step} failed: {message}")]
    StepFailed { step: usize, message: String },
}

#[derive(Clone, Debug)]
pub struct PluginPack {
    pub manifest: PluginManifest,
    pub source: PathBuf,
    pub enabled: bool,
}

impl PluginPack {
    pub fn from_json(text: &str, source: impl Into<PathBuf>) -> Result<Self, PluginError> {
        let manifest: PluginManifest = serde_json::from_str(text)?;
        let problems = manifest.validate();
        if !problems.is_empty() {
            return Err(PluginError::Invalid(problems));
        }
        Ok(Self {
            manifest,
            source: source.into(),
            enabled: true,
        })
    }

    pub fn from_file(path: &Path) -> Result<Self, PluginError> {
        let text = std::fs::read_to_string(path)?;
        Self::from_json(&text, path)
    }

    /// Loads `manifest.json` from a directory, or a `.roplugin` JSON file.
    pub fn from_path(path: &Path) -> Result<Self, PluginError> {
        if path.is_dir() {
            let manifest = path.join("manifest.json");
            if !manifest.exists() {
                return Err(PluginError::NotFound(format!(
                    "{} has no manifest.json",
                    path.display()
                )));
            }
            Self::from_file(&manifest)
        } else {
            Self::from_file(path)
        }
    }

    pub fn command(&self, id: &str) -> Option<&CommandSpec> {
        self.manifest.commands.iter().find(|c| c.id == id)
    }

    pub fn display_name(&self) -> String {
        format!("{} v{}", self.manifest.name, self.manifest.version)
    }
}

#[derive(Clone, Debug, Default)]
pub struct PluginRegistry {
    packs: Vec<PluginPack>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn packs(&self) -> &[PluginPack] {
        &self.packs
    }

    /// Installs a pack, rejecting duplicate ids.
    pub fn add(&mut self, pack: PluginPack) -> Result<(), PluginError> {
        if self.packs.iter().any(|p| p.manifest.id == pack.manifest.id) {
            return Err(PluginError::Invalid(vec![format!(
                "a pack with id '{}' is already installed",
                pack.manifest.id
            )]));
        }
        self.packs.push(pack);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.packs.len();
        self.packs.retain(|p| p.manifest.id != id);
        before != self.packs.len()
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(pack) = self.packs.iter_mut().find(|p| p.manifest.id == id) {
            pack.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.packs
            .iter()
            .find(|p| p.manifest.id == id)
            .map(|p| p.enabled)
            .unwrap_or(false)
    }

    /// Loads every pack in a directory. Returns per-file errors without
    /// aborting the whole scan.
    pub fn load_dir(&mut self, dir: &Path) -> Vec<(PathBuf, PluginError)> {
        let mut errors = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(err) => {
                errors.push((dir.to_path_buf(), PluginError::Io(err)));
                return errors;
            }
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths {
            let is_candidate = path.is_dir()
                || path
                    .extension()
                    .map(|e| e == "roplugin" || e == "json")
                    .unwrap_or(false);
            if !is_candidate {
                continue;
            }
            match PluginPack::from_path(&path) {
                Ok(pack) => {
                    if let Err(err) = self.add(pack) {
                        errors.push((path, err));
                    }
                }
                Err(err) => errors.push((path.clone(), err)),
            }
        }
        errors
    }

    /// All commands from enabled packs, with the pack they belong to.
    pub fn commands(&self) -> Vec<(&PluginPack, &CommandSpec)> {
        self.packs
            .iter()
            .filter(|p| p.enabled)
            .flat_map(|p| p.manifest.commands.iter().map(move |c| (p, c)))
            .collect()
    }

    pub fn find_command(&self, id: &str) -> Option<(&PluginPack, &CommandSpec)> {
        self.packs
            .iter()
            .filter(|p| p.enabled)
            .find_map(|p| p.command(id).map(|c| (p, c)))
    }

    /// Executes a command by id. Disabled or unknown commands fail.
    pub fn execute(&self, id: &str, project: &mut Project) -> Result<MacroReport, PluginError> {
        let (pack, _) = self
            .find_command(id)
            .ok_or_else(|| PluginError::NotFound(format!("command '{id}'")))?;
        execute_command(&pack.manifest, id, project)
    }

    pub fn material_presets(&self) -> Vec<(&PluginPack, &MaterialPreset)> {
        self.packs
            .iter()
            .filter(|p| p.enabled)
            .flat_map(|p| p.manifest.material_presets.iter().map(move |m| (p, m)))
            .collect()
    }

    pub fn tips(&self) -> Vec<&str> {
        self.packs
            .iter()
            .flat_map(|p| p.manifest.tips.iter().map(|t| t.as_str()))
            .collect()
    }
}

/// A minimal example manifest, also written to `examples/plugins/` in the repo.
pub const EXAMPLE_MANIFEST: &str = r#"{
  "id": "roedit.example.lighting",
  "name": "Example Lighting Rig",
  "version": "1.0.0",
  "author": "RoEdit contributors",
  "description": "A three-point lighting setup plus a studio material, as a data-driven macro pack.",
  "api_version": 1,
  "capabilities": ["mesh", "material", "light", "environment"],
  "commands": [
    {
      "id": "roedit.example.lighting.three_point",
      "label": "Three Point Lighting",
      "category": "Lighting",
      "description": "Key, fill and rim lights around the origin with a soft studio sky.",
      "steps": [
        {"op": "add_light", "kind": "point", "position": [3.0, 4.0, 3.0], "intensity": 28.0, "color": [1.0, 0.96, 0.9]},
        {"op": "add_light", "kind": "point", "position": [-3.5, 2.5, 2.0], "intensity": 10.0, "color": [0.8, 0.86, 1.0]},
        {"op": "add_light", "kind": "spot", "position": [0.0, 5.0, -4.0], "intensity": 20.0, "color": [1.0, 1.0, 1.0]},
        {"op": "set_environment", "sun_intensity": 0.6, "sun_angle_deg": 2.0, "exposure": 1.05}
      ]
    },
    {
      "id": "roedit.example.lighting.plinth",
      "label": "Plinth With Chrome Ball",
      "category": "Props",
      "description": "Adds a bevelled plinth and a chrome sphere on top.",
      "steps": [
        {"op": "add_primitive", "primitive": "cube", "size": [3.0, 0.6, 3.0], "position": [0.0, 0.3, 0.0], "name": "Plinth"},
        {"op": "apply_mesh_op", "operator": "bevel", "amount": 0.05},
        {"op": "add_primitive", "primitive": "sphere", "size": [1.0, 1.0, 1.0], "position": [0.0, 1.12, 0.0], "name": "Chrome Ball"}
      ]
    }
  ],
  "material_presets": [
    {"name": "Studio Chrome", "color": [0.95, 0.96, 0.98], "metallic": 1.0, "roughness": 0.05},
    {"name": "Studio Concrete", "color": [0.55, 0.55, 0.53], "metallic": 0.0, "roughness": 0.85}
  ],
  "tips": [
    "Macros run through the host operators, so undo works exactly as it does for manual edits.",
    "Pack manifests are validated before installation: unknown primitives or ops are rejected."
  ]
}"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pack(dir: &Path, name: &str, id: &str) -> PathBuf {
        let pack_dir = dir.join(name);
        std::fs::create_dir_all(&pack_dir).unwrap();
        let json = EXAMPLE_MANIFEST.replace("roedit.example.lighting", id);
        std::fs::write(pack_dir.join("manifest.json"), json).unwrap();
        pack_dir
    }

    #[test]
    fn example_manifest_is_valid() {
        let pack = PluginPack::from_json(EXAMPLE_MANIFEST, "example").unwrap();
        assert_eq!(pack.manifest.commands.len(), 2);
        assert_eq!(pack.manifest.material_presets.len(), 2);
        assert!(pack
            .command("roedit.example.lighting.three_point")
            .is_some());
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(PluginPack::from_json("{not json", "x").is_err());
    }

    #[test]
    fn registry_loads_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_pack(dir.path(), "lighting", "test.lighting");
        write_pack(dir.path(), "props", "test.props");
        let mut reg = PluginRegistry::new();
        let errors = reg.load_dir(dir.path());
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(reg.packs().len(), 2);
        assert_eq!(reg.commands().len(), 4);
        assert_eq!(reg.material_presets().len(), 4);
        assert!(!reg.tips().is_empty());
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let mut reg = PluginRegistry::new();
        reg.add(PluginPack::from_json(EXAMPLE_MANIFEST, "a").unwrap())
            .unwrap();
        assert!(reg
            .add(PluginPack::from_json(EXAMPLE_MANIFEST, "b").unwrap())
            .is_err());
    }

    #[test]
    fn disabled_packs_hide_their_commands() {
        let mut reg = PluginRegistry::new();
        let pack = PluginPack::from_json(EXAMPLE_MANIFEST, "a").unwrap();
        let id = pack.manifest.id.clone();
        reg.add(pack).unwrap();
        assert_eq!(reg.commands().len(), 2);
        assert!(reg.set_enabled(&id, false));
        assert_eq!(reg.commands().len(), 0);
        assert!(reg.find_command("roedit.example.lighting.plinth").is_none());
        assert!(!reg.set_enabled("missing", true));
    }

    #[test]
    fn registry_executes_and_reports() {
        let mut reg = PluginRegistry::new();
        reg.add(PluginPack::from_json(EXAMPLE_MANIFEST, "a").unwrap())
            .unwrap();
        let mut project = Project::new("Registry");
        let report = reg
            .execute("roedit.example.lighting.plinth", &mut project)
            .unwrap();
        assert_eq!(report.entities_added, 2);
        assert_eq!(project.scene.entities.len(), 2);
        assert!(reg.execute("nope", &mut project).is_err());
    }

    #[test]
    fn removal_works() {
        let mut reg = PluginRegistry::new();
        reg.add(PluginPack::from_json(EXAMPLE_MANIFEST, "a").unwrap())
            .unwrap();
        assert!(reg.remove(&reg.packs()[0].manifest.id.clone()));
        assert!(reg.packs().is_empty());
        assert!(!reg.remove("nothing"));
    }
}
