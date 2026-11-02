#!/usr/bin/env bash
# Mutation testing: cargo-mutants changes the code in small ways, such as
# flipping a comparison or emptying a function, and runs the tests for each
# change. A change no test notices is a missed mutant: code whose behavior
# nothing checks.
# Tests run with cargo-nextest, one process per test, like CI: they change
# environment variables. The window tests need a display, so run this under
# xvfb-run on a machine without one.
# Usage: scripts/mutants.sh diff BASE   mutants in the lines of src/ changed since BASE
#        scripts/mutants.sh shard K/N   the Kth of N equal parts of every mutant
# Exits 2 when a mutant was missed and 3 when one timed out (cargo-mutants).
# Results land in build/mutants/mutants.out, or MUTANTS_OUTPUT.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
usage='Usage: scripts/mutants.sh diff BASE | shard K/N'
version=27.1.0
if [[ $(cargo mutants --version 2>/dev/null) != "cargo-mutants $version" ]]; then
    # Unlocked, so it builds with crossbeam-epoch 0.9.20 or later: its lock
    # file pins 0.9.18 (RUSTSEC-2026-0204). CI installs a prebuilt binary.
    cargo install cargo-mutants --version "$version"
fi
output=${MUTANTS_OUTPUT:-build/mutants}
mkdir -p "$output"
args=(--test-tool nextest --no-shuffle --output "$output"
    --timeout "${MUTANTS_TIMEOUT:-300}"
    # CI tests the unchanged code in its own jobs.
    --baseline skip)
# A CI checkout is disposable: mutate it in place, so the restored build
# cache serves every mutant. Elsewhere cargo-mutants works on a copy.
if [[ -n ${CI:-} ]]; then args+=(--in-place); fi
case "${1:-}" in
    diff)
        base=${2:?$usage}
        diff="$output/changes.diff"
        git diff "$(git merge-base "$base" HEAD)" HEAD -- 'src/*.rs' > "$diff"
        if [[ ! -s $diff ]]; then
            echo 'No source changes to mutate.'
            exit 0
        fi
        args+=(--in-diff "$diff")
        ;;
    shard)
        args+=(--shard "${2:?$usage}" --sharding round-robin)
        ;;
    *)
        echo "$usage" >&2
        exit 2
        ;;
esac
cargo mutants "${args[@]}"
