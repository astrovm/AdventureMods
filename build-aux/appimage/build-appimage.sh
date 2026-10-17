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
		;;
	aarch64 | arm64)
		APPIMAGE_ARCH="aarch64"
		LINUXDEPLOY_ARCH="aarch64"
		;;
	*)
		echo "Unsupported AppImage architecture: $BUILD_ARCH" >&2
		exit 1
		;;
esac

LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage"

echo "==> Setting up build directory"
rm -rf "$BUILD_DIR"
mkdir -p "$APPDIR"
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

bash "$PROJECT_DIR/build-aux/build-tools.sh" "$APPDIR/usr/bin"

# An AppImage can't bundle glibc, so it only starts where the host's is at
# least as new as the one it was built against.
MAX_GLIBC="2.39"
needed_glibc="$(objdump -T "$APPDIR"/usr/bin/* | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -n 1)"
if [ "$(printf '%s\n%s\n' "$needed_glibc" "$MAX_GLIBC" | sort -V | tail -n 1)" != "$MAX_GLIBC" ]; then
	echo "The AppImage needs glibc $needed_glibc, newer than $MAX_GLIBC. Build it on Ubuntu 24.04." >&2
	exit 1
fi
echo "==> Needs glibc $needed_glibc (at most $MAX_GLIBC)"

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
