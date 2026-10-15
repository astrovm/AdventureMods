#!/bin/sh
# List every file of an untouched Steam install, so tests and questions about
# the game don't need the game. Prints one line per file: path, size, SHA-1
# and the first 8 bytes in hex, tab separated, sorted by path.
#
#   sh build-aux/steam-manifest.sh "<library>/steamapps" 71250 > tests/fixtures/steam/sadx.tsv
#   sh build-aux/steam-manifest.sh "<library>/steamapps" 213610 > tests/fixtures/steam/sa2.tsv
#
# Run it right after a fresh install, before any setup touches the files.
set -eu

[ $# -eq 2 ] || {
	echo "usage: $0 STEAMAPPS_DIR APP_ID" >&2
	exit 2
}
steamapps=$1
app=$2
acf="$steamapps/appmanifest_$app.acf"
[ -f "$acf" ] || {
	echo "$acf not found" >&2
	exit 1
}

field() {
	sed -n "s/^[[:space:]]*\"$1\"[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$acf" | head -n 1
}
name=$(field name | sed 's/[[:space:]]*$//')
dir="$steamapps/common/$(field installdir)"

echo "# $name, Steam app $app, build $(field buildid), $(field SizeOnDisk) bytes"
echo "# path	size	sha1	first 8 bytes"
cd "$dir"
find . -type f | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r file; do
	printf '%s\t%s\t%s\t%s\n' "$file" "$(wc -c <"$file" | tr -d ' ')" \
		"$(sha1sum <"$file" | cut -c1-40)" \
		"$(head -c 8 "$file" | od -An -tx1 | tr -d ' \n')"
done
