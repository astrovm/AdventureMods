#!/bin/sh
# Run upload-snaps.sh against a fake snapcraft that answers the way the Snap
# Store does, including the endless wait behind a pending review.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

# The fake reads what to do for each architecture from FAKE_x86_64 and
# FAKE_aarch64: released, review, queued (polls forever), hang (polls
# forever without saying why) or error.
cat >"$tmp/snapcraft" <<'EOF'
#!/bin/sh
for snap; do :; done
case $snap in
*x86_64.snap) mode=$FAKE_x86_64 ;;
*aarch64.snap) mode=$FAKE_aarch64 ;;
esac
echo "Pushing $snap"
case $mode in
released) echo "Revision 7 created for 'adventure-mods' and released to 'stable'" ;;
review)
	echo "Status: will need manual review"
	echo "Error: Issues while processing snap: - human review required" >&2
	exit 2
	;;
queued)
	while :; do
		echo "Status: Waiting for previous upload(s) to complete their review process"
		sleep 1
	done
	;;
hang) while :; do echo "Status: processing"; sleep 1; done ;;
error) echo "Error: invalid credentials" >&2; exit 3 ;;
esac
EOF
chmod +x "$tmp/snapcraft"

export SNAPCRAFT="$tmp/snapcraft" SNAP_UPLOAD_TIMEOUT=2 SNAPCRAFT_STORE_CREDENTIALS=fake

run() {
	FAKE_x86_64=$1 FAKE_aarch64=$2 sh "$here/snap/upload-snaps.sh" v9.9.9 "$tmp" >"$tmp/out" 2>&1
}

run released released
run review released
grep -q "::notice::The x86_64 snap is uploaded and waiting for review" "$tmp/out"
# The first upload is held for review, so the second waits behind it.
run review queued
grep -q "::notice::The aarch64 snap is uploaded and waiting for review" "$tmp/out"
grep -q "Waiting for previous upload" "$tmp/out"

if run released hang; then echo "a hang without a review must fail" >&2; exit 1; fi
grep -q "::error::Uploading the aarch64 snap took over 2 seconds" "$tmp/out"
if run error released; then echo "an upload error must fail" >&2; exit 1; fi
grep -q "invalid credentials" "$tmp/out"
if SNAPCRAFT_STORE_CREDENTIALS='' run released released; then
	echo "missing credentials must fail" >&2
	exit 1
fi
grep -q "SNAPCRAFT_STORE_CREDENTIALS is required" "$tmp/out"

echo "upload-snaps.sh: all cases pass"
