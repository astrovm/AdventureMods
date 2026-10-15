#!/bin/sh
# Extract the app's text into po/adventure-mods.pot, then merge it into every
# catalog in po/LINGUAS. Needs GNU gettext 0.24 or newer, which reads Rust.
set -eu

cd "$(dirname "$0")/.."
pot=po/adventure-mods.pot

# Every source file that marks text, tests left out.
sources=$(grep -rlE --include='*.rs' '(^|[^a-z_])(tr|trc|trf|trn|tr_noop)!\(' src |
	grep -v -e '_tests\.rs$' -e '^src/i18n\.rs$' | LC_ALL=C sort)

# Each run writes the header, so each one names the package.
package="--package-name=adventure-mods --msgid-bugs-address=https://github.com/astrovm/AdventureMods/issues"

xgettext $package --language=Rust --from-code=UTF-8 \
	--add-comments=Translators: --add-location=file --no-wrap \
	-k -k'tr!' -k'trc!:1c,2' -k'trf!' -k'trn!:1,2' -k'tr_noop!' \
	-o "$pot" $sources
# The text has arrows and ellipses, so the template must say it is UTF-8
# before more files join it.
sed -i.bak 's/charset=CHARSET/charset=UTF-8/' "$pot" && rm "$pot.bak"
xgettext $package --join-existing --from-code=UTF-8 --language=Desktop -k -kComment -kKeywords \
	--add-location=file --no-wrap -o "$pot" data/io.github.astrovm.AdventureMods.desktop.in
xgettext $package --join-existing --from-code=UTF-8 --add-location=file --no-wrap -o "$pot" \
	data/io.github.astrovm.AdventureMods.metainfo.xml.in

# The date changes on every run; leave it out so the template only changes
# when the text does.
sed -i.bak '/^"POT-Creation-Date: /d' "$pot" && rm "$pot.bak"

# Text that left the code leaves the catalogs too.
while read -r lang; do
	msgmerge --quiet --update --backup=none --no-fuzzy-matching \
		--add-location=file --no-wrap "po/$lang.po" "$pot"
	msgattrib --no-obsolete --add-location=file --no-wrap --output-file="po/$lang.po" "po/$lang.po"
done < po/LINGUAS
