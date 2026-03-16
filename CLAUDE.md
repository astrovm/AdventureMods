# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Adventure Mods is a GTK4/libadwaita Flatpak app (`io.github.astrovm.AdventureMods`) written in Rust that automates Sonic Adventure DX and SA2 mod setup on Linux with Steam/Proton. It replaces a bash script with a GUI wizard.

## Build & Test Commands

```bash
# Cargo build (requires PKG_CONFIG_PATH on some systems)
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo build

# Check without building
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo check

# Run all tests
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo test

# Run a single test
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo test test_parse_libraryfolders

# Meson build (full build with resources)
meson setup builddir && meson compile -C builddir

# Flatpak build
flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.json
```

System deps: `libgtk-4-dev`, `libadwaita-1-dev` (which pulls in pango, gdk-pixbuf, graphene, etc.)

## Architecture

### Hybrid Build System
Meson is the outer build system (handles GResources, schemas, desktop files, i18n). It delegates Rust compilation to Cargo via `build-aux/cargo-build.sh`. The `src/config.rs.in` template is processed by Meson's `configure_file()` to inject build-time constants (APP_ID, VERSION, PKGDATADIR); `src/config.rs` provides fallback defaults so `cargo build` works standalone.

### GTK Subclassing Pattern
All widgets use the gtk-rs composite template pattern:
- Private `mod imp {}` with `#[glib::object_subclass]` + `#[derive(gtk::CompositeTemplate)]`
- `#[template_child]` bindings to `.ui` file widgets
- Public type via `glib::wrapper!` macro
- UI templates live in `data/resources/ui/*.ui` and are compiled into a GResource bundle

### Async Model
No tokio — the app uses GLib's async runtime:
- `glib::spawn_future_local()` for async tasks on the main thread (UI-safe)
- `gio::spawn_blocking()` for blocking work (file I/O, Steam detection) on the thread pool
- `async-channel` bridges progress updates from download tasks back to UI

### Flatpak Sandbox Strategy
`src/external/flatpak.rs` detects the sandbox via `/.flatpak-info` and routes commands through `flatpak-spawn --host` when inside Flatpak, or runs them directly otherwise. This is the foundation for all host interactions: protontricks, ProtonUp-Qt, Flatpak install/check.

### UI Navigation Flow
`AdwNavigationView` stack: Welcome Page → Setup Page (per game). The Setup Page is a multi-step wizard driven by `setup::steps::steps_for_game()` which returns a `Vec<SetupStep>`. Each step has a `StepKind` (Auto, Info, ExternalAction, Download, ModSelection) that determines the UI and behavior. Steps advance automatically on success or show error with retry.

### Steam Detection
`steam/vdf.rs` is a custom recursive-descent parser for Valve's VDF format. `steam/library.rs` reads `libraryfolders.vdf` to find SADX (appid 71250) and SA2 (appid 213610) install paths across multiple Steam library folders.

### Mod Installation
Both games share a common mod installation pipeline in `setup/common.rs`: `ModEntry` structs with a `ModSource` enum (`GameBanana { file_id }` | `DirectUrl { url }`), plus shared `install_mod_manager()` and `install_mod()` functions. `external/download.rs` uses reqwest with streaming (`bytes_stream()`) for progress tracking and validates Content-Type to reject HTML error pages. GameBanana mods are downloaded via dl URLs; direct URLs point to dcmods.unreliable.network, GitHub releases, or GitLab archives. SA2 has 12 recommended mods (`setup/sa2.rs`), SADX has 19 (`setup/sadx.rs`). SADX also installs a separate mod loader via `sadx::install_mod_loader()` and configures SA Mod Manager via JSON profile files (`Manager.json`, `Profiles.json`, `Default.json`) in `SAManager/SADX/`, matching the official Windows installer.

## Key Conventions

- App ID: `io.github.astrovm.AdventureMods` (release) / `io.github.astrovm.AdventureMods.Devel` (dev profile adds `.devel` CSS class)
- GResource prefix: `/io/github/astrovm/AdventureMods/`
- Error handling: `anyhow::Result` throughout, errors displayed in UI (never panic)
- External tool Flatpak IDs: protontricks = `com.github.Matoking.protontricks`, ProtonUp-Qt = `net.davidotek.pupgui2`
