#!/bin/sh
set -eu

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

source_dir="$tmp_dir/source"
build_dir="$tmp_dir/build"
bin_dir="$tmp_dir/bin"
output_dir="$tmp_dir/output"
mkdir -p "$source_dir/cargo/vendor" "$bin_dir" "$output_dir"
: > "$source_dir/Cargo.toml"

cat > "$bin_dir/cargo" <<'EOF'
#!/bin/sh
set -eu
printf '%s\n' "$CARGO_HOME" > "$TEST_OUTPUT/cargo-home"
printf '%s\n' "$CARGO_TARGET_DIR" > "$TEST_OUTPUT/target-dir"
printf '%s\n' "${CARGO_NET_OFFLINE:-}" > "$TEST_OUTPUT/offline"
printf '%s\n' "$@" > "$TEST_OUTPUT/args"
EOF
chmod +x "$bin_dir/cargo"

TEST_OUTPUT="$output_dir" PATH="$bin_dir:$PATH" \
    sh "$(dirname "$0")/cargo-test.sh" "$build_dir" "$source_dir"

default_home="$build_dir/target/cargo-home"
test "$(cat "$output_dir/cargo-home")" = "$default_home"
test "$(cat "$output_dir/target-dir")" = "$build_dir/target"
test "$(cat "$output_dir/offline")" = true
grep -Fqx 'replace-with = "vendored-sources"' "$default_home/config.toml"
grep -Fqx "directory = \"$source_dir/cargo/vendor\"" "$default_home/config.toml"

expected_args=$(cat <<EOF
test
--locked
--manifest-path
$source_dir/Cargo.toml
EOF
)
test "$(cat "$output_dir/args")" = "$expected_args"

custom_home="$tmp_dir/custom-cargo-home"
rm -f "$output_dir"/*
TEST_OUTPUT="$output_dir" PATH="$bin_dir:$PATH" CARGO_HOME="$custom_home" \
    sh "$(dirname "$0")/cargo-test.sh" "$build_dir" "$source_dir"

test "$(cat "$output_dir/cargo-home")" = "$custom_home"
test -f "$custom_home/config.toml"
