# Troubleshooting

Log lines are written to the console and to the in-app **Log** dock. Set
`RUST_LOG=debug` (or `trace`) before launching to get more detail:

```powershell
$env:RUST_LOG = "debug"
.\roedit.exe
```

## The application does not start

**Symptoms:** the process exits immediately or the window never appears.

- Check the console output. A missing `wgpu` adapter produces an
  `AdapterNotFound`-style error.
- Force a backend: `$env:WGPU_BACKEND = "dx12"` (or `vulkan`, `gl`). The wgpu
  viewport needs DX12 on Windows.
- Update the GPU driver. Remote desktop sessions and virtual machines often
  expose no DX12 adapter; use a physical machine or a VM with GPU
  passthrough for the viewport.
- Confirm you are running a 64-bit build on 64-bit Windows.

If the window opens but the viewport is black, see the next section. Rendering
stills through the CPU path tracer works even without a usable GPU adapter.

## The viewport preview never appears

The preview is path traced on the CPU and refreshes after edits.

1. Press **F5** to request a preview explicitly.
2. Make sure the scene is not empty — add a primitive from **Create**.
3. Press **F** to frame the scene; the camera may be pointing away from it.
4. Check the **Log** dock for `Preview error:` lines.
5. Large resolutions with high sample counts are slow. The preview is capped at
   1280×720 and a handful of samples by design; stills use the project
   settings.

## Renders are slow or memory-hungry

- Lower samples and bounces. Halving samples roughly halves the time.
- Render at a lower resolution and upscale for drafts.
- Keep "denoise" on for low-sample drafts.
- Close other memory-heavy applications; each render thread keeps its own
  sample buffers.
- See [performance](performance.md) for the cost model.

## Import fails

- The Asset Library lists exactly which formats the build supports. MP4, MOV,
  MP3, FBX, SVG and text glTF (`.gltf`) are **not** supported.
- OBJ/STL/GLB files must be readable; binary STL is detected by its header and
  triangle count, so truncated files are rejected.
- Images larger than available memory fail to decode. EXR files must be
  readable by the `exr` crate (scanline or tiled, common channel layouts).
- If the native file dialog does not open, type the path into the import field
  and press Import. This is the supported fallback.

## Nothing happens when I press a shortcut

- A text field has focus. Shortcuts without Ctrl/Alt are intentionally
  suppressed while typing.
- The binding may have been cleared in the shortcut editor. Open **F1** and
  check, or press **Ctrl+K** and run the command by name.
- Some commands have no default binding (primitives, mesh operators, UV
  projections, sculpt brushes, image operations, material presets). Use menus,
  workspace buttons or the palette.

## A shortcut does nothing after rebinding

The editor rejects invalid bindings such as `Ctrl+` or `Hyper+Q`. A valid
binding needs a modifier set plus a key from the supported list (letters,
digits, F1–F12, Space, Delete, Home, End, brackets, comma, period, minus,
equals). Rebind again with a valid combination.

## Extension pack is rejected

The log prints the exact validation problem, including the command id and step
number. Common causes:

- a step uses a capability the manifest does not list;
- an unknown primitive, mesh operator, light kind, track kind, render mode or
  quality name;
- a parameter outside its allowed range (for example `subdivide` amount 8);
- a duplicate command id or pack id.

See [plugin development](plugin-development.md) for the accepted vocabulary.

## Recovery prompt appears every launch

The autosave file at `%APPDATA%\RoEditStudio\autosave.roedit` is only removed
after a successful manual save. Choose **Recover it**, save the project with
Ctrl+S, and the file is cleared. **Discard** deletes it immediately.

If the prompt reappears after saving, the save may have failed (read-only
folder, full disk); the log records the error.

## Settings are not remembered

Settings are written on exit, when you press **Save settings**, and when saving
shortcuts. A crash skips the exit save. Check that
`%APPDATA%\RoEditStudio\settings.json` is writable; an invalid file is ignored
and replaced with defaults, with a warning in the log.

## The UI is too small or too large

Open **Settings** and move the **UI scale** slider (0.75–2.0). The value is
stored in settings and re-applied at startup. On high-DPI displays, Windows
scaling multiplies with this value.

## Undo does not revert what I expected

Undo is snapshot based and per edit. A sculpt stroke, a drag of a transform
gizmo and a colour-slider adjustment each record one entry when the interaction
starts, so a long drag is a single undo step. Macro execution is one step. If a
slider changes a material without recording history, the change is still marked
dirty; undo returns to the last recorded state.

## Reporting a bug

Include:

- Windows version and GPU model;
- the exact steps;
- the last 50 lines of the console log (or the in-app log contents);
- whether the issue reproduces on a fresh project.

Use the bug report issue template.
