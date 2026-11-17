# Keyboard shortcuts

Every command can be rebound in **Help → Keyboard Shortcuts**. Bindings are
stored per command id in `%APPDATA%\RoEditStudio\settings.json`.

Shortcuts without Ctrl or Alt are ignored while a text field has focus, so
typing `w` in a name field never switches tools. Shortcuts containing Ctrl or
Alt still fire, matching common desktop behaviour.

## File

| Command | Default |
| --- | --- |
| New project | Ctrl+N |
| Open project | Ctrl+O |
| Save | Ctrl+S |
| Save as | Ctrl+Shift+S |
| Import file | Ctrl+I |
| Export / render dialog | Ctrl+E |
| Settings | Ctrl+, |
| Quit | Ctrl+Q |

## Edit and view

| Command | Default |
| --- | --- |
| Undo | Ctrl+Z |
| Redo | Ctrl+Y |
| Duplicate selection | Ctrl+D |
| Delete selection | Delete |
| Frame selection | F |
| Reset camera | Home |
| Toggle grid | Shift+G |
| Toggle wireframe | Z |
| Command palette | Ctrl+K |
| Keyboard shortcuts help | F1 |

## Workspaces

| Workspace | Key |
| --- | --- |
| Model | 1 |
| Sculpt | 2 |
| UV | 3 |
| Texture | 4 |
| GFX | 5 |
| Rig | 6 |
| Animate | 7 |
| Materials | 8 |
| Render | 9 |
| Video | 0 |
| Assets | *(none — click the tab or use the palette)* |
| Plugins | *(none — click the tab or use the palette)* |

## Tools

| Tool | Key |
| --- | --- |
| Select | Q |
| Move | W |
| Rotate | E |
| Scale | R |
| Sculpt | T |
| Paint | B |
| Text | X |
| Hand | H |

## Creation, mesh and UV

Primitives, rigs, mesh operators, UV projections, sculpt brushes, image
operations and material presets have **no default bindings**; run them from the
menus, the workspace buttons, or the palette (for example type `bevel` or
`subdivide`). You can assign keys to any of them in the shortcut editor.

| Command | Default |
| --- | --- |
| Add text layer (GFX) | Ctrl+T |
| Add layer group (GFX) | Ctrl+G |

## Render

| Command | Default |
| --- | --- |
| Render still | F12 |
| Refresh preview | F5 |

## Video and animation

| Command | Default |
| --- | --- |
| Play / pause | Space |
| Split clip at playhead | Ctrl+B |
| Add marker | M |
| Keyframe selected transform | K |
| Delete keyframe at playhead | Shift+K |

## Command palette

Press **Ctrl+K**, type part of a command name, category or id, and use ↑/↓ to
move and Enter to run. For example:

- `bevel`, `subdivide`, `weld` — mesh operators
- `rig` — R6/R15 construction
- `quality` — render presets
- `albedo`, `normals` — render modes
- `chroma` — keying
- `blend` — GFX blend modes

Invalid bindings are rejected: a binding needs a key (for example `Ctrl+`,
`Hyper+Q` and `Ctrl+NotAKey` are refused).
