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
# A debug build decodes screenshots and draws far slower.
if grep -qa '/target/debug/' "$files_dir/bin/adventure-mods"; then
    echo "adventure-mods is a debug build" >&2
    exit 1
fi
test -x "$files_dir/bin/7zz"
test -x "$files_dir/bin/hpatchz"
test -f "$files_dir/share/applications/$app_id.desktop"
test -f "$files_dir/share/metainfo/$app_id.metainfo.xml"
test -f "$files_dir/share/icons/hicolor/scalable/apps/$app_id.svg"
test -f "$metadata"

desktop-file-validate "$files_dir/share/applications/$app_id.desktop"
appstreamcli validate --no-net "$files_dir/share/metainfo/$app_id.metainfo.xml"

# Steam mounts, and no way to run commands outside the sandbox
grep -qE '(\.steam|Steam)' "$metadata"
if grep -q 'org\.freedesktop\.Flatpak' "$metadata"; then
    echo "The Flatpak can run commands outside its sandbox" >&2
    exit 1
fi

# The bundled tools are built from source with the formats setup needs
flatpak build "$build_dir" 7zz i | grep -q 'Rar5'
flatpak build "$build_dir" hpatchz -v >/dev/null

flatpak build "$build_dir" adventure-mods --version
