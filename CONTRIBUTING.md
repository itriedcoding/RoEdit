# Contributing to RoEdit Studio

Thanks for helping. This project values **honest, working features** over
surface area. A smaller correct change is better than a broad half-working one.

## Ground rules

- Do not add UI that is not wired to real behaviour. No placeholder buttons,
  fake progress, disabled-without-explanation controls or "coming soon" menus.
- If a feature is limited, document the limitation in the UI or in `docs/`.
- Never claim a build, test or release succeeded unless it did.
- Keep the offline-first promise: no telemetry, no network calls, no AI
  features.

## Setting up

```powershell
git clone https://github.com/itriedcoding/RoEdit.git
cd RoEdit
cargo test --workspace
cargo run --release
```

Prerequisites are described in [docs/build.md](docs/build.md).

## Before opening a pull request

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

- Add or update tests for the logic you changed. Engine code belongs in the
  relevant crate's `#[cfg(test)]` module; UI-free app logic can be tested
  through `RoEditApp::dispatch`.
- Keep new user-facing actions in the command registry so they appear in menus,
  the palette and the shortcut editor.
- Route project mutations through `Document::edit` (or snapshot once and use
  `silent_edit` for drags) so undo stays correct.
- For new import/export formats, update the capability matrix so the UI and
  docs stay truthful.
- For new extension operators, add the capability, validation, execution and a
  test with a sample pack.

## Code style

- Rust 2021, formatted with `rustfmt` defaults.
- Prefer small modules over large files; the app already splits panels by file.
- Document public items and explain non-obvious math.
- Errors: use `thiserror` in library crates, `Result<T, String>` at UI
  boundaries, and log with `log` macros rather than `println!`.
- Avoid new dependencies unless they are clearly justified and maintained.

## Commits and pull requests

- One logical change per commit; describe *why* in the message.
- Reference the issue in the PR description and say what you verified.
- Screenshots are welcome but must be real captures from a running build.
- If you cannot run the GUI (for example in CI or a headless environment), say
  so explicitly in the PR.

## Documentation

- User-facing docs live in `docs/`.
- Keep shortcut tables in sync with the command registry defaults.
- When behaviour changes, update the changelog under "Unreleased".

## Reporting issues

Use the bug report template and include Windows version, GPU, exact steps and
the relevant log lines. Feature requests should describe the workflow the
feature unblocks, not only the control to add.

## License

By contributing you agree that your contributions are licensed under the MIT
license in [LICENSE](LICENSE).
