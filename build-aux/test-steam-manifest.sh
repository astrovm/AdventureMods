#!/bin/sh
set -eu

root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT HUP INT TERM
steamapps="$root/steamapps"
game="$steamapps/common/Fake Game"
mkdir -p "$game/Sound Data/b"
cat >"$steamapps/appmanifest_42.acf" <<'ACF'
"AppState"
{
	"appid"		"42"
	"name"		"Fake Game "
	"installdir"		"Fake Game"
	"SizeOnDisk"		"14"
	"buildid"		"7"
}
ACF
printf 'MZ\220\000\003\000\000\000tail' >"$game/Game.exe"
printf 'ab' >"$game/Sound Data/b/a.adx"
: >"$game/empty"

expected=$(printf '%s\n' \
	'# Fake Game, Steam app 42, build 7, 14 bytes' \
	'# path	size	sha1	first 8 bytes' \
	"Game.exe	12	$(sha1sum <"$game/Game.exe" | cut -c1-40)	4d5a900003000000" \
	'Sound Data/b/a.adx	2	da23614e02469a0d7c7bd1bdab5c9c474b1904dc	6162' \
	'empty	0	da39a3ee5e6b4b0d3255bfef95601890afd80709	')
test "$(sh "$(dirname "$0")/steam-manifest.sh" "$steamapps" 42)" = "$expected"

if sh "$(dirname "$0")/steam-manifest.sh" "$steamapps" 43 2>/dev/null; then
	echo "a missing app should fail" >&2
	exit 1
fi
