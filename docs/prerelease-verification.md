# v0.9.0-preview.1 verification

The upstream merge preserves the fork's main-window navigation, inline editor
tabs, resizable panes, draft persistence, smooth scrolling, and contract/item
identity boundaries. Sharing uses the existing account/character row menus and
the Probe formations toolbar; it does not restore upstream's detached editors.

Local automated checks: 184 workspace tests passed (91 UI/model tests and 93 core
tests); one optional live release-download test is ignored in the normal suite.
Formatting and strict Clippy are required. The release workflow repeats these
checks before packaging on Windows, Linux and macOS, and checks that every
established release package is present. Integrity hashes stay in the Actions log.

The tests cover share file/text round trips, malformed data, precise floats,
formation merging and undo, contract-safe imports, refusing imports over editor
drafts, inline navigation, previous editor controls, backups and checksums.

Live desktop and EVE behavior still require manual acceptance. Log EVE out before
applying settings; use the generated backups to restore the original if needed.

## Manual acceptance

- [ ] From an account/character row's menu, choose **Share settings**. Pick groups,
  add a note, and export a file or clipboard text. Chat/market are unchecked by
  default; search history and drone item assignments are unavailable.
- [ ] Choose **Import shared settings**, open the file or paste text, inspect the
  groups and added/changed/removed counts, then apply selected groups. Only the
  matching DAT kind should accept them; unrelated values should be retained.
- [ ] Keep a draft in each editor and attempt a shared-settings import. The import
  should refuse to write and leave the draft and preview intact. Save/reset the
  draft, retry, and confirm the import succeeds with the normal backup preference.
- [ ] In **Probe formations**, copy the selected/all formations or save a file;
  import into the editor. New formations should append once, duplicates should
  not multiply, and Undo should remove the imported batch without writing to DAT.
- [ ] Import a standard upstream Market share containing contract filters. Market
  preferences should import, while the destination's contract identity/filters
  stay unchanged. Normal full sync should still clear the cached My Contracts
  owner; verify **Contracts → My Contracts → Refresh** in the destination client.
- [ ] Check that all editor tabs remain inline, selectors and drafts survive tab
  changes, and sharing opens a dialog without moving the existing controls.
- [ ] Resize sidebars and table columns, scroll long lists, and check sorting,
  search, row selection, aliases, backup restore and profile-copy controls.
- [ ] In **Window layout**, test move/resize, snapping, undo/redo, stacks, geometry
  reset and preview-only visibility. Check the separate location/target/fighter
  dropdowns, matching element settings and account/character save ownership.
- [ ] Edit/copy overview columns, tabs, filters, brackets and appearance. Check
  Neocom add/remove/restore, ship-label fields, chat dividers and full-height
  cargo/chat columns. Reopen in EVE and verify persistence. See the more detailed
  [editor checklist](editor-verification.md).
- [ ] Download and start the appropriate native package on each supported system.
  Confirm the displayed version is `0.9.0-preview.1`, package architecture matches
  the machine, the app opens existing profiles, and settings/backups remain intact.
