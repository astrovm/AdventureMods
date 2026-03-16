use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::common::ModEntry;

/// Convert a Linux path to a Wine Z: drive path with backslashes.
fn linux_to_wine_path(path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    format!("Z:{}", resolved.to_string_lossy().replace('/', "\\"))
}

/// Map from mod name to the folder name the GameBanana archive creates.
/// SA2 mods have `dir_name: None`, so we need this lookup.
fn mod_folder_name(mod_name: &str) -> &str {
    match mod_name {
        "SA2 Render Fix" => "sa2-render-fix",
        "Retranslated Story -COMPLETE-" => "Retranslated Story -COMPLETE-",
        "HD GUI: SA2 Edition" => "HD GUI for SA2",
        "IMPRESSive" => "IMPRESSive",
        "Stage Atmosphere Tweaks" => "StageAtmosphereTweaks",
        "SA2 Volume Controls" => "SA2VolumeControls",
        "Mech Sound Improvement" => "Mech Sound Improvement",
        "SASDL" => "SASDL",
        "SA2 Input Controls" => "sa2-input-controls",
        "Better Radar" => "SA2BetterRadar",
        "HedgePanel - Sonic + Shadow Tweaks" => "HedgePanel",
        "Sonic: New Tricks" => "Sonic New Tricks",
        "Retranslated Hints" => "Retranslated Hints",
        _ => mod_name,
    }
}

/// Generate all SA Mod Manager v3 configuration files for SA2.
pub fn generate_sa2_config(game_path: &Path, selected_mods: &[&ModEntry]) -> Result<()> {
    let profile = build_default_profile(game_path, selected_mods);

    write_manager_json(game_path)?;
    write_profiles_json(game_path, "mods/.modloader/profiles")?;
    write_default_json(game_path, &profile, "mods/.modloader/profiles")?;
    write_samanager_txt(game_path)?;
    write_user_config(game_path)?;

    tracing::info!("SA2 configuration files generated");
    Ok(())
}

// --- JSON structures ---

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ManagerJson {
    settings_version: u32,
    current_set_game: u32,
    theme: u32,
    language: u32,
    mod_author: String,
    enable_developer_mode: bool,
    keep_manager_open: bool,
    update_settings: UpdateSettings,
    game_entries: Vec<GameEntry>,
    keep_mod_order: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct UpdateSettings {
    enable_manager_boot_check: bool,
    enable_mods_boot_check: bool,
    enable_loader_boot_check: bool,
    update_time_out_c_d: u32,
    update_check_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct GameEntry {
    name: String,
    directory: String,
    executable: String,
    #[serde(rename = "Type")]
    game_type: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ProfilesJson {
    profile_index: u32,
    profiles_list: Vec<ProfileEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ProfileEntry {
    name: String,
    filename: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct DefaultProfile {
    settings_version: u32,
    graphics: Graphics,
    test_spawn: TestSpawn,
    patches: BTreeMap<String, bool>,
    debug_settings: DebugSettings,
    game_path: String,
    enabled_mods: Vec<String>,
    enabled_codes: Vec<String>,
    mods_list: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Graphics {
    selected_screen: u32,
    horizontal_resolution: u32,
    vertical_resolution: u32,
    enable_pause_on_inactive: bool,
    custom_window_width: u32,
    custom_window_height: u32,
    enable_keep_resolution_ratio: bool,
    enable_resizable_window: bool,
    screen_mode: u32,
    stretch_to_window: bool,
    game_text_language: u32,
    skip_intro: bool,
    refresh_rate: u32,
    disable_border_image: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct TestSpawn {
    use_character: bool,
    use_player2: bool,
    use_level: bool,
    use_event: bool,
    use_save: bool,
    level_index: i32,
    mission_index: u32,
    character_index: i32,
    player2_index: i32,
    event_index: i32,
    save_index: i32,
    game_text_language: u32,
    game_voice_language: u32,
    use_manual: bool,
    use_event_manual: bool,
    use_position: bool,
    #[serde(rename = "XPosition")]
    x_position: u32,
    #[serde(rename = "YPosition")]
    y_position: u32,
    #[serde(rename = "ZPosition")]
    z_position: u32,
    rotation: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct DebugSettings {
    enable_debug_console: bool,
    enable_debug_screen: bool,
    enable_debug_file: bool,
    enable_debug_crash_log: bool,
    enable_show_console: Option<bool>,
}

// --- Builders ---

fn build_default_profile(game_path: &Path, selected_mods: &[&ModEntry]) -> DefaultProfile {
    let mod_dirs: Vec<String> = selected_mods
        .iter()
        .map(|m| mod_folder_name(m.name).to_string())
        .collect();

    let mut patches = BTreeMap::new();
    for &(name, enabled) in RECOMMENDED_PATCHES {
        patches.insert(name.to_string(), enabled);
    }

    DefaultProfile {
        settings_version: 3,
        graphics: Graphics {
            selected_screen: 1,
            horizontal_resolution: 1920,
            vertical_resolution: 1080,
            enable_pause_on_inactive: true,
            custom_window_width: 640,
            custom_window_height: 480,
            enable_keep_resolution_ratio: false,
            enable_resizable_window: false,
            screen_mode: 1,
            stretch_to_window: false,
            game_text_language: 0,
            skip_intro: false,
            refresh_rate: 60,
            disable_border_image: false,
        },
        test_spawn: TestSpawn {
            use_character: false,
            use_player2: false,
            use_level: false,
            use_event: false,
            use_save: false,
            level_index: -1,
            mission_index: 0,
            character_index: -1,
            player2_index: -1,
            event_index: -1,
            save_index: -1,
            game_text_language: 0,
            game_voice_language: 1,
            use_manual: false,
            use_event_manual: false,
            use_position: false,
            x_position: 0,
            y_position: 0,
            z_position: 0,
            rotation: 0,
        },
        patches,
        debug_settings: DebugSettings {
            enable_debug_console: false,
            enable_debug_screen: false,
            enable_debug_file: false,
            enable_debug_crash_log: true,
            enable_show_console: None,
        },
        game_path: linux_to_wine_path(game_path),
        enabled_mods: mod_dirs.clone(),
        enabled_codes: Vec::new(),
        mods_list: mod_dirs,
    }
}

/// Recommended SA2 patches: (name, enabled by default).
const RECOMMENDED_PATCHES: &[(&str, bool)] = &[
    ("FramerateLimiter", true),
    ("DisableExitPrompt", true),
    ("SyncLoad", true),
    ("ExtendVertexBuffer", true),
    ("EnvMapFix", true),
    ("ScreenFadeFix", true),
    ("CECarFix", true),
    ("ParticlesFix", true),
];

// --- File writers ---

fn write_manager_json(game_path: &Path) -> Result<()> {
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
            name: "Sonic Adventure 2".to_string(),
            directory: format!("{}\\", wine_path),
            executable: "sonic2app.exe".to_string(),
            game_type: 2,
        }],
        keep_mod_order: false,
    };

    let dir = game_path.join("SAManager");
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(&manager)?;
    std::fs::write(dir.join("Manager.json"), json).context("Failed to write Manager.json")
}

fn write_profiles_json(game_path: &Path, rel_dir: &str) -> Result<()> {
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

fn write_default_json(game_path: &Path, profile: &DefaultProfile, rel_dir: &str) -> Result<()> {
    let dir = game_path.join(rel_dir);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(profile)?;
    std::fs::write(dir.join("Default.json"), json).context("Failed to write Default.json")
}

fn write_samanager_txt(game_path: &Path) -> Result<()> {
    let wine_path = linux_to_wine_path(game_path);
    let dir = game_path.join("mods/.modloader");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("samanager.txt"), format!("{}\\\n", wine_path))
        .context("Failed to write samanager.txt")
}

fn write_user_config(game_path: &Path) -> Result<()> {
    let dir = game_path.join("Config");
    std::fs::create_dir_all(&dir)?;

    let xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
               <Configs xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
               xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
               FullScreen=\"0\" Display=\"0\" Res=\"0\" \
               Width=\"1920\" Height=\"1080\" RefreshRate=\"60\" \
               Language=\"0\" Analytics=\"0\" />\n";

    std::fs::write(dir.join("UserConfig.cfg"), xml).context("Failed to write UserConfig.cfg")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::common::{ModEntry, ModSource};

    fn test_mods() -> Vec<ModEntry> {
        vec![
            ModEntry {
                name: "SA2 Render Fix",
                source: ModSource::GameBanana { file_id: 1626250 },
                description: "Test",
                before_image: None,
                after_image: None,
                dir_name: None,
            },
            ModEntry {
                name: "Better Radar",
                source: ModSource::GameBanana { file_id: 1580535 },
                description: "Test",
                before_image: None,
                after_image: None,
                dir_name: None,
            },
        ]
    }

    #[test]
    fn test_mod_folder_name_mapping() {
        assert_eq!(mod_folder_name("SA2 Render Fix"), "sa2-render-fix");
        assert_eq!(mod_folder_name("HD GUI: SA2 Edition"), "HD GUI for SA2");
        assert_eq!(mod_folder_name("Better Radar"), "SA2BetterRadar");
        assert_eq!(mod_folder_name("HedgePanel - Sonic + Shadow Tweaks"), "HedgePanel");
        assert_eq!(mod_folder_name("Sonic: New Tricks"), "Sonic New Tricks");
        assert_eq!(mod_folder_name("SA2 Volume Controls"), "SA2VolumeControls");
        assert_eq!(mod_folder_name("SA2 Input Controls"), "sa2-input-controls");
        assert_eq!(mod_folder_name("Stage Atmosphere Tweaks"), "StageAtmosphereTweaks");
    }

    #[test]
    fn test_generate_creates_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let game_path = tmp.path();

        let mods = test_mods();
        let mod_refs: Vec<&ModEntry> = mods.iter().collect();
        generate_sa2_config(game_path, &mod_refs).unwrap();

        assert!(game_path.join("SAManager/Manager.json").is_file());
        assert!(game_path.join("mods/.modloader/profiles/Profiles.json").is_file());
        assert!(game_path.join("mods/.modloader/profiles/Default.json").is_file());
        assert!(game_path.join("mods/.modloader/samanager.txt").is_file());
        assert!(game_path.join("Config/UserConfig.cfg").is_file());
    }

    #[test]
    fn test_manager_json_format() {
        let tmp = tempfile::tempdir().unwrap();
        write_manager_json(tmp.path()).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["SettingsVersion"], 3);
        assert_eq!(parsed["GameEntries"][0]["Name"], "Sonic Adventure 2");
        assert_eq!(parsed["GameEntries"][0]["Executable"], "sonic2app.exe");
        assert_eq!(parsed["GameEntries"][0]["Type"], 2);
    }

    #[test]
    fn test_default_profile_mods_and_patches() {
        let tmp = tempfile::tempdir().unwrap();

        let mods = test_mods();
        let mod_refs: Vec<&ModEntry> = mods.iter().collect();
        generate_sa2_config(tmp.path(), &mod_refs).unwrap();

        let content = std::fs::read_to_string(
            tmp.path().join("mods/.modloader/profiles/Default.json"),
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["SettingsVersion"], 3);

        let enabled = parsed["EnabledMods"].as_array().unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0], "sa2-render-fix");
        assert_eq!(enabled[1], "SA2BetterRadar");

        let mods_list = parsed["ModsList"].as_array().unwrap();
        assert_eq!(enabled, mods_list);

        let patches = parsed["Patches"].as_object().unwrap();
        assert_eq!(patches["FramerateLimiter"], true);
        assert_eq!(patches["CECarFix"], true);
        assert_eq!(patches.len(), RECOMMENDED_PATCHES.len());
    }

    #[test]
    fn test_no_sadx_specific_fields() {
        let tmp = tempfile::tempdir().unwrap();
        generate_sa2_config(tmp.path(), &[]).unwrap();

        let content = std::fs::read_to_string(
            tmp.path().join("mods/.modloader/profiles/Default.json"),
        )
        .unwrap();

        // SA2 profile should NOT have SADX-specific sections
        assert!(!content.contains("\"Controller\""));
        assert!(!content.contains("\"Sound\""));
        assert!(!content.contains("\"EnableVsync\""));
        assert!(!content.contains("\"RenderBackend\""));
        // Should have SA2-specific fields
        assert!(content.contains("\"SkipIntro\""));
        assert!(content.contains("\"RefreshRate\""));
    }

    #[test]
    fn test_user_config_xml() {
        let tmp = tempfile::tempdir().unwrap();
        write_user_config(tmp.path()).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("Config/UserConfig.cfg")).unwrap();
        assert!(content.contains("<?xml version"));
        assert!(content.contains("FullScreen=\"0\""));
        assert!(content.contains("Width=\"1920\""));
        assert!(content.contains("Height=\"1080\""));
        assert!(content.contains("Language=\"0\""));
    }

    #[test]
    fn test_samanager_txt_format() {
        let tmp = tempfile::tempdir().unwrap();
        write_samanager_txt(tmp.path()).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/samanager.txt")).unwrap();
        assert!(content.starts_with("Z:\\"));
        assert!(content.ends_with("\\\n"));
    }

    #[test]
    fn test_empty_mod_selection() {
        let tmp = tempfile::tempdir().unwrap();
        generate_sa2_config(tmp.path(), &[]).unwrap();

        let content = std::fs::read_to_string(
            tmp.path().join("mods/.modloader/profiles/Default.json"),
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["EnabledMods"].as_array().unwrap().is_empty());
        assert!(!parsed["Patches"].as_object().unwrap().is_empty());
    }
}
