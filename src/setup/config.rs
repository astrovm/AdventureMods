use std::path::Path;

use anyhow::{Context, Result};
use gtk::gio;
use gtk::prelude::SettingsExt;
use serde::Serialize;

use super::common::ModEntry;
use crate::steam::game::GameKind;

/// Convert a Linux path to a Wine Z: drive path with backslashes.
pub fn linux_to_wine_path(path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    format!("Z:{}\\", resolved.to_string_lossy().replace('/', "\\"))
}

/// Return the game's canonical `system` directory path.
pub fn system_dir(game_path: &Path) -> std::path::PathBuf {
    game_path.join("system")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleLanguage {
    English,
    Japanese,
    French,
    German,
    Spanish,
    Italian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceLanguage {
    English,
    Japanese,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageSelection {
    pub subtitle: SubtitleLanguage,
    pub voice: VoiceLanguage,
}

impl SubtitleLanguage {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "english" => Ok(Self::English),
            "japanese" => Ok(Self::Japanese),
            "french" => Ok(Self::French),
            "german" => Ok(Self::German),
            "spanish" => Ok(Self::Spanish),
            "italian" => Ok(Self::Italian),
            _ => anyhow::bail!("Unknown subtitle language '{value}'"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::English => "english",
            Self::Japanese => "japanese",
            Self::French => "french",
            Self::German => "german",
            Self::Spanish => "spanish",
            Self::Italian => "italian",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Japanese => "日本語",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::Italian => "Italiano",
        }
    }

    pub fn supported_for(game_kind: GameKind) -> &'static [Self] {
        match game_kind {
            GameKind::SADX => &[
                SubtitleLanguage::Japanese,
                SubtitleLanguage::English,
                SubtitleLanguage::French,
                SubtitleLanguage::Spanish,
                SubtitleLanguage::German,
            ],
            GameKind::SA2 => &[
                SubtitleLanguage::English,
                SubtitleLanguage::German,
                SubtitleLanguage::Spanish,
                SubtitleLanguage::French,
                SubtitleLanguage::Italian,
                SubtitleLanguage::Japanese,
            ],
        }
    }
}

impl VoiceLanguage {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "english" => Ok(Self::English),
            "japanese" => Ok(Self::Japanese),
            _ => anyhow::bail!("Unknown voice language '{value}'"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::English => "english",
            Self::Japanese => "japanese",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Japanese => "日本語",
        }
    }

    pub fn all() -> &'static [Self] {
        const ALL: &[VoiceLanguage] = &[VoiceLanguage::Japanese, VoiceLanguage::English];
        ALL
    }
}

impl LanguageSelection {
    pub fn defaults_for(game_kind: GameKind) -> Self {
        match game_kind {
            GameKind::SADX => Self {
                subtitle: SubtitleLanguage::English,
                voice: VoiceLanguage::Japanese,
            },
            GameKind::SA2 => Self {
                subtitle: SubtitleLanguage::English,
                voice: VoiceLanguage::Japanese,
            },
        }
    }
}

pub fn subtitle_settings_key(game_kind: GameKind) -> &'static str {
    match game_kind {
        GameKind::SADX => "sadx-subtitle-language",
        GameKind::SA2 => "sa2-subtitle-language",
    }
}

pub fn voice_settings_key(game_kind: GameKind) -> &'static str {
    match game_kind {
        GameKind::SADX => "sadx-voice-language",
        GameKind::SA2 => "sa2-voice-language",
    }
}

pub fn app_settings() -> Option<gio::Settings> {
    let schema_source = gio::SettingsSchemaSource::default();
    let has_schema = schema_source.is_some_and(|s| s.lookup(crate::config::APP_ID, true).is_some());
    has_schema.then(|| gio::Settings::new(crate::config::APP_ID))
}

pub fn load_language_selection(
    settings: Option<&gio::Settings>,
    game_kind: GameKind,
) -> LanguageSelection {
    let defaults = LanguageSelection::defaults_for(game_kind);
    let Some(settings) = settings else {
        return defaults;
    };

    let subtitle = SubtitleLanguage::parse(&settings.string(subtitle_settings_key(game_kind)))
        .ok()
        .filter(|language| SubtitleLanguage::supported_for(game_kind).contains(language))
        .unwrap_or(defaults.subtitle);
    let voice = VoiceLanguage::parse(&settings.string(voice_settings_key(game_kind)))
        .ok()
        .filter(|language| VoiceLanguage::all().contains(language))
        .unwrap_or(defaults.voice);

    LanguageSelection { subtitle, voice }
}

pub fn save_language_selection(
    settings: Option<&gio::Settings>,
    game_kind: GameKind,
    selection: LanguageSelection,
) {
    let Some(settings) = settings else {
        return;
    };

    let _ = settings.set_string(
        subtitle_settings_key(game_kind),
        selection.subtitle.as_str(),
    );
    let _ = settings.set_string(voice_settings_key(game_kind), selection.voice.as_str());
}

// --- Shared JSON structures for SA Mod Manager config files ---

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ManagerJson {
    pub settings_version: u32,
    pub current_set_game: u32,
    pub theme: u32,
    pub language: u32,
    pub mod_author: String,
    pub enable_developer_mode: bool,
    pub keep_manager_open: bool,
    pub update_settings: UpdateSettings,
    pub game_entries: Vec<GameEntry>,
    #[serde(rename = "managerWidth")]
    pub manager_width: u32,
    #[serde(rename = "managerHeight")]
    pub manager_height: u32,
    pub keep_mod_order: bool,
    pub use_software_rendering: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UpdateSettings {
    pub enable_manager_boot_check: bool,
    pub enable_mods_boot_check: bool,
    pub enable_loader_boot_check: bool,
    pub update_time_out_c_d: u32,
    pub update_check_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GameEntry {
    pub name: String,
    pub directory: String,
    pub executable: String,
    #[serde(rename = "Type")]
    pub game_type: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProfilesJson {
    pub profile_index: u32,
    pub profiles_list: Vec<ProfileEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProfileEntry {
    pub name: String,
    pub filename: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct DebugSettings {
    pub enable_debug_console: bool,
    pub enable_debug_screen: bool,
    pub enable_debug_file: bool,
    pub enable_debug_crash_log: bool,
    pub enable_show_console: Option<bool>,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self {
            enable_debug_console: false,
            enable_debug_screen: false,
            enable_debug_file: false,
            enable_debug_crash_log: true,
            enable_show_console: None,
        }
    }
}

// --- Shared file writers ---

pub fn write_manager_json(game_path: &Path, game_kind: GameKind) -> Result<()> {
    let wine_path = linux_to_wine_path(game_path);
    let manager = ManagerJson {
        settings_version: 3,
        current_set_game: 0,
        theme: 0,
        language: 0,
        mod_author: String::new(),
        enable_developer_mode: false,
        keep_manager_open: true,
        update_settings: UpdateSettings {
            enable_manager_boot_check: true,
            enable_mods_boot_check: true,
            enable_loader_boot_check: true,
            update_time_out_c_d: 0,
            update_check_count: 0,
        },
        game_entries: vec![GameEntry {
            name: game_kind.name().to_string(),
            directory: wine_path,
            executable: game_kind.game_executable().to_string(),
            game_type: game_kind.manager_game_type(),
        }],
        manager_width: 590,
        manager_height: 600,
        keep_mod_order: false,
        use_software_rendering: true,
    };

    let dir = game_path.join("SAManager");
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(&manager)?;
    std::fs::write(dir.join("Manager.json"), json).context("Failed to write Manager.json")
}

pub fn write_profiles_json(game_path: &Path, rel_dir: &str) -> Result<()> {
    let profiles = ProfilesJson {
        profile_index: 0,
        profiles_list: vec![ProfileEntry {
            name: "Default".to_string(),
            filename: "Default.json".to_string(),
        }],
    };

    let dir = game_path.join(rel_dir);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(&profiles)?;
    std::fs::write(dir.join("Profiles.json"), json).context("Failed to write Profiles.json")
}

pub fn write_default_json<T: Serialize>(
    game_path: &Path,
    profile: &T,
    rel_dir: &str,
) -> Result<()> {
    let dir = game_path.join(rel_dir);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(profile)?;
    std::fs::write(dir.join("Default.json"), json).context("Failed to write Default.json")
}

pub fn write_samanager_txt(game_path: &Path) -> Result<()> {
    let wine_path = linux_to_wine_path(game_path);
    let dir = game_path.join("mods/.modloader");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("samanager.txt"), format!("{}\n", wine_path))
        .context("Failed to write samanager.txt")
}

/// Collect mod directory names from selected mods.
pub fn mod_dir_names(selected_mods: &[&ModEntry]) -> Vec<String> {
    selected_mods
        .iter()
        .filter_map(|m| m.dir_name.map(|d| d.to_string()))
        .collect()
}

/// Build a patches map from a list of (name, enabled) pairs.
pub fn build_patches(patches: &[(&str, bool)]) -> std::collections::BTreeMap<String, bool> {
    patches
        .iter()
        .map(|&(name, enabled)| (name.to_string(), enabled))
        .collect()
}

/// Generate config files for a game, dispatching to the game-specific generator.
pub fn generate_config(
    game_path: &Path,
    game_kind: GameKind,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
    language_selection: LanguageSelection,
) -> Result<()> {
    match game_kind {
        GameKind::SADX => super::sadx_config::generate_sadx_config(
            game_path,
            selected_mods,
            width,
            height,
            language_selection,
        ),
        GameKind::SA2 => super::sa2_config::generate_sa2_config(
            game_path,
            selected_mods,
            width,
            height,
            language_selection,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_to_wine_path() {
        let path = Path::new("/home/user/.steam/steamapps/common/Sonic Adventure DX");
        assert_eq!(
            linux_to_wine_path(path),
            "Z:\\home\\user\\.steam\\steamapps\\common\\Sonic Adventure DX\\"
        );
    }

    #[test]
    fn test_system_dir_always_uses_lowercase() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();
        assert_eq!(system_dir(tmp.path()), tmp.path().join("system"));
    }

    #[test]
    fn test_write_manager_json_sadx() {
        let tmp = tempfile::tempdir().unwrap();
        write_manager_json(tmp.path(), GameKind::SADX).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["GameEntries"][0]["Name"], "Sonic Adventure DX");
        assert_eq!(parsed["GameEntries"][0]["Executable"], "sonic.exe");
        assert_eq!(parsed["GameEntries"][0]["Type"], 1);
    }

    #[test]
    fn test_write_manager_json_sa2() {
        let tmp = tempfile::tempdir().unwrap();
        write_manager_json(tmp.path(), GameKind::SA2).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["GameEntries"][0]["Name"], "Sonic Adventure 2");
        assert_eq!(parsed["GameEntries"][0]["Executable"], "sonic2app.exe");
        assert_eq!(parsed["GameEntries"][0]["Type"], 2);
    }

    #[test]
    fn test_write_profiles_json() {
        let tmp = tempfile::tempdir().unwrap();
        write_profiles_json(tmp.path(), "mods/.modloader/profiles").unwrap();

        let path = tmp.path().join("mods/.modloader/profiles/Profiles.json");
        let content = std::fs::read_to_string(path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["ProfileIndex"], 0);
        assert_eq!(parsed["ProfilesList"][0]["Name"], "Default");
    }

    #[test]
    fn test_write_samanager_txt() {
        let tmp = tempfile::tempdir().unwrap();
        write_samanager_txt(tmp.path()).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/samanager.txt")).unwrap();
        assert!(content.starts_with("Z:\\"));
        assert!(content.ends_with("\\\n"));
    }

    #[test]
    fn test_mod_dir_names() {
        use crate::setup::common::{ModEntry, ModSource};
        let mods = [
            ModEntry {
                name: "A",
                source: ModSource::DirectUrl { url: "https://a" },
                description: "",
                full_description: None,
                pictures: &[],
                dir_name: Some("DirA"),
                links: &[],
            },
            ModEntry {
                name: "B",
                source: ModSource::DirectUrl { url: "https://b" },
                description: "",
                full_description: None,
                pictures: &[],
                dir_name: None,
                links: &[],
            },
        ];
        let refs: Vec<&ModEntry> = mods.iter().collect();
        let dirs = mod_dir_names(&refs);
        assert_eq!(dirs, vec!["DirA"]);
    }

    #[test]
    fn test_manager_json_field_names() {
        let tmp = tempfile::tempdir().unwrap();
        write_manager_json(tmp.path(), GameKind::SADX).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();
        assert!(
            content.contains("\"UpdateTimeOutCD\""),
            "Should be UpdateTimeOutCD"
        );
    }

    #[test]
    fn subtitle_language_parses_known_values() {
        assert_eq!(
            SubtitleLanguage::parse("english").unwrap(),
            SubtitleLanguage::English
        );
        assert_eq!(
            SubtitleLanguage::parse("japanese").unwrap(),
            SubtitleLanguage::Japanese
        );
        assert_eq!(
            SubtitleLanguage::parse("italian").unwrap(),
            SubtitleLanguage::Italian
        );
    }

    #[test]
    fn voice_language_rejects_unknown_value() {
        assert!(VoiceLanguage::parse("klingon").is_err());
    }

    #[test]
    fn sadx_defaults_match_existing_behavior() {
        let defaults = LanguageSelection::defaults_for(GameKind::SADX);
        assert_eq!(defaults.subtitle, SubtitleLanguage::English);
        assert_eq!(defaults.voice, VoiceLanguage::Japanese);
    }

    #[test]
    fn sa2_defaults_match_existing_behavior() {
        let defaults = LanguageSelection::defaults_for(GameKind::SA2);
        assert_eq!(defaults.subtitle, SubtitleLanguage::English);
        assert_eq!(defaults.voice, VoiceLanguage::Japanese);
    }

    #[test]
    fn settings_keys_match_game_language_preferences() {
        assert_eq!(
            subtitle_settings_key(GameKind::SADX),
            "sadx-subtitle-language"
        );
        assert_eq!(voice_settings_key(GameKind::SADX), "sadx-voice-language");
        assert_eq!(
            subtitle_settings_key(GameKind::SA2),
            "sa2-subtitle-language"
        );
        assert_eq!(voice_settings_key(GameKind::SA2), "sa2-voice-language");
    }
}
