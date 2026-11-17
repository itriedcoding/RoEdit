# Building and releasing

## Local build (Windows)

```powershell
cargo build --release
.\target\release\roedit.exe
```

Requirements: Rust stable with the MSVC toolchain and Visual Studio Build Tools
(Desktop development with C++). The binary is a single executable; assets such
as the bundled DejaVu fonts are compiled in.

## Tests and checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

The suite covers the engine crates and the application's document, command,
state, preview and image-operation logic, plus a test that loads and executes
the example extension pack. Shader sources are validated through `naga` in
`roedit-render` tests.

## Cross-checking the Windows target from Linux

A compile check for the Windows target does not need Windows, only the target's
standard library:

```bash
rustup target add x86_64-pc-windows-msvc
cargo check --workspace --target x86_64-pc-windows-msvc
```

`cargo check` type-checks Windows-only code paths (the `rfd` file dialogs are
gated with `cfg(windows)`). It does not link, so it does not prove the final
executable links or runs; the CI workflow performs a real `cargo build
--release` on `windows-latest`.

## Continuous integration

`.github/workflows/windows.yml` runs on pushes to `main`, pull requests, tags
matching `v*`, and manual dispatch:

1. **test** — format check, clippy (for Linux and for the
   `x86_64-pc-windows-msvc` target, which type-checks the Windows-only code
   paths) and `cargo test --workspace --all-targets` on `ubuntu-latest`.
2. **build** — `cargo build --release` on `windows-latest`, then packages
   `roedit.exe`, `README.md` and `LICENSE` into `roedit-windows-x64.zip` and
   uploads it as an artifact.
3. **release** — downloads the artifact and publishes a GitHub release with
   `roedit-windows-x64.zip` and its SHA-256 attached. On a push to `main` the
   release tag comes from the `version` field in `Cargo.toml` (`v0.1.0`); a
   push of a `v*` tag publishes that tag instead. Re-running updates the same
   release rather than creating duplicates.

## Cutting a release

Bump `version` in the workspace `Cargo.toml`, update `CHANGELOG.md`, and push to
`main`. The workflow tests, builds and publishes `v<version>`. To release an
specific tag explicitly:

```bash
git tag v0.1.0
git push origin v0.1.0
```

There is no code signing; users should verify the SHA-256 of the downloaded
archive against the release notes if they need integrity checking.

## Versioning

The application follows semantic versioning. The project file format has an
independent `FORMAT_VERSION` (currently 1); loading a newer format version is
rejected with an explicit error rather than silently dropping data.

## Reproducibility notes

- `Cargo.lock` is committed; CI builds use it.
- The Windows build uses the default `wgpu` backend set; no GPU SDK is needed
  at build time.
- Fonts under `assets/fonts` are bundled at compile time through
  `include_bytes!` in `roedit-image`.
