#!/usr/bin/env bash
# Run the AppImage catalog's own test on a local AppImage: code/worker.sh from
# AppImage/appimage.github.io, set up as its .github/workflows/test.yml does.
# It runs the app in firejail, which loop-mounts the AppImage as root, so it
# needs a real host with sudo, such as a GitHub ubuntu-22.04 runner (the
# catalog uses one). A rootless container is not enough; for a quick local
# check, use test-window.sh.
#
# It also changes the host as root, as the catalog's runner does: it installs
# packages and copies firejail into /lib, /usr and /etc. So it runs only on
# GitHub Actions, or with ALLOW_HOST_CHANGES=1 on a throwaway machine.
#
# Usage: catalog-test.sh <AppImage> <catalog entry name>
# It writes log.txt, findings.txt and the screenshot to $CATALOG_RESULT
# (default: appimage-catalog-result). It fails when the catalog test fails,
# makes no screenshot, or finds a warning or error that is not in $known.
set -euo pipefail

# The catalog changes its test often. Update this commit on purpose.
CATALOG_COMMIT="ba37bcf481d0e8429b4a5ef67d095c279dac7aa2"

# The findings this app is known to have. Any other warning or error fails,
# so that a fixed warning, such as the icon format, cannot come back.
# - No appdata file present: the catalog looks only for *appdata.xml, and its
#   old appstreamcli fails on the current file under that name.
# - The window is mostly dark: the app's blue design. The catalog ignores it
#   for its screenshot-ok label.
known='^WARNING: (No appdata file present|The window is mostly dark)'

if [ ! -f "${1:-}" ] || [ -z "${2:-}" ]; then
	echo "Usage: $0 <AppImage> <catalog entry name>" >&2
	exit 2
fi
if [ "${GITHUB_ACTIONS:-}" != true ] && [ "${ALLOW_HOST_CHANGES:-}" != 1 ]; then
	echo "This test changes the host as root. Run it on GitHub Actions, or set ALLOW_HOST_CHANGES=1 on a throwaway machine." >&2
	exit 2
fi
appimage="$(readlink -f "$1")"
name="$2"
result="$(readlink -f "${CATALOG_RESULT:-appimage-catalog-result}")"
work="$(mktemp -d)"
serve="$work/serve"
catalog="$work/catalog"
rm -rf "$result"
mkdir -p "$result" "$serve"

pids=()
cleanup() {
	if [ "${#pids[@]}" -gt 0 ]; then
		kill "${pids[@]}" 2>/dev/null || true
	fi
	# worker.sh extracts firejail into the work folder as root.
	sudo rm -rf "$work"
}
trap cleanup EXIT

git clone -q --filter=blob:none https://github.com/AppImage/appimage.github.io "$catalog"
git -C "$catalog" checkout -q "$CATALOG_COMMIT"
cd "$catalog"

echo "==> Installing the catalog's test packages"
packages="$(sed -n 's/^ *packages: *//p' .github/workflows/test.yml)"
if [ -z "$packages" ]; then
	echo "Found no package list in the catalog's test.yml" >&2
	exit 1
fi
sudo apt-get update -qq
# shellcheck disable=SC2086 # The list is space-separated.
sudo apt-get install -y -qq --no-install-recommends $packages >/dev/null
# worker.sh converts the desktop file with dv (dupervisor), as the catalog
# workflow installs it. It skips asar, which only Electron apps need.
sudo gem install dupervisor -v 1.0.5 >/dev/null
bash code/fetch-deps.sh >/dev/null
cp -a deps/. .
# The catalog workflow stops when this fails, so this test stops too.
sudo bash code/prep-dummy-soundcard.sh >/dev/null

# The catalog downloads the AppImage from the link in data/<name>. Serve this
# one under its own file name, so that the name checks see the real name.
cp "$appimage" "$serve/"
# GitHub runners have Node.js.
node -e '
const http = require("http"), fs = require("fs"), path = require("path");
const dir = process.argv[1];
http.createServer((request, response) => {
    const file = path.join(dir, path.basename(decodeURIComponent(request.url.split("?")[0])));
    fs.stat(file, (error, stat) => {
        if (error || !stat.isFile()) { response.writeHead(404); return response.end(); }
        response.writeHead(200, { "Content-Length": stat.size });
        if (request.method === "HEAD") return response.end();
        fs.createReadStream(file).pipe(response);
    });
}).listen(8765, "127.0.0.1");
' "$serve" &
pids+=($!)
url="http://127.0.0.1:8765/$(basename "$appimage")"
for _ in $(seq 20); do
	curl -fsI "$url" >/dev/null 2>&1 && break
	sleep 0.5
done
echo "$url" >"data/$name"

mkdir -p "$HOME/.icewm"
cat >"$HOME/.icewm/preferences" <<'EOF'
ShowTaskBar = 0
TaskBarAutoHide = 1
TaskBarShowWorkspaces = 0
TaskBarShowAllWindows = 0
TaskBarShowClock = 0
TaskBarShowMailboxStatus = 0
TaskBarShowCPUStatus = 0
TaskBarShowWindowListMenu = 0
EOF
Xvfb :99 -screen 0 800x600x24 >/dev/null 2>&1 &
pids+=($!)
export DISPLAY=:99 IS_PULLREQUEST=true STRICT=true

echo "==> Running the catalog test"
set +e
timeout --kill-after=30 600 bash -e code/worker.sh "$catalog/data/$name" >log.txt 2>&1
status=$?
set -e
cp log.txt "$result/"
cp "database/$name/screenshot.png" "$result/" 2>/dev/null || true
cp failure-screens/*.png "$result/" 2>/dev/null || true
grep -hE '^(ERROR|WARNING|NOTE): ' log.txt | sort -u >"$result/findings.txt" || true
bash code/diagnose.sh log.txt >"$result/hints.md" 2>/dev/null || true

echo "==> Findings"
cat "$result/findings.txt"
if [ "$status" -ne 0 ]; then
	echo "The catalog test failed (exit $status). See $result/log.txt." >&2
	tail -n 40 log.txt >&2
	exit 1
fi
if [ ! -f "$result/screenshot.png" ]; then
	echo "The catalog test made no screenshot." >&2
	exit 1
fi
unknown="$(grep -E '^(WARNING|ERROR): ' "$result/findings.txt" | grep -Ev "$known" || true)"
if [ -n "$unknown" ]; then
	echo "The catalog test found new warnings or errors:" >&2
	echo "$unknown" >&2
	exit 1
fi
echo "The catalog test passed."
