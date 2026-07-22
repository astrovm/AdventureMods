#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
    echo "Usage: $0 BUILD_DIR MANIFEST APP_ID" >&2
    exit 2
fi

build_dir=$1
manifest=$2
app_id=$3
files_dir="$build_dir/files"

test -x "$files_dir/bin/adventure-mods"
test -x "$files_dir/bin/7zz"
test -x "$files_dir/bin/hpatchz"
test -f "$files_dir/share/applications/$app_id.desktop"
test -f "$files_dir/share/metainfo/$app_id.metainfo.xml"
test -f "$files_dir/share/icons/hicolor/scalable/apps/$app_id.svg"

desktop-file-validate "$files_dir/share/applications/$app_id.desktop"
appstreamcli validate --no-net "$files_dir/share/metainfo/$app_id.metainfo.xml"
glib-compile-schemas --strict --dry-run "$files_dir/share/glib-2.0/schemas"

flatpak-builder --run "$build_dir" "$manifest" adventure-mods --version
