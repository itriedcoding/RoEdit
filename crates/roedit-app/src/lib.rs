//! RoEdit Studio application: twelve workspaces over the engine crates.
//!
//! The UI is immediate-mode egui. All edits go through [`Document::edit`], so
//! undo, autosave and the dirty flag behave consistently for every tool.

pub mod app;
pub mod command;
pub mod document;
pub mod preview;
pub mod state;
pub mod ui;

pub use app::RoEditApp;
pub use command::{Action, Command, ImageOpKind, MeshOpKind, PrimitiveKind, UvKind};
pub use document::Document;
pub use state::{Tool, UiState, Workspace};
