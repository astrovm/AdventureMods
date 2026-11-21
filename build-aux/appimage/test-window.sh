#!/usr/bin/env bash
# Start the AppImage on a minimal Ubuntu 22.04 with only a virtual X server,
# the way the AppImage catalog tests it, and check that its window opens.
# Usage: test-window.sh <AppImage> [platform, such as linux/arm64]
set -euo pipefail

if [ ! -f "${1:-}" ]; then
	echo "Usage: $0 <AppImage> [platform]. No AppImage at '${1:-}'." >&2
	exit 2
fi
appimage="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
platform="${2:-}"
engine="$(command -v podman || command -v docker)"
platform_args=()
if [ -n "$platform" ]; then
	platform_args=(--platform "$platform")
fi

"$engine" run --rm "${platform_args[@]}" \
	-e DEBIAN_FRONTEND=noninteractive \
	-v "$appimage:/app.AppImage:ro" \
	docker.io/library/ubuntu:22.04 \
	bash -c '
		set -euo pipefail
		apt-get update -qq
		# Xvfb brings libX11 and libxcb; Mesa brings OpenGL. Nothing else.
		apt-get install -y -qq --no-install-recommends \
			xvfb x11-utils libgl1-mesa-dri libgl1 libegl1 >/dev/null
		Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &
		export DISPLAY=:99 HOME=/tmp/home
		mkdir -p "$HOME"
		for _ in $(seq 30); do
			xdpyinfo >/dev/null 2>&1 && break
			sleep 0.5
		done
		xdpyinfo >/dev/null
		cd /tmp
		/app.AppImage --appimage-extract-and-run >/tmp/app.log 2>&1 &
		app=$!
		for _ in $(seq 60); do
			# The window is mapped, not only created.
			state="$(xwininfo -name "Adventure Mods" 2>/dev/null || true)"
			if grep -q "IsViewable" <<<"$state"; then
				# The app must survive its first frames too.
				sleep 3
				if kill -0 "$app" 2>/dev/null; then
					echo "The window opened."
					kill "$app" 2>/dev/null || true
					exit 0
				fi
				echo "The window opened, but the app then stopped:" >&2
				cat /tmp/app.log >&2
				exit 1
			fi
			if ! kill -0 "$app" 2>/dev/null; then
				break
			fi
			sleep 1
		done
		echo "The window did not open within 60 seconds:" >&2
		cat /tmp/app.log >&2
		exit 1
	'
