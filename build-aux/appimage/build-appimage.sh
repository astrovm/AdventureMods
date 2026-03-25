#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$PROJECT_DIR/appimage-build"
APPDIR="$BUILD_DIR/AppDir"

LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
GTK_PLUGIN_URL="https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"
HPATCHZ_URL="https://github.com/sisong/HDiffPatch/releases/download/v4.12.2/hdiffpatch_v4.12.2_bin_linux64.zip"
P7ZIP_URL="https://github.com/p7zip-project/p7zip/archive/refs/tags/v17.05.tar.gz"

cleanup() {
    rm -rf "$BUILD_DIR/tmp"
}
trap cleanup EXIT

echo "==> Setting up build directory"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/tmp" "$APPDIR"

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
unzip -o -j "$BUILD_DIR/tmp/hpatchz.zip" "linux64/hpatchz" -d "$BUILD_DIR/tmp/"
install -Dm755 "$BUILD_DIR/tmp/hpatchz" "$APPDIR/usr/bin/hpatchz"

echo "==> Building p7zip"
wget -q -O "$BUILD_DIR/tmp/p7zip.tar.gz" "$P7ZIP_URL"
tar xf "$BUILD_DIR/tmp/p7zip.tar.gz" -C "$BUILD_DIR/tmp/"
make -C "$BUILD_DIR/tmp/p7zip-17.05" 7z -j"$(nproc)"
make -C "$BUILD_DIR/tmp/p7zip-17.05" install DEST_DIR= DEST_HOME="$APPDIR/usr"

echo "==> Compiling GSettings schemas"
glib-compile-schemas "$APPDIR/usr/share/glib-2.0/schemas/"

echo "==> Downloading linuxdeploy"
wget -q -O "$BUILD_DIR/linuxdeploy" "$LINUXDEPLOY_URL"
chmod +x "$BUILD_DIR/linuxdeploy"
wget -q -O "$BUILD_DIR/linuxdeploy-plugin-gtk.sh" "$GTK_PLUGIN_URL"
chmod +x "$BUILD_DIR/linuxdeploy-plugin-gtk.sh"

echo "==> Bundling libraries"
export DEPLOY_GTK_VERSION=4
export NO_STRIP=1

cd "$BUILD_DIR"

# First pass: let linuxdeploy + GTK plugin bundle libraries (no output yet).
./linuxdeploy --appimage-extract-and-run \
    --appdir "$APPDIR" \
    --plugin gtk \
    --desktop-file "$APPDIR/usr/share/applications/io.github.astrovm.AdventureMods.desktop" \
    --icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/io.github.astrovm.AdventureMods.svg"

# Replace the GTK plugin hook with our own. The default hook forces
# GDK_BACKEND=x11 and sets GTK_THEME, both of which break libadwaita apps.
echo "==> Patching apprun hooks for libadwaita"
cp "$SCRIPT_DIR/apprun-hooks/adventure-mods.sh" "$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"

# Remove the bundled GStreamer media backend. The app doesn't use media
# playback and the module causes harmless but noisy errors on startup.
rm -f "$APPDIR"/usr/lib/gtk-4.0/4.0.0/media/libmedia-gstreamer.so

# Second pass: produce the AppImage.
./linuxdeploy --appimage-extract-and-run \
    --appdir "$APPDIR" \
    --output appimage

echo "==> Done! AppImage created:"
ls -lh "$BUILD_DIR"/Adventure_Mods*.AppImage 2>/dev/null || ls -lh "$BUILD_DIR"/*.AppImage
