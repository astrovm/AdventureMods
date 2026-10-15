#!/bin/sh
# Subset Noto Sans JP to the Japanese text of the app, so the fonts stay
# small. Run it again after po/ja.po changes. Needs uv, which runs fonttools.
#
#   sh build-aux/subset-japanese-fonts.sh NotoSansJP-Medium.otf NotoSansJP-Bold.otf
#
# Both fonts come from Sans/SubsetOTF/JP in https://github.com/notofonts/noto-cjk.
set -eu

[ $# -eq 2 ] || {
	echo "usage: $0 NotoSansJP-Medium.otf NotoSansJP-Bold.otf" >&2
	exit 2
}
medium=$(realpath "$1")
bold=$(realpath "$2")
cd "$(dirname "$0")/.."

# Every character of the Japanese catalog that Rubik lacks, all kana and
# full-width forms so new text rarely needs a new subset, and "日本語", the
# label of the Japanese option in every language.
text=$(mktemp)
trap 'rm -f "$text"' EXIT
{
	grep -E '^(msgstr|")' po/ja.po
	printf '日本語'
} | LC_ALL=C.UTF-8 perl -CSD -ne 'print grep { ord >= 0x2000 } split //' >"$text"
unicodes="U+3000-30FF,U+FF00-FFEF"

for weight in Medium Bold; do
	if [ "$weight" = Medium ]; then source=$medium; else source=$bold; fi
	uvx --from fonttools pyftsubset "$source" \
		--text-file="$text" --unicodes="$unicodes" \
		--layout-features='*' --no-hinting --desubroutinize \
		--output-file="data/fonts/NotoSansJP-$weight-Subset.otf"
done
