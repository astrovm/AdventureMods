#!/bin/sh
set -eu

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

source_dir="$tmp_dir/source"
build_dir="$tmp_dir/build"
bin_dir="$tmp_dir/bin"
output_dir="$tmp_dir/output"
mkdir -p "$source_dir" "$bin_dir" "$output_dir"
: > "$source_dir/Cargo.toml"

# Records its arguments and leaves a binary where Cargo would.
cat > "$bin_dir/cargo" <<'SCRIPT'
#!/bin/sh
set -eu
printf '%s\n' "$@" > "$TEST_OUTPUT/args"
profile=debug
for arg in "$@"; do
    if [ "$arg" = --release ]; then
        profile=release
    fi
done
mkdir -p "$CARGO_TARGET_DIR/$profile"
echo "$profile" > "$CARGO_TARGET_DIR/$profile/adventure-mods"
SCRIPT
chmod +x "$bin_dir/cargo"

for buildtype in release plain debugoptimized minsize debug; do
    rm -rf "$build_dir" "$output_dir"/*
    mkdir -p "$build_dir"
    TEST_OUTPUT="$output_dir" PATH="$bin_dir:$PATH" \
        sh "$(dirname "$0")/cargo-build.sh" \
        "$build_dir" "$source_dir" "$output_dir/binary" "$buildtype" adventure-mods \
        > /dev/null
    if [ "$buildtype" = debug ]; then
        expected=debug
    else
        expected=release
    fi
    test "$(cat "$output_dir/binary")" = "$expected"
done
