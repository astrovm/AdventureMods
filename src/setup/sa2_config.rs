use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::common::ModEntry;
use super::{config, sa2};
use crate::steam::game::GameKind;

/// Generate all SA Mod Manager v3 configuration files for SA2.
pub fn generate_sa2_config(
    game_path: &Path,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
    language_selection: config::LanguageSelection,
) -> Result<()> {
    let profile =
        build_default_profile(game_path, selected_mods, width, height, language_selection);

    config::write_manager_json(game_path, GameKind::SA2)?;
    config::write_profiles_json(game_path, "mods/.modloader/profiles")?;
    config::write_default_json(game_path, &profile, "mods/.modloader/profiles")?;
    config::write_samanager_txt(game_path)?;
    write_user_config(game_path, width, height, language_selection)?;

    tracing::info!("SA2 configuration files generated");
    Ok(())
}

// --- SA2-specific profile structures ---

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct DefaultProfile {
    settings_version: u32,
    graphics: Graphics,
    test_spawn: TestSpawn,
    patches: BTreeMap<String, bool>,
    debug_settings: config::DebugSettings,
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

// --- Builders ---

fn build_default_profile(
    game_path: &Path,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
    language_selection: config::LanguageSelection,
) -> DefaultProfile {
    let enabled_mods = config::mod_dir_names(selected_mods);
    let all_recommended: Vec<&ModEntry> = sa2::RECOMMENDED_MODS.iter().collect();
    let mods_list = config::mod_dir_names(&all_recommended);

    DefaultProfile {
        settings_version: 3,
        graphics: Graphics {
            selected_screen: 1,
            horizontal_resolution: width,
            vertical_resolution: height,
            enable_pause_on_inactive: true,
            custom_window_width: 640,
            custom_window_height: 480,
            enable_keep_resolution_ratio: false,
            enable_resizable_window: false,
            screen_mode: 1,
            stretch_to_window: false,
            game_text_language: subtitle_code(language_selection.subtitle),
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
            game_text_language: subtitle_code(language_selection.subtitle),
            game_voice_language: voice_code(language_selection.voice),
            use_manual: false,
            use_event_manual: false,
            use_position: false,
            x_position: 0,
            y_position: 0,
            z_position: 0,
            rotation: 0,
        },
        patches: config::build_patches(RECOMMENDED_PATCHES),
        debug_settings: config::DebugSettings::default(),
        game_path: config::linux_to_wine_path(game_path),
        enabled_mods,
        enabled_codes: Vec::new(),
        mods_list,
    }
}

fn subtitle_code(language: config::SubtitleLanguage) -> u32 {
    match language {
        config::SubtitleLanguage::English => 0,
        config::SubtitleLanguage::German => 1,
        config::SubtitleLanguage::Spanish => 2,
        config::SubtitleLanguage::French => 3,
        config::SubtitleLanguage::Italian => 4,
        config::SubtitleLanguage::Japanese => 5,
    }
}

fn voice_code(language: config::VoiceLanguage) -> u32 {
    match language {
        config::VoiceLanguage::Japanese => 0,
        config::VoiceLanguage::English => 1,
    }
}

/// Recommended SA2 patches: (name, enabled by default).
const RECOMMENDED_PATCHES: &[(&str, bool)] = &[
    ("FramerateLimiter", false),
    ("DisableExitPrompt", true),
    ("SyncLoad", true),
    ("ExtendVertexBuffer", true),
    ("EnvMapFix", true),
    ("ScreenFadeFix", true),
    ("CECarFix", true),
    ("ParticlesFix", true),
];

// --- SA2-specific file writers ---

fn write_user_config(
    game_path: &Path,
    width: u32,
    height: u32,
    language_selection: config::LanguageSelection,
) -> Result<()> {
    let dir = game_path.join("Config");
    std::fs::create_dir_all(&dir)?;

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <Configs xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
         FullScreen=\"0\" Display=\"0\" Res=\"0\" \
         Width=\"{}\" Height=\"{}\" RefreshRate=\"60\" \
         Language=\"{}\" Analytics=\"0\" />\n",
        width,
        height,
        subtitle_code(language_selection.subtitle)
    );

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
                slug: "sa2-render-fix",
                source: ModSource::DirectUrl {
                    url: "https://github.com/shaddatic/sa2b-render-fix/releases/latest/download/sa2-render-fix.7z",
                },
                description: "Test",
                full_description: None,
                pictures: &[],
                dir_name: Some("sa2-render-fix"),
                links: &[],
            },
            ModEntry {
                name: "Better Radar",
                slug: "better-radar",
                source: ModSource::DirectUrl {
                    url: "https://github.com/kellsnc/SA2BetterRadar/releases/latest/download/SA2BetterRadar.7z",
                },
                description: "Test",
                full_description: None,
                pictures: &[],
                dir_name: Some("SA2BetterRadar"),
                links: &[],
            },
        ]
    }

    #[test]
    fn test_generate_creates_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let game_path = tmp.path();

        let mods = test_mods();
        let mod_refs: Vec<&ModEntry> = mods.iter().collect();
        generate_sa2_config(
            game_path,
            &mod_refs,
            1920,
            1080,
            config::LanguageSelection::defaults_for(GameKind::SA2),
        )
        .unwrap();

        assert!(game_path.join("SAManager/Manager.json").is_file());
        assert!(
            game_path
                .join("mods/.modloader/profiles/Profiles.json")
                .is_file()
        );
        assert!(
            game_path
                .join("mods/.modloader/profiles/Default.json")
                .is_file()
        );
        assert!(game_path.join("mods/.modloader/samanager.txt").is_file());
        assert!(game_path.join("Config/UserConfig.cfg").is_file());
    }

    #[test]
    fn test_default_profile_mods_and_patches() {
        let tmp = tempfile::tempdir().unwrap();

        let mods = test_mods();
        let mod_refs: Vec<&ModEntry> = mods.iter().collect();
        generate_sa2_config(
            tmp.path(),
            &mod_refs,
            1920,
            1080,
            config::LanguageSelection::defaults_for(GameKind::SA2),
        )
        .unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["SettingsVersion"], 3);

        let enabled = parsed["EnabledMods"].as_array().unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0], "sa2-render-fix");
        assert_eq!(enabled[1], "SA2BetterRadar");

        let mods_list = parsed["ModsList"].as_array().unwrap();
        assert_eq!(mods_list.len(), sa2::RECOMMENDED_MODS.len());

        let patches = parsed["Patches"].as_object().unwrap();
        assert_eq!(patches["FramerateLimiter"], false);
        assert_eq!(patches["CECarFix"], true);
        assert_eq!(patches.len(), RECOMMENDED_PATCHES.len());
    }

    #[test]
    fn test_no_sadx_specific_fields() {
        let tmp = tempfile::tempdir().unwrap();
        generate_sa2_config(
            tmp.path(),
            &[],
            1920,
            1080,
            config::LanguageSelection::defaults_for(GameKind::SA2),
        )
        .unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/profiles/Default.json"))
                .unwrap();

        assert!(!content.contains("\"Controller\""));
        assert!(!content.contains("\"Sound\""));
        assert!(!content.contains("\"EnableVsync\""));
        assert!(!content.contains("\"RenderBackend\""));
        assert!(content.contains("\"SkipIntro\""));
        assert!(content.contains("\"RefreshRate\""));
    }

    #[test]
    fn test_user_config_xml() {
        let tmp = tempfile::tempdir().unwrap();
        write_user_config(
            tmp.path(),
            1920,
            1080,
            config::LanguageSelection::defaults_for(GameKind::SA2),
        )
        .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("Config/UserConfig.cfg")).unwrap();
        assert!(content.contains("<?xml version"));
        assert!(content.contains("FullScreen=\"0\""));
        assert!(content.contains("Width=\"1920\""));
        assert!(content.contains("Height=\"1080\""));
        assert!(content.contains("Language=\"0\""));
    }

    #[test]
    fn test_empty_mod_selection() {
        let tmp = tempfile::tempdir().unwrap();
        generate_sa2_config(
            tmp.path(),
            &[],
            1920,
            1080,
            config::LanguageSelection::defaults_for(GameKind::SA2),
        )
        .unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["EnabledMods"].as_array().unwrap().is_empty());
        assert!(!parsed["Patches"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_selected_languages_are_written_to_profile_and_user_config() {
        let tmp = tempfile::tempdir().unwrap();
        let selection = config::LanguageSelection {
            subtitle: config::SubtitleLanguage::German,
            voice: config::VoiceLanguage::Japanese,
        };

        generate_sa2_config(tmp.path(), &[], 1920, 1080, selection).unwrap();

        let profile =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&profile).unwrap();
        assert_eq!(
            parsed["Graphics"]["GameTextLanguage"],
            subtitle_code(selection.subtitle)
        );
        assert_eq!(
            parsed["TestSpawn"]["GameTextLanguage"],
            subtitle_code(selection.subtitle)
        );
        assert_eq!(
            parsed["TestSpawn"]["GameVoiceLanguage"],
            voice_code(selection.voice)
        );

        let user_config =
            std::fs::read_to_string(tmp.path().join("Config/UserConfig.cfg")).unwrap();
        assert!(user_config.contains(&format!(
            "Language=\"{}\"",
            subtitle_code(selection.subtitle)
        )));
    }
}
