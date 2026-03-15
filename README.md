# Adventure Mods

A Linux desktop app for setting up Sonic Adventure DX and Sonic Adventure 2 mods with Steam and Proton.

Adventure Mods automates what would otherwise be a tedious manual process: installing mod managers, downloading recommended mods, configuring protontricks, and setting up .NET runtimes — all through a step-by-step GUI wizard.

## Features

- Automatic detection of SADX and SA2 Steam installations (including external drives)
- Step-by-step setup wizard for each game
- GE-Proton and .NET runtime configuration via protontricks
- SADX mod installer download and launch
- SA2 Mod Manager installation with 13 curated recommended mods from GameBanana
- Download progress tracking
- Works inside a Flatpak sandbox with host command support

## Requirements

- Steam with Sonic Adventure DX (71250) and/or Sonic Adventure 2 (213610) installed
- [GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom) (installed via [ProtonUp-Qt](https://flathub.org/apps/net.davidotek.pupgui2))
- [protontricks](https://flathub.org/apps/com.github.Matoking.protontricks) (installed automatically if missing)

## Installation

### Flatpak (recommended)

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

Or build with Cargo directly (for development):

```sh
cargo build
```

## How It Works

1. **Game Detection** — Parses Steam's `libraryfolders.vdf` to find installed Sonic Adventure games
2. **Dependency Check** — Ensures protontricks and ProtonUp-Qt are available, installing them from Flathub if needed
3. **Runtime Setup** — Guides GE-Proton configuration and installs .NET Framework 4.8 via protontricks
4. **Mod Installation**
   - **SADX**: Downloads and launches the SADX Mod Installer through the game's Wine prefix
   - **SA2**: Installs SA Mod Manager, then downloads selected mods from GameBanana

## Technology

- **Rust** with **GTK4** and **libadwaita** for a native GNOME desktop experience
- **Meson** build system with Cargo integration
- **Flatpak** sandbox with `flatpak-spawn --host` for protontricks and other host commands
- **GameBanana API v11** for mod download URL resolution

## License

[MIT](LICENSE)
