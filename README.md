# RoEdit Studio

A native Windows desktop creative suite for Roblox-style 3D work, GFX design, rendering, animation and video compositing. Written in Rust with an immediate-mode [egui](https://github.com/emilk/egui) interface, a CPU path tracer, and no AI features.

> **Project status: alpha.** The engine crates are covered by 350+ unit and integration tests that run on every CI build. The Windows GUI builds in CI; the graphical front-end has not been run on a machine with a display in this development environment, so treat the UI as unverified-by-eye. See [Verified vs. unverified](#verified-vs-unverified) before relying on it.

## What it does

RoEdit Studio is one application with twelve workspaces that share a single project document:

| Workspace | Working features |
| --- | --- |
| **3D Modeling** | 11 procedural primitives (cube, sphere, cylinder, cone, torus, capsule, disc, grid, wedge, corner wedge, Roblox part), hierarchy, transforms, duplicate, delete, 15 mesh operators (subdivide, bevel, extrude, inset, Laplacian smooth, decimate, solidify, triangulate, normal recalculation, weld, mirror X/Y/Z, array, flip) |
| **Sculpting** | Six vertex brushes (draw, inflate, smooth, flatten, pinch, crease) applied through a surface raycast with adjustable radius and strength |
| **UV Editing** | Box, planar, cylindrical and spherical projection, per-face unwrap with island packing, UV layout view |
| **Texture Paint** | Brush painting directly into project images: normal, erase, multiply, add, blur and clone modes with size, hardness, opacity and flow |
| **GFX Studio** | Resolution-independent text and group layers, 19 blend modes, layer opacity/visibility, canvas presets from 1080×1080 to 2560×1440, flatten to PNG |
| **Rigging** | Roblox R6 (6 parts) and R15 (15 parts) rig construction with correct part names, sizes and stud-to-metre scale; the `roedit-anim` crate provides analytic two-bone IK, CCD and R6→R15 retargeting |
| **Animation** | Keyframes on entity translation, rotation and scale; dope sheet with per-track enable/disable; playback applies sampled transforms to the scene |
| **Material Graph** | PBR material editing (base colour, metallic, roughness, opacity, emission, IOR, normal strength, double-sided), texture slots, 12 material presets |
| **Rendering** | Multi-threaded CPU path tracer with BVH acceleration, direct lighting, emissive materials, sky/environment lighting, AOV modes (path trace, AO, albedo, normals, depth, shadow matte), quality presets, PNG/JPEG/TIFF/EXR output |
| **Video Editing** | Multitrack timeline, colour/text/image clips, seven transitions, chroma key, audio mixing with fades, frame-sequence export plus `mix.wav` |
| **Asset Library** | Project images and meshes, image adjustments and filters (exposure, invert, threshold, posterize, sepia, vignette, hue, saturation, contrast, blur, sharpen, median, chroma key), honest format support matrix |
| **Extensions** | Data-driven JSON extension packs: validated macros, material presets and tips, with per-step capability checks |

Additional systems: undo/redo history over the whole project, autosave and crash recovery, a persisted settings file, customisable shortcuts for every command, a searchable command palette (Ctrl+K), and a log/performance dock.

## Requirements

- Windows 10 or Windows 11, 64-bit
- A DirectX 12 capable GPU for the wgpu viewport (the CPU path tracer itself needs no GPU)
- 4 GB RAM minimum, 8 GB recommended for large scenes

## Install and run

Prebuilt binaries are produced by the GitHub Actions workflow in `.github/workflows/windows.yml` and attached to tagged releases as `roedit-windows-x64.zip`. Download, extract, and run `roedit.exe`; no installer is provided yet and the binary is not code-signed.

## Build from source

```powershell
# Prerequisites: Rust stable (MSVC toolchain) and Visual Studio Build Tools with the
# "Desktop development with C++" workload.
git clone https://github.com/itriedcoding/RoEdit.git
cd RoEdit
cargo build --release
.\target\release\roedit.exe
```

Run the test suite with:

```powershell
cargo test --workspace
```

See [docs/build.md](docs/build.md) for cross-checking the Windows target from Linux and for CI details.

## Documentation

- [Installation](docs/installation.md)
- [Getting started](docs/getting-started.md)
- [User manual](docs/user-manual.md)
- [Tutorials](docs/tutorials.md)
- [Keyboard shortcuts](docs/shortcuts.md)
- [Plugin development](docs/plugin-development.md)
- [API reference](docs/api.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Performance](docs/performance.md)
- [Building and releasing](docs/build.md)
- [Contributing](CONTRIBUTING.md)

## Project layout

| Crate | Responsibility |
| --- | --- |
| `roedit-core` | Math, ids, mesh/image containers, camera, materials and environment, scene graph, animation, GFX document, timeline, node graph, render settings, project file format, history, settings |
| `roedit-mesh` | Primitives, edit operators, Catmull-Clark subdivision, sculpt brushes, UV mapping, Roblox R6/R15 rigs and export validation |
| `roedit-image` | Colour and LUT handling, blending, compositing/flattening, brushes, filters, adjustments, text rendering (fontdue), chroma keying, procedural textures |
| `roedit-anim` | Skeletons, two-bone IK, CCD, pose blending, retargeting |
| `roedit-io` | OBJ, STL, GLB import/export; WAV read/write; image load/save (PNG, JPEG, TIFF, BMP, WebP, EXR); format validation |
| `roedit-render` | BVH, PCG32 sampling, CPU path tracer, AOVs, wgpu viewport and validated WGSL shaders |
| `roedit-video` | Compositor, transitions, audio mixing, frame-sequence export, codec support matrix |
| `roedit-plugins` | Extension manifest schema, validation, macro execution, registry |
| `roedit-app` | Native desktop application: document, history, commands, background jobs, UI |

Meshes and images are stored behind `Arc`, so undo snapshots and duplicated objects share buffers instead of copying them.

## File formats

| Format | Import | Export | Notes |
| --- | --- | --- | --- |
| `.roedit` (JSON) | Yes | Yes | Native project format, format version 1 |
| OBJ | Yes | Yes | Export writes a companion `.mtl` |
| STL | Yes (binary + ASCII) | Yes (binary) | Geometry only |
| GLB (glTF 2.0 binary) | Yes | Yes | Positions, normals, UVs, node transforms |
| PNG / JPEG / TIFF / BMP / WebP / EXR | Yes | PNG, JPEG, TIFF, EXR | EXR is float, the rest 8-bit |
| WAV | Yes (PCM 8/16/24/32, float 32/64) | Yes (mix output) | Timeline audio mix |
| MP4 / MOV / MP3 | No | No | No video/audio codec is bundled |
| FBX / SVG / glTF (`.gltf` text) | No | No | Listed as unsupported in the Asset Library |

The Asset Library shows this matrix inside the application, generated from the same code that gates imports, so the UI never offers a format the engine cannot handle.

## Verified vs. unverified

Verified on every CI run and in this repository:

- 353 unit/integration tests across nine crates (`cargo test --workspace`)
- WGSL shader validation through `naga` for the viewport pipelines
- Path tracer correctness tests: lit sphere, sky gradient, deterministic sampling, AOV outputs
- Round-trip tests for the project format, OBJ/STL/GLB, WAV and every image format
- Macro execution tests for the shipped example extension pack
- Windows GUI compilation (`cargo build --release`) in GitHub Actions

Not verified in this development environment:

- The application has **not been launched with a display**: this Linux VM has no GPU, no Vulkan ICD and no X server. Layout, input handling and rendering of the egui front-end have therefore not been observed by the author.
- No Windows `.exe` has been produced locally; the CI workflow builds it.
- No release has been published yet.

## Limitations

These are real product limitations, not placeholders:

- **Video is an image sequence, not an encoded movie.** Export writes numbered PNG/JPEG/TIFF frames and a WAV mix. Use an external tool such as ffmpeg to produce MP4.
- **Extensions are data-driven.** JSON packs describe macro steps; native dynamic libraries and scripting languages are deliberately not loaded.
- **No FBX, SVG or text glTF import, and no MP4/MOV/MP3 support.**
- **Rigging workspace exposes construction and rig data; the dedicated IK pose manipulators are provided by `roedit-anim` in code, not as viewport gizmos.**
- **The 3D workspace preview is CPU path traced**, so it is progressive rather than a real-time shaded viewport. The wgpu viewport pipeline is available and shader-validated, but the app currently previews through the CPU renderer for identical output.
- **No installer, code signing, auto-update or crash reporter.**

## License

MIT — see [LICENSE](LICENSE).
