# Adventure Mods

The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux. It finds your Steam games, downloads community mods, and handles all the setup for you. Mod managers, runtimes, resolution, load order, everything is ready to go so you can play the best versions of both games right away.

<p align="center">
  <img src="data/screenshots/welcome.png" alt="Welcome" width="400">
  &nbsp;&nbsp;
  <img src="data/screenshots/mod-selection.png" alt="Mod Selection" width="400">
</p>

## Features

- Finds SADX and SA2 across all your Steam library folders
- 29 SADX mods and 13 SA2 mods to pick from
- Quick presets: DX Enhanced and Dreamcast Restoration
- Gets mod managers, mods, and dependencies all at once
- Sets up native resolution, window mode, and optimal settings
- Installs .NET runtime and protontricks if they're missing

## Requirements

- Steam with Sonic Adventure DX (app 71250) and/or Sonic Adventure 2 (app 213610)
- [protontricks](https://flathub.org/apps/com.github.Matoking.protontricks) (installed automatically if missing)

## Installation

### AppImage

Download the latest AppImage from [GitHub Releases](https://github.com/astrovm/AdventureMods/releases/latest/download/Adventure_Mods-x86_64.AppImage).

Then make it executable and run it:

```sh
chmod +x Adventure_Mods-x86_64.AppImage
./Adventure_Mods-x86_64.AppImage
```

## Development

### Flatpak build

```sh
flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.Devel.json
```

### Native build

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

### AppImage build

Build locally in Docker with `debian:13`:

```sh
docker run --rm -v "$(pwd):/src" -w /src debian:13 bash -c '
  export HOST_UID="$(id -u)"
  export HOST_GID="$(id -g)"
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
