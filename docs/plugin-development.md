# Plugin development

RoEdit extensions are **data-driven JSON packs**. A pack cannot ship native
code, cannot load a dynamic library and cannot run a scripting language: it
declares macro steps that the host validates and executes with its own
operators. This makes installing a pack equivalent to pasting a document.

The schema lives in `roedit-plugins/src/manifest.rs`; a working example is
[`examples/plugins/roedit-starter-pack.json`](../examples/plugins/roedit-starter-pack.json),
and a test (`crates/roedit-plugins/tests/example_pack.rs`) loads and executes it
on every CI run.

## Folder layout

Put one or more `*.json` packs in a folder, then point **Extensions → Extension
folder** at it and press **Reload packs**. Files are loaded in directory order;
duplicate pack ids are rejected.

## Manifest

```json
{
  "id": "author.pack",
  "name": "My Pack",
  "version": "1.0.0",
  "author": "Your name",
  "description": "What the pack provides",
  "api_version": 1,
  "capabilities": ["mesh", "material", "light", "environment", "render", "timeline"],
  "commands": [
    {
      "id": "author.pack.command",
      "label": "Build a tower",
      "description": "Optional explanation shown in the UI",
      "category": "Props",
      "steps": [
        { "op": "add_primitive", "primitive": "cube", "position": [0, 0.5, 0], "size": [1, 1, 1] },
        { "op": "apply_mesh_op", "operator": "bevel", "amount": 0.02 }
      ]
    }
  ],
  "material_presets": [
    { "name": "Painted Steel", "color": [0.3, 0.4, 0.6], "metallic": 0.8, "roughness": 0.35 }
  ],
  "tips": ["Macros are ordinary undoable edits."]
}
```

`id` must use lowercase letters, digits, `.`, `-` and `_`. `api_version` must be
`1` today. Unknown fields are ignored by serde; missing optional fields use the
defaults shown in the schema.

## Capabilities

Every step declares a capability requirement; the pack must list it or
validation fails.

| Capability | Required by |
| --- | --- |
| `mesh` | `add_primitive`, `apply_mesh_op` |
| `material` | `add_material` |
| `light` | `add_light` |
| `environment` | `set_environment` |
| `render` | `set_render` |
| `timeline` | `add_timeline_track`, `add_timeline_clip` |

## Steps

### `add_primitive`

```json
{ "op": "add_primitive", "primitive": "sphere", "position": [0, 1, 0], "size": [1, 1, 1], "material": "Optional Material Name", "name": "Optional Object Name" }
```

Primitives: `cube`, `sphere`, `cylinder`, `cone`, `torus`, `capsule`, `disc`,
`grid`, `wedge`, `corner_wedge`, `roblox_part`. Sizes must be positive and
finite.

### `apply_mesh_op`

```json
{ "op": "apply_mesh_op", "operator": "subdivide", "amount": 2 }
```

| Operator | `amount` meaning |
| --- | --- |
| `subdivide` | cuts per edge, 1–4 |
| `bevel` | bevel width in metres, > 0 |
| `extrude` | extrusion distance in metres, > 0 |
| `smooth` | iterations, 1–20 |
| `decimate` | target ratio, 0.05–0.95 |
| `solidify` | shell thickness in metres, > 0 |
| `triangulate` | ignored |
| `recalc_normals` | ignored |

The operator applies to the object created or selected earlier in the macro
sequence, which is why ordering matters.

### `add_material`

```json
{ "op": "add_material", "name": "Warm Plastic", "color": [0.8, 0.4, 0.2], "metallic": 0.0, "roughness": 0.5, "emission": 0.0 }
```

Colour channels must be within 0–1. `emission` is a strength multiplier.

### `add_light`

```json
{ "op": "add_light", "kind": "spot", "position": [2, 3, 2], "intensity": 30, "color": [1, 0.95, 0.9] }
```

Kinds: `point`, `directional`, `spot`. Intensity cannot be negative.

### `set_environment`

```json
{ "op": "set_environment", "sun_intensity": 1.6, "sun_angle_deg": 2.0, "sky_top": [0.05, 0.08, 0.16], "sky_horizon": [0.5, 0.6, 0.8], "exposure": 1.1 }
```

Every field is optional; omitted fields keep their current value.

### `set_render`

```json
{ "op": "set_render", "quality": "high", "samples": 256, "max_bounces": 6, "mode": "path_trace" }
```

Qualities: `draft`, `low`, `medium`, `high`, `ultra`.
Modes: `path_trace`, `ambient_occlusion`, `albedo`, `normals`, `depth`,
`shadow_matte`.

### `add_timeline_track`

```json
{ "op": "add_timeline_track", "name": "Opener", "kind": "video" }
```

Kinds: `video`, `image`, `text`, `audio`, `adjustment`.

### `add_timeline_clip`

```json
{ "op": "add_timeline_clip", "track": "Opener", "color": [0.1, 0.3, 0.6], "start": 0.0, "duration": 2.0 }
```

`start` must be ≥ 0 and `duration` > 0. The track is matched by name.

## Validation

`PluginManifest::validate` checks ids, API version, duplicate command ids, empty
commands, capability coverage, step parameters and preset ranges. Every problem
is reported with the command id and step number, for example:

```
command 'author.pack.command' step 2 (Apply mesh operator): unknown mesh op 'warp' (known: subdivide, bevel, ...)
```

A pack with any problem is not installed. Execution re-checks the plan and
stops at the first failing step; the macro runs inside a single history entry,
so one Ctrl+Z rolls the whole macro back.

## Writing and testing a pack

1. Copy the example pack and edit the ids, labels and steps.
2. Run `cargo test -p roedit-plugins` to check the shipped example still
   validates.
3. Load your folder in the Extensions workspace; validation errors appear in
   the log and as a toast.
4. Run each command on a scratch project and check the object counts in the
   log.

You can validate a pack without the UI by writing a small Rust test that builds
a `PluginRegistry`, calls `load_dir`, and asserts that the returned error list
is empty.

## Security model

- No native code, no `eval`, no shell commands, no network access.
- The host operator table is the entire vocabulary; unknown operations are
  rejected before execution.
- Packs cannot delete objects, change the file path or write files.
- Loading is read-only with respect to disk outside the chosen folder.

If you need behaviour outside this vocabulary, extend the host operator table in
`roedit-plugins/src/execute.rs` behind a new capability and add tests for it —
that is a code contribution, not a pack.
