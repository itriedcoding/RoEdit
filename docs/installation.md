# Installation

RoEdit Studio targets 64-bit Windows 10 and Windows 11.

## System requirements

| Component | Minimum | Recommended |
| --- | --- | --- |
| OS | Windows 10 21H2 x64 | Windows 11 x64 |
| CPU | 4 cores with SSE4.2 | 8+ cores (the path tracer scales with cores) |
| RAM | 4 GB | 8–16 GB for large scenes |
| GPU | DirectX 12 capable adapter (for the wgpu viewport) | Any recent discrete GPU |
| Disk | 150 MB | 1 GB for renders and exports |

The CPU path tracer does not need a GPU. A machine without a DX12 adapter can
still render, but the wgpu-based 3D viewport pipeline cannot initialise.

## Option 1: download a release

1. Open the repository's **Releases** page.
2. Download `roedit-windows-x64.zip` from the newest tag.
3. Extract the archive anywhere writable (for example `C:\Tools\RoEdit`).
4. Run `roedit.exe`.

There is **no installer and no code signing** yet. Windows SmartScreen may warn
about an unsigned executable; that is expected for locally built or newly
published binaries.

Releases are produced by the workflow in `.github/workflows/windows.yml`. If no
release exists yet, build from source.

## Option 2: build from source

Prerequisites:

- [Rust](https://rustup.rs/) stable with the default MSVC toolchain
  (`rustup default stable-x86_64-pc-windows-msvc`)
- Visual Studio 2022 Build Tools with the **Desktop development with C++**
  workload (provides the linker and Windows SDK)
- Git

```powershell
git clone https://github.com/itriedcoding/RoEdit.git
cd RoEdit
cargo build --release
.\target\release\roedit.exe
```

The first build downloads and compiles the dependency graph (egui, wgpu, image
codecs, fontdue) and takes several minutes. Incremental builds are much faster.

## Where RoEdit stores data

| Data | Path |
| --- | --- |
| Settings | `%APPDATA%\RoEditStudio\settings.json` |
| Autosave | `%APPDATA%\RoEditStudio\autosave.roedit` |
| Extension packs | any folder you choose; the default suggestion is `%APPDATA%\RoEditStudio\plugins` |
| Renders (default) | `<working directory>\renders\` |
| Exports (default) | `<working directory>\exports\` and the timeline export folder |

Settings are written on exit and when you press **Save settings**. The autosave
file is removed after a successful manual save and is offered for recovery on
the next launch if the application closed with unsaved changes.

## Uninstalling

Delete the extracted folder. Optionally delete `%APPDATA%\RoEditStudio` to
remove settings, autosave and extension packs.
