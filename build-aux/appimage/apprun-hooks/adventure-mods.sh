export APPDIR="${APPDIR:-"$(dirname "$(realpath "$0")")"}"

# App-specific paths
export ADVENTURE_MODS_PKGDATADIR="$APPDIR/usr/share/adventure-mods"
export PATH="$APPDIR/usr/bin:$PATH"

# GTK / GLib paths (replaces linuxdeploy-plugin-gtk defaults)
export XDG_DATA_DIRS="$APPDIR/usr/share:/usr/share:${XDG_DATA_DIRS:-/usr/share}"
export GSETTINGS_SCHEMA_DIR="$APPDIR/usr/share/glib-2.0/schemas"
export GTK_EXE_PREFIX="$APPDIR/usr"
export GTK_PATH="$APPDIR/usr/lib/gtk-4.0"
export GDK_PIXBUF_MODULE_FILE="$APPDIR/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
export GI_TYPELIB_PATH="$APPDIR/usr/lib/girepository-1.0"

# Do NOT set GDK_BACKEND — let GTK auto-detect (Wayland preferred, X11 fallback).
# Do NOT set GTK_THEME — libadwaita manages its own styling.

# Disable GStreamer media backend (the app doesn't use media playback and
# the bundled/system GStreamer versions may conflict).
export GTK_MEDIA=none
export GST_PLUGIN_SYSTEM_PATH=""
