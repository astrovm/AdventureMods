// Build-time constants, overridden by meson configure_file in production builds.
// These defaults allow `cargo build` to work outside meson.
pub const APP_ID: &str = "io.github.astrovm.AdventureMods.Devel";
pub const VERSION: &str = "0.1.0-devel";
#[allow(dead_code)]
pub const GETTEXT_PACKAGE: &str = "adventure-mods";
#[allow(dead_code)]
pub const LOCALEDIR: &str = "/usr/share/locale";
pub const PKGDATADIR: &str = "/usr/share/adventure-mods";
pub const PROFILE: &str = "development";
