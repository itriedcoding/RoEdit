# API reference

RoEdit Studio is divided into nine crates. The library crates are usable
outside the editor; the front-end is `roedit-app`.

> **Stability:** the application is at `0.1.0`. The project file format is
> versioned (`FORMAT_VERSION = 1`), but Rust APIs may change between `0.x`
> releases. Each crate documents its own modules with `cargo doc --workspace
> --no-deps`.

## Workspace members

| Crate | Highlights |
| --- | --- |
| `roedit-core` | `math`, `ids`, `mesh::Mesh`, `raster::RasterImage`, `camera::Camera`, `material::{Material, Environment}`, `scene::Scene`, `animation::{Animation, Track, Target, AnimValue}`, `gfx::GfxDoc`, `timeline::Timeline`, `nodegraph`, `render_settings`, `project::Project`, `history::History`, `settings::AppSettings` |
| `roedit-mesh` | `primitives`, `edit`, `subdiv`, `sculpt`, `uv`, `roblox`, `Axis` |
| `roedit-image` | `color`, `blend`, `composite`, `brush`, `filters`, `adjust`, `text`, `keying`, `procedural` |
| `roedit-anim` | `rig` (skeletons, two-bone IK, CCD, pose library, retargeting) |
| `roedit-io` | `obj`, `stl`, `gltf`, `wav`, `image_io`, `validate` |
| `roedit-render` | `bvh`, `rng::Pcg32`, `pathtracer` (`build_render_scene`, `render_float`, `to_raster`), `viewport`, `shaders` |
| `roedit-video` | `compositor`, `transitions`, `audio`, `export`, `codec::support_matrix` |
| `roedit-plugins` | `manifest`, `execute`, `PluginRegistry` |
| `roedit-app` | `Document`, `RoEditApp`, `command`, `state`, `ui`, `preview` |

## Geometry

```rust
use roedit_core::math::Vec3;
use roedit_mesh::{edit, primitives};

let mut mesh = primitives::cube(Vec3::new(1.0, 2.0, 0.5));
let all: Vec<usize> = (0..mesh.faces.len()).collect();
edit::subdivide_faces(&mut mesh, &all, 2).expect("valid faces");
edit::bevel_faces(&mut mesh, &all, 0.02, 1).expect("valid faces");
mesh.recompute_normals();
assert!(mesh.triangle_count() > 0);
```

`Scene::mesh_mut` copies on write: duplicate objects share their mesh until one
of them is edited.

## Scene and project

```rust
use roedit_core::math::Vec3;
use roedit_core::project::Project;
use roedit_mesh::primitives;

let mut project = Project::new("Example");
let mesh = project.scene.add_mesh(primitives::uv_sphere(0.5, 24, 12));
let entity = project.scene.add_entity("Ball", None);
if let Some(e) = project.scene.entity_mut(entity) {
    e.mesh = Some(mesh);
    e.transform.translation = Vec3::new(0.0, 1.0, 0.0);
}
let bytes = project.save_to_path(std::path::Path::new("example.roedit")).unwrap();
assert!(bytes > 0);
let loaded = Project::load_from_path(std::path::Path::new("example.roedit")).unwrap();
assert_eq!(loaded.scene.entities.len(), project.scene.entities.len());
```

## Rendering

```rust
use roedit_core::camera::Camera;
use roedit_render::{build_render_scene, render_float, to_raster, RenderRequest};

let camera = Camera::default();
let environment = project.scene.environment.clone();
let lights: Vec<_> = project.scene.lights.values().cloned().collect();
let scene = build_render_scene(&project.scene, &environment, &camera, &lights);
let request = RenderRequest {
    width: 320,
    height: 180,
    samples: 16,
    max_bounces: 3,
    seed: 7,
    mode: roedit_core::render_settings::RenderMode::PathTrace,
    exposure: environment.exposure,
    tonemap: environment.tonemap,
    transparent_background: false,
    firefly_clamp: 24.0,
    denoise: true,
    region: None,
};
let linear = render_float(&scene, &request, &|_progress| {});
let image = to_raster(&linear, &request);
assert_eq!((image.width, image.height), (320, 180));
```

Rendering is CPU-parallel through rayon. The same module exposes
`RenderMode::{Albedo, Normals, Depth, AmbientOcclusion, ShadowMatte}` for AOV
passes.

## Images

```rust
use roedit_core::math::Vec2;
use roedit_core::raster::RasterImage;
use roedit_image::brush::{stamp, BrushMode, BrushSettings};

let mut image = RasterImage::filled(256, 256, [20, 20, 24, 255]);
let settings = BrushSettings {
    mode: BrushMode::Normal,
    color: [1.0, 0.3, 0.2, 1.0],
    size: 48.0,
    ..Default::default()
};
let painted = stamp(&mut image, Vec2::new(128.0, 128.0), &settings);
assert!(painted > 0);
```

## Timeline and export

```rust
use roedit_video::{ExportOptions, SaveFormat};

let options = ExportOptions {
    directory: "renders/shot".into(),
    prefix: "frame".into(),
    format: SaveFormat::Png,
    start_frame: 0,
    end_frame: 29,
    jpeg_quality: 92,
    fps: 30.0,
    overwrite: true,
    write_audio: true,
};
```

Compile a `Compositor` from a timeline, then call
`roedit_video::export_sequence(&mut compositor, &options, &mut |progress| {})`.
The returned report contains the frame count, directory and optional audio path.

## Extensions

```rust
use roedit_plugins::PluginRegistry;

let mut registry = PluginRegistry::new();
let errors = registry.load_dir(std::path::Path::new("examples/plugins"));
assert!(errors.is_empty());
let mut project = roedit_core::project::Project::new("Macro");
let report = registry
    .execute("roedit.starter.crate", &mut project)
    .expect("macro runs");
assert!(report.steps > 0);
```

## Front-end integration

`roedit_app::Document` wraps a `Project` with undo history, path, dirty flag and
autosave. All edits should use `Document::edit(label, |project| ...)`, which
snapshots first. Continuous drags may snapshot once and then use
`silent_edit`.

`roedit_app::command::all_commands()` returns the command registry used by the
menus, the palette and the shortcut system; `Action` is the enum each command
dispatches.

## Codec support

`roedit_video::codec::support_matrix()` and `roedit_io::list_formats()` describe
what the build can actually read and write. The UI reads these functions
instead of hard-coding a list, so the documented matrix and the behaviour cannot
drift apart.
