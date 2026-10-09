#!/usr/bin/env bash
# Pack the AppImage's AppDir as a Snap. Run build-aux/appimage/build-appimage.sh first.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$PROJECT_DIR/appimage-build"
APPDIR="$BUILD_DIR/AppDir"
SNAP_DIR="$BUILD_DIR/snap"
APP_ID="io.github.astrovm.AdventureMods"

case "$(uname -m)" in
	x86_64) arch=x86_64 snap_arch=amd64 ;;
	aarch64 | arm64) arch=aarch64 snap_arch=arm64 ;;
	*)
		echo "Unsupported Snap architecture: $(uname -m)" >&2
		exit 1
		;;
esac

if [ ! -x "$APPDIR/AppRun" ]; then
	echo "Missing $APPDIR/AppRun. Build the AppImage first." >&2
	exit 1
fi

version="$(sh "$PROJECT_DIR/build-aux/cargo-version.sh")"

echo "==> Packing ${snap_arch} Snap"
rm -rf "$SNAP_DIR"
mkdir -p "$SNAP_DIR"
cp -a "$APPDIR/." "$SNAP_DIR/"
mkdir -p "$SNAP_DIR/meta/gui"
sed -e "s/ARCHITECTURE/$snap_arch/" -e "s/^version: .*/version: '$version'/" \
	"$SCRIPT_DIR/snap.yaml" >"$SNAP_DIR/meta/snap.yaml"
cp "$APPDIR/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg" "$SNAP_DIR/meta/gui/icon.svg"
# shellcheck disable=SC2016
sed 's|^Icon=.*|Icon=${SNAP}/meta/gui/icon.svg|' \
	"$APPDIR/usr/share/applications/$APP_ID.desktop" >"$SNAP_DIR/meta/gui/adventure-mods.desktop"

snap pack "$SNAP_DIR" "$BUILD_DIR"
mv "$BUILD_DIR/adventure-mods_${version}_${snap_arch}.snap" "$BUILD_DIR/AdventureMods-v${version}-${arch}.snap"

echo "==> Done! Snap created:"
ls -lh "$BUILD_DIR/AdventureMods-v${version}-${arch}.snap"
