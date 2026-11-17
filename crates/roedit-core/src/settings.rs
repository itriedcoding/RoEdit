//! Application settings persisted next to the user's configuration
//! (`%APPDATA%/RoEditStudio/settings.json` on Windows).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub window: [u8; 3],
    pub panel: [u8; 3],
    pub panel_alt: [u8; 3],
    pub widget: [u8; 3],
    pub widget_hover: [u8; 3],
    pub widget_active: [u8; 3],
    pub accent: [u8; 3],
    pub text: [u8; 3],
    pub text_dim: [u8; 3],
    pub border: [u8; 3],
    pub viewport: [u8; 3],
    pub grid: [u8; 3],
    pub warning: [u8; 3],
    pub error: [u8; 3],
    pub success: [u8; 3],
}

impl Theme {
    pub fn midnight() -> Self {
        Self {
            name: "Midnight".into(),
            window: [18, 19, 24],
            panel: [25, 27, 34],
            panel_alt: [30, 33, 41],
            widget: [40, 44, 55],
            widget_hover: [52, 58, 72],
            widget_active: [64, 72, 90],
            accent: [86, 156, 255],
            text: [222, 228, 240],
            text_dim: [136, 145, 163],
            border: [45, 50, 62],
            viewport: [22, 24, 30],
            grid: [58, 64, 78],
            warning: [240, 180, 80],
            error: [240, 96, 96],
            success: [110, 205, 140],
        }
    }

    pub fn graphite() -> Self {
        Self {
            name: "Graphite".into(),
            window: [28, 28, 28],
            panel: [36, 36, 36],
            panel_alt: [42, 42, 42],
            widget: [54, 54, 54],
            widget_hover: [66, 66, 66],
            widget_active: [80, 80, 80],
            accent: [240, 150, 60],
            text: [232, 232, 232],
            text_dim: [150, 150, 150],
            border: [58, 58, 58],
            viewport: [24, 24, 24],
            grid: [70, 70, 70],
            warning: [240, 190, 90],
            error: [230, 100, 100],
            success: [120, 200, 150],
        }
    }

    pub fn ocean() -> Self {
        Self {
            name: "Ocean".into(),
            window: [12, 24, 32],
            panel: [17, 32, 43],
            panel_alt: [21, 40, 53],
            widget: [30, 54, 70],
            widget_hover: [40, 70, 90],
            widget_active: [52, 88, 112],
            accent: [64, 200, 210],
            text: [214, 235, 240],
            text_dim: [130, 160, 172],
            border: [34, 62, 79],
            viewport: [14, 28, 36],
            grid: [46, 78, 94],
            warning: [240, 190, 90],
            error: [240, 110, 110],
            success: [120, 210, 160],
        }
    }

    pub fn light() -> Self {
        Self {
            name: "Studio Light".into(),
            window: [232, 234, 238],
            panel: [244, 245, 248],
            panel_alt: [236, 238, 242],
            widget: [214, 218, 226],
            widget_hover: [202, 208, 219],
            widget_active: [186, 196, 212],
            accent: [36, 110, 210],
            text: [28, 32, 40],
            text_dim: [92, 100, 115],
            border: [196, 202, 212],
            viewport: [205, 209, 216],
            grid: [150, 158, 172],
            warning: [176, 112, 20],
            error: [190, 50, 50],
            success: [30, 130, 80],
        }
    }

    pub fn high_contrast() -> Self {
        Self {
            name: "High Contrast".into(),
            window: [0, 0, 0],
            panel: [10, 10, 10],
            panel_alt: [20, 20, 20],
            widget: [32, 32, 32],
            widget_hover: [60, 60, 60],
            widget_active: [255, 255, 0],
            accent: [255, 220, 0],
            text: [255, 255, 255],
            text_dim: [210, 210, 210],
            border: [120, 120, 120],
            viewport: [0, 0, 0],
            grid: [90, 90, 90],
            warning: [255, 200, 0],
            error: [255, 90, 90],
            success: [90, 255, 150],
        }
    }

    pub fn presets() -> Vec<Theme> {
        vec![
            Self::midnight(),
            Self::graphite(),
            Self::ocean(),
            Self::light(),
            Self::high_contrast(),
        ]
    }

    pub fn find(name: &str) -> Theme {
        Self::presets()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(Self::midnight)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PanelLayout {
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    pub left_visible: bool,
    pub right_visible: bool,
    pub bottom_visible: bool,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            left_width: 260.0,
            right_width: 320.0,
            bottom_height: 240.0,
            left_visible: true,
            right_visible: true,
            bottom_visible: true,
        }
    }
}

/// Every action that can be bound to a key.
pub const ACTIONS: &[(&str, &str)] = &[
    ("File: New Project", "Ctrl+N"),
    ("File: Open Project", "Ctrl+O"),
    ("File: Save", "Ctrl+S"),
    ("File: Save As", "Ctrl+Shift+S"),
    ("File: Export Image", "Ctrl+E"),
    ("Edit: Undo", "Ctrl+Z"),
    ("Edit: Redo", "Ctrl+Y"),
    ("Edit: Duplicate", "Ctrl+D"),
    ("Edit: Delete", "Delete"),
    ("Edit: Select All", "Ctrl+A"),
    ("Transform: Move Tool", "W"),
    ("Transform: Rotate Tool", "E"),
    ("Transform: Scale Tool", "R"),
    ("Transform: Toggle Local/Global", "Alt+Space"),
    ("Model: Extrude", "Alt+E"),
    ("Model: Inset", "Alt+I"),
    ("Model: Bevel", "Alt+B"),
    ("Model: Subdivide", "Ctrl+1"),
    ("Model: Mirror", "Ctrl+M"),
    ("Mesh: Recalculate Normals", "Shift+N"),
    ("Render: Render Image", "F12"),
    ("View: Frame Selected", "F"),
    ("View: Toggle Wireframe", "Shift+Z"),
    ("View: Toggle Grid", "Shift+G"),
    ("Workspace: Command Palette", "Ctrl+K"),
    ("Workspace: Search", "Ctrl+F"),
    ("Brush: Decrease Size", "["),
    ("Brush: Increase Size", "]"),
    ("Playback: Play/Pause", "Space"),
    ("Timeline: Add Marker", "M"),
    ("Timeline: Split Clip", "Ctrl+B"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub ui_scale: f32,
    pub reduce_motion: bool,
    pub autosave_enabled: bool,
    pub autosave_interval_secs: u64,
    pub recent_projects: Vec<String>,
    pub recent_tools: Vec<String>,
    pub shortcuts: BTreeMap<String, String>,
    pub layouts: BTreeMap<String, PanelLayout>,
    pub max_undo_steps: usize,
    pub low_memory_mode: bool,
    pub viewport_quality: String,
    pub render_threads: usize,
    pub show_startup_screen: bool,
    pub last_project: Option<String>,
    pub default_workspace: String,
    pub check_updates: bool,
    pub grid_snap: f32,
    pub angle_snap_deg: f32,
    pub show_tooltips: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut shortcuts = BTreeMap::new();
        for (action, binding) in ACTIONS {
            shortcuts.insert(action.to_string(), binding.to_string());
        }
        Self {
            theme: "Midnight".into(),
            ui_scale: 1.0,
            reduce_motion: false,
            autosave_enabled: true,
            autosave_interval_secs: 120,
            recent_projects: Vec::new(),
            recent_tools: Vec::new(),
            shortcuts,
            layouts: BTreeMap::new(),
            max_undo_steps: 100,
            low_memory_mode: false,
            viewport_quality: "High".into(),
            render_threads: 0,
            show_startup_screen: true,
            last_project: None,
            default_workspace: "3D Modeling".into(),
            check_updates: false,
            grid_snap: 0.25,
            angle_snap_deg: 15.0,
            show_tooltips: true,
        }
    }
}

impl AppSettings {
    pub fn theme_obj(&self) -> Theme {
        Theme::find(&self.theme)
    }

    pub fn binding(&self, action: &str) -> Option<&str> {
        self.shortcuts.get(action).map(|s| s.as_str())
    }

    pub fn set_binding(&mut self, action: &str, binding: &str) {
        self.shortcuts
            .insert(action.to_string(), binding.to_string());
    }

    pub fn reset_shortcuts(&mut self) {
        self.shortcuts = Self::default().shortcuts;
    }

    pub fn push_recent_project(&mut self, path: &str) {
        self.recent_projects.retain(|p| p != path);
        self.recent_projects.insert(0, path.to_string());
        self.recent_projects.truncate(12);
        self.last_project = Some(path.to_string());
    }

    pub fn push_recent_tool(&mut self, tool: &str) {
        self.recent_tools.retain(|t| t != tool);
        self.recent_tools.insert(0, tool.to_string());
        self.recent_tools.truncate(10);
    }

    pub fn settings_path() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::settings_path();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<AppSettings>(&bytes) {
                Ok(mut s) => {
                    s.repair();
                    s
                }
                Err(e) => {
                    log::warn!("Settings file is invalid, using defaults: {e}");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn repair(&mut self) {
        if !(0.75..=2.0).contains(&self.ui_scale) {
            self.ui_scale = 1.0;
        }
        if self.autosave_interval_secs < 15 {
            self.autosave_interval_secs = 15;
        }
        if !(16..=2000).contains(&self.max_undo_steps) {
            self.max_undo_steps = 100;
        }
        if self.grid_snap <= 0.0 {
            self.grid_snap = 0.25;
        }
        if !(1.0..=90.0).contains(&self.angle_snap_deg) {
            self.angle_snap_deg = 15.0;
        }
        for (action, binding) in ACTIONS {
            self.shortcuts
                .entry(action.to_string())
                .or_insert_with(|| binding.to_string());
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        std::fs::rename(&tmp, &path)
    }
}

pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("RoEditStudio");
        }
        return PathBuf::from(".").join("RoEditStudio");
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("roedit-studio");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config/roedit-studio");
    }
    PathBuf::from(".roedit-studio")
}

/// Directory holding the built-in asset library and user presets.
pub fn data_dir() -> PathBuf {
    if cfg!(windows) {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("RoEditStudio");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/roedit-studio");
    }
    PathBuf::from(".roedit-data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcuts_are_complete() {
        let s = AppSettings::default();
        for (action, _) in ACTIONS {
            assert!(s.shortcuts.contains_key(*action), "missing {action}");
        }
        assert_eq!(s.binding("Edit: Undo"), Some("Ctrl+Z"));
    }

    #[test]
    fn repair_fixes_out_of_range_values() {
        let mut s = AppSettings {
            ui_scale: 12.0,
            max_undo_steps: 0,
            autosave_interval_secs: 1,
            ..Default::default()
        };
        s.repair();
        assert_eq!(s.ui_scale, 1.0);
        assert_eq!(s.max_undo_steps, 100);
        assert_eq!(s.autosave_interval_secs, 15);
    }

    #[test]
    fn recent_lists_dedupe_and_cap() {
        let mut s = AppSettings::default();
        for i in 0..20 {
            s.push_recent_project(&format!("/p/{i}.roedit"));
        }
        assert_eq!(s.recent_projects.len(), 12);
        s.push_recent_project("/p/5.roedit");
        assert_eq!(s.recent_projects[0], "/p/5.roedit");
        assert_eq!(
            s.recent_projects
                .iter()
                .filter(|p| *p == "/p/5.roedit")
                .count(),
            1
        );
    }

    #[test]
    fn themes_have_distinct_names() {
        let presets = Theme::presets();
        let names: std::collections::BTreeSet<_> = presets.iter().map(|t| t.name.clone()).collect();
        assert_eq!(names.len(), presets.len());
        assert_eq!(Theme::find("nope").name, "Midnight");
    }

    #[test]
    fn settings_survive_json_roundtrip() {
        let mut s = AppSettings {
            theme: "Ocean".into(),
            ..Default::default()
        };
        s.set_binding("Edit: Undo", "Ctrl+Shift+Z");
        let json = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.theme, "Ocean");
        assert_eq!(back.binding("Edit: Undo"), Some("Ctrl+Shift+Z"));
    }
}
