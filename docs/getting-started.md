# Getting started

This walkthrough takes about ten minutes and touches modelling, materials,
lighting, rendering and saving.

## 1. Launch and look around

Start `roedit.exe`. The window is divided into four docks around a central
workspace:

- **Top** — menus (File, Edit, Create, Mesh, UV, Render, Video, View, Help),
  the twelve workspace tabs, the tool strip, and the palette/status area.
- **Left** — scene hierarchy with visibility toggles, lights, materials and
  scene statistics.
- **Right** — inspector for the current workspace (object transform, material
  PBR controls, environment, render settings, performance).
- **Bottom** — timeline, animation dope sheet, log and performance tabs.
- **Centre** — the active workspace surface: 3D viewport, 2D canvas, UV layout
  or asset browser.

If an autosave from a previous session exists, a **Recover autosave** dialog
appears. Choose *Recover it* to open that project or *Discard* to delete it.

## 2. Add and transform an object

1. On the **Model** workspace, use **Create → Primitives → Cube** (or the
   **+ Add** button above the hierarchy).
2. The cube appears selected. In the right-hand inspector, drag the Position
   fields or use the **Move** tool (W) and drag in the viewport.
3. Drag in empty space to orbit, hold Shift or the middle mouse button to pan,
   and scroll to zoom. **F** frames the selection.

The viewport preview is rendered by the CPU path tracer and refreshes a moment
after each edit.

## 3. Give it a material

1. Open the **Materials** menu and pick a preset such as *Brushed Metal*.
2. The preset is added to the scene and assigned to the selected object.
3. In the inspector, adjust **Metallic**, **Roughness**, **Base** colour and
   **Emission strength**. Changes update the preview.

## 4. Light the scene

1. Use **Create → Light → Point**.
2. Select the light in the left dock and edit its position, intensity and
   colour in the inspector.
3. The environment section controls sky colour, sun intensity, ambient light,
   exposure and fog.

## 5. Render a still

1. Switch to the **Render** workspace.
2. Choose a quality preset (Draft through Ultra) or set samples and bounces
   directly.
3. Set the output path, for example `renders/first.png`, and press **F12**.

The render runs on a worker thread; a progress bar appears in the top bar. When
it finishes, the log records the resolution, sample count, duration and file
path. Output formats are PNG, JPEG, TIFF and EXR.

## 6. Save the project

**Ctrl+S** saves a `.roedit` JSON document. The first save asks for a path.
Autosave writes to `%APPDATA%\RoEditStudio\autosave.roedit` while the document
has unsaved changes, and the top bar shows an unsaved-changes dot.

## Where to go next

- [User manual](user-manual.md) for every workspace in detail.
- [Tutorials](tutorials.md) for a Roblox character, a GFX thumbnail and a
  short video sequence.
- [Keyboard shortcuts](shortcuts.md) and the command palette (**Ctrl+K**).
