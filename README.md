<h1 align="center">EVE Wrench Re</h1>

<p align="center">
  <strong>A settings manager for EVE Online</strong><br>
  Back up, restore, sync, and edit your account and character settings, safely.
</p>

<p align="center">
  <img alt="CI" src="https://github.com/khivus/WrenchAppRe/actions/workflows/ci.yml/badge.svg">
  <img alt="Latest release" src="https://img.shields.io/github/v/release/khivus/WrenchAppRe?include_prereleases&sort=semver">
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-informational">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-blue">
</p>

---

This is fork of [EVE Wrench](https://github.com/eve-wrench/eve-wrench-app).
Fork releases continue with standard semantic versions, starting at
**0.4.1**. See [CHANGELOG.md](CHANGELOG.md) for the changes. Update checks use
this fork's releases only.

<img width="1440" height="800" alt="image" src="https://github.com/user-attachments/assets/8d8886f2-8a9c-4193-aa53-71ceea77b618" />

## What it does

EVE stores every account and character's UI settings (overview, window layout,
probe formations, keybinds, and more) in local binary files. Managing those by
hand means copying opaque `.dat` files around and hoping you didn't clobber
something. EVE Wrench turns that into a safe, visual workflow.

- **🔄 Sync** settings from one account or character to many, and choose exactly
  which parts to copy
- **💾 Back up & restore** any account or character, with automatic backups
  before every change
- **🛰️ Edit probe formations** in a dedicated 3D editor, with presets, no client
  restart required
- **🤖 Set up drone groups** named with max-skill stats, picked from every drone
  in the game
- **🗂️ Export & import** your whole setup as a single portable archive
- **🏷️ Identify** accounts and characters with ESI portraits and custom aliases
- **🌐 Every server**: Tranquility, Singularity, Thunderdome, and Serenity

## ❤️ Standing on the shoulders of TrueBrain

> **Everything EVE Wrench does beyond copying whole files exists because of
> [TrueBrain](https://github.com/TrueBrain).** His
> [blue-marshal-rs](https://github.com/TrueBrain/blue-marshal-rs) is a Rust
> implementation of EVE's `blue.Marshal` binary format, the thing that makes
> `core_user_*.dat` and `core_char_*.dat` files readable and writable at all.
> Building on CCP's previously published code, he worked out how the format fits
> together and built a lossless decode/encode round trip.
>
> Selective copying, the probe formation editor, and reading individual settings
> are all powered by his library. Thank you!

## Features

### 🔄 Settings sync

Pick a **source** account or character, add one or more **targets**, and copy in
one click. Account settings only copy to accounts and character settings only to
characters; the app enforces it.

**Selective copy.** Everything is copied by default, but each setting group has a
checkbox: **unchecked groups keep the target's own values.** So you can push your
overview, window layout, and suppressed dialogs everywhere without wiping a
character's hand-arranged module slots or typed search history. Only the groups
you select are touched; caches, per-station state, and edit history never
migrate, and every target is backed up first.

**Whole profiles.** Use **Sync profile...** beside the profile selector to copy
all accounts and characters together to a named profile on another server or
the same server. This copies the entire `settings_<name>` folder unchanged,
including `core_public__.yaml`, `prefs.ini` and subfolders. No existing account
or character files are required in the destination; **Choose folder...** also
lets you select an empty server folder outside the detected list.

An existing destination profile is replaced completely, including files that
only existed there. Close EVE before copying. If **Back up before changes** is
enabled, the entire previous folder is kept in `.eve-wrench-profile-backups`
beside the server's profile folders, with its exact path shown after copying.
These folder backups are separate from the Backups tab; restore one by copying
it back as the desired `settings_<name>` folder while EVE is closed.

**Account groups**

| Group                        | What it copies                                                                             |
| ---------------------------- | ------------------------------------------------------------------------------------------ |
| Overview profiles            | Overview presets, tab setup, state colors, and column layout                               |
| Probe formations             | Custom probe scanner formations                                                            |
| Drone group names & ship favorites | Group definitions and per-ship favorite groups (on by default)                          |
| Include drone assignments     | Also replace group memberships when copying drone groups (off by default)                   |
| Suppressed dialogs           | The "don't ask again" confirmation checkboxes                                              |
| Audio                        | Sound levels and muted sound categories                                                    |
| Camera & graphics            | Camera sensitivity and inertia, plus effect toggles (missiles, turrets, trails, asteroids) |
| Market & contracts           | Market filters, the market quickbar, and saved contract searches                           |
| Module slot layout           | HUD module arrangement and linked weapon groups (off by default)                           |
| Window tab groups            | Which windows are grouped together into tabs                                               |
| Search history & suggestions | Typed search history and autocomplete (off by default)                                     |

**Character groups**

| Group                        | What it copies                                                 |
| ---------------------------- | -------------------------------------------------------------- |
| Window layout                | Window positions, sizes, and open, stacked, or minimized state |
| Neocom sidebar               | The Neocom (left sidebar) button arrangement                   |
| Chat channels                | Your joined player chat channels                               |
| Info panels                  | In-flight, hangar, and structure info panel modes              |
| Docked panels                | Docked map and skill planner panel state                       |
| Search history & suggestions | Typed search history and autocomplete (off by default)         |

### 💾 Backups

- **Create** named backups from the ⋯ menu on any account or character
- **Restore** a backup to its original entity, or **apply** it to any compatible one
- **Bulk manage** in the Backups tab: sort by name or time, select multiple, and
  delete in one action
- **Automatic backups before changes**: toggle _"Back up before changes"_ in the
  settings menu (on by default); it guards selective copies and formation edits

Backups live inside EVE's own settings folders, so they're easy to find and
survive a reinstall of EVE Wrench.

### 🛰️ Probe formation editor

Open **Probe formations** in the navigation row after **Backups**. The editor
uses the main window, with visible server, profile, and account selectors. It
starts from the selected profile, or **Default** when no profile was selected,
and prefers the selected/default account within that profile. Drafts are kept
per account when switching tabs or accounts.

- **Presets** to start from: **Blank**, **Pinpoint**, **Drifter** (one probe
  parked behind the Drifter), and a **Directional** set that stacks all 8 probes
  along one axis (North, South, East, West, Up, Down)
- **Holographic 3D scanner** preview: drag to rotate, scroll to zoom anywhere in
  the pane, with compass axes (N/S · W/E · U/D), range rings, and a height tether
  under each probe. Theme-aware, light and dark
- **Precise editing**: per-probe coordinates in km, discrete scan ranges (0.25 to
  32 AU), reorder formations, and scale a whole formation (a negative factor
  shrinks it)
- **Safe saves**: writes re-encode the file losslessly and back it up first.
  Saving offers to sync only probe settings to chosen other accounts.
- The pattern previously labeled **Sphere** is now called **Cube**.

### 🤖 Drone groups

Open **Drone groups** after **Probe formations** in the navigation row. It uses
the same server, profile, and account selectors and stays in the main window.

- **Groups**: one group per drone family (Hobgoblin covers the T1, T2, navy and
  faction versions), one per EWAR effect and size (Light ECM, Heavy Stasis
  Web), plus logistics, mining, salvage and special drones, with their in-game
  icons
- **Naming**: pick a style or write a pattern such as
  `{name}[ | {damage}][ | {dps}][ | {speed}]`. Stats are the T2 drone at max
  skills, simulated with DarkFenX's [refine](https://github.com/DarkFenX/refine)
  engine: _Hobgoblin | Therm | 99dps | 4.2km/s_
  Add `{type}` for Small, Medium, Heavy, Sentry, Special, EWAR, Logistic or
  Industry; `{type_short}` gives Sml, Med, Hev, Sen, Spec, Ewar, Logi or Ind.
  For example, `{name} [{type_short}]` produces _Hobgoblin Sml_.
  Tokens can go in any order: `{type_short} | {name}` produces _Sml | Hobgoblin_.
  Keep `{name}` outside optional `[sections]` so every group has a family name.
- **Review** every group before saving, then choose **Append missing groups**
  or **Replace all drone groups**. Append preserves existing groups and drone
  memberships, adding only missing names. Replace deletes every previous drone
  group and membership and creates only the selected empty groups. Other account
  settings are preserved. Close EVE before saving.
- After saving, choose whether to sync group definitions and favorites to other
  accounts. Sync follows the same **Append** or **Replace** choice: Append keeps
  existing target groups and memberships; Replace removes them and installs only
  the saved empty groups. Unrelated settings stay unchanged. Automatic backups
  follow **Back up before changes**.

The app checks upstream for newer drone catalog data in the background and
keeps a bundled catalog available offline. Catalog refreshes do not install
or replace the app.

**Copying existing groups.** In **What to copy**, enable **Drone group names &
ship favorites** and leave **Include drone assignments** unchecked to copy
group definitions and ship favorites while preserving the target's memberships.
Groups with the same name keep the target's drone items and group IDs, new
source groups arrive empty, and target-only groups holding drones are retained.
Favorite references are updated to the retained group IDs. If the same name
belongs to different drone categories, the target group is kept.

Enable **Include drone assignments** as well to replace group memberships with
the source's individual drone item IDs. This option is available while group
copying is enabled. Leave **Drone group names & ship favorites** unchecked to keep
all of the target's groups, memberships, and favorites. Both choices are saved
automatically. Group names/favorites default to on; assignments default to off.
Explicitly saved checkbox preferences are retained.

Groups and ship favorites live in the account settings, rather than in each
character file. Membership refers to individual drone items, so copying groups
does not recreate a ship's drone types and quantities or assign another
character's drones. For reusable empty groups, use the **Drone groups** editor.
Named drone-only setup snapshots are not currently available; the existing
account backup saves the whole account file, including its drone settings.
Such a snapshot could restore assignments to the same drone items. Reapplying
groups to replacement drones after repackaging or reloading would also require
identifying the current drone item IDs and mapping them to the saved groups;
that information is not available from these settings alone.

### 🗂️ Import & export

Export **all** your settings, backups, and aliases into a single `.zip` archive
(with a checksummed manifest) to move between machines or share a setup. Importing
analyzes the archive first, showing new, unchanged, and conflicting files, and
lets you choose which conflicts to overwrite, backing up anything it replaces.

### 🏷️ Identity: portraits & aliases

- For **Tranquility** and **Singularity** characters, names, portraits, and
  corporations are pulled from EVE's **ESI** API (SISI mirrors TQ, so those
  resolve too)
- EVE doesn't store account names, only numeric IDs, so assign **aliases** to
  tell accounts apart. Aliases also help on servers where ESI isn't available
  (Thunderdome, Serenity). They're stored locally and persist

### 🌐 Cross-server support

| Server          | Description               |
| --------------- | ------------------------- |
| **Tranquility** | Main production server    |
| **Singularity** | Public test server        |
| **Thunderdome** | Tournament / event server |
| **Serenity**    | Chinese server            |

Each server's settings are separate; the app detects every installed server
automatically.

### ⚙️ Extras

- **Always Show Bracket Text**: show ship labels on all brackets in space, not
  just selected targets (handy for PvP awareness; may cost performance in 200+
  pilot fights; needs a client restart)
- **Fork updates**: a small top-right indicator and Update button replace the
  update dialog. Checks run at startup, every six hours, and from Settings.

---

## Installation

Grab the latest fork build from [**Releases**](https://github.com/khivus/WrenchAppRe/releases).

Choose the package for your system:

| System | Downloads |
| --- | --- |
| Windows x64 | Portable `.exe` (run directly) or `x64-setup.exe` installer |
| macOS Apple Silicon | `aarch64-apple-darwin.dmg` or `.app.tar.gz` |
| macOS Intel | `x86_64-apple-darwin.dmg` or `.app.tar.gz` |
| Linux x64 | `.deb`, `.AppImage`, or `x86_64-linux.tar.gz` |

**SHA256SUMS.txt** contains checksums for all release downloads.

On Arch-based distros, build a package from the bundled PKGBUILD:

```bash
git clone https://github.com/khivus/WrenchAppRe.git
cd WrenchAppRe/packaging/aur
makepkg -si
```

If the app doesn't start, run it from a terminal with `RUST_LOG=debug` and include
the output in your issue. The same log is written to `eve-wrench.log` in the data
folder (`~/.local/share/com.timkunze.eve-wrench/` on Linux).

## Quick start

1. **Launch**: EVE Wrench scans for installations and lists every account and character
2. **Pick a server** with the tabs at the top, then a **Profile** directly below
3. **Set a source** with the ↑ button on any row (or the ⋯ menu)
4. **Add targets** with the ↓ button on compatible rows
5. **Choose what to copy** in the right panel, then hit **Copy Settings**
6. **Back up first** from the ⋯ menu whenever you want a manual safety net

> **Tip:** Accounts and characters start in numeric ID order. Click a column header
> to sort by ID, name, or modification time; the table titles are plain labels. Use
> _"Add all"_ to target a whole profile, and give accounts aliases so you're not
> staring at raw IDs.

Accounts have separate **Account ID** and **Custom name** columns, which sort
independently. The character table starts with **Name**, **Character ID**, and
**Modified**. Its **Columns** menu adds **Corporation**, **Alliance**, and **Created**;
each optional column is sortable, and the selection is saved immediately. Wider
tables scroll horizontally while keeping their headers aligned. Character names,
corporations, alliances, and creation dates use EVE's public API without login.

The search box filters both accounts and characters by names, custom aliases, IDs,
corporations, alliances, creation dates, profiles,
servers, settings file paths, entry types, and displayed modification times.
Search is case-insensitive; every word must match somewhere in the row's details.
Clear the search to show all entries again. While searching, **Add all** adds only
matching entries from that profile. Use the scrollbar or mouse wheel to browse
long lists.

Only the selected profile's accounts and characters appear, in two side-by-side
tables with characters on the left, accounts on the right, independent scrollbars,
and fixed sorting headers. Each
server remembers its selected profile while the app is open. Below the default
character/account selectors, **Add targets: This profile / This server / All servers** adds every
compatible account and character in that scope, including entries hidden by
search. These buttons preserve existing targets and skip source files and
duplicates. With a saved character and account source, both types are selected;
with a single source, only its type is selected. Selection does not copy files;
use **Copy Settings** and confirm to apply the changes.

Long browser and selected-target lists render only visible rows, keeping large
target selections inexpensive to repaint. Mouse-wheel scrolling in the browser
uses smaller, animated steps. Default-source menus also use numeric ID order.

The navigation-row cogwheel contains the **Dark theme** switch; changes apply immediately
and are saved for the next launch. **Extra / Always Show Bracket Text** is also
there, with a separate switch for each detected server.

**What to copy** preferences are saved on every checkbox click and restored at
startup. Account and character choices are retained when switching sources or
profiles; options without a saved preference keep their original defaults.

Above search, **Default character + account** lets you choose and save both source
files. Choose the account explicitly: EVE's settings files do not identify which
account owns a character. The saved choices are restored at startup. You can add
account and character targets together; each receives settings from its matching
source, using the normal confirmation, copy options, and automatic backups.
Selecting an account and a character manually keeps both as sources, with each
target using the matching type. Selecting another source of the same type
replaces only that source. Manual backups remain single-source.
Manual selections temporarily override the defaults; **Use default**
restores the saved pair. Choose **None** in either selector to remove that saved
choice. If a saved file is unavailable, the app shows that status and disables
targets of that type until a valid source is chosen.

---

## How EVE stores settings

EVE keeps all settings locally. Understanding the layout helps with manual
troubleshooting.

**Root directory**

| OS      | Path                                                                                                |
| ------- | --------------------------------------------------------------------------------------------------- |
| macOS   | `~/Library/Application Support/CCP/EVE/`                                                            |
| Windows | `%LocalAppData%\CCP\EVE\`                                                                           |
| Linux   | `~/.local/share/Steam/steamapps/compatdata/8500/pfx/drive_c/users/steamuser/AppData/Local/CCP/EVE/` |

**Inside a profile folder**

```
settings_Default/
├── core_user_12345678.dat      # Account settings (window layout, overview, audio)
├── core_char_90000001.dat      # Character settings (chat, info panels, dscan)
├── prefs.ini                   # Client preferences (some not exposed in-game)
└── backups/                    # EVE Wrench backups
    └── {name}_{user|char}_{id}_{timestamp}.bak
```

`core_user_*` files are keyed by **account** ID; `core_char_*` by **character** ID
(IDs of 90,000,000 and above are player characters). These `.dat` files are
`blue.Marshal` binary, which is exactly what blue-marshal-rs decodes.

---

## Development

**Prerequisites:** [Rust](https://rustup.rs/) stable. On Linux, GPUI also needs
`clang libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev libvulkan1`.

```bash
git clone https://github.com/khivus/WrenchAppRe.git
cd WrenchAppRe
cargo run -p eve-wrench
```

The workspace has two crates:

| Crate                     | Contents                                                                        |
| ------------------------- | ------------------------------------------------------------------------------- |
| `crates/eve-wrench-core`  | Settings discovery, backups, selective copy, formations, drone groups, archives, ESI, updates |
| `crates/eve-wrench`       | The desktop app, built with [GPUI](https://www.gpui.rs/) and [GPUI Kit](https://gpui-kit.com) |

**Commands**

On Windows, close EVE Wrench and run `.\build.cmd` from the repository.
It checks for running EVE Wrench
processes and skips the build if any are open. Successful builds use the single
`target/release/eve-wrench.exe` filename. Direct equivalent:
`cargo build --locked --release -p eve-wrench` after closing the app.

Reload and settings controls are in the navigation row, outside native window
dragging. Reload feedback stays inline so it cannot cover the controls.

| Command                                            | Description                        |
| -------------------------------------------------- | ---------------------------------- |
| `cargo run -p eve-wrench`                          | Run the app                        |
| `cargo test --workspace`                           | Run all tests                      |
| `cargo clippy --workspace --all-targets`           | Lint                               |
| `cargo fmt --all`                                  | Format                             |
| `cargo packager --release -p eve-wrench`           | Build installers ([cargo-packager](https://github.com/crabnebula-dev/cargo-packager)) |

Translations live in `crates/eve-wrench/locales` (English and Simplified Chinese).

**Drone data.** `crates/eve-wrench-core/data/drone_groups.json` is generated by
`tools/drone-catalog`, which downloads the SDE and simulates every drone with
refine. The tool is not part of the workspace: refine needs the nightly pinned in
its `rust-toolchain.toml`, and it is AGPL licensed, so it is never linked into
the app. Run `cargo run --release` in that folder to regenerate the file. The
_Drone catalog_ workflow checks daily for a new SDE build and opens a pull
request with the regenerated file. This fork fetches runtime catalog updates
from the original `eve-wrench/eve-wrench-app` repository's `main` branch.
It needs _Allow GitHub Actions to create and approve pull requests_ enabled in
the repository's Actions settings.

CI runs `cargo fmt`, `clippy -D warnings`, and `test` on every push and PR.

**Portable updates.** The app checks `khivus/WrenchAppRe` releases. Windows x64
updates use `EVE-Wrench_<version>_x64-portable.exe` and GitHub's SHA-256 asset
digest. The app downloads and verifies the executable, then an unelevated
helper waits for it to close, replaces the original executable at the same
path, and restarts it. A backup is kept until the new window opens; failed
replacement/restart restores the old executable. Settings are stored separately.
The app must be in a folder you can write to, such as Downloads or a personal
Apps folder. Protected folders show an inline error; there is no UAC request.
Releases without a verified matching asset link to their release page.

`build.cmd --features update-preview` keeps the top-right button visible even
when no update is available. This is an appearance preview; normal `build.cmd`
and GitHub release builds omit the flag and only show available updates.

**Tech stack:** Rust, GPUI, GPUI Kit,
[blue-marshal](https://github.com/TrueBrain/blue-marshal-rs), EVE ESI, Lucide.

## License

MIT

---

<p align="center">
  <sub>Not affiliated with CCP Games. EVE Online and all related logos are trademarks of CCP hf.</sub>
</p>
