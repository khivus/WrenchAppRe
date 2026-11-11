# Editor and contract sync verification

## Automated verification completed

- Workspace tests: 172 passed (89 UI/model tests and 83 core tests); one
  opt-in network download test remains ignored.
- Strict Clippy for the workspace and all targets passed; formatting passed.
- Tests cover real pointer-driven sidebar/table resizing, independent preview
  dropdowns, selection routing, optional ship-label color/font edits and
  reordering, DAT round trips, checksum preservation, backups and contract sync.
- Verification uses headless UI interactions and layout measurements. Live desktop
  inspection and in-game acceptance remain unverified.

## Manual checks in the app

Use a backed-up profile and keep EVE logged out while saving settings.

- Open **Window layout**, select a character and its account, then zoom in/out
  and use **Fit**. Titles, sample text, icons, rows and columns should scale
  together. Preview samples should stay inside their saved window rectangles.
- Drag the left/canvas/right dividers. Lists and editors should begin at the top,
  fill the available height, and scroll independently. Resize the app window and
  switch characters; content must remain reachable. Divider widths last for the
  current editor session.
- Repeat divider/scroll checks in **Probe formations**, **Confirmations**, the
  character/account browser, and the sync pane. Probe selection and transforms,
  confirmation bulk actions, table sorting/searching and target selection should
  continue working.
- Drag a character/account table-header edge to resize that column. Header and
  rows must stay aligned, and resizing must not change sorting. Double-click its
  edge to restore the default width. Optional corporation/alliance/date columns
  must still show/hide and horizontal scrolling must keep headers aligned.
- Use the three preview dropdowns: location, locked targets (Off or 1–12), and
  fighters (Off/3/4/5). Counts must be independent, docked scenes must hide
  in-space elements, and preview choices must not create pending file edits.
- Toggle **Visible windows only** in the left sidebar. Try search, individual
  eyes, category eyes and Show/Hide all. These should affect only preview
  visibility; saved open/minimized/stack state must remain unchanged.
- Select overview and chat windows from both the canvas and sidebar. The right
  editor must match the selected element. Select individual overview tabs in
  either place; the matching tab should be edited. Click targets, fighters and
  Neocom icons, and their sidebar entries, to open those editors.
- Use the right pane's **Window** tab to change geometry and state. Check undo,
  redo, reset, stack movement and Reset position and size. A reset should remove
  only that window/stack family's geometry after Save, with a backup.
- Edit overview tab names/colors, preset/bracket assignments, column order,
  visibility and widths, sorting, object groups, state filters, flag/background
  priorities, colors and blinking. Try the general **Options** toggles too.
  Preview changes should appear where supported; Save account and Save character
  must affect their respective DAT only, with verified backups.
- Expand **Overview filters** and select a preset directly. Expand **Saved
  overview values** and choose ship labels, global columns, tab assignments,
  legacy or other saved records. Check paging and list reordering; edit existing
  values and reopen them after saving. Ship-label font/color may start empty;
  entering a value should work without leaving a stale validation error.
- Enter an invalid width, color or tab name. Save must remain disabled until it
  is corrected or the draft is reloaded. Check that drafts survive switching
  characters/accounts and continue blocking app updates/profile replacement.
- Edit chat member width, input height and member visibility. Save account,
  reopen the same channel and verify the preview; switch stacked chat tabs too.
- Edit Neocom width, side and button order, fighter position, and target anchor,
  orientation and lock. Save the owning DAT and reopen it. Changing the target
  or fighter preview count must remain a preview-only operation.
- In Neocom, remove a saved shortcut/group and restore it from **Add Neocom
  button**. Add a standard shortcut such as Contracts, then save and reopen the
  character. Its icon, placement and command should work in EVE. Saved custom
  properties and nested group contents must survive; an item already pinned
  inside a group must not appear as an available duplicate.
- In **Tabs & columns**, change a source tab's visible columns, order and widths,
  then use **Copy columns to…** for a single tab, a window's tabs, and all other
  tabs. Check that the destination columns and widths match, while its name,
  filters, brackets, colors, assignments and sorting remain unchanged. Both
  account and character drafts need saving; unrelated tabs must stay unchanged.
- In chat/cargo previews, verify that the first message, member name, navigation
  entry and item start at the top of the content area. The vertical dividers must
  reach the chat input or cargo-capacity footer at multiple zoom levels.

## Live EVE checks

- With EVE closed, sync one source character to another (and accounts if desired).
  Log into the destination, open **Contracts → My Contracts**, and refresh.
  The current character/corporation should be resolved rather than the source
  character name. Ordinary sync removes `ui/mycontracts_filter_owner` from the
  output; it does not edit the source file. Market-only sync excludes contract
  filters. Exact backup restores and whole-profile copies remain exact copies.
- Open the edited overview in EVE and inspect tab assignments, filters, labels,
  colors, columns and widths. Reopen the client to check persistence. Legacy and
  temporary records are editable as saved data; their client interpretation is
  not established by a successful binary round trip.
- Check chat panes, target placement, fighters, Neocom and window stacks in
  space, station and citadel. The app uses schematic sample objects and cannot
  validate live ship capabilities or the exact runtime visibility of each window.

Unknown null/empty records retain their original structure. The editor does not
invent types or new schema for them. The profile-wide public YAML and installed
overview-package identity are outside the account overview editor.
