#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_DIR"

# Ubuntu 24.04 keeps the AppImage's glibc needs at 2.39, so it runs on
# distros that old and newer. CI builds it the same way.
engine="$(command -v podman || command -v docker)"

# CI hands in sccache to reuse crates compiled by earlier runs.
cache_args=()
if [ -n "${SCCACHE_PATH:-}" ]; then
	cache_args=(
		-v "$SCCACHE_PATH:/usr/local/bin/sccache:ro"
		-e RUSTC_WRAPPER=sccache
		-e SCCACHE_GHA_ENABLED
		-e ACTIONS_CACHE_SERVICE_V2
		-e ACTIONS_RESULTS_URL
		-e ACTIONS_RUNTIME_TOKEN
	)
fi

"$engine" run --rm \
	"${cache_args[@]}" \
	-e DEBIAN_FRONTEND=noninteractive \
	-v "$PROJECT_DIR:/src" \
	-w /src \
	docker.io/library/ubuntu:24.04 \
	bash -c '
		apt-get update -qq
		apt-get install -y -qq \
			build-essential pkg-config meson gettext libudev-dev \
			desktop-file-utils appstream \
			wget unzip file libfuse2t64 curl git sudo zsync
		curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
		export PATH="$HOME/.cargo/bin:$PATH"
		bash build-aux/appimage/build-appimage.sh
		# Wait for the cache uploads, then show how many crates it reused.
		if [ -n "${RUSTC_WRAPPER:-}" ]; then
			sccache --stop-server
		fi
		# Hand the output to whoever owns the checkout. Under rootless podman
		# that is the container root, which already maps to the host user.
		chown -R "$(stat -c %u:%g /src)" /src/appimage-build
	'
