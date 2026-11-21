#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_DIR"

# An AppImage uses the host's glibc, so it only starts where that glibc is at
# least as new as the build system's. Ubuntu 22.04 is the oldest Ubuntu LTS in
# standard support, and the AppImage catalog tests on it. Its glibc 2.35 sets
# the minimum. CI builds it the same way.
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
	docker.io/library/ubuntu:22.04 \
	bash -c '
		set -euo pipefail
		apt-get update -qq
		apt-get install -y -qq \
			build-essential pkg-config ninja-build python3-pip gettext \
			libudev-dev desktop-file-utils appstream \
			wget unzip file libfuse2 curl git sudo zsync
		pip3 install --quiet --no-cache-dir -r build-aux/appimage/requirements.txt
		curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
		export PATH="$HOME/.cargo/bin:$PATH"
		bash build-aux/appimage/build-appimage.sh
		# Wait for the cache uploads, then show how many crates it reused.
		# A server that already stopped when idle is not a build failure.
		if [ -n "${RUSTC_WRAPPER:-}" ]; then
			sccache --stop-server || true
		fi
		# Hand the output to whoever owns the checkout. Under rootless podman
		# that is the container root, which already maps to the host user.
		chown -R "$(stat -c %u:%g /src)" /src/appimage-build
	'
