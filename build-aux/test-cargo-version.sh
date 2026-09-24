#!/bin/sh
set -eu

tmp_file=$(mktemp)
trap 'rm -f "$tmp_file"' EXIT HUP INT TERM
cat > "$tmp_file" <<'EOF'
[workspace.package]
version = "9.9.9"

[package]
name = "synthetic-app"
version = "1.2.3"

[dependencies]
version = "8.8.8"
EOF

test "$(sh "$(dirname "$0")/cargo-version.sh" "$tmp_file")" = 1.2.3
test "$(sh "$(dirname "$0")/cargo-version.sh")" = "$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
