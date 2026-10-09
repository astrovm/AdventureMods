#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$PROJECT_DIR/appimage-build"
APPDIR="$BUILD_DIR/AppDir"

LINUXDEPLOY_VERSION="1-alpha-20251107-1"
BUILD_ARCH="$(uname -m)"
case "$BUILD_ARCH" in
	x86_64)
		APPIMAGE_ARCH="x86_64"
		LINUXDEPLOY_ARCH="x86_64"
		SEVENZIP_ARCH="x64"
		HPATCHZ_ARCH="linux64"
		;;
	aarch64 | arm64)
		APPIMAGE_ARCH="aarch64"
		LINUXDEPLOY_ARCH="aarch64"
		SEVENZIP_ARCH="arm64"
		HPATCHZ_ARCH="linux_arm64"
		;;
	*)
		echo "Unsupported AppImage architecture: $BUILD_ARCH" >&2
		exit 1
		;;
esac

LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage"
HPATCHZ_URL="https://github.com/sisong/HDiffPatch/releases/download/v5.1.3/hdiffpatch_v5.1.3_bin_${HPATCHZ_ARCH}.zip"
SEVENZIP_URL="https://github.com/ip7z/7zip/releases/download/26.04/7z2604-linux-${SEVENZIP_ARCH}.tar.xz"

cleanup() {
	rm -rf "$BUILD_DIR/tmp"
}
trap cleanup EXIT

echo "==> Setting up build directory"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/tmp" "$APPDIR"
echo "==> Building ${APPIMAGE_ARCH} AppImage"

echo "==> Configuring Meson"
meson setup "$BUILD_DIR/meson" "$PROJECT_DIR" \
	--prefix=/usr \
	-Dprofile=default \
	-Dbuildtype=release

echo "==> Building"
meson compile -C "$BUILD_DIR/meson"

echo "==> Installing to AppDir"
DESTDIR="$APPDIR" meson install -C "$BUILD_DIR/meson"

echo "==> Downloading hpatchz"
wget -q -O "$BUILD_DIR/tmp/hpatchz.zip" "$HPATCHZ_URL"
unzip -o -j "$BUILD_DIR/tmp/hpatchz.zip" "${HPATCHZ_ARCH}/hpatchz" -d "$BUILD_DIR/tmp/"
install -Dm755 "$BUILD_DIR/tmp/hpatchz" "$APPDIR/usr/bin/hpatchz"

echo "==> Downloading 7-Zip"
wget -q -O "$BUILD_DIR/tmp/7zip.tar.xz" "$SEVENZIP_URL"
tar xf "$BUILD_DIR/tmp/7zip.tar.xz" -C "$BUILD_DIR/tmp/"
install -Dm755 "$BUILD_DIR/tmp/7zz" "$APPDIR/usr/bin/7zz"

echo "==> Downloading linuxdeploy"
wget -q -O "$BUILD_DIR/linuxdeploy" "$LINUXDEPLOY_URL"
chmod +x "$BUILD_DIR/linuxdeploy"

# The bundled 7zz and hpatchz sit next to the app; find them first.
mkdir -p "$APPDIR/apprun-hooks"
cat >"$APPDIR/apprun-hooks/adventure-mods.sh" <<'HOOK'
export PATH="$APPDIR/usr/bin:$PATH"
HOOK

echo "==> Bundling libraries and creating the AppImage"
cd "$BUILD_DIR"
# OpenGL, Vulkan, Wayland and xkbcommon are loaded at runtime from the host,
# so they match its graphics drivers and are not bundled.
export NO_STRIP=1
export LDAI_UPDATE_INFORMATION="gh-releases-zsync|astrovm|AdventureMods|latest|AdventureMods-v*-${APPIMAGE_ARCH}.AppImage.zsync"
./linuxdeploy --appimage-extract-and-run \
	--appdir "$APPDIR" \
	--desktop-file "$APPDIR/usr/share/applications/io.github.astrovm.AdventureMods.desktop" \
	--icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/io.github.astrovm.AdventureMods.svg" \
	--output appimage

generated_name="Adventure_Mods-${APPIMAGE_ARCH}.AppImage"
version="$(sh "$PROJECT_DIR/build-aux/cargo-version.sh")"
appimage_name="AdventureMods-v${version}-${APPIMAGE_ARCH}.AppImage"

if [ ! -f "$generated_name" ]; then
	echo "Expected AppImage was not generated: $generated_name" >&2
	exit 1
fi

mv "$generated_name" "$appimage_name"
rm -f "$generated_name.zsync" "$appimage_name.zsync"
zsyncmake -u "$appimage_name" -o "$appimage_name.zsync" "$appimage_name"

echo "==> Done! AppImage created:"
ls -lh "$appimage_name" "$appimage_name.zsync"
