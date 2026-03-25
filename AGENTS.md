# Repository Guidelines

This file provides guidance to AI coding agents when working with code in this repository.

## Project Overview

Adventure Mods is a GTK4/libadwaita Flatpak app (`io.github.astrovm.AdventureMods`) written in Rust that automates Sonic Adventure DX and SA2 mod setup on Linux with Steam/Proton. It replaces a bash script with a GUI wizard.

## Project Structure & Module Organization
`src/` contains the Rust application code. Keep GTK/libadwaita UI code in `src/ui/`, setup flow and game-specific logic in `src/setup/`, Steam parsing in `src/steam/`, and download/archive/protontricks integrations in `src/external/`. Meson generates `src/config.rs` from `src/config.rs.in`, so edit the template, not the generated file. Packaging assets live in `data/`, translations in `po/`, and Flatpak/Meson helpers in `build-aux/`.

## Build & Test Commands

Install the GTK4/libadwaita/Meson development packages from `README.md` first; otherwise `cargo` will fail at `pkg-config` checks for libraries such as `gtk4`, `pango`, and `gdk-pixbuf-2.0`.

System deps: `libgtk-4-dev`, `libadwaita-1-dev` (which pulls in pango, gdk-pixbuf, graphene, etc.)

```bash
# Cargo build (requires PKG_CONFIG_PATH on some systems)
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo build

# Check without building
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo check

# Run all tests
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo test

# Run a single test
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig cargo test test_parse_libraryfolders

# Format and lint
cargo fmt --all
cargo clippy --all-targets

# Meson build (full build with resources)
meson setup builddir && meson compile -C builddir

# Flatpak build
flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.json
```

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
`src/external/flatpak.rs` detects the sandbox via `/.flatpak-info` and routes commands through `flatpak-spawn --host` when inside Flatpak, or runs them directly otherwise. This is the foundation for all host interactions: protontricks, Flatpak install/check.

### UI Navigation Flow
`AdwNavigationView` stack: Welcome Page → Setup Page (per game). The Setup Page is a multi-step wizard driven by `setup::steps::steps_for_game()` which returns a `Vec<SetupStep>`. Each step has a `StepKind` (Auto, Info, ExternalAction, Download, ModSelection) that determines the UI and behavior. Steps advance automatically on success or show error with retry.

### Steam Detection
`steam/vdf.rs` is a custom recursive-descent parser for Valve's VDF format. `steam/library.rs` reads `libraryfolders.vdf` to find SADX (appid 71250) and SA2 (appid 213610) install paths across multiple Steam library folders.

### Mod Installation
Both games share a common mod installation pipeline in `setup/common.rs`: `ModEntry` structs with a `ModSource` enum (`GameBanana { file_id }` | `DirectUrl { url }`), plus shared `install_mod_manager()` and `install_mod()` functions. `external/download.rs` uses reqwest with streaming (`bytes_stream()`) for progress tracking and validates Content-Type to reject HTML error pages. GameBanana mods are downloaded via dl URLs; direct URLs point to dcmods.unreliable.network, GitHub releases, or GitLab archives. SA2 has 13 recommended mods (`setup/sa2.rs`), SADX has 29 (`setup/sadx.rs`). The app generates core configuration files (`Default.json`, `UserConfig.cfg`) during mod manager installation to ensure optimal settings (native resolution, window mode) without requiring initial manual configuration.

## Coding Style & Naming Conventions
Follow default Rust formatting: 4-space indentation, trailing commas where `rustfmt` adds them, and grouped `use` statements similar to `src/ui/setup_page.rs`. Use `snake_case` for modules, files, and functions, `PascalCase` for types and GTK widget wrappers, and `SCREAMING_SNAKE_CASE` for constants. Prefer small, focused modules and keep game-specific behavior in `sadx*` or `sa2*` files rather than branching throughout shared code.

## Testing Guidelines
Tests are colocated with implementation under `#[cfg(test)] mod tests`. Add coverage next to the changed code instead of creating ad hoc test folders. Favor deterministic unit tests for config generation, VDF parsing, step sequencing, and download/setup helpers. Run `cargo test` before opening a PR; use `cargo test setup::sadx` or a similar path filter when iterating.

## Key Conventions

- App ID: `io.github.astrovm.AdventureMods` (release) / `io.github.astrovm.AdventureMods.Devel` (dev profile adds `.devel` CSS class)
- GResource prefix: `/io/github/astrovm/AdventureMods/`
- Error handling: `anyhow::Result` throughout, errors displayed in UI (never panic)
- External tool Flatpak IDs: protontricks = `com.github.Matoking.protontricks`

## Commit & Pull Request Guidelines
Recent history uses Conventional Commits such as `feat:`, `refactor:`, and `chore:`. Keep subjects imperative and specific, for example `feat: add SA2 patch defaults`. PRs should explain user-visible behavior, list verification commands run, and include screenshots for UI changes. Link any related issue and call out packaging or Flatpak impact when relevant.

## Security & Configuration Tips
Do not commit local build outputs from `target/`, `build/`, or extraction directories. Treat external download URLs, archive handling, and `flatpak-spawn --host` integrations as sensitive paths: validate changes carefully and avoid broad filesystem or host-command access.
