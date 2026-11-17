# Changelog

All notable changes to RoEdit Studio are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
semantic versioning for the application while the internal crates remain at
`0.x` until the editor front-end is stabilised.

## [0.1.0] — 2026-11-17

First alpha of the native desktop suite.

### Added

- Nine-crate Rust workspace: `roedit-core`, `roedit-mesh`, `roedit-image`,
  `roedit-anim`, `roedit-io`, `roedit-render`, `roedit-video`,
  `roedit-plugins`, `roedit-app`.
- Native egui/eframe desktop application with twelve workspaces, dockable
  panels, dark theme presets, log and performance docks.
- 150+ registered commands with default shortcuts, per-command rebinding,
  and a searchable command palette.
- Scene graph with hierarchy, transforms, materials, lights, visibility,
  selection, raycasting and structural validation.
- 11 procedural primitives and 15 mesh operators, including Catmull-Clark
  subdivision, bevel, extrude, inset, decimate, solidify, mirror and array.
- Six sculpt brushes driven by surface raycasts.
- UV box/planar/cylindrical/spherical projection plus per-face unwrap and
  packing.
- Texture painting with six brush modes and thirteen image operations.
- Layer-based GFX document with 19 blend modes, text layers, layer groups and
  PNG flattening.
- Roblox R6 and R15 rig construction with part metadata and stud-to-metre
  scaling; `roedit-anim` provides two-bone IK, CCD, pose blending and
  retargeting.
- Keyframe animation with dope sheet, playback and sampled transforms.
- CPU path tracer with BVH, direct lighting, emissive materials, environment
  lighting, AOV modes, quality presets, progress reporting and
  PNG/JPEG/TIFF/EXR output.
- Video timeline with colour/text clips, seven transitions, chroma keying,
  audio mixing and frame-sequence export with `mix.wav`.
- Import/export for OBJ, STL, GLB, PNG, JPEG, TIFF, BMP, WebP, EXR and WAV,
  with an in-app support matrix.
- Data-driven JSON extension packs with capability validation and an example
  pack under `examples/plugins`.
- Project autosave, crash recovery, persisted settings and recent-project
  lists.
- 353 automated tests plus WGSL validation, and a GitHub Actions workflow that
  tests and builds the Windows executable and attaches it to tagged releases.
- Documentation covering installation, first project, manual, tutorials,
  shortcuts, plugin authoring, API, troubleshooting, performance and build.

### Known limitations

- Video export produces image sequences and WAV audio; MP4/MOV/MP3 are not
  supported.
- No FBX, SVG or text glTF import.
- The GUI has not been run interactively in the development environment; only
  compilation, engine tests and shader validation are verified.
- No installer or code signing.
