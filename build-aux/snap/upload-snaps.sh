#!/bin/sh
# Upload a release's snaps to the Snap Store's stable channel.
# Usage: upload-snaps.sh <tag> <folder with AdventureMods-<tag>-<arch>.snap>
#
# Needs SNAPCRAFT_STORE_CREDENTIALS. Tests can set SNAPCRAFT (the command to
# run) and SNAP_UPLOAD_TIMEOUT (seconds to wait for each upload).
set -eu

tag=$1
dir=$2
snapcraft=${SNAPCRAFT:-snapcraft}
limit=${SNAP_UPLOAD_TIMEOUT:-1200}

if [ -z "${SNAPCRAFT_STORE_CREDENTIALS:-}" ]; then
	echo "::error::SNAPCRAFT_STORE_CREDENTIALS is required to upload the snaps"
	exit 1
fi

log=$(mktemp)
status_file=$(mktemp)
trap 'rm -f "$log" "$status_file"' EXIT HUP INT TERM

for arch in x86_64 aarch64; do
	# snapcraft polls the store until it has processed the upload, with no
	# time limit. While an earlier revision waits for a human review, the
	# store queues this one behind it and snapcraft would poll for days. The
	# upload is registered before polling starts, so stopping it then loses
	# nothing: the store releases it to stable once the review passes.
	# Verbose output prints each status the store reports, live.
	{
		status=0
		timeout --kill-after=30 "$limit" "$snapcraft" upload --verbosity=verbose \
			--release=stable "$dir/AdventureMods-$tag-$arch.snap" 2>&1 || status=$?
		echo "$status" >"$status_file"
	} | tee "$log"
	status=$(cat "$status_file")

	if [ "$status" -eq 0 ]; then
		continue
	fi
	# Held for review ("will need manual review"), or queued behind an
	# upload that is ("Waiting for previous upload(s) to complete their
	# review process"). Either way the revision is in.
	if grep -q -i -e 'need manual review' -e 'complete their review process' \
		-e 'previous upload' "$log"; then
		echo "::notice::The $arch snap is uploaded and waiting for review in the Snap Store"
	elif [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
		echo "::error::Uploading the $arch snap took over $limit seconds, and the store never said it was waiting for a review"
		exit 1
	else
		exit "$status"
	fi
done
