#!/bin/sh
set -eu

if command -v podman >/dev/null 2>&1; then
    engine=podman
elif command -v docker >/dev/null 2>&1; then
    engine=docker
else
    echo 'Install Podman or Docker to run the isolated coverage check.' >&2
    exit 127
fi

task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
image=localhost/adventuremods-coverage:rust1.100.0
nice -n 10 "$engine" build --tag "$image" \
    --file "$task_root/scripts/coverage.Containerfile" "$task_root/scripts"

set -- "$engine" run --rm --memory 4g --memory-swap 4g \
    --user "$(id -u):$(id -g)" \
    --volume "$task_root:/workspace" --workdir /workspace \
    --env CARGO_HOME=/workspace/target/agent/cargo-home \
    --env CARGO_TARGET_DIR=/workspace/target/agent \
    --env CARGO_BUILD_JOBS=2 \
    --env LIBGL_ALWAYS_SOFTWARE=1
if [ "$engine" = podman ]; then
    set -- "$@" --userns keep-id
fi

# The container has no host sockets, credentials, devices or home directory.
# It cannot test host integration or a physical controller.
exec nice -n 10 "$@" "$image" sh -ec '
    mkdir -p "$CARGO_HOME"
    cargo fmt --check
    cargo clippy --locked --all-targets -- -D warnings
    xvfb-run cargo llvm-cov nextest --locked --workspace --all-targets \
        --no-fail-fast --fail-under-lines 100 --show-missing-lines
'
