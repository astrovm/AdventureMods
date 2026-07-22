#!/bin/sh
# Meson↔Cargo test runner — mirrors cargo-build.sh env handling.
set -eu

MESON_BUILD_ROOT=$1
MESON_SOURCE_ROOT=$2

export CARGO_TARGET_DIR="$MESON_BUILD_ROOT/target"

# Prefer CARGO_HOME from the environment (Flatpak sets this); otherwise match cargo-build.sh.
if [ -z "${CARGO_HOME:-}" ]; then
    export CARGO_HOME="$CARGO_TARGET_DIR/cargo-home"
fi

# Offline only when a vendored crate tree is present.
if [ -d "$MESON_SOURCE_ROOT/cargo/vendor" ]; then
    export CARGO_NET_OFFLINE=true
fi

exec cargo test --locked --manifest-path "$MESON_SOURCE_ROOT/Cargo.toml"
