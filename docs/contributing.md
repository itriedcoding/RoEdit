# Contributing to the documentation and code

The full guide is [CONTRIBUTING.md](../CONTRIBUTING.md) at the repository root.
This page lists documentation-specific expectations.

## Documentation rules

1. **Describe what the build does.** If a feature is partial, say which part
   works and which does not.
2. **Keep the format matrix in sync** with `roedit_io::list_formats()` and
   `roedit_video::codec::support_matrix()`. If you add a format, update the code
   first and the docs second.
3. **Shortcut tables must match the registry.** Defaults live in
   `crates/roedit-app/src/command.rs`; the table in `docs/shortcuts.md`
   duplicates them for readers.
4. **Do not add screenshots that are mockups.** Real captures only, and only
   from a build that actually runs.
5. **Tutorials must be reproducible.** Every click should map to a real
   command, and expected output (log lines, file paths) should be accurate.

## Adding a page

- Put it in `docs/`, link it from the README table and, if relevant, from
  `docs/getting-started.md`.
- Use sentence case headings and fenced code blocks with a language tag.
- Keep instructions Windows-first; note POSIX differences in a short paragraph
  rather than rewriting every command.

## Review checklist

- [ ] Commands and menu paths exist in the current UI.
- [ ] File paths and settings locations are accurate.
- [ ] Limitations are stated, not implied away.
- [ ] Links resolve (relative links inside `docs/`).
- [ ] No fabricated test results, benchmark numbers or release dates.
