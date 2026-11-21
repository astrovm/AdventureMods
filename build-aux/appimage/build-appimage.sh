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

echo "==> Downloading linuxdeploy"
wget -q -O "$BUILD_DIR/linuxdeploy" "$LINUXDEPLOY_URL"
chmod +x "$BUILD_DIR/linuxdeploy"

# The bundled 7zz and hpatchz sit next to the app; find them first.
mkdir -p "$APPDIR/apprun-hooks"
cat >"$APPDIR/apprun-hooks/adventure-mods.sh" <<'HOOK'
export PATH="$APPDIR/usr/bin:$PATH"
HOOK

# File managers and the AppImage catalog show .DirIcon as the thumbnail, and
# the catalog wants a PNG. linuxdeploy links it to the SVG, unless it exists.
icon_name="io.github.astrovm.AdventureMods"
icon_png="usr/share/icons/hicolor/256x256/apps/$icon_name.png"
mkdir -p "$APPDIR/$(dirname "$icon_png")"
rsvg-convert -w 256 -h 256 -o "$APPDIR/$icon_png" \
	"$APPDIR/usr/share/icons/hicolor/scalable/apps/$icon_name.svg"
ln -sfn "$icon_png" "$APPDIR/.DirIcon"

echo "==> Bundling libraries and creating the AppImage"
cd "$BUILD_DIR"
# winit opens the X11 and Wayland libraries at runtime, so linuxdeploy can't
# see them. OpenGL, EGL, libX11, libxcb and libwayland-client come from the
# host: they are on the AppImage exclude list, because they must match its
# graphics drivers and display server. These are not on it, and minimal
# systems don't have them, so the AppImage brings them. linuxdeploy adds
# what they need in turn, such as libxcb-xkb and libXrender.
runtime_libs=()
for lib in libxkbcommon.so.0 libxkbcommon-x11.so.0 libXcursor.so.1 libXi.so.6; do
	path="$(/sbin/ldconfig -p | awk -v lib="$lib" '$1 == lib { print $NF; exit }')"
	if [ -z "$path" ]; then
		echo "Missing $lib on the build system. Install it in build-container.sh." >&2
		exit 1
	fi
	runtime_libs+=(--library "$path")
done
export NO_STRIP=1
export LDAI_UPDATE_INFORMATION="gh-releases-zsync|astrovm|AdventureMods|latest|AdventureMods-v*-${APPIMAGE_ARCH}.AppImage.zsync"
./linuxdeploy --appimage-extract-and-run \
	--appdir "$APPDIR" \
	--desktop-file "$APPDIR/usr/share/applications/io.github.astrovm.AdventureMods.desktop" \
	--icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/io.github.astrovm.AdventureMods.svg" \
	"${runtime_libs[@]}" \
	--output appimage

# An AppImage uses the host's glibc, so it only starts where that glibc is at
# least as new as the newest one any bundled file needs. Ubuntu 22.04, the
# oldest Ubuntu LTS in standard support, has glibc 2.35. The AppImage catalog
# tests on it. This checks the app and the libraries linuxdeploy bundled.
MAX_GLIBC="2.35"
needed_glibc="$(
	find "$APPDIR" -type f -print0 |
		while IFS= read -r -d '' file; do
			if [ "$(head -c 4 "$file" | od -An -c | tr -d ' ')" = '177ELF' ]; then
				# A static program or object file has no dynamic symbols.
				objdump -T "$file" || true
			fi
		done |
		{ grep -o 'GLIBC_[0-9.]*' || true; } | sed 's/GLIBC_//' | sort -uV | tail -n 1
)"
if [ -z "$needed_glibc" ]; then
	echo "Found no glibc symbols in $APPDIR. The check cannot work." >&2
	exit 1
fi
if [ "$(printf '%s\n%s\n' "$needed_glibc" "$MAX_GLIBC" | sort -V | tail -n 1)" != "$MAX_GLIBC" ]; then
	echo "The AppImage needs glibc $needed_glibc, newer than $MAX_GLIBC. Build it on Ubuntu 22.04." >&2
	exit 1
fi
echo "==> Needs glibc $needed_glibc (at most $MAX_GLIBC)"

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
