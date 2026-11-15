# v0.9.0-preview.2 verification

This prerelease includes the fork's overview/filter/ship-label editor changes,
bounded object previews, native chat divider fixes, docked bookmark visibility,
header-only collapsed windows and preserved Save controls.

## Selective upstream integration

- Included upstream `58976d0`: confirmation dialogs for client build 3687575.
- Included upstream `87001c9`: static Windows C runtime, removing the portable
  executable's dependency on a separately installed VC++ redistributable.
- Excluded upstream `03a098f`: drone catalog update, as requested.
- Excluded upstream `162e49a`: upstream version bump; the fork uses its own
  `0.9.0-preview.2` public version.

## Automated release requirements

The release workflow runs formatting, strict all-target Clippy and workspace
checks on Windows, Linux, Apple Silicon macOS and Intel macOS, then packages
and verifies the established nine release assets. Integrity hashes remain in
Actions logs rather than the public release. Both workspace crates and the
application display use version 0.9.0-preview.2.

## Manual acceptance

Desktop appearance and live EVE acceptance remain manual. Use a backed-up
profile and keep EVE logged out when applying settings.

- Follow the [overview/chat/window checklist](overview-editor-verification.md),
  including friendly filters, subdued backgrounds, random ship names and types,
  tab placement/order, label reordering, native chat widths/heights, collapsed
  header alignment, docked bookmarks and Save-button visibility.
- Check the [existing editor and sync checklist](editor-verification.md),
  including selective sync exclusions, sharing and destination identity safety.
- Check the new confirmation entries and preserve existing confirmation choices.
- Start the Windows portable executable on a machine without the VC++ runtime.
- Start each native platform package; verify the version, architecture, update
  discovery, existing profiles and backups.

Public release notes are maintained in
[the release body](releases/v0.9.0-preview.2.md).
