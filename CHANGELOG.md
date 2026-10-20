# Changelog

## 0.7.0 - 2026-10-20

### Added

- **Sync profile...** copies the selected profile's entire settings folder to a
  named profile on another server or the same server, including every account,
  character, `core_public__.yaml`, `prefs.ini` and subfolder. Empty server folders
  are available as destinations, and **Choose folder...** supports other folders.
- Existing destination profiles are replaced completely. With **Back up before
  changes** enabled, their complete previous folder is retained under
  `.eve-wrench-profile-backups` beside the server's profile folders.

### Changed

- Move the custom account name's pencil button to the left of the name.

### Fixed

- UI tests use the platform's Select All shortcut (Command+A on macOS,
  Ctrl+A on Windows/Linux), so account-name editing, drone naming and profile
  copying replace the existing input correctly on every CI platform.

## 0.6.0 - 2026-10-20

### Added

- Background fork-only release checks at startup and every six hours, plus a
  manual Settings check. Updates appear in a compact top-right indicator.
- Windows portable self-update without elevation: SHA-256/PE verification,
  same-path replacement after exit, restart, rollback on failure, and staging
  cleanup. Protected folders report an inline error. Unsaved editor drafts
  must be saved before restarting.
- Drone naming tokens `{type}` and `{type_short}` for size/type names such as
  Small, Medium, Heavy, Sentry and Logistic, or Sml, Med, Hev, Sen and Logi.
- Saved **What to copy** switches for **Drone group names & ship favorites**
  (on by default) and **Include drone assignments** (off by default). Enable
  groups/favorites and leave assignments off to copy definitions while keeping
  each target's own drone memberships.
- When preserving assignments, matching group names keep the target's drone
  items and group IDs, new groups arrive empty, and occupied target-only groups
  are retained. Favorite references follow the retained group IDs. For a name
  collision between different drone categories, the target group is kept.
- Inline **Probe formations** and **Drone groups** tabs after **Backups**, with
  shared server/profile/account context and drafts retained per account.
- Prompts after editor saves to sync only the edited section to selected
  accounts, preserving unrelated settings and following the drone save mode.
- Drone saves explicitly offer **Append missing groups** or **Replace all drone
  groups**. Replace removes previous groups and memberships; append retains them.
- Manual account and character sources can be selected together, like defaults.
- `build.cmd` skips release building while an EVE Wrench process is running and
  uses one canonical executable filename.

### Fixed

- Drone editor sync now carries the Append/Replace choice to every selected
  account. Replace removes old groups and memberships instead of retaining
  occupied target groups; Append keeps all existing groups, even empty ones.
  The sync prompt explains the mode. Backups and unrelated settings are preserved.
- Short drone type names consistently start with a capital letter. Previously
  saved lowercase names remain recognizable.
- Drone naming accepts `{name}` anywhere, including type or stats first.
  Existing groups are recognized after reordering, and the naming input shows
  an example before validation. `{name}` stays required outside optional sections.
- Refresh counts use two short lines so server and backup totals remain visible.
- Reload/settings controls moved out of the native title-bar drag region.
  Reload feedback is inline instead of a notification that could cover controls.
- Rename the probe **Sphere** pattern to **Cube**.
- Account custom-name editing now fills its table column instead of shrinking
  the input to a few pixels. Long names keep the pencil button visible. Adding,
  renaming, clearing, saving with Enter or the checkmark, and cancelling are
  covered by a headless UI test with disk persistence checks.

### Notes

- Windows x64 portable release verified with 89 workspace tests, formatting,
  Clippy with warnings denied, a real helper replacement/restart fixture,
  and a live fork-release download with SHA-256 verification.
- Release builds omit `update-preview`; the Update button appears only when
  a newer fork release is available.
- Drone groups and ship favorites are stored per account. Their membership uses
  individual drone item IDs; copying them does not recreate a drone loadout or
  reassign replacements after repackaging or reloading. Automatic restoration
  would need a mapping from the new drone item IDs to saved groups; the settings
  alone do not provide it. No live EVE verification has been performed.

## 0.5.1 - 2026-10-19

### Added

- Merge upstream drone group presets from `570cf45`: open **Drone groups** in
  an account's menu to select drone families, customize names with max-skill
  statistics, and review changes before saving.
- Bundled drone catalog, cached item icons, background catalog refresh from
  upstream, and the standalone catalog generator and workflow.
- Save the naming pattern alongside the fork's existing source, copy-group,
  and character-column preferences. Drone group writes use automatic backups
  and preserve groups that contain drones.

### Release

- Windows x64 portable executable with SHA-256 checksums.
- Verified on Windows with formatting, Clippy with warnings denied, and 68
  workspace tests, including drone file round-trips, backups, checksum
  preservation, saved preferences, and headless dialog interactions.
- Live EVE testing was not performed. macOS and Linux binaries are not included.

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
