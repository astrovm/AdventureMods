#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 BUILD_DIR APP_ID" >&2
    exit 2
fi

build_dir=$1
app_id=$2
files_dir="$build_dir/files"
metadata="$build_dir/metadata"

test -x "$files_dir/bin/adventure-mods"
test -x "$files_dir/bin/7zz"
test -x "$files_dir/bin/hpatchz"
test -f "$files_dir/share/applications/$app_id.desktop"
test -f "$files_dir/share/metainfo/$app_id.metainfo.xml"
test -f "$files_dir/share/icons/hicolor/scalable/apps/$app_id.svg"
test -f "$metadata"

desktop-file-validate "$files_dir/share/applications/$app_id.desktop"
appstreamcli validate --no-net "$files_dir/share/metainfo/$app_id.metainfo.xml"
glib-compile-schemas --strict --dry-run "$files_dir/share/glib-2.0/schemas"

# Host integration finish-args (Steam mounts + flatpak-spawn talk-name)
grep -q 'org\.freedesktop\.Flatpak' "$metadata"
grep -qE '(\.steam|Steam)' "$metadata"
flatpak build "$build_dir" sh -c 'command -v flatpak-spawn >/dev/null'

flatpak build "$build_dir" adventure-mods --version
