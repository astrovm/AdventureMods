# Adventure Mods

A GTK4/libadwaita app that automates Sonic Adventure DX and Sonic Adventure 2 mod setup on Linux with Steam and Proton.

## Screenshots

| Welcome | Mod Selection |
|---------|---------------|
| ![Welcome](data/screenshots/welcome.png) | ![Mod Selection](data/screenshots/mod-selection.png) |

## Features

- Automatic detection of SADX and SA2 Steam installations across all drives
- Step-by-step setup wizard with download progress tracking
- 29 curated SADX mods and 13 curated SA2 mods with per-mod selection
- Mod presets for quick configuration (DX Enhanced, Dreamcast Restoration)
- Native mod and mod manager installation — no Windows tools needed
- Automatic .NET runtime and protontricks setup
- Native monitor resolution detection for optimal game configuration
- Flatpak sandbox support

## Requirements

- Steam with Sonic Adventure DX (app 71250) and/or Sonic Adventure 2 (app 213610) installed
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

1. **Game Detection** — Parses Steam's `libraryfolders.vdf` to find installed Sonic Adventure games
2. **Dependency Check** — Ensures protontricks is available, installing it from Flathub if needed
3. **Runtime Setup** — Installs .NET Desktop Runtime 8.0 and Visual C++ 2015-2022 via protontricks
4. **Mod Installation** — Installs mod managers and curated mods with native resolution configuration
   - **SADX**: Converts the Steam version to the 2004 release, installs the SADX Mod Loader, SA Mod Manager, and up to 29 mods from dcmods, GameBanana, GitHub, and GitLab
   - **SA2**: Installs SA Mod Manager and up to 13 mods from GameBanana

## License

[MIT](LICENSE)
