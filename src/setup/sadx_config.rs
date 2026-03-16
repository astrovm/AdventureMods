use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::common::ModEntry;

/// Convert a Linux path to a Wine Z: drive path with backslashes.
fn linux_to_wine_path(path: &Path) -> String {
    // Resolve symlinks so Wine sees the canonical path
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    format!("Z:{}", resolved.to_string_lossy().replace('/', "\\"))
}

/// Generate all SA Mod Manager v4 configuration files for SADX.
pub fn generate_sadx_config(
    game_path: &Path,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
) -> Result<()> {
    let profile = build_default_profile(game_path, selected_mods, width, height);

    write_manager_json(game_path)?;
    write_profiles_json(game_path, "SAManager/SADX/profiles")?;
    write_profiles_json(game_path, "mods/.modloader/profiles")?;
    write_default_json(game_path, &profile, "SAManager/SADX/profiles")?;
    write_default_json(game_path, &profile, "mods/.modloader/profiles")?;
    write_samanager_txt(game_path)?;
    write_sonic_dx_ini(game_path)?;

    tracing::info!("SADX configuration files generated");
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
    controller: Controller,
    sound: Sound,
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

fn build_default_profile(
    game_path: &Path,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
) -> DefaultProfile {
    let mod_dirs: Vec<String> = selected_mods
        .iter()
        .filter_map(|m| m.dir_name.map(|d| d.to_string()))
        .collect();

    let mut patches = BTreeMap::new();
    for &(name, enabled) in RECOMMENDED_PATCHES {
        patches.insert(name.to_string(), enabled);
    }

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
            game_voice_language: 1,
            use_manual: false,
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
            name: "Sonic Adventure DX".to_string(),
            directory: format!("{}\\", wine_path),
            executable: "sonic.exe".to_string(),
            game_type: 1,
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

fn write_sonic_dx_ini(game_path: &Path) -> Result<()> {
    let system_dir = if game_path.join("System").is_dir() {
        game_path.join("System")
    } else if game_path.join("system").is_dir() {
        game_path.join("system")
    } else {
        game_path.join("System")
    };
    std::fs::create_dir_all(&system_dir)?;

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

    std::fs::write(system_dir.join("sonicDX.ini"), ini).context("Failed to write sonicDX.ini")
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
                before_image: None,
                after_image: None,
                dir_name: Some("TestModA"),
            },
            ModEntry {
                name: "Test Mod B",
                source: ModSource::DirectUrl {
                    url: "https://example.com/b.7z",
                },
                description: "Test mod B",
                before_image: None,
                after_image: None,
                dir_name: Some("TestModB"),
            },
        ]
    }

    #[test]
    fn test_linux_to_wine_path() {
        let path = Path::new("/home/user/.steam/steamapps/common/Sonic Adventure DX");
        assert_eq!(
            linux_to_wine_path(path),
            "Z:\\home\\user\\.steam\\steamapps\\common\\Sonic Adventure DX"
        );
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
    fn test_manager_json_format() {
        let tmp = tempfile::tempdir().unwrap();
        write_manager_json(tmp.path()).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["SettingsVersion"], 3);
        assert_eq!(parsed["CurrentSetGame"], 0);
        assert_eq!(parsed["GameEntries"][0]["Name"], "Sonic Adventure DX");
        assert_eq!(parsed["GameEntries"][0]["Executable"], "sonic.exe");
        assert_eq!(parsed["GameEntries"][0]["Type"], 1);
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
        assert_eq!(enabled, mods_list);

        // Patches are key→bool, not array
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
    fn test_samanager_txt_format() {
        let tmp = tempfile::tempdir().unwrap();
        write_samanager_txt(tmp.path()).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/samanager.txt")).unwrap();
        let wine_path = linux_to_wine_path(tmp.path());
        assert_eq!(content, format!("{}\\\n", wine_path));
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
        assert!(parsed["ModsList"].as_array().unwrap().is_empty());
        // Patches should still be present even with no mods
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
        // Verify exact field names that serde PascalCase might get wrong
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

        let manager = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();
        assert!(
            manager.contains("\"UpdateTimeOutCD\""),
            "Should be UpdateTimeOutCD"
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
