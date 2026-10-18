<h1 align="center">EVE Wrench</h1>

<p align="center">
  <strong>A settings manager for EVE Online</strong><br>
  Back up, restore, sync, and edit your account and character settings, safely.
</p>

<p align="center">
  <img alt="CI" src="https://github.com/khivus/eve-wrench-app/actions/workflows/ci.yml/badge.svg">
  <img alt="Latest release" src="https://img.shields.io/github/v/release/khivus/eve-wrench-app?include_prereleases&sort=semver">
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-informational">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-blue">
</p>

---

This is [khivus's fork](https://github.com/khivus/eve-wrench-app) of
[EVE Wrench](https://github.com/eve-wrench/eve-wrench-app), based on upstream
0.4.0. Fork releases continue with standard semantic versions, starting at
**0.4.1**. See [CHANGELOG.md](CHANGELOG.md) for the changes. Upstream update
checks are disabled; download new fork versions from this repository's releases.

<img width="1904" height="1021" alt="eve-wrench-demo" src="https://github.com/user-attachments/assets/83479f1b-ae8e-4348-93ee-5eb28de5ae72" />

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

**Account groups**

| Group                        | What it copies                                                                             |
| ---------------------------- | ------------------------------------------------------------------------------------------ |
| Overview profiles            | Overview presets, tab setup, state colors, and column layout                               |
| Probe formations             | Custom probe scanner formations                                                            |
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

Open the ⋯ menu on any **account** and pick **Probe formations** to launch a
dedicated editor window (with the same custom title bar as the main app).

- **Presets** to start from: **Blank**, **Pinpoint**, **Drifter** (one probe
  parked behind the Drifter), and a **Directional** set that stacks all 8 probes
  along one axis (North, South, East, West, Up, Down)
- **Holographic 3D scanner** preview: drag to rotate, scroll to zoom anywhere in
  the pane, with compass axes (N/S · W/E · U/D), range rings, and a height tether
  under each probe. Theme-aware, light and dark
- **Precise editing**: per-probe coordinates in km, discrete scan ranges (0.25 to
  32 AU), reorder formations, and scale a whole formation (a negative factor
  shrinks it)
- **Safe saves**: writes re-encode the file losslessly, back it up first, and
  stay in sync live with the main window

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
- **Upstream updates disabled in this fork**: startup checks, manual checks, and
  the upstream update dialog are commented out.

---

## Installation

Grab the latest fork build from [**Releases**](https://github.com/khivus/eve-wrench-app/releases).

Version **0.4.1** provides **EVE-Wrench_0.4.1_x64-portable.exe** for Windows x64.
Download and run it directly; no installation is needed. The release also includes
SHA-256 checksums. macOS and Linux source support and packaging workflows are
retained from upstream, but this fork release does not include builds for them.

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

The top-bar cogwheel contains the **Dark theme** switch; changes apply immediately
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
Selecting another source temporarily overrides the defaults; **Use default**
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
git clone https://github.com/khivus/eve-wrench-app.git
cd eve-wrench-app
cargo run -p eve-wrench
```

The workspace has two crates:

| Crate                     | Contents                                                                        |
| ------------------------- | ------------------------------------------------------------------------------- |
| `crates/eve-wrench-core`  | Settings discovery, backups, selective copy, formations, archives, ESI, updates |
| `crates/eve-wrench`       | The desktop app, built with [GPUI](https://www.gpui.rs/) and [GPUI Kit](https://gpui-kit.com) |

**Commands**

| Command                                            | Description                        |
| -------------------------------------------------- | ---------------------------------- |
| `cargo run -p eve-wrench`                          | Run the app                        |
| `cargo test --workspace`                           | Run all tests                      |
| `cargo clippy --workspace --all-targets`           | Lint                               |
| `cargo fmt --all`                                  | Format                             |
| `cargo packager --release -p eve-wrench`           | Build installers ([cargo-packager](https://github.com/crabnebula-dev/cargo-packager)) |

Translations live in `crates/eve-wrench/locales` (English and Simplified Chinese).

CI runs `cargo fmt`, `clippy -D warnings`, and `test` on every push and PR.

**Auto-updates.** Disabled in this fork. The original updater code and release
packaging configuration are retained for reference; upstream checks and the
installation UI are commented out.

**Tech stack:** Rust, GPUI, GPUI Kit,
[blue-marshal](https://github.com/TrueBrain/blue-marshal-rs), EVE ESI, Lucide.

## License

MIT

---

<p align="center">
  <sub>Not affiliated with CCP Games. EVE Online and all related logos are trademarks of CCP hf.</sub>
</p>
