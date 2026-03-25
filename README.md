# Adventure Mods

The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux. It finds your Steam games, downloads community mods, and handles all the setup for you. Mod managers, runtimes, resolution, load order, everything is ready to go so you can play the best versions of both games right away.

| ![Welcome](data/screenshots/welcome.png) | ![Mod Selection](data/screenshots/mod-selection.png) |
|---|---|

## Features

- Finds SADX and SA2 across all your Steam library folders
- 29 SADX mods and 13 SA2 mods to pick from
- Quick presets: DX Enhanced and Dreamcast Restoration
- Gets mod managers, mods, and dependencies all at once
- Sets up native resolution, window mode, and optimal settings
- Installs .NET runtime and protontricks if they're missing
- Runs as a Flatpak or natively

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

1. **Game Detection** reads Steam's `libraryfolders.vdf` to find your Sonic Adventure games
2. **Dependency Check** makes sure protontricks is available, installing it from Flathub if needed
3. **Runtime Setup** installs .NET Desktop Runtime 8.0 and Visual C++ 2015-2022 through protontricks
4. **Mod Installation** sets up mod managers and mods with native resolution config
   - **SADX**: Converts the Steam version to the 2004 release, installs the SADX Mod Loader, SA Mod Manager, and up to 29 mods from dcmods, GameBanana, GitHub, and GitLab
   - **SA2**: Installs SA Mod Manager and up to 13 mods from GameBanana

## License

[MIT](LICENSE)
