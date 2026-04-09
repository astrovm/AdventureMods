# Adventure Mods

The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux. It finds your Steam games, downloads community mods, and handles all the setup for you. Mod managers, runtimes, resolution, load order, everything is ready to go so you can play the best versions of both games right away.

<p align="center">
  <img src="data/screenshots/welcome.png" alt="Welcome" width="400">
  &nbsp;&nbsp;
  <img src="data/screenshots/mod-selection.png" alt="Mod Selection" width="400">
</p>

## Features

- Finds SADX and SA2 across all your Steam library folders
- 29 SADX mods and 12 SA2 mods to pick from
- Quick SADX presets: DX Enhanced and Dreamcast Restoration
- Gets mod managers, mods, and dependencies all at once
- Sets up native resolution, window mode, and optimal settings
- Installs .NET runtime if it's missing

## Requirements

- Steam with Sonic Adventure DX (app 71250) and/or Sonic Adventure 2 (app 213610)

## Installation

### AppImage

Download the latest AppImage from [GitHub Releases](https://github.com/astrovm/AdventureMods/releases/latest/download/Adventure_Mods-x86_64.AppImage).

Then make it executable and run it:

```sh
chmod +x Adventure_Mods-x86_64.AppImage
./Adventure_Mods-x86_64.AppImage
```

## CLI

The same app can run in terminal mode when you pass a subcommand.

### AppImage

```sh
./Adventure_Mods-x86_64.AppImage detect
./Adventure_Mods-x86_64.AppImage list-mods --game sadx
./Adventure_Mods-x86_64.AppImage setup --game sa2 --all-mods
```

### Flatpak

```sh
flatpak run io.github.astrovm.AdventureMods detect
flatpak run io.github.astrovm.AdventureMods list-mods --game sadx
flatpak run io.github.astrovm.AdventureMods setup --game sa2 --all-mods
```

### Detect installed games

```sh
./Adventure_Mods-x86_64.AppImage detect
./Adventure_Mods-x86_64.AppImage detect --libraryfolders-vdf ~/.local/share/Steam/steamapps/libraryfolders.vdf
```

### List available presets and mods

```sh
./Adventure_Mods-x86_64.AppImage list-mods --game sadx
./Adventure_Mods-x86_64.AppImage list-mods --game sa2
```

### Run a full headless setup

```sh
./Adventure_Mods-x86_64.AppImage setup --game sadx --preset "DX Enhanced"
./Adventure_Mods-x86_64.AppImage setup --game sa2 --mods sa2-render-fix,hd-gui-sa2-edition
./Adventure_Mods-x86_64.AppImage setup --game sadx --game-path "/path/to/Sonic Adventure DX" --width 2560 --height 1440
```

In a terminal, `setup` opens a guided wizard when game selection, install path, or mod selection is missing. A fully specified command runs directly.

For a headless run, pass the full selection explicitly with flags such as `--game`, `--game-path`, `--preset`, `--all-mods`, or `--mods`.

### Command reference

| Command | Purpose |
|--------|---------|
| `detect` | Show detected SADX and SA2 installs plus inaccessible Steam libraries |
| `list-mods --game <sadx|sa2>` | Show available presets and recommended mods for one game |
| `setup ...` | Install runtimes, mod manager, selected mods, and generated config files |

### Common flags

- `--game sadx|sa2` picks the game explicitly
- `--game-path /path/to/game` skips Steam detection for setup
- `--preset "DX Enhanced"` uses a named preset when the selected game provides presets
- `--all-mods` installs every recommended mod for the selected game
- `--mods id1,id2` selects specific mods in headless mode
- `--libraryfolders-vdf /path/to/libraryfolders.vdf` points detection at a specific Steam library file
- repeat `--steam-library /path/to/library` to add extra Steam library roots during detection
- `--width` and `--height` override the generated game resolution for setup

## Development

Build instructions for local development:

### Flatpak build

```sh
flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.Devel.json
```

### AppImage build

Build locally in Docker with `debian:13`:

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

The output is `appimage-build/Adventure_Mods-x86_64.AppImage`.

## License

[MIT](LICENSE)
