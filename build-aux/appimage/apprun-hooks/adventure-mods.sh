export ADVENTURE_MODS_PKGDATADIR="$APPDIR/usr/share/adventure-mods"
export PATH="$APPDIR/usr/bin:$PATH"

# Prevent GTK from loading the bundled GStreamer media backend, which fails
# because system GStreamer libraries expect symbols from a newer GLib than
# what the AppImage bundles. The app doesn't use media playback.
export GTK_MEDIA=none

# Prevent system GStreamer plugins from being loaded (they may be
# incompatible with the bundled GLib version).
export GST_PLUGIN_SYSTEM_PATH=""
