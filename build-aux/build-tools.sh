#!/usr/bin/env bash
# Build 7zz and hpatchz from source into DEST_DIR, for the AppImage and the
# Snap. The Flatpak builds them the same way in build-aux/flatpak/7zip.json
# and hdiffpatch.json; a test keeps the sources and flags in sync.
set -euo pipefail

if [ "$#" -ne 1 ]; then
	echo "Usage: $0 DEST_DIR" >&2
	exit 2
fi
DEST_DIR="$1"

SEVENZIP_URL="https://github.com/ip7z/7zip/releases/download/26.04/7z2604-src.tar.xz"
SEVENZIP_SHA256="9691944c0fe0d01bb49373a704fb983fd33bc98b1738695179dfbf99ac1734f6"
HDIFFPATCH_URL="https://github.com/sisong/HDiffPatch/archive/refs/tags/v5.1.3.tar.gz"
HDIFFPATCH_SHA256="435c46447e7a1ace0327b67da2ec50a06e7f02d19960a182b493ff2e3f91c2b0"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
jobs="$(nproc)"

fetch() {
	curl -fsSL -o "$2" "$1"
	echo "$3  $2" | sha256sum -c - >/dev/null
}

echo "==> Building 7-Zip"
fetch "$SEVENZIP_URL" "$tmp/7zip.tar.xz" "$SEVENZIP_SHA256"
mkdir -p "$tmp/7zip"
tar xf "$tmp/7zip.tar.xz" -C "$tmp/7zip"
make -C "$tmp/7zip/CPP/7zip/Bundles/Alone2" -f ../../cmpl_gcc.mak -j"$jobs" 'CFLAGS_WARN_WALL=-Wall -Wextra' >/dev/null

echo "==> Building hpatchz"
fetch "$HDIFFPATCH_URL" "$tmp/hdiffpatch.tar.gz" "$HDIFFPATCH_SHA256"
mkdir -p "$tmp/HDiffPatch"
tar xf "$tmp/hdiffpatch.tar.gz" -C "$tmp/HDiffPatch" --strip-components=1
# HDiffPatch builds LZMA, which the SADX patch uses, from ../lzma/C: the
# LZMA SDK that 7-Zip's sources include.
ln -s 7zip "$tmp/lzma"
make -C "$tmp/HDiffPatch" -j"$jobs" hpatchz LDEF=0 ZLIB=0 ZSTD=0 MD5=0 XXH=0 VCD=0 BSD=0 BZIP2=0 >/dev/null

install -Dm755 "$tmp/7zip/CPP/7zip/Bundles/Alone2/b/g/7zz" "$DEST_DIR/7zz"
install -Dm755 "$tmp/HDiffPatch/hpatchz" "$DEST_DIR/hpatchz"
