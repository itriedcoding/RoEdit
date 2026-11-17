# Examples

## `plugins/roedit-starter-pack.json`

A complete, validated extension pack used by the documentation and by
`crates/roedit-plugins/tests/example_pack.rs`.

To load it:

1. Start RoEdit Studio.
2. Open the **Plugins** workspace.
3. Set the extension folder to this `examples/plugins` directory.
4. Press **Reload packs**.
5. Run one of the three commands:
   - **Build Crate Prop** — cube, bevel, wood material and a point light;
   - **Studio Product Lighting** — environment, spot light and high render
     quality;
   - **Two Clip Opener** — a video track with two colour clips.

The macros are ordinary undoable edits. See
[plugin development](../docs/plugin-development.md) for the schema.

The test loads this directory and executes all three commands against a fresh
project on every `cargo test` run, so an invalid example cannot be released
silently.
