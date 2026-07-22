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

# Vendored sources (same setup as cargo-build.sh) + offline when a vendor tree exists.
if [ -d "$MESON_SOURCE_ROOT/cargo/vendor" ]; then
    mkdir -p "$CARGO_HOME"
    if [ ! -f "$CARGO_HOME/config" ] && [ ! -f "$CARGO_HOME/config.toml" ]; then
        cat > "$CARGO_HOME/config.toml" <<TOML
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$MESON_SOURCE_ROOT/cargo/vendor"
TOML
    fi
    export CARGO_NET_OFFLINE=true
fi

exec cargo test --locked --manifest-path "$MESON_SOURCE_ROOT/Cargo.toml"
