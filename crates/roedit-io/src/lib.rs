//! File formats for RoEdit Studio.
//!
//! Import/export is deliberately explicit about what is and is not supported so
//! the UI can show honest messages instead of failing silently.

pub mod gltf;
pub mod image_io;
pub mod obj;
pub mod stl;
pub mod validate;
pub mod wav;

pub use gltf::{export_glb, import_glb};
pub use image_io::{load_image, load_image_bytes, save_image, SaveFormat};
pub use obj::{export_obj, import_obj, import_obj_multi};
pub use stl::{export_stl_ascii, export_stl_binary, import_stl};
pub use validate::{check_supported, detect_kind, list_formats, FileKind, FormatSupport};
pub use wav::{read_wav, write_wav_pcm16, AudioBuffer};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported format: {0}")]
    Unsupported(String),
    #[error("invalid file: {0}")]
    Invalid(String),
    #[error("image error: {0}")]
    Image(String),
    #[error("audio error: {0}")]
    Audio(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
