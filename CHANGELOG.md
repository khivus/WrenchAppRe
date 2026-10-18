# Changelog

## 0.4.1 - 2026-10-18

First release of khivus's fork, based on EVE Wrench 0.4.0.

### Added

- Separate character and account tables, with independent scrollbars and numeric
  ID sorting. Account IDs and custom names have separate sortable columns.
- Search across both tables, including names, aliases, IDs, corporation, alliance,
  creation date, profile, server, file path, entry type, and modification time.
- Saved default character and account sources, with mixed targets receiving
  settings from the source of the matching type.
- A profile selector below server selection, plus buttons to add all compatible
  targets from the current profile, current server, or all servers.
- Saved character column preferences: name, ID, and modification time by default;
  optional corporation, alliance, and creation date from EVE's public API.
- Saved copy-group preferences on each checkbox click.

### Changed

- Moved bulk target controls below the default-source selectors.
- Moved theme and bracket-text options into the top-bar cogwheel menu.
- Render only visible browser and target rows, use indexed target membership,
  and defer copy-plan allocation until copying to keep large selections responsive.
- Use smaller, animated browser mouse-wheel scrolling steps.
- Disable upstream update checks and installation for this fork.

### Fixed

- Account search now filters accounts by ID and custom name as well as metadata.
- Theme switching updates the window immediately and persists across restarts.
- Character and account scrollbar interactions remain independent.
- Mouse-wheel animation no longer crashes the app.
- Target backgrounds span the full row, and server badges align on the right.

### Release

- Windows x64 portable executable with SHA-256 checksums.
- Verified on Windows with formatting, Clippy with warnings denied, and 59 tests,
  including headless UI checks with 400 selected targets.
- macOS and Linux binaries are not included in this release.
