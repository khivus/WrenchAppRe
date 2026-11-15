# Overview editor changes — manual verification

The automated checks use headless GPUI and synthetic DAT fixtures. Desktop appearance and acceptance by the live EVE client remain unverified. Save using a backed-up profile with EVE logged out.

## Preview and selection

- Open Window layout, choose the character/account, and click an overview title, empty body, or sample row. The right pane should open that overview's editor. Select a different tab, select another window, and return by clicking the overview body: the editor should retain the tab currently displayed in that window.
- Scroll the canvas wheel one notch in each direction, without moving the pointer or performing another action. The overview should redraw immediately. Also check zoom buttons, Fit, dragging and resizing; all preview content should stay inside window bounds.
- Compare the overview and selected-item preview with medium EVE font at 90% UI scaling. The overview uses 10.8-pixel text and 16.2-pixel rows at desktop scale with a condensed Windows font approximation. EVE's font is not bundled; this is a layout estimate, not a precise glyph-width guarantee. Non-Windows systems may use a fallback font.
- Narrow Corporation and Alliance until `[WWWWW]` loses its closing bracket, and narrow distance/velocity/name/type columns. Content should clip at column edges, including partial glyphs, without ellipses or spillover. Check column resizing, visibility, order, sorting and copying columns still work. The selected-item sample now includes a thumbnail placeholder and two text lines above the action row.

## Filters and colors

- In Overview → Filters, expand Ship, Celestial and another category. All categories and state filters should be reachable in one scrolling page, without pagination. Search by category name, group name and ID; matching categories should expand. Clear search and toggle Selected groups only.
- Use a category's switch to select all, then deselect all. Its selected/total count should match. Change an individual group and check the count and preset update; other categories and unknown saved group IDs should survive. Presets shared by tabs should remain shared.
- In Colors, test both color tags and backgrounds. The color square should show the saved custom color; open it, choose a palette color, enter a custom hex color, and use Default to remove the override. Check enabled and blinking switches still work. Repeat for tab colors and Ships label colors.

## Ship labels and tab placement

- In Overview → Ships, use an account with saved shipLabels. Toggle visibility, bold, italic and underline; edit existing prefixes, suffixes and font sizes; choose/reset custom colors. Move components up and down. The local label sample should follow saved order and line breaks. Unknown fields should survive saving. Accounts without saved labels explain that the initial configuration must be created in EVE and reloaded.
- In Overview → Tabs & columns, choose the destination saved overview window. The tab should disappear from its previous window and appear at the destination. Move it left/right and check preview order, names, filters, widths and sorting remain attached to the same tab. Save account, reload, and confirm placement/order persist. This editor moves tabs among existing saved window instances; it does not create a new overview window or its character geometry.
- Log into EVE and check tab placement/order and bracket labels. If checking labels for unselected objects, enable the existing Always Show Bracket Text option and restart the client as required. Client acceptance of externally reordered tabs remains a manual check.

## Neocom, settings and sync

- Open Neocom → Add button. Try supported shortcuts (for example Wallet, Market, Fitting, Mail, Fleet, Contracts or The Agency) and any saved definitions. Already pinned items should be excluded. Remove/restore an item and reorder items; custom icons and nested saved groups should survive. The catalog is conservative and does not claim to list every EVE command.
- Open the settings popover in a short app window and scroll to its bottom. The complete version string and update button should remain reachable. Check folder selection, backup toggle, language, import/export, bracket text and theme controls.
- Confirm Force D-scan to 14.4 AU when syncing account settings is initially off and persists after restarting the app. With it off, sync retains ordinary copy behavior. With it on, sync account settings to multiple destination accounts, then inspect D-scan in EVE: each destination should display maximum range (14.4 AU). Character-only sync must be unaffected. This preference changes normal account sync destinations; it does not immediately rewrite all profiles or the source account.
- Save account and character edits separately, reload both, and check verified backups and unrelated preferences. Recheck geometry undo/redo, window stacks, chat settings, target/fighter previews, and selective sync exclusions.

## Follow-up: privacy, pinned controls and simulated grid

- Selected item is Jita. Static character examples use fictional Citizen / Example Character names; the simulated grid may use character names already resolved in the local profile, without embedding them in source files. Generic fallback display dimensions remain generic; real screen measurements are used only to edit the selected layout.
- Scroll long filters and Ships pages: tab selector, section selector and scope description must remain at the top. Root category arrows must share a left edge, with child groups indented beneath them.
- Move native ship label records in both directions, save and reload. Reordering must preserve opaque client fields and numeric component/color representations.
- Select tabs with different object-group and state presets. The simulated grid should change, including fleet/corporation/alliance, standings and other relationship scenarios. Always-show states override hidden states on a matching object; groups still gate inclusion. Check the Options switches controlling application to structures/other objects.
- The cached simulation includes a representative for every bundled eligible group and each supported state, plus overlapping relationship examples. Rendering is capped to window capacity. This estimates filter behavior; it is not live EVE telemetry, exhaustive combinations of states, or a complete item-type database. Groups without a specific ship representative use their catalog name.
