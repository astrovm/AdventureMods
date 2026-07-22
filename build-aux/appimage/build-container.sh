#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_DIR"

podman run --rm \
	-e DEBIAN_FRONTEND=noninteractive \
	-e HOST_UID="$(id -u)" \
	-e HOST_GID="$(id -g)" \
	-v "$PROJECT_DIR:/src" \
	-w /src \
	debian:13 \
	bash -c '
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
			desktop-file-utils appstream libxml2-utils \
			wget unzip file libfuse2 curl git glslc libdrm-dev sudo zsync \
			librsvg2-dev libgirepository1.0-dev
		curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
		export PATH="$HOME/.cargo/bin:$PATH"
		bash build-aux/appimage/build-appimage.sh
		chown -R "$HOST_UID:$HOST_GID" /src/appimage-build
	'
