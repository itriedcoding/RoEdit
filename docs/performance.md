# Performance

RoEdit keeps its engine work off the UI thread and shares buffers between
snapshots. This page explains where time goes and which settings matter.

## Rendering cost model

The still renderer is a CPU path tracer:

- Cost grows linearly with **pixels × samples**.
- Cost grows with **max bounces**; each extra bounce adds another ray per
  sample, plus shadow rays at every diffuse hit.
- A BVH is built per render, so mesh count affects setup more than per-pixel
  cost once the BVH exists.
- Denoising adds a separable filter pass (small compared with path tracing).
- Direct light sampling uses the scene's point/spot/directional lights; many
  shadow-casting lights increase per-hit work.

Practical levers, in order of effect:

1. **Resolution.** Half width and height is a quarter of the work.
2. **Samples.** Linear. Draft renders at 16–32 samples with denoise are useful
   for composition; 256–1024 for finals.
3. **Bounces.** 2–4 is enough for most product/character shots; 6+ only for
   interiors or glass.
4. **Firefly clamp.** Lower values reduce noise from tiny bright highlights at
   the cost of dimming them.
5. **Lights.** Disable shadow casting on lights that do not need it.

The viewport preview is deliberately small (at most 1280×720, low sample count,
2 bounces) so edits remain responsive. It is debounced: a burst of edits
triggers one preview, not one per frame.

## Threading

`roedit-render` parallelises over scanlines with rayon, so it uses all logical
cores by default. The Settings page stores a **render threads** preference; the
current build still uses rayon's global pool, so treat that field as a
placeholder until the pool is wired into render startup.

Background work runs on dedicated threads:

- `roedit-preview` — viewport previews;
- `roedit-video-preview` — timeline frame previews;
- `roedit-render` — final stills;
- `roedit-sequence` — frame-sequence exports;
- `roedit-flatten` — GFX flattening.

Each reports through a channel; the UI polls and uploads textures when results
arrive. Only one preview and one job run at a time, which keeps memory bounded.

## Memory

- Meshes and images are `Arc`-shared. Undo snapshots and duplicates point at the
  same buffers until a copy-on-write edit occurs.
- History depth is limited (default 100 states). Lower it in Settings for large
  scenes; each entry holds a project snapshot, not a deep copy of geometry.
- **Low memory mode** reduces cached preview work and disables extra animation
  polish. It is a preference stored in settings.
- Image size dominates memory: a 4K RGBA image is ~33 MB. The Asset Library
  shows each image's byte size so you can spot the expensive ones.

## UI

The interface is immediate mode. Per-frame work is mostly layout and a handful
of custom painters:

- wireframe drawing is budgeted to 4000 line segments;
- UV drawing is capped at 6000 faces;
- the frame-time graph keeps 120 samples;
- the log keeps 500 entries.

If the UI feels heavy, close the wireframe overlay and the stats overlay, and
lower the window size; the path-traced preview is independent of window size
because it renders at a fixed internal resolution.

## Measuring

The **Performance** tab shows:

- frame time (current/average), FPS;
- last preview time and last render time;
- scene counters (objects, meshes, vertices, triangles, images, image bytes).

Every render logs its resolution, sample count and duration. Use those numbers,
not guesses, when comparing settings.
