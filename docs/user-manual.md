# User manual

This manual describes the behaviour of each part of RoEdit Studio. Where a
feature is limited, the limitation is stated in place rather than hidden.

## Document model

A project (`.roedit`, JSON, format version 1) contains:

- scene graph: entities (name, parent, children, transform, mesh, material,
  light, visibility, tags), meshes, PBR materials, images, lights, camera and
  environment;
- animation: typed keyframe tracks for entity transforms, camera, materials,
  lights and clips;
- GFX document: canvas size, background and ordered layers (text, group or
  raster);
- timeline: tracks, clips, markers, playhead, frame rate and duration;
- render settings and project metadata.

Every edit goes through one history path: the previous state is pushed to the
undo stack, then the project is mutated. **Ctrl+Z**/**Ctrl+Y** therefore work
the same way for modelling, painting, GFX and timeline operations. The undo
limit is configurable in Settings (default 100 states).

Meshes and images are reference counted (`Arc`), so snapshots and duplicated
objects share data instead of copying it. A mesh edited after duplication is
copied on write.

## Top bar

- **Menus** mirror the command registry; each entry shows its shortcut.
- **Workspace tabs** switch between Model, Sculpt, UV, Texture, GFX, Rig,
  Animate, Materials, Render, Video, Assets and Plugins. Number keys 1–9 and 0
  select them unless a text field has focus.
- **Tool strip**: Select (Q), Move (W), Rotate (E), Scale (R), Sculpt (T),
  Paint (B), Text (X), Hand (H).
- **View toggles**: Grid, Wireframe and Stats overlay.
- **Right side**: running job progress, preview spinner, unsaved indicator,
  and the palette button (**Ctrl+K**).

## Scene hierarchy (left dock)

- Click an object to select it; the eye button toggles visibility (children
  inherit effective visibility).
- The filter box matches names case-insensitively within the tree.
- **Focus** frames the selection, **Duplicate** creates a copy with a shared
  mesh, **Delete** removes the object and its descendants.
- Lights section: enable/disable and select lights.
- Materials section: colour swatch, selection, and **Assign** to the current
  object.
- The footer shows object/mesh/vertex/triangle counts and the pending undo
  label.

## Inspector (right dock)

Contents change with the workspace:

- **Object** — rename, visibility, position/rotation/scale, reset transform,
  drop to floor, zero rotation, frame, duplicate. Rotation is edited in degrees
  using YXZ Euler order.
- **Material** — base colour, metallic, roughness, opacity, emission colour and
  strength, IOR, normal strength, double-sided flag, five texture slots
  (albedo, ORM, normal, emission, opacity) fed from the selected project image,
  duplicate material.
- **Sculpt** — brush list, radius, strength and a short hint.
- **UV** — projections and a count of faces with UVs.
- **Animation** — time slider, channel selector (translation/rotation/scale),
  key insert/delete, play/pause and apply-at-time.
- **Render** — resolution, samples, bounces, mode, quality preset, output path
  and format, JPEG quality, "use project resolution", render button.
- **Environment** — sky mode, sky colours, sun intensity, ambient, exposure,
  fog, sun direction.
- **Statistics** — scene and performance counters with a frame-time graph.

## 3D viewport

- **Orbit** with left-drag using Select (or Alt-drag), **pan** with Shift-drag
  or middle-drag, **zoom** with the wheel.
- **Click** to select the object under the cursor (raycast against visible
  meshes). Clicking empty space clears the selection.
- **Move/Rotate/Scale** drag the selected object on the camera plane; a single
  history entry is recorded per drag.
- **Sculpt** applies the active brush where the ray hits the surface, in mesh
  local space so scaled objects behave correctly.
- Overlays: ground grid with coloured axes, selection bounding box, wireframe
  (all visible meshes, budgeted), axis gizmo and a stats corner.
- The image is a CPU path-traced preview; it refreshes after edits with a short
  debounce.

## Sculpting

Brushes: Draw, Inflate, Smooth, Flatten, Pinch, Crease. Radius is in world
units and divided by the object's average scale to stay consistent. Strength
controls displacement per dab. Each stroke is one undo step.

## UV editing

Projections overwrite existing UVs:

- **Box** maps each face by its dominant normal axis.
- **Planar** projects along Y (world axes) with scale and offset.
- **Cylindrical** and **Spherical** use the mesh bounds.
- **Unwrap and Pack** builds an island per face and packs them; the log records
  island and skipped-face counts.

The UV canvas draws UV wireframe within the 0–1 square.

## Texture painting

Choose a project image (or import one), set colour, mode, size, hardness and
opacity, then drag on the canvas. Painting writes into the document's image;
use **File → Export** or the Assets workspace to write a PNG copy. Image
adjustments apply the same operations as the Asset Library (blur, sharpen,
median, chroma key and so on), each as one undo step.

## GFX Studio

- Canvas presets 1920×1080, 1080×1080, 1080×1920, 1280×720 and 2560×1440.
- Add text layers with editable content, position, size and colour; the active
  layer can also be edited inline.
- Add groups; group layers hold other layers when created from the layer list.
- 19 blend modes (normal, multiply, screen, overlay, soft light, hard light,
  colour dodge/burn, darken/lighten, difference, exclusion, add, subtract,
  hue/saturation/colour/luminosity, plus divide where available).
- Layer visibility, opacity and blend mode are editable.
- **Flatten → PNG** renders the composition on a worker thread to
  `renders/gfx_flatten.png`.
- Freehand raster painting is available in the Texture workspace; GFX itself
  is text/layer based.

## Rigging

**Create → Roblox Rig → R6 or R15** builds a root object with one child per
part. Part names, sizes and centres come from `roedit_mesh::roblox`; sizes are
converted from studs to metres with `STUD_TO_METER`. The rig is ordinary scene
geometry, so parts can be selected, posed and animated.

`roedit-anim` includes skeletons, analytic two-bone IK, CCD solving, pose
blending and R6→R15 retargeting. These are library APIs covered by tests; the
viewport does not yet expose interactive IK handles.

## Animation

- Select an object, choose a channel, set the time and press **K** (or the
  inspector's **Key** button) to insert a keyframe.
- The dope sheet lists tracks with per-track enable/disable, key markers that
  jump the playhead, and track deletion.
- **Space** plays the timeline; sampled transform tracks are applied to the
  scene each frame. **Apply at time** forces the current time's pose.
- Easing between keys uses the key handles; linear is the default.

## Rendering

The renderer is a multi-threaded CPU path tracer using a BVH, cosine-weighted
diffuse bounces, GGX-style specular, emissive materials, environment sampling,
point/spot/directional lights with shadows, firefly clamping and optional
denoising.

Modes: Path Trace, Ambient Occlusion, Albedo, Normals, Depth, Shadow Matte.
Quality presets: Draft, Low, Medium, High, Ultra. Stills honour project
resolution or fall back to 1280×720 when "use project settings" is off.
Progress and timing are reported in the top bar, log and performance dock.

## Video editing

- The timeline holds video/image/text/audio tracks. **Add colour** and **Add
  text** create clips at the playhead; **Split** cuts the clip under the
  playhead; **Delete clip** removes the selected clip; **Marker** adds a named
  marker.
- Click the ruler to move the playhead, drag to scrub. The program monitor
  composites the frame at the playhead (colour/text/image clips, transitions,
  chroma key and fades).
- **Export sequence** writes individual frames in the chosen format and prefix,
  plus `mix.wav` when the timeline has audio. There is **no MP4/MOV encoder**;
  use ffmpeg externally for a movie file.
- Seven transitions are available in the compositor API (cross dissolve, wipe,
  slide, zoom, dip to colour, additive and luma-style).

## Asset Library

- Lists images with dimensions and size, and meshes with vertex/triangle
  counts.
- Select an image, then **Assign to material** to set it as the selected
  material's albedo map, or **Export copy** to write a PNG.
- Import accepts a typed path when no native dialog is available.
- The format support table is generated from the importer's own capability
  matrix.

## Extensions

Choose a folder containing `.json` packs and press **Reload packs**. Each pack
lists its name, id, version, commands and tips. **Run** executes a macro as one
undoable edit; the log records step and object counts. Invalid packs are
reported with the specific validation error and are not installed.

## Commands, shortcuts and settings

- **Ctrl+K** opens the command palette, which searches labels, categories, ids
  and descriptions.
- Settings persist theme (Midnight, Graphite, Ocean, Studio Light, High
  Contrast), UI scale, reduced motion, low-memory mode, autosave interval, undo
  limit, snapping, render thread preference, extension folder and recent
  projects.
- Shortcuts are stored per command id; the shortcut editor rejects invalid
  bindings and clears a binding when emptied.
