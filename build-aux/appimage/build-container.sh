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
	docker.io/library/ubuntu:26.04 \
	bash -c '
		apt-get update -qq
		apt-get install -y -qq \
			build-essential pkg-config meson libudev-dev \
			desktop-file-utils appstream \
			wget unzip file libfuse2t64 curl git sudo zsync
		curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
		export PATH="$HOME/.cargo/bin:$PATH"
		bash build-aux/appimage/build-appimage.sh
		chown -R "$HOST_UID:$HOST_GID" /src/appimage-build
	'
