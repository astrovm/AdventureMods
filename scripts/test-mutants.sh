#!/usr/bin/env bash
# Checks scripts/mutants.sh and scripts/mutants-comment.sh with a fake cargo
# and a fake gh, so no build or network is involved.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
root=$PWD
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() { echo "FAIL $*" >&2; exit 1; }
expect_code() {
    local expected=$1 status=0
    shift
    "$@" >"$work/output" 2>&1 || status=$?
    [[ $status == "$expected" ]] || { cat "$work/output" >&2; fail "exit $status, wanted $expected: $*"; }
}

# A repository shaped like this one, with a fake cargo that records its arguments.
repo="$work/repo"
mkdir -p "$repo/scripts" "$repo/src/nested" "$repo/tests" "$work/bin"
cp scripts/mutants.sh "$repo/scripts/"
printf 'fn a() {}\n' >"$repo/src/a.rs"
printf 'fn n() {}\n' >"$repo/src/nested/n.rs"
printf 'fn t() {}\n' >"$repo/tests/t.rs"
printf 'notes\n' >"$repo/src/notes.md"
cat >"$work/bin/cargo" <<'CARGO'
#!/bin/bash
if [[ $1 == mutants && $2 == --version ]]; then echo "cargo-mutants ${FAKE_VERSION:-27.1.0}"; exit 0; fi
printf '%s\n' "$*" >>"$CARGO_CALLS"
[[ $1 == mutants ]] && exit "${FAKE_STATUS:-0}"
exit 0
CARGO
chmod +x "$work/bin/cargo"
git -C "$repo" init -q -b main
git -C "$repo" add -A
git -C "$repo" -c user.name=t -c user.email=t@example.com -c core.hooksPath=/dev/null commit -q -m base
git -C "$repo" checkout -q -b change
mutants() {
    : >"$work/calls"
    (cd "$repo" && env -u CI PATH="$work/bin:$PATH" CARGO_CALLS="$work/calls" "$@")
}

# Nothing changed in src/: nothing to mutate.
printf 'fn t() { 1; }\n' >"$repo/tests/t.rs"
printf 'more notes\n' >>"$repo/src/notes.md"
git -C "$repo" -c user.name=t -c user.email=t@example.com -c core.hooksPath=/dev/null commit -q -am other
expect_code 0 mutants scripts/mutants.sh diff main
grep -q 'No source changes to mutate' "$work/output" || fail 'no message for an empty diff'
[[ ! -s $work/calls ]] || fail 'cargo ran with nothing to mutate'

# Only changed Rust files of src/ go to cargo-mutants, nested ones included.
printf 'fn a() { 1; }\n' >"$repo/src/a.rs"
printf 'fn n() { 2; }\n' >"$repo/src/nested/n.rs"
git -C "$repo" -c user.name=t -c user.email=t@example.com -c core.hooksPath=/dev/null commit -q -am core
expect_code 0 mutants scripts/mutants.sh diff main
grep -Fq 'mutants --test-tool nextest' "$work/calls" || fail 'not run with nextest'
grep -Fq -- '--in-diff build/mutants/changes.diff' "$work/calls" || fail 'no --in-diff'
grep -Fq -- '--baseline skip' "$work/calls" || fail 'baseline not skipped'
grep -Fq -- '--in-place' "$work/calls" && fail 'in-place outside CI'
diff="$repo/build/mutants/changes.diff"
grep -q 'a/src/a.rs' "$diff" || fail 'a.rs missing from the diff'
grep -q 'a/src/nested/n.rs' "$diff" || fail 'nested file missing from the diff'
grep -q 'tests/t.rs' "$diff" && fail 'a test file in the diff'
grep -q 'notes.md' "$diff" && fail 'markdown in the diff'

# CI mutates in place; the output directory can move.
expect_code 0 mutants env CI=true MUTANTS_OUTPUT=build/other scripts/mutants.sh diff main
grep -Fq -- '--in-place' "$work/calls" || fail 'no --in-place in CI'
grep -Fq -- '--output build/other' "$work/calls" || fail 'output directory ignored'

# A part of everything.
expect_code 0 mutants scripts/mutants.sh shard 3/64
grep -Fq -- '--shard 3/64 --sharding round-robin' "$work/calls" || fail 'shard arguments wrong'

# cargo-mutants' exit status (2 missed, 3 timeout) passes through; the workflow decides.
expect_code 2 mutants env FAKE_STATUS=2 scripts/mutants.sh shard 0/64
expect_code 3 mutants env FAKE_STATUS=3 scripts/mutants.sh shard 0/64

# Bad arguments; a wrong cargo-mutants version is installed first.
expect_code 2 mutants scripts/mutants.sh
expect_code 1 mutants scripts/mutants.sh shard
expect_code 0 mutants env FAKE_VERSION=26.0.0 scripts/mutants.sh shard 0/64
grep -Fq 'install cargo-mutants --version 27.1.0' "$work/calls" || fail 'old cargo-mutants kept'

# Mutation results keep one comment on the pull request, gone once every
# mutant is caught, through a synthetic GitHub API.
cat >"$work/bin/gh" <<'GH'
#!/bin/bash
printf '%s\n' "$*" >>"$GH_CALLS"
[[ $* == *--paginate* ]] && cat "$GH_OLD"
exit 0
GH
chmod +x "$work/bin/gh"
results="$work/mutants"
mkdir -p "$results"
printf 'a.rs:1:1: replace f with g\n' >"$results/missed.txt"
printf 'a.rs:2:1: one\na.rs:3:1: two\n' >"$results/caught.txt"
comment() {
    : >"$work/calls"
    printf '%s' "$1" >"$work/old"
    env PATH="$work/bin:$PATH" GH_CALLS="$work/calls" GH_OLD="$work/old" GITHUB_REPOSITORY=o/r PR=7 RUN=https://run \
        FORK="${2:-false}" GITHUB_STEP_SUMMARY="$work/summary" "$root/scripts/mutants-comment.sh" "$results"
}
comment ''
grep -q '^api repos/o/r/issues/7/comments -f body=<!-- mutants -->' "$work/calls" || fail 'no new comment'
grep -q '2 of 3 changes caught by a test' "$work/calls" || fail 'wrong tally'
grep -q 'a.rs:1:1: replace f with g' "$work/calls" || fail 'lived mutant not listed'
comment 99
grep -q '^api -X PATCH repos/o/r/issues/comments/99 -f body=' "$work/calls" || fail 'comment not edited'
comment '' true
[[ ! -s $work/calls ]] || fail 'a fork commented'
grep -q 'a.rs:1:1: replace f with g' "$work/summary" || fail 'no run summary'
rm "$results/missed.txt"
comment 99
grep -q '^api -X DELETE repos/o/r/issues/comments/99$' "$work/calls" || fail 'comment kept after all caught'
# Only a missed mutant and no caught list: zero caught, not a blank.
printf 'a.rs:1:1: replace f with g\n' >"$results/missed.txt"
rm "$results/caught.txt"
comment ''
grep -q '0 of 1 changes caught by a test' "$work/calls" || fail 'no caught list is not zero'
# Nothing to mutate leaves no results at all, which is the same as all caught.
rm -r "$results"
comment 99
grep -q '^api -X DELETE repos/o/r/issues/comments/99$' "$work/calls" || fail 'comment kept with no results'
# A list too long for one comment is cut between lines and says where the rest is.
rm -rf "$results"
mkdir -p "$results"
for i in $(seq 1 2000); do printf 'src/some/long/path/file.rs:%d:5: replace a with b in some_function_name\n' "$i"; done >"$results/missed.txt"
comment ''
grep -qF 'api repos/o/r/issues/7/comments' "$work/calls" || fail 'no comment for a long list'
[[ $(LC_ALL=C wc -c <"$work/calls") -le 60500 ]] || fail 'long comment over 60,000 bytes'
[[ $(grep -c '^```' "$work/calls") == 2 ]] || fail 'long comment fence unbalanced'
grep -q 'more are not shown here' "$work/calls" || fail 'no note about the cut'
grep -q 'file.rs:1:5: replace a with b' "$work/calls" || fail 'first row lost'
grep -q '\[Full report\](https://run)' "$work/calls" || fail 'no link to the full report'
last=$(grep -E '^src/some' "$work/calls" | tail -n 1)
[[ $last == *'in some_function_name' ]] || fail 'a row was cut in the middle'
echo 'PASS mutation helper scripts'
