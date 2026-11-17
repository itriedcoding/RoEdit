//! Honest container support matrix.
//!
//! RoEdit Studio writes image sequences and WAV audio. Encoding H.264/H.265
//! requires a codec library that is not linked into this build, so MP4/MOV
//! export is offered as "sequence + audio" and the UI says so explicitly.

#[derive(Clone, Debug, PartialEq)]
pub struct ContainerSupport {
    pub extension: &'static str,
    pub label: &'static str,
    pub import: bool,
    pub export: bool,
    pub note: &'static str,
}

const MATRIX: [ContainerSupport; 9] = [
    ContainerSupport {
        extension: "png",
        label: "PNG sequence",
        import: true,
        export: true,
        note: "Lossless RGBA frames. Recommended master format.",
    },
    ContainerSupport {
        extension: "jpg",
        label: "JPEG sequence",
        import: true,
        export: true,
        note: "Smaller files, no alpha, quality slider applies.",
    },
    ContainerSupport {
        extension: "tif",
        label: "TIFF sequence",
        import: true,
        export: true,
        note: "Lossless with alpha, larger than PNG.",
    },
    ContainerSupport {
        extension: "exr",
        label: "OpenEXR sequence",
        import: true,
        export: true,
        note: "Float storage; RoEdit writes tonemapped values into linear EXR.",
    },
    ContainerSupport {
        extension: "wav",
        label: "WAV audio",
        import: true,
        export: true,
        note: "PCM 8/16/24/32 and float 32/64 import, 16-bit stereo mix export.",
    },
    ContainerSupport {
        extension: "mp4",
        label: "MP4 (H.264)",
        import: false,
        export: false,
        note: "No video codec is bundled. Export a PNG sequence and audio, then mux in an external encoder.",
    },
    ContainerSupport {
        extension: "mov",
        label: "QuickTime MOV",
        import: false,
        export: false,
        note: "No video codec is bundled. Use an image sequence instead.",
    },
    ContainerSupport {
        extension: "gif",
        label: "Animated GIF",
        import: false,
        export: false,
        note: "Not implemented in this build.",
    },
    ContainerSupport {
        extension: "mp3",
        label: "MP3 audio",
        import: false,
        export: false,
        note: "No MP3 decoder is bundled. Convert to WAV first.",
    },
];

/// The full matrix, for the export dialog and the documentation.
pub fn support_matrix() -> Vec<ContainerSupport> {
    MATRIX.to_vec()
}

/// Formats the matrix as plain text lines (used by the docs and export dialog).
pub fn describe_matrix() -> String {
    MATRIX
        .iter()
        .map(|c| {
            format!(
                "{:<5} {:<18} import: {:<3} export: {:<3} {}",
                c.extension,
                c.label,
                if c.import { "yes" } else { "no" },
                if c.export { "yes" } else { "no" },
                c.note
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One-line guidance for users who need a single video file.
pub fn mux_hint() -> &'static str {
    "RoEdit writes frame sequences plus mix.wav. To get an MP4, encode the \
sequence in any external tool, e.g. ffmpeg -framerate 30 -i shot_%05d.png -i \
mix.wav -c:v libx264 -pix_fmt yuv420p out.mp4"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_export_and_containers_do_not() {
        let matrix = support_matrix();
        let png = matrix.iter().find(|c| c.extension == "png").unwrap();
        assert!(png.export && png.import);
        let mp4 = matrix.iter().find(|c| c.extension == "mp4").unwrap();
        assert!(!mp4.export && !mp4.import);
        assert!(mp4.note.contains("sequence"));
    }

    #[test]
    fn description_is_honest_about_codecs() {
        let text = describe_matrix();
        assert!(text.contains("mp4"));
        assert!(text.contains("PNG sequence"));
        assert!(mux_hint().contains("ffmpeg"));
    }

    #[test]
    fn every_entry_has_a_note() {
        for c in support_matrix() {
            assert!(!c.note.is_empty(), "{} has no note", c.extension);
            assert!(!c.label.is_empty());
        }
    }
}
