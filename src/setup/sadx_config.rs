use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::common::ModEntry;
use super::{config, sadx};
use crate::steam::game::GameKind;

/// Generate all SA Mod Manager v4 configuration files for SADX.
pub fn generate_sadx_config(
    game_path: &Path,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
) -> Result<()> {
    let profile = build_default_profile(game_path, selected_mods, width, height);

    config::write_manager_json(game_path, GameKind::SADX)?;
    config::write_profiles_json(game_path, "SAManager/SADX/profiles")?;
    config::write_profiles_json(game_path, "mods/.modloader/profiles")?;
    config::write_default_json(game_path, &profile, "SAManager/SADX/profiles")?;
    config::write_default_json(game_path, &profile, "mods/.modloader/profiles")?;
    config::write_samanager_txt(game_path)?;
    write_sonic_dx_ini(game_path)?;

    tracing::info!("SADX configuration files generated");
    Ok(())
}

// --- SADX-specific profile structures ---

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct DefaultProfile {
    settings_version: u32,
    graphics: Graphics,
    controller: Controller,
    sound: Sound,
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
    enable43_resolution_ratio: bool,
    enable_vsync: bool,
    enable_pause_on_inactive: bool,
    custom_window_width: u32,
    custom_window_height: u32,
    enable_keep_resolution_ratio: bool,
    enable_resizable_window: bool,
    fill_mode_background: u32,
    #[serde(rename = "FillModeFMV")]
    fill_mode_fmv: u32,
    mode_texture_filtering: u32,
    #[serde(rename = "ModeUIFiltering")]
    mode_ui_filtering: u32,
    #[serde(rename = "EnableUIScaling")]
    enable_ui_scaling: bool,
    enable_forced_mipmapping: bool,
    enable_forced_texture_filter: bool,
    screen_mode: u32,
    game_frame_rate: u32,
    game_fog_mode: u32,
    game_clip_level: u32,
    show_mouse_in_fullscreen: bool,
    stretch_to_window: bool,
    disable_border_image: bool,
    enable_custom_window: bool,
    enable_borderless: bool,
    enable_screen_scaling: bool,
    render_backend: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Controller {
    enabled_input_mod: bool,
    vanilla_mouse_use_drag: bool,
    vanilla_mouse_start: u32,
    vanilla_mouse_attack: u32,
    vanilla_mouse_jump: u32,
    vanilla_mouse_action: u32,
    vanilla_mouse_flute: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Sound {
    enable_game_music: bool,
    enable_game_sound: bool,
    enable_game_sound3_d: bool,
    enable_bass_music: bool,
    #[serde(rename = "EnableBassSFX")]
    enable_bass_sfx: bool,
    game_music_volume: u32,
    game_sound_volume: u32,
    #[serde(rename = "SEVolume")]
    se_volume: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct TestSpawn {
    use_character: bool,
    use_level: bool,
    use_event: bool,
    use_game_mode: bool,
    use_save: bool,
    level_index: i32,
    act_index: u32,
    character_index: i32,
    event_index: i32,
    game_mode_index: i32,
    save_index: i32,
    game_text_language: u32,
    game_voice_language: u32,
    use_manual: bool,
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
) -> DefaultProfile {
    let enabled_mods = config::mod_dir_names(selected_mods);
    let all_recommended: Vec<&ModEntry> = sadx::RECOMMENDED_MODS.iter().collect();
    let mods_list = config::mod_dir_names(&all_recommended);

    DefaultProfile {
        settings_version: 4,
        graphics: Graphics {
            selected_screen: 1,
            horizontal_resolution: width,
            vertical_resolution: height,
            enable43_resolution_ratio: false,
            enable_vsync: true,
            enable_pause_on_inactive: true,
            custom_window_width: 640,
            custom_window_height: 480,
            enable_keep_resolution_ratio: false,
            enable_resizable_window: false,
            fill_mode_background: 2,
            fill_mode_fmv: 1,
            mode_texture_filtering: 0,
            mode_ui_filtering: 0,
            enable_ui_scaling: true,
            enable_forced_mipmapping: true,
            enable_forced_texture_filter: true,
            screen_mode: 2,
            game_frame_rate: 0,
            game_fog_mode: 0,
            game_clip_level: 0,
            show_mouse_in_fullscreen: false,
            stretch_to_window: false,
            disable_border_image: false,
            enable_custom_window: false,
            enable_borderless: true,
            enable_screen_scaling: true,
            render_backend: 1,
        },
        controller: Controller {
            enabled_input_mod: true,
            vanilla_mouse_use_drag: false,
            vanilla_mouse_start: 0,
            vanilla_mouse_attack: 0,
            vanilla_mouse_jump: 0,
            vanilla_mouse_action: 0,
            vanilla_mouse_flute: 0,
        },
        sound: Sound {
            enable_game_music: true,
            enable_game_sound: true,
            enable_game_sound3_d: true,
            enable_bass_music: true,
            enable_bass_sfx: false,
            game_music_volume: 100,
            game_sound_volume: 100,
            se_volume: 100,
        },
        test_spawn: TestSpawn {
            use_character: false,
            use_level: false,
            use_event: false,
            use_game_mode: false,
            use_save: false,
            level_index: -1,
            act_index: 0,
            character_index: -1,
            event_index: -1,
            game_mode_index: -1,
            save_index: -1,
            game_text_language: 1,
            game_voice_language: 0,
            use_manual: false,
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

/// Recommended patches: (name, enabled by default).
const RECOMMENDED_PATCHES: &[(&str, bool)] = &[
    ("HRTFSound", true),
    ("KeepCamSettings", true),
    ("FixVertexColorRendering", true),
    ("MaterialColorFix", true),
    ("NodeLimit", true),
    ("FOVFix", true),
    ("SkyChaseResolutionFix", true),
    ("Chaos2CrashFix", true),
    ("ChunkSpecularFix", true),
    ("E102NGonFix", true),
    ("ChaoPanelFix", true),
    ("PixelOffSetFix", true),
    ("LightFix", true),
    ("KillGBIX", false),
    ("DisableCDCheck", true),
    ("ExtendedSaveSupport", true),
    ("CrashGuard", true),
    ("XInputFix", false),
];

// --- SADX-specific file writers ---

fn write_sonic_dx_ini(game_path: &Path) -> Result<()> {
    let sys = config::system_dir(game_path);
    std::fs::create_dir_all(&sys)?;

    let ini = "[sonicDX]\n\
               framerate=1\n\
               fogemulation=0\n\
               sound3d=1\n\
               screensize=0\n\
               cliplevel=0\n\
               sevoice=1\n\
               bgm=1\n\
               screen=0\n\
               mousemode=0\n\
               bgmv=100\n\
               voicev=100\n";

    std::fs::write(sys.join("sonicDX.ini"), ini).context("Failed to write sonicDX.ini")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::common::{ModEntry, ModSource};

    fn test_mods() -> Vec<ModEntry> {
        vec![
            ModEntry {
                name: "Test Mod A",
                source: ModSource::DirectUrl {
                    url: "https://example.com/a.7z",
                },
                description: "Test mod A",
                full_description: None,
                pictures: &[],
                dir_name: Some("TestModA"),
                links: &[],
            },
            ModEntry {
                name: "Test Mod B",
                source: ModSource::DirectUrl {
                    url: "https://example.com/b.7z",
                },
                description: "Test mod B",
                full_description: None,
                pictures: &[],
                dir_name: Some("TestModB"),
                links: &[],
            },
        ]
    }

    #[test]
    fn test_generate_creates_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let game_path = tmp.path();
        std::fs::create_dir_all(game_path.join("System")).unwrap();

        let mods = test_mods();
        let mod_refs: Vec<&ModEntry> = mods.iter().collect();
        generate_sadx_config(game_path, &mod_refs, 1920, 1080).unwrap();

        assert!(game_path.join("SAManager/Manager.json").is_file());
        assert!(
            game_path
                .join("SAManager/SADX/profiles/Profiles.json")
                .is_file()
        );
        assert!(
            game_path
                .join("SAManager/SADX/profiles/Default.json")
                .is_file()
        );
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
        assert!(game_path.join("System/sonicDX.ini").is_file());
    }

    #[test]
    fn test_default_profile_mods_and_patches() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();

        let mods = test_mods();
        let mod_refs: Vec<&ModEntry> = mods.iter().collect();
        generate_sadx_config(tmp.path(), &mod_refs, 1920, 1080).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["SettingsVersion"], 4);

        let enabled = parsed["EnabledMods"].as_array().unwrap();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0], "TestModA");
        assert_eq!(enabled[1], "TestModB");

        let mods_list = parsed["ModsList"].as_array().unwrap();
        assert_eq!(mods_list.len(), sadx::RECOMMENDED_MODS.len());

        let patches = parsed["Patches"].as_object().unwrap();
        assert_eq!(patches["HRTFSound"], true);
        assert_eq!(patches["KillGBIX"], false);
        assert_eq!(patches["CrashGuard"], true);
        assert_eq!(patches.len(), RECOMMENDED_PATCHES.len());
    }

    #[test]
    fn test_profiles_match_across_locations() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();

        let mods = test_mods();
        let mod_refs: Vec<&ModEntry> = mods.iter().collect();
        generate_sadx_config(tmp.path(), &mod_refs, 1920, 1080).unwrap();

        let sam_default =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Default.json"))
                .unwrap();
        let loader_default =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/profiles/Default.json"))
                .unwrap();
        assert_eq!(sam_default, loader_default);

        let sam_profiles =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Profiles.json"))
                .unwrap();
        let loader_profiles =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/profiles/Profiles.json"))
                .unwrap();
        assert_eq!(sam_profiles, loader_profiles);
    }

    #[test]
    fn test_sonic_dx_ini_content() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();
        write_sonic_dx_ini(tmp.path()).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("System/sonicDX.ini")).unwrap();
        assert!(content.contains("[sonicDX]"));
        assert!(content.contains("framerate=1"));
        assert!(content.contains("bgmv=100"));
        assert!(content.contains("voicev=100"));
    }

    #[test]
    fn test_empty_mod_selection() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();

        generate_sadx_config(tmp.path(), &[], 1920, 1080).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["EnabledMods"].as_array().unwrap().is_empty());
        assert_eq!(
            parsed["ModsList"].as_array().unwrap().len(),
            sadx::RECOMMENDED_MODS.len()
        );
        assert!(!parsed["Patches"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_json_field_names_match_sa_mod_manager() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();
        generate_sadx_config(tmp.path(), &[], 1920, 1080).unwrap();

        let profile =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Default.json"))
                .unwrap();
        assert!(
            profile.contains("\"EnableGameSound3D\""),
            "Should be EnableGameSound3D"
        );
        assert!(
            profile.contains("\"EnableBassSFX\""),
            "Should be EnableBassSFX"
        );
        assert!(profile.contains("\"SEVolume\""), "Should be SEVolume");
        assert!(profile.contains("\"FillModeFMV\""), "Should be FillModeFMV");
        assert!(
            profile.contains("\"ModeUIFiltering\""),
            "Should be ModeUIFiltering"
        );
        assert!(
            profile.contains("\"EnableUIScaling\""),
            "Should be EnableUIScaling"
        );
    }

    #[test]
    fn test_game_path_in_profile() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();

        generate_sadx_config(tmp.path(), &[], 1920, 1080).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        let game_path = parsed["GamePath"].as_str().unwrap();
        assert!(game_path.starts_with("Z:\\"));
    }
}
