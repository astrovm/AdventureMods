# Adventure Mods

The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux. Finds your Steam installs, downloads community mods, and handles mod managers, runtimes, resolution, load order, and language settings so you can play right away.

<p align="center">
  <img src="data/screenshots/welcome.png" alt="Welcome" width="400">
  &nbsp;&nbsp;
  <img src="data/screenshots/mod-selection.png" alt="Mod Selection" width="400">
</p>

## Features

- Detects SADX and SA2 across all Steam library folders
- Includes 29 SADX mods and 12 SA2 mods
- Provides SADX presets: DX Enhanced and Dreamcast Restoration
- Installs mod managers, mods, and dependencies in one step
- Configures native resolution, window mode, and optimal settings
- Saves subtitle and voice language selection per game

## Requirements

Steam with Sonic Adventure DX (app 71250) and/or Sonic Adventure 2 (app 213610).

## Installation

### Flatpak bundle

Download the Flatpak bundle for your CPU architecture from
[GitHub Releases](https://github.com/astrovm/AdventureMods/releases/latest), then:

```sh
flatpak install --user AdventureMods-<arch>.flatpak
```

Flatpak downloads the required GNOME runtime from Flathub. Application updates
require downloading the bundle from the next Adventure Mods release.

### Gear Lever (recommended)

[Gear Lever](https://flathub.org/apps/it.mijorus.gearlever) handles desktop integration and updates:

```sh
flatpak install flathub it.mijorus.gearlever
```

Download the latest AppImage for your CPU architecture from [GitHub Releases](https://github.com/astrovm/AdventureMods/releases/latest), then open it with Gear Lever.

### Manual

Download the latest AppImage for your CPU architecture from [GitHub Releases](https://github.com/astrovm/AdventureMods/releases/latest), then:

```sh
chmod +x Adventure_Mods-<arch>.AppImage
./Adventure_Mods-<arch>.AppImage
```

Running without a subcommand launches the GUI. Pass a subcommand to use CLI mode.

## CLI

<p align="center">
  <img src="data/screenshots/cli.png" alt="CLI" width="600">
</p>

| Command | Description |
|---------|-------------|
| `detect` | Show detected game installs and inaccessible Steam libraries |
| `list-mods --game sadx\|sa2` | List available presets and mods for a game |
| `setup` | Install runtimes, mod manager, mods, and config files |

**Setup flags**

| Flag | Description |
|------|-------------|
| `--game sadx\|sa2` | Select the game |
| `--game-path /path` | Override Steam detection with an explicit install path |
| `--preset "Name"` | Use a named preset (SADX only) |
| `--all-mods` | Install all recommended mods |
| `--mods slug1,slug2` | Select specific mods by slug |
| `--subtitle-language` | Set subtitle language (see below) |
| `--voice-language` | Set voice language: `japanese` or `english` |
| `--width`, `--height` | Override detected resolution |

<details>
<summary>Advanced flags</summary>

| Flag | Description |
|------|-------------|
| `--libraryfolders-vdf /path` | Use a specific `libraryfolders.vdf` file |
| `--steam-library /path` | Add an extra Steam library root (repeatable) |

</details>

**Subtitle languages**

- SADX: `japanese`, `english`, `french`, `spanish`, `german`
- SA2: `english`, `german`, `spanish`, `french`, `italian`, `japanese`

> [!TIP]
> `setup` opens an interactive wizard when game, path, or mod selection is missing. Specify all options for a fully headless run.

**Examples**

```sh
./Adventure_Mods-<arch>.AppImage setup --game sadx --preset "DX Enhanced"
./Adventure_Mods-<arch>.AppImage setup --game sa2 --all-mods
./Adventure_Mods-<arch>.AppImage setup --game sa2 --mods sa2-render-fix,better-radar
./Adventure_Mods-<arch>.AppImage setup --game sa2 --subtitle-language japanese --voice-language english
./Adventure_Mods-<arch>.AppImage setup --game sadx --game-path "/path/to/SADX" --width 2560 --height 1440
```

## Development

### Flatpak

```sh
flatpak-builder --force-clean --user --install-deps-from=flathub --install \
  build build-aux/io.github.astrovm.AdventureMods.Devel.json
```

### AppImage

Build in Podman with `debian:13`:

```sh
make appimage
```

Output: `appimage-build/Adventure_Mods*.AppImage` and `.zsync`.

The AppImage build is native-architecture: x86_64 hosts produce x86_64 AppImages,
and ARM64/aarch64 hosts produce aarch64 AppImages. GitHub release builds publish
both architectures.

## License

[MIT](LICENSE)
