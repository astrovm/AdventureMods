#!/bin/sh
set -eu

manifest=${1:-$(dirname "$0")/../Cargo.toml}
awk -F '"' '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
        print $2
        found = 1
        exit
    }
    END { if (!found) exit 1 }
' "$manifest"
