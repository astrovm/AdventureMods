# Adventure Mods

The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux. Finds your Steam installs, downloads community mods, and handles mod managers, runtimes, resolution, load order, and language settings so you can play right away.

<p align="center">
  <img src="data/screenshots/welcome.png" alt="Welcome" width="400">
  &nbsp;&nbsp;
  <img src="data/screenshots/mod-selection.png" alt="Mod Selection" width="400">
</p>

## Features

- Detects SADX and SA2 across all Steam library folders
- 29 SADX mods and 12 SA2 mods
- SADX presets: DX Enhanced and Dreamcast Restoration
- Installs mod managers, mods, and dependencies in one step
- Configures native resolution, window mode, and optimal settings
- Lets you choose subtitle and voice language during setup
- Installs .NET runtime if missing

## Requirements

Steam with Sonic Adventure DX (app 71250) and/or Sonic Adventure 2 (app 213610).

## Installation

Download the latest AppImage from [GitHub Releases](https://github.com/astrovm/AdventureMods/releases/latest/download/Adventure_Mods-x86_64.AppImage), then:

```sh
chmod +x Adventure_Mods-x86_64.AppImage
./Adventure_Mods-x86_64.AppImage
```

No subcommand launches the GUI. Pass a subcommand to run in CLI mode.

## CLI

```sh
./Adventure_Mods-x86_64.AppImage detect
./Adventure_Mods-x86_64.AppImage list-mods --game sadx
./Adventure_Mods-x86_64.AppImage setup --game sadx --preset "DX Enhanced"
./Adventure_Mods-x86_64.AppImage setup --game sa2 --all-mods
./Adventure_Mods-x86_64.AppImage setup --game sa2 --mods sa2-render-fix,better-radar
./Adventure_Mods-x86_64.AppImage setup --game sa2 --subtitle-language japanese --voice-language english
./Adventure_Mods-x86_64.AppImage setup --game sadx --game-path "/path/to/SADX" --width 2560 --height 1440
```

> [!TIP]
> `setup` opens an interactive wizard when game, path, or mod selection is missing. For a fully headless run, specify all options explicitly.

**Commands**

| Command | Description |
|---------|-------------|
| `detect` | Show detected game installs and inaccessible Steam libraries |
| `list-mods --game <sadx\|sa2>` | List available presets and mods for a game |
| `setup` | Install runtimes, mod manager, mods, and config files |

**Setup flags**

| Flag | Description |
|------|-------------|
| `--game sadx\|sa2` | Select the game |
| `--game-path /path` | Override Steam detection for setup |
| `--preset "Name"` | Use a named preset (SADX only) |
| `--all-mods` | Install all recommended mods |
| `--mods id1,id2` | Select specific mods |
| `--subtitle-language value` | Override generated subtitle language |
| `--voice-language value` | Override generated voice language |
| `--width`, `--height` | Override generated resolution |
| `--libraryfolders-vdf /path` | Use a specific Steam library file |
| `--steam-library /path` | Add an extra Steam library root (repeatable) |

**Language values**

Voice: `japanese`, `english`

SADX subtitles: `japanese`, `english`, `french`, `spanish`, `german`

SA2 subtitles: `english`, `german`, `spanish`, `french`, `italian`, `japanese`

Selections are saved per game and reused the next time you run setup.

**Flatpak**

```sh
flatpak run io.github.astrovm.AdventureMods <subcommand>
```

## Development

### Flatpak

```sh
flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.Devel.json
```

### AppImage

Build in Docker with `debian:13`:

```sh
docker run --rm \
  -e HOST_UID="$(id -u)" \
  -e HOST_GID="$(id -g)" \
  -v "$(pwd):/src" \
  -w /src \
  debian:13 bash -c '
  apt-get update -qq
  apt-get install -y -qq \
    build-essential pkg-config meson gettext python3-pip python3-setuptools \
    libgtk-4-dev libadwaita-1-dev libglib2.0-dev \
    libgraphene-1.0-dev libpango1.0-dev \
    libcairo2-dev libgdk-pixbuf-2.0-dev libepoxy-dev \
    libwayland-dev libxkbcommon-dev libvulkan-dev \
    libx11-dev libxrandr-dev libxi-dev libxext-dev \
    libxcursor-dev libxdamage-dev libxfixes-dev \
    libxinerama-dev libxcomposite-dev \
    wayland-protocols libcloudproviders-dev \
    libsass-dev sassc libappstream-dev \
    desktop-file-utils appstream \
    wget unzip file libfuse2 curl git glslc libdrm-dev sudo
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
  export PATH="$HOME/.cargo/bin:$PATH"
  bash build-aux/appimage/build-appimage.sh
  chown -R "$HOST_UID:$HOST_GID" /src/appimage-build
'
```

Output: `appimage-build/Adventure_Mods-x86_64.AppImage`

## License

[MIT](LICENSE)
