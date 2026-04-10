/// Source for downloading a mod.
pub enum ModSource {
    /// Look up the latest file for a GameBanana item via the Core API and download it.
    GameBananaItem {
        item_type: &'static str,
        item_id: u32,
    },
    DirectUrl {
        url: &'static str,
    },
}

/// An external link for a mod (e.g. GitHub repo, GameBanana page).
pub struct ModLink {
    pub label: &'static str,
    pub url: &'static str,
}

/// A recommended mod entry.
pub struct ModEntry {
    pub name: &'static str,
    pub source: ModSource,
    pub description: &'static str,
    pub full_description: Option<&'static str>,
    pub pictures: &'static [&'static str],
    /// Expected directory name inside `mods/`. Used when a flat archive
    /// (no top-level subdirectory) needs to be wrapped in the correct folder.
    pub dir_name: Option<&'static str>,
    /// External links for this mod (project pages, source repos, etc.).
    pub links: &'static [ModLink],
}

/// A preset for selecting a group of mods.
pub struct ModPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub mod_names: &'static [&'static str],
}
