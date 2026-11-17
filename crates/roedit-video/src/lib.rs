//! Video editing: timeline compositing, transitions, audio mixing and frame
//! sequence export.
//!
//! Container encoding (MP4/MOV) is **not** included in this build; sequences of
//! PNG/JPEG/TIFF/EXR frames plus a mixed WAV are written instead, and
//! [`codec::support_matrix`] states that in the UI and docs.

pub mod audio;
pub mod codec;
pub mod compositor;
pub mod export;
pub mod transitions;

pub use audio::{mix_timeline_audio, AudioMix};
pub use codec::{support_matrix, ContainerSupport};
pub use compositor::{Compositor, MediaLibrary};
pub use export::{export_sequence, ExportOptions, ExportReport};
pub use transitions::{apply_transition, transition_mask};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VideoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(String),
    #[error("nothing to export: the timeline has no visible clips")]
    Empty,
    #[error("invalid export range: {0}")]
    Range(String),
    #[error("audio error: {0}")]
    Audio(String),
}

impl From<roedit_io::ImportError> for VideoError {
    fn from(e: roedit_io::ImportError) -> Self {
        VideoError::Image(e.to_string())
    }
}
