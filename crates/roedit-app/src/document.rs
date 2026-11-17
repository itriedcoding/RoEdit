//! Document wrapper: project + undo history + file path + autosave.

use roedit_core::history::History;
use roedit_core::project::Project;
use roedit_core::settings::AppSettings;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Document {
    pub project: Project,
    pub history: History<Project>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub last_saved: Option<f64>,
    /// Path of the file this document was recovered from, if any.
    pub recovered_from: Option<PathBuf>,
    pub last_action: Option<String>,
}

impl Document {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            history: History::new(100),
            path: None,
            dirty: false,
            last_saved: None,
            recovered_from: None,
            last_action: None,
        }
    }

    pub fn untitled() -> Self {
        Self::new(Project::new("Untitled Project"))
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.roedit", self.project.meta.name));
        if self.dirty {
            format!("• {name}")
        } else {
            name
        }
    }

    /// Records a snapshot, then mutates the project. Use for every user edit so
    /// undo and the dirty flag stay in sync.
    pub fn edit<F>(&mut self, label: impl Into<String>, f: F)
    where
        F: FnOnce(&mut Project),
    {
        let label = label.into();
        self.history.record(label.clone(), self.project.snapshot());
        f(&mut self.project);
        self.project.touch();
        self.dirty = true;
        self.last_action = Some(label);
    }

    /// Like `edit` but skips snapshotting for continuous drags; the caller must
    /// record once when the drag starts.
    pub fn silent_edit<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Project),
    {
        f(&mut self.project);
        self.project.touch();
        self.dirty = true;
    }

    pub fn undo(&mut self) -> Option<String> {
        let current = self.project.clone();
        let (label, state) = self.history.undo(&current)?;
        self.project = state;
        self.dirty = true;
        self.last_action = Some(format!("Undo {label}"));
        Some(label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let current = self.project.clone();
        let (label, state) = self.history.redo(&current)?;
        self.project = state;
        self.dirty = true;
        self.last_action = Some(format!("Redo {label}"));
        Some(label)
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo_label()
    }

    pub fn set_history_limit(&mut self, limit: usize) {
        self.history = History::new(limit);
    }

    pub fn save_to(&mut self, path: &Path) -> Result<u64, String> {
        let bytes = self
            .project
            .save_to_path(path)
            .map_err(|e| format!("could not save {}: {e}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        self.last_saved = Some(now_unix());
        self.recovered_from = None;
        Ok(bytes)
    }

    pub fn save(&mut self) -> Result<u64, String> {
        let Some(path) = self.path.clone() else {
            return Err("the project has no file path yet; use Save As".into());
        };
        self.save_to(&path)
    }

    pub fn load(path: &Path) -> Result<Document, String> {
        let project = Project::load_from_path(path)
            .map_err(|e| format!("could not open {}: {e}", path.display()))?;
        let mut doc = Document::new(project);
        doc.path = Some(path.to_path_buf());
        doc.last_saved = Some(now_unix());
        Ok(doc)
    }

    /// Settings directory used for autosave and settings (same folder).
    pub fn config_dir() -> PathBuf {
        AppSettings::settings_path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn autosave_path() -> PathBuf {
        Self::config_dir().join("autosave.roedit")
    }

    /// Writes the autosave file; returns its path and size.
    pub fn write_autosave(&self) -> Result<(PathBuf, u64), String> {
        let path = Self::autosave_path();
        let bytes = self
            .project
            .save_to_path(&path)
            .map_err(|e| format!("autosave failed: {e}"))?;
        Ok((path, bytes))
    }

    pub fn autosave_exists() -> bool {
        Self::autosave_path().exists()
    }

    /// Removes the autosave file (after a successful save or explicit discard).
    pub fn clear_autosave() {
        let path = Self::autosave_path();
        if path.exists() {
            if let Err(err) = std::fs::remove_file(&path) {
                log::warn!("could not remove autosave {}: {err}", path.display());
            }
        }
    }

    /// Loads the autosave as a recovered document.
    pub fn recover_autosave() -> Option<Document> {
        let path = Self::autosave_path();
        if !path.exists() {
            return None;
        }
        match Project::load_from_path(&path) {
            Ok(project) => {
                let mut doc = Document::new(project);
                doc.dirty = true;
                doc.recovered_from = Some(path);
                Some(doc)
            }
            Err(err) => {
                log::warn!("autosave could not be read: {err}");
                None
            }
        }
    }

    pub fn stats_line(&self) -> String {
        let scene = self.project.scene.statistics();
        format!(
            "{} objects · {} meshes · {} verts · {} tris",
            scene.entities, scene.unique_meshes, scene.vertices, scene.triangles
        )
    }
}

fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::math::Vec3;
    use roedit_mesh::primitives;

    #[test]
    fn edit_records_history_and_dirty() {
        let mut doc = Document::untitled();
        assert!(!doc.dirty);
        doc.edit("Add cube", |p| {
            let mesh = primitives::cube(Vec3::ONE);
            p.scene.add_mesh(mesh);
        });
        assert!(doc.dirty);
        assert!(doc.can_undo());
        let before = doc.project.scene.meshes.len();
        doc.undo();
        assert_eq!(doc.project.scene.meshes.len(), before - 1);
        doc.redo();
        assert_eq!(doc.project.scene.meshes.len(), before);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.roedit");
        let mut doc = Document::untitled();
        doc.edit("Add sphere", |p| {
            let mesh = primitives::uv_sphere(1.0, 16, 8);
            p.scene.add_mesh(mesh);
        });
        let bytes = doc.save_to(&path).unwrap();
        assert!(bytes > 100);
        assert!(!doc.dirty);
        assert_eq!(doc.path.as_deref(), Some(path.as_path()));
        let loaded = Document::load(&path).unwrap();
        assert_eq!(
            loaded.project.scene.meshes.len(),
            doc.project.scene.meshes.len()
        );
        assert_eq!(loaded.title(), "project.roedit");
    }

    #[test]
    fn save_without_path_is_an_error() {
        let mut doc = Document::untitled();
        assert!(doc.save().is_err());
    }

    #[test]
    fn title_marks_unsaved_changes() {
        let mut doc = Document::untitled();
        let clean = doc.title();
        doc.edit("x", |_| {});
        assert!(doc.title().starts_with('•'));
        assert!(!clean.starts_with('•'));
    }

    #[test]
    fn autosave_path_lives_in_config_dir() {
        let path = Document::autosave_path();
        assert_eq!(path.file_name().unwrap(), "autosave.roedit");
        assert!(path.starts_with(Document::config_dir()));
    }

    #[test]
    fn stats_describe_the_scene() {
        let doc = Document::untitled();
        let line = doc.stats_line();
        assert!(line.contains("objects"));
        assert!(line.contains("tris"));
    }

    #[test]
    fn recovering_a_missing_autosave_returns_none() {
        // Do not delete a real autosave that might belong to the user: only
        // assert the negative case when no file exists.
        if !Document::autosave_exists() {
            assert!(Document::recover_autosave().is_none());
        }
    }
}
