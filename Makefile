.PHONY: appimage flatpak

# Local Flatpak install (Devel by default). Override for a production build:
#   make flatpak FLATPAK_MANIFEST=build-aux/io.github.astrovm.AdventureMods.json
FLATPAK_BUILD_DIR ?= build
FLATPAK_MANIFEST ?= build-aux/io.github.astrovm.AdventureMods.Devel.json

appimage:
	./build-aux/appimage/build-container.sh

flatpak:
	flatpak-builder --force-clean --user --install-deps-from=flathub --install \
		$(FLATPAK_BUILD_DIR) $(FLATPAK_MANIFEST)
