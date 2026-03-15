#!/bin/sh
# Meson↔Cargo integration script

export MESON_BUILD_ROOT="$1"
export MESON_SOURCE_ROOT="$2"
export CARGO_TARGET_DIR="$MESON_BUILD_ROOT"/target
export OUTPUT="$3"
export BUILDTYPE="$4"
export APP_BIN="$5"

# If CARGO_HOME is not already set (e.g. by Flatpak), use a local one
if [ -z "$CARGO_HOME" ]; then
    export CARGO_HOME="$CARGO_TARGET_DIR"/cargo-home
fi

# Set up vendored sources if the vendor directory exists (Flatpak offline build)
if [ -d "$MESON_SOURCE_ROOT/cargo/vendor" ]; then
    mkdir -p "$CARGO_HOME"
    cat > "$CARGO_HOME/config.toml" <<TOML
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$MESON_SOURCE_ROOT/cargo/vendor"
TOML
fi

if [ "$BUILDTYPE" = "release" ]; then
    echo "RELEASE MODE"
    cargo build --manifest-path "$MESON_SOURCE_ROOT"/Cargo.toml --release && \
        cp "$CARGO_TARGET_DIR"/release/"$APP_BIN" "$OUTPUT"
else
    echo "DEBUG MODE"
    cargo build --manifest-path "$MESON_SOURCE_ROOT"/Cargo.toml && \
        cp "$CARGO_TARGET_DIR"/debug/"$APP_BIN" "$OUTPUT"
fi
