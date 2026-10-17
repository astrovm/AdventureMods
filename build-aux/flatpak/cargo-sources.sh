#!/bin/sh
# Write build-aux/flatpak/cargo-sources.json from Cargo.lock. The Flatpak
# build gets every crate from it and needs no network, as on Flathub.
set -eu

GENERATOR_COMMIT=74697c75b630d7330e77250fc13cb5ea688d9479
GENERATOR_SHA256=0a2db6be87d75910facef28ab46d4d6460802e8419ab850d0caa6a364d26b380

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

generator="$tmp/flatpak-cargo-generator.py"
curl -fsSL -o "$generator" \
	"https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/$GENERATOR_COMMIT/cargo/flatpak-cargo-generator.py"
echo "$GENERATOR_SHA256  $generator" | sha256sum -c - >/dev/null

output="$PROJECT_DIR/build-aux/flatpak/cargo-sources.json"
# The Flatpak CI image ships the generator's Python packages; elsewhere uv
# installs them from the script's own dependency list.
if python3 -c 'import aiohttp, tomlkit, yaml' 2>/dev/null; then
	python3 "$generator" "$PROJECT_DIR/Cargo.lock" -o "$output"
else
	uv run --script "$generator" "$PROJECT_DIR/Cargo.lock" -o "$output"
fi
