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

GTK4_VERSION="4.20.3"
GTK4_URL="https://download.gnome.org/sources/gtk/4.20/gtk-${GTK4_VERSION}.tar.xz"
LIBADWAITA_VERSION="1.8.5.1"
LIBADWAITA_URL="https://download.gnome.org/sources/libadwaita/1.8/libadwaita-${LIBADWAITA_VERSION}.tar.xz"

cleanup() {
	rm -rf "$BUILD_DIR/tmp"
}
trap cleanup EXIT

# GTK 4.20+ requires Meson >= 1.5.0. Install via pip if the system version is too old.
REQUIRED_MESON="1.5.0"
CURRENT_MESON="$(meson --version 2>/dev/null || echo 0)"
if [ "$(printf '%s\n' "$REQUIRED_MESON" "$CURRENT_MESON" | sort -V | head -1)" != "$REQUIRED_MESON" ]; then
	echo "==> Upgrading Meson (need >= ${REQUIRED_MESON}, have ${CURRENT_MESON})"
	pip3 install --quiet --break-system-packages --force-reinstall meson
	hash -r
	echo "    Meson upgraded to $(meson --version)"
fi

echo "==> Setting up build directory"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/tmp" "$APPDIR"

# Build GTK4 and libadwaita from source for smooth animations (GTK 4.20+),
# while linking against the host glibc for broad compatibility.
echo "==> Building GTK4 ${GTK4_VERSION} from source"
wget -q -O "$BUILD_DIR/tmp/gtk4.tar.xz" "$GTK4_URL"
tar xf "$BUILD_DIR/tmp/gtk4.tar.xz" -C "$BUILD_DIR/tmp/"
meson setup "$BUILD_DIR/tmp/gtk4-build" "$BUILD_DIR/tmp/gtk-${GTK4_VERSION}" \
	--prefix=/usr --buildtype=release \
	-Dmedia-gstreamer=disabled \
	-Dprint-cups=disabled \
	-Dbuild-demos=false \
	-Dbuild-examples=false \
	-Dbuild-tests=false \
	-Dbuild-testsuite=false \
	-Dintrospection=disabled \
	-Ddocumentation=false
meson compile -C "$BUILD_DIR/tmp/gtk4-build"
sudo meson install -C "$BUILD_DIR/tmp/gtk4-build"
sudo ldconfig

echo "==> Building libadwaita ${LIBADWAITA_VERSION} from source"
wget -q -O "$BUILD_DIR/tmp/libadwaita.tar.xz" "$LIBADWAITA_URL"
tar xf "$BUILD_DIR/tmp/libadwaita.tar.xz" -C "$BUILD_DIR/tmp/"
meson setup "$BUILD_DIR/tmp/adw-build" "$BUILD_DIR/tmp/libadwaita-${LIBADWAITA_VERSION}" \
	--prefix=/usr --buildtype=release \
	-Dintrospection=disabled \
	-Ddocumentation=false \
	-Dtests=false \
	-Dexamples=false \
	-Dvapi=false
meson compile -C "$BUILD_DIR/tmp/adw-build"
sudo meson install -C "$BUILD_DIR/tmp/adw-build"
sudo ldconfig

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
	--exclude-library 'libvulkan.so.*' \
	--exclude-library 'libwayland-egl.so.*' \
	--plugin gtk \
	--desktop-file "$APPDIR/usr/share/applications/io.github.astrovm.AdventureMods.desktop" \
	--icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/io.github.astrovm.AdventureMods.svg"

# Replace the GTK plugin hook with our own. The default hook forces
# GDK_BACKEND=x11 and sets GTK_THEME, both of which break libadwaita apps.
echo "==> Patching apprun hooks for libadwaita"
cp "$SCRIPT_DIR/apprun-hooks/adventure-mods.sh" "$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"

# Keep low-level graphics loader libraries on the host side. Bundling these
# while still relying on host Mesa/ICD drivers can lead to a mixed graphics
# stack with worse animation smoothness than the Flatpak runtime.
echo "==> Removing bundled graphics loader libraries"
rm -f \
	"$APPDIR/usr/lib/libvulkan.so.1" \
	"$APPDIR/usr/lib/libwayland-egl.so.1"

# Remove the bundled GStreamer media backend. The app doesn't use media
# playback and the module causes errors due to GLib version mismatches.
rm -f "$APPDIR"/usr/lib/gtk-4.0/4.0.0/media/libmedia-gstreamer.so

# Second pass: produce the AppImage.
./linuxdeploy --appimage-extract-and-run \
	--appdir "$APPDIR" \
	--exclude-library 'libvulkan.so.*' \
	--exclude-library 'libwayland-egl.so.*' \
	--output appimage

echo "==> Done! AppImage created:"
ls -lh "$BUILD_DIR"/Adventure_Mods*.AppImage 2>/dev/null || ls -lh "$BUILD_DIR"/*.AppImage
