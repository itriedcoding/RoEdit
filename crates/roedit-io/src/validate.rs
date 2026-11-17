//! Format detection and an explicit support matrix.

use crate::ImportError;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Obj,
    Stl,
    Glb,
    Gltf,
    Fbx,
    Png,
    Jpeg,
    Tiff,
    Bmp,
    Webp,
    Exr,
    Svg,
    Wav,
    Mp3,
    Mp4,
    Mov,
    Unknown,
}

impl FileKind {
    pub fn label(self) -> &'static str {
        match self {
            FileKind::Obj => "Wavefront OBJ",
            FileKind::Stl => "STL",
            FileKind::Glb => "glTF Binary (GLB)",
            FileKind::Gltf => "glTF JSON",
            FileKind::Fbx => "FBX",
            FileKind::Png => "PNG",
            FileKind::Jpeg => "JPEG",
            FileKind::Tiff => "TIFF",
            FileKind::Bmp => "BMP",
            FileKind::Webp => "WebP",
            FileKind::Exr => "OpenEXR",
            FileKind::Svg => "SVG",
            FileKind::Wav => "WAV",
            FileKind::Mp3 => "MP3",
            FileKind::Mp4 => "MP4",
            FileKind::Mov => "QuickTime MOV",
            FileKind::Unknown => "Unknown",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            FileKind::Obj => "obj",
            FileKind::Stl => "stl",
            FileKind::Glb => "glb",
            FileKind::Gltf => "gltf",
            FileKind::Fbx => "fbx",
            FileKind::Png => "png",
            FileKind::Jpeg => "jpg",
            FileKind::Tiff => "tif",
            FileKind::Bmp => "bmp",
            FileKind::Webp => "webp",
            FileKind::Exr => "exr",
            FileKind::Svg => "svg",
            FileKind::Wav => "wav",
            FileKind::Mp3 => "mp3",
            FileKind::Mp4 => "mp4",
            FileKind::Mov => "mov",
            FileKind::Unknown => "",
        }
    }
}

/// Detects a format from the file extension and magic bytes.
pub fn detect_kind(path: Option<&Path>, bytes: &[u8]) -> FileKind {
    if bytes.len() >= 12 {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            return FileKind::Png;
        }
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return FileKind::Jpeg;
        }
        if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
            return FileKind::Tiff;
        }
        if bytes.starts_with(b"glTF") {
            return FileKind::Glb;
        }
        if bytes.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
            return FileKind::Exr;
        }
        if bytes.starts_with(b"BM") {
            return FileKind::Bmp;
        }
        if bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE" {
            return FileKind::Wav;
        }
        if bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            return FileKind::Webp;
        }
        if bytes.starts_with(b"ID3") {
            return FileKind::Mp3;
        }
        if &bytes[4..8] == b"ftyp" {
            let brand = &bytes[8..12];
            if brand == b"qt  " {
                return FileKind::Mov;
            }
            return FileKind::Mp4;
        }
        if bytes.starts_with(b"Kaydara FBX") || bytes.starts_with(b"; FBX") {
            return FileKind::Fbx;
        }
    }
    if let Some(path) = path {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        return match ext.as_str() {
            "obj" => FileKind::Obj,
            "stl" => FileKind::Stl,
            "glb" => FileKind::Glb,
            "gltf" => FileKind::Gltf,
            "fbx" => FileKind::Fbx,
            "png" => FileKind::Png,
            "jpg" | "jpeg" => FileKind::Jpeg,
            "tif" | "tiff" => FileKind::Tiff,
            "bmp" => FileKind::Bmp,
            "webp" => FileKind::Webp,
            "exr" => FileKind::Exr,
            "svg" => FileKind::Svg,
            "wav" => FileKind::Wav,
            "mp3" => FileKind::Mp3,
            "mp4" => FileKind::Mp4,
            "mov" => FileKind::Mov,
            _ => FileKind::Unknown,
        };
    }
    FileKind::Unknown
}

#[derive(Clone, Debug)]
pub struct FormatSupport {
    pub kind: FileKind,
    pub supported: bool,
    pub message: String,
}

/// Explains whether a format can be imported, and why not when it cannot.
pub fn check_supported(kind: FileKind) -> FormatSupport {
    let (supported, message) = match kind {
        FileKind::Obj => (true, "Meshes with UVs and groups (models without UVs are accepted)".into()),
        FileKind::Stl => (true, "Binary and ASCII STL (geometry only, no UVs)".into()),
        FileKind::Glb => (true, "glTF 2.0 binary with positions, normals, UVs and node transforms".into()),
        FileKind::Gltf => (
            false,
            "glTF JSON files reference external buffers; export or convert to .glb and import that instead".into(),
        ),
        FileKind::Fbx => (
            false,
            "FBX parsing is not available in this build. Import OBJ/GLB, or convert the FBX first (Roblox Studio can export OBJ)".into(),
        ),
        FileKind::Png | FileKind::Jpeg | FileKind::Tiff | FileKind::Bmp | FileKind::Webp => {
            (true, "Raster images are imported as 8-bit RGBA".into())
        }
        FileKind::Exr => (true, "OpenEXR images are converted from linear float to RGBA8".into()),
        FileKind::Svg => (
            false,
            "SVG rasterisation is not available in this build; export a PNG from your vector editor instead".into(),
        ),
        FileKind::Wav => (true, "PCM/float WAV audio up to 32-bit, any channel count".into()),
        FileKind::Mp3 => (
            false,
            "MP3 decoding is not available in this build; convert to WAV first".into(),
        ),
        FileKind::Mp4 => (
            false,
            "MP4 video decoding is not available in this build. Use image sequences or WAV audio for video editing".into(),
        ),
        FileKind::Mov => (
            false,
            "QuickTime decoding is not available in this build. Use image sequences or WAV audio for video editing".into(),
        ),
        FileKind::Unknown => (false, "The file type could not be recognised".into()),
    };
    FormatSupport {
        kind,
        supported,
        message,
    }
}

/// Full support matrix for the import dialog and the docs.
pub fn list_formats() -> Vec<FormatSupport> {
    [
        FileKind::Obj,
        FileKind::Stl,
        FileKind::Glb,
        FileKind::Gltf,
        FileKind::Fbx,
        FileKind::Png,
        FileKind::Jpeg,
        FileKind::Tiff,
        FileKind::Bmp,
        FileKind::Webp,
        FileKind::Exr,
        FileKind::Svg,
        FileKind::Wav,
        FileKind::Mp3,
        FileKind::Mp4,
        FileKind::Mov,
    ]
    .into_iter()
    .map(check_supported)
    .collect()
}

/// Ensures a format is importable, otherwise returns a descriptive error.
pub fn require_supported(kind: FileKind) -> Result<(), ImportError> {
    let support = check_supported(kind);
    if support.supported {
        Ok(())
    } else {
        Err(ImportError::Unsupported(format!(
            "{}: {}",
            kind.label(),
            support.message
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_magic_bytes() {
        assert_eq!(
            detect_kind(None, &[0x89, b'P', b'N', b'G', 0, 0, 0, 0, 0, 0, 0, 0]),
            FileKind::Png
        );
        assert_eq!(detect_kind(None, b"RIFF\0\0\0\0WAVEfmt "), FileKind::Wav);
        assert_eq!(
            detect_kind(None, b"glTF\x02\0\0\0\x1c\0\0\0JSON"),
            FileKind::Glb
        );
        assert_eq!(
            detect_kind(
                None,
                &[0, 0, 0, 0x18, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm']
            ),
            FileKind::Mp4
        );
        assert_eq!(
            detect_kind(None, &[0x76, 0x2F, 0x31, 0x01, 0, 0, 0, 0, 0, 0, 0, 0]),
            FileKind::Exr
        );
    }

    #[test]
    fn detects_by_extension() {
        assert_eq!(detect_kind(Some(Path::new("a.obj")), b""), FileKind::Obj);
        assert_eq!(detect_kind(Some(Path::new("a.FBX")), b""), FileKind::Fbx);
        assert_eq!(
            detect_kind(Some(Path::new("a.txt")), b""),
            FileKind::Unknown
        );
    }

    #[test]
    fn unsupported_formats_explain_themselves() {
        let mp4 = check_supported(FileKind::Mp4);
        assert!(!mp4.supported);
        assert!(mp4.message.contains("image sequences"));
        let fbx = check_supported(FileKind::Fbx);
        assert!(!fbx.supported);
        assert!(fbx.message.contains("OBJ"));
        assert!(require_supported(FileKind::Mp4).is_err());
        assert!(require_supported(FileKind::Obj).is_ok());
    }

    #[test]
    fn support_matrix_covers_common_formats() {
        let list = list_formats();
        assert!(list.len() >= 12);
        assert!(list.iter().any(|f| f.kind == FileKind::Png && f.supported));
        assert!(list.iter().any(|f| f.kind == FileKind::Fbx && !f.supported));
    }
}
