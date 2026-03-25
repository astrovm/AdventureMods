# Adventure Mods

Set up mods for Sonic Adventure DX and Sonic Adventure 2 on Linux. Detects your Steam installations, downloads a curated collection of community mods, and configures everything automatically. Mod managers, runtimes, resolution, and load order are all set up so you can jump straight into the definitive versions of both games.

| ![Welcome](data/screenshots/welcome.png) | ![Mod Selection](data/screenshots/mod-selection.png) |
|---|---|

## Features

- Automatic detection of SADX and SA2 across all Steam library folders
- 29 curated SADX mods and 13 SA2 mods, individually selectable
- Presets for quick setup: DX Enhanced and Dreamcast Restoration
- Downloads and installs mod managers, mods, and dependencies in one go
- Configures native resolution, window mode, and optimal settings
- Installs .NET runtime and protontricks automatically if missing
- Works from a Flatpak sandbox or natively

## Requirements

- Steam with Sonic Adventure DX (app 71250) and/or Sonic Adventure 2 (app 213610)
- [protontricks](https://flathub.org/apps/com.github.Matoking.protontricks) (installed automatically if missing)

## Installation

### Flatpak

```sh
flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.json
```

### From source

Install system dependencies:

```sh
# Debian/Ubuntu
sudo apt install libgtk-4-dev libadwaita-1-dev meson

# Fedora
sudo dnf install gtk4-devel libadwaita-devel meson

# Arch
sudo pacman -S gtk4 libadwaita meson
```

Build and install:

```sh
meson setup builddir
meson compile -C builddir
meson install -C builddir
```

Or build with Cargo directly for development:

```sh
cargo build
```

## How It Works

1. **Game Detection** parses Steam's `libraryfolders.vdf` to find installed Sonic Adventure games
2. **Dependency Check** ensures protontricks is available, installing it from Flathub if needed
3. **Runtime Setup** installs .NET Desktop Runtime 8.0 and Visual C++ 2015-2022 via protontricks
4. **Mod Installation** installs mod managers and curated mods with native resolution configuration
   - **SADX**: Converts the Steam version to the 2004 release, installs the SADX Mod Loader, SA Mod Manager, and up to 29 mods from dcmods, GameBanana, GitHub, and GitLab
   - **SA2**: Installs SA Mod Manager and up to 13 mods from GameBanana

## License

[MIT](LICENSE)
