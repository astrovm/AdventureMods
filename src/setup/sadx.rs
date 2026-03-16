use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download};

use super::common::{ModEntry, ModSource};

/// Direct URL for the SADX Mod Loader archive.
const SADX_MOD_LOADER_URL: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXModLoader.7z";

/// Direct URL for the Steam-to-2004 conversion tools archive.
const STEAM_TOOLS_URL: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/steam_tools.7z";

/// Base URL for mods hosted on dcmods.unreliable.network.
#[cfg(test)]
const DCMODS_BASE: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/";

/// Recommended SADX mods.
pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "ADX Audio",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/ADXAudio.7z",
        },
        description: "High-quality ADX audio replacement",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "SADX: Fixed Edition",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXFE.7z",
        },
        description: "Comprehensive bug fix and restoration mod",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Smooth Camera",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/smooth-cam.7z",
        },
        description: "Smoother camera movement",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Pause Hide",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/pause-hide.7z",
        },
        description: "Hide HUD when pausing for screenshots",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Frame Limit",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-frame-limit.7z",
        },
        description: "Accurate frame rate limiter",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Onion Blur",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-onion-blur.7z",
        },
        description: "Dreamcast-style motion blur effect",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/onion_blur_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/onion_blur_after.jpg"),
    },
    ModEntry {
        name: "Idle Chatter",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/idle-chatter.7z",
        },
        description: "Restores character idle voice lines",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Steam Achievements",
        source: ModSource::DirectUrl {
            url: "https://mm.reimuhakurei.net/sadxmods/SteamAchievements.7z",
        },
        description: "Enables Steam achievements with mods",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Lantern Engine",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-dc-lighting.7z",
        },
        description: "Dreamcast-accurate lighting engine",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/lantern_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/lantern_after.jpg"),
    },
    ModEntry {
        name: "Dreamcast Conversion",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z",
        },
        description: "Restores Dreamcast visuals and features",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_conv_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_conv_after.jpg"),
    },
    ModEntry {
        name: "Dreamcast DLC",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DLCs.7z",
        },
        description: "Restored Dreamcast downloadable content",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "SADX Style Water",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-style-water.7z",
        },
        description: "Improved water rendering",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/water_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/water_after.jpg"),
    },
    ModEntry {
        name: "Sound Overhaul",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SoundOverhaul.7z",
        },
        description: "Restored and improved sound effects",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Time of Day",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/TrainDaytime.7z",
        },
        description: "Dynamic time-of-day lighting changes",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Dreamcast Characters Pack",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SA1_Chars.7z",
        },
        description: "Original Dreamcast character models",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_chars_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_chars_after.jpg"),
    },
    ModEntry {
        name: "Super Sonic",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-super-sonic.7z",
        },
        description: "Playable Super Sonic in action stages",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "HD GUI 2",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/HD_DCStyle.7z",
        },
        description: "High resolution GUI textures for menus, HUD and icons",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/hd_gui_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/hd_gui_after.jpg"),
    },
    ModEntry {
        name: "SADX Launcher",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/AppLauncher.7z",
        },
        description: "Tool to configure game controls and settings",
        before_image: None,
        after_image: None,
    },
    ModEntry {
        name: "Icon Data",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/icondata.7z",
        },
        description: "Custom game window icons",
        before_image: None,
        after_image: None,
    },
];

/// Convert the Steam version of SADX to the 2004 version using HDiffPatch.
///
/// The Steam version of sonic.exe is binary-incompatible with the mod loader.
/// This downloads `steam_tools.7z` (containing `patch_steam_inst.dat`) and
/// applies a directory diff patch that converts ~124 game files to the 2004
/// version that the mod loader expects.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn convert_steam_to_2004(
    game_path: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    // Detect the system directory (can be System or system on Linux)
    let system_dir = if game_path.join("System").is_dir() {
        game_path.join("System")
    } else if game_path.join("system").is_dir() {
        game_path.join("system")
    } else {
        // Default to System but we'll likely fail later anyway if missing
        game_path.join("System")
    };

    // Skip if already converted. Check multiple markers since previous setups
    // (including the official Windows installer) leave different traces.
    let chrmodels_orig = system_dir.join("CHRMODELS_orig.dll");
    if chrmodels_orig.exists() {
        tracing::info!("Game appears already converted (CHRMODELS_orig.dll exists), skipping");
        return Ok(());
    }

    if game_path.join("SADXModLoader.dll").exists() {
        tracing::info!("Game appears already converted (SADXModLoader.dll exists), skipping");
        return Ok(());
    }

    if game_path.join("sonic.exe").exists() {
        tracing::info!("Game appears already converted (sonic.exe exists), skipping");
        return Ok(());
    }

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("steam_tools.7z");

    download::download_file(STEAM_TOOLS_URL, &archive_path, progress)?;

    let extract_dir = temp_dir.path().join("steam_tools");
    archive::extract(&archive_path, &extract_dir)?;

    let patch_file = extract_dir.join("patch_steam_inst.dat");
    if !patch_file.is_file() {
        anyhow::bail!("patch_steam_inst.dat not found in steam_tools.7z");
    }

    // Apply the directory diff patch using hpatchz (bundled in the Flatpak)
    // We use a separate output directory to avoid hpatchz failing due to 
    // in-place modification conflicts or permission issues with its own temp dir.
    let out_dir = temp_dir.path().join("patched_game");
    std::fs::create_dir_all(&out_dir)?;

    let game_str = game_path.to_string_lossy().trim_end_matches('/').to_string();
    let patch_str = patch_file.to_string_lossy().to_string();
    let out_str = out_dir.to_string_lossy().trim_end_matches('/').to_string();

    // The hpatchz patch was built on a case-insensitive Windows filesystem.
    // On Linux (case-sensitive), directory names must match exactly.
    // Steam on Linux may extract directories with different casing than what
    // the patch expects, so we normalize them before patching.
    normalize_case_for_patch(game_path)?;

    tracing::info!("Applying Steam-to-2004 patch to {}", game_str);

    let output = std::process::Command::new("hpatchz")
        .arg("-f")
        .arg(&game_str)
        .arg(&patch_str)
        .arg(&out_str)
        .output()
        .context("Failed to run hpatchz — is HDiffPatch installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        tracing::error!("hpatchz failed. Stderr:\n{}", stderr);

        // If it failed due to "open oldFile ERROR", it's likely a source file mismatch.
        if stderr.contains("open oldFile ERROR!") || stderr.contains("check oldPathType") {
             tracing::error!("Source file mismatch detected. This usually happens if the game is already modded or corrupted.");
             anyhow::bail!("Steam-to-2004 conversion failed. Your game installation might be modified or corrupted. Please verify game integrity in Steam and try again.\n\nDetails:\n{stderr}");
        }

        anyhow::bail!("Steam-to-2004 conversion failed:\n{stdout}\n{stderr}");
    }

    // Success! Now we move the patched files back to the game directory.
    // hpatchz in directory mode produces a new directory with the patched files.
    // We want to merge/overwrite these into the game directory.
    tracing::info!("Patch applied successfully to temp dir, moving files back...");

    // Move files from out_dir to game_path
    move_dir_contents(&out_dir, game_path)?;

    // The patched output uses lowercase "system" (matching the patch).
    // Rename back to "System" since the mod loader and configure_game() expect it.
    let system_lower = game_path.join("system");
    let system_upper = game_path.join("System");
    if system_lower.is_dir() && !system_upper.exists() {
        std::fs::rename(&system_lower, &system_upper)
            .context("Failed to rename system → System after patching")?;
        tracing::info!("Renamed system → System for mod loader compatibility");
    }

    tracing::info!("Steam-to-2004 conversion complete");
    Ok(())
}

/// Rename directories to match the casing the hpatchz patch expects.
///
/// The patch was created on Windows (case-insensitive). Steam on Linux may
/// extract directories with different casing. Known mismatches:
/// - `System` → `system`
/// - `SoundData/VOICE_JP` → `SoundData/voice_jp`
/// - `SoundData/VOICE_US` → `SoundData/voice_us`
/// - `SoundData/SE` → `SoundData/se`
/// - `SoundData/*/WMA` → `SoundData/*/wma`
fn normalize_case_for_patch(game_path: &Path) -> Result<()> {
    // Helper: rename src → dst if src exists and dst doesn't
    let rename_if_needed = |src: &Path, dst: &Path| -> Result<()> {
        if src.is_dir() && !dst.exists() {
            std::fs::rename(src, dst).with_context(|| {
                format!(
                    "Failed to rename {} → {}",
                    src.display(),
                    dst.display()
                )
            })?;
            tracing::info!(
                "Renamed {} → {} for patch compatibility",
                src.file_name().unwrap_or_default().to_string_lossy(),
                dst.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        Ok(())
    };

    // System → system
    rename_if_needed(
        &game_path.join("System"),
        &game_path.join("system"),
    )?;

    let sound_data = game_path.join("SoundData");
    if sound_data.is_dir() {
        // VOICE_JP → voice_jp, VOICE_US → voice_us, SE → se
        rename_if_needed(&sound_data.join("VOICE_JP"), &sound_data.join("voice_jp"))?;
        rename_if_needed(&sound_data.join("VOICE_US"), &sound_data.join("voice_us"))?;
        rename_if_needed(&sound_data.join("SE"), &sound_data.join("se"))?;

        // WMA → wma inside voice dirs
        for dir_name in &["voice_jp", "voice_us"] {
            let voice_dir = sound_data.join(dir_name);
            if voice_dir.is_dir() {
                rename_if_needed(&voice_dir.join("WMA"), &voice_dir.join("wma"))?;
            }
        }
    }

    Ok(())
}

fn move_dir_contents(from: &Path, to: &Path) -> Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let dest = to.join(name);

        if path.is_dir() {
            if dest.exists() && !dest.is_dir() {
                std::fs::remove_file(&dest)?;
            }
            if !dest.exists() {
                std::fs::create_dir_all(&dest)?;
            }
            move_dir_contents(&path, &dest)?;
        } else {
            if dest.exists() && dest.is_dir() {
                std::fs::remove_dir_all(&dest)?;
            }
            // Overwrite existing files
            std::fs::rename(&path, &dest).or_else(|_| {
                // Fallback to copy+remove if rename fails (e.g. across filesystems)
                std::fs::copy(&path, &dest)?;
                std::fs::remove_file(&path)?;
                Ok::<(), std::io::Error>(())
            })?;
        }
    }
    Ok(())
}

/// Download and install the SADX Mod Loader into the game directory.
///
/// The mod loader hooks into the game via a CHRMODELS.dll proxy:
/// 1. Archive contents go into `mods/.modloader/`
/// 2. Original `System/CHRMODELS.dll` is backed up to `System/CHRMODELS_orig.dll`
/// 3. `SADXModLoader.dll` is copied to `System/CHRMODELS.dll`
///
/// When `sonic.exe` starts, it loads the fake CHRMODELS.dll which is actually
/// the mod loader. The mod loader then loads the original via CHRMODELS_orig.dll.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_mod_loader(
    game_path: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    // Detect the system directory (can be System or system on Linux)
    let system_dir = if game_path.join("System").is_dir() {
        game_path.join("System")
    } else if game_path.join("system").is_dir() {
        game_path.join("system")
    } else {
        game_path.join("System")
    };

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("SADXModLoader.7z");

    download::download_file(SADX_MOD_LOADER_URL, &archive_path, progress)?;

    // Extract into mods/.modloader/
    let modloader_dir = game_path.join("mods").join(".modloader");
    std::fs::create_dir_all(&modloader_dir)?;
    archive::extract(&archive_path, &modloader_dir)?;

    // Backup original System/CHRMODELS.dll if not already backed up
    let chrmodels = system_dir.join("CHRMODELS.dll");
    let chrmodels_orig = system_dir.join("CHRMODELS_orig.dll");
    if chrmodels.is_file() && !chrmodels_orig.exists() {
        std::fs::rename(&chrmodels, &chrmodels_orig)
            .context("Failed to backup CHRMODELS.dll to CHRMODELS_orig.dll")?;
        tracing::info!("Backed up CHRMODELS.dll to CHRMODELS_orig.dll");
    }

    // Copy SADXModLoader.dll as System/CHRMODELS.dll (the proxy hook)
    let loader_dll = modloader_dir.join("SADXModLoader.dll");
    if !loader_dll.is_file() {
        anyhow::bail!("SADXModLoader.dll not found in archive");
    }
    std::fs::copy(&loader_dll, &chrmodels)
        .context("Failed to copy SADXModLoader.dll to System/CHRMODELS.dll")?;

    tracing::info!("SADX Mod Loader installed to {}", game_path.display());
    Ok(())
}

/// Map a mod display name to its extracted directory name.
fn mod_dir_name(name: &str) -> &str {
    match name {
        "SADX: Fixed Edition" => "SADXFE",
        "Dreamcast Conversion" => "DreamcastConversion",
        "Dreamcast Characters Pack" => "SA1_Chars",
        "Lantern Engine" => "sadx-dc-lighting",
        "Sound Overhaul" => "SoundOverhaul",
        "Dreamcast DLC" => "DLC",
        "ADX Audio" => "ADXAudio",
        "Super Sonic" => "sadx-super-sonic",
        "Frame Limit" => "sadx-frame-limit",
        "Idle Chatter" => "idle-chatter",
        "Pause Hide" => "pause-hide",
        "Onion Blur" => "sadx-onion-blur",
        "Smooth Camera" => "smooth-cam",
        "SADX Style Water" => "sadx-style-water",
        "Steam Achievements" => "SteamAchievements",
        "HD GUI 2" => "HD_DCStyle",
        "SADX Launcher" => "AppLauncher",
        "Icon Data" => "icondata",
        "Time of Day" => "TrainDaytime",
        _ => name,
    }
}

/// Configure SA Mod Manager with JSON profile files matching the official installer.
///
/// Writes three files:
/// - `SAManager/Manager.json` — manager settings and game entry
/// - `SAManager/SADX/profiles/Profiles.json` — profile index
/// - `SAManager/SADX/profiles/Default.json` — mod list, patches, codes, and game settings
pub fn configure_mod_loader(game_path: &Path, selected_mods: &[&ModEntry]) -> Result<()> {
    // SA Mod Manager runs under Wine/Proton and expects Windows-style paths.
    let game_dir_wine = format!("Z:{}", game_path.to_string_lossy().replace('/', "\\"));
    let sa_manager_dir = game_path.join("SAManager");
    let profiles_dir = sa_manager_dir.join("SADX").join("profiles");
    std::fs::create_dir_all(&profiles_dir)?;

    // Manager.json — lives at SAManager/ (the root, not per-game)
    let manager_json = format!(
        r#"{{
  "SettingsVersion": 3,
  "CurrentSetGame": 0,
  "Theme": 2,
  "Language": 0,
  "ModAuthor": "Unknown",
  "EnableDeveloperMode": false,
  "KeepManagerOpen": true,
  "UpdateSettings": {{
    "EnableManagerBootCheck": true,
    "EnableModsBootCheck": true,
    "EnableLoaderBootCheck": true,
    "UpdateTimeOutCD": 0,
    "UpdateCheckCount": 1
  }},
  "GameEntries": [
    {{
      "Name": "Sonic Adventure DX",
      "Directory": "{}",
      "Executable": "sonic.exe",
      "Type": 1
    }}
  ],
  "KeepModOrder": false
}}"#,
        game_dir_wine
    );
    let manager_path = sa_manager_dir.join("Manager.json");
    std::fs::write(&manager_path, manager_json).context("Failed to write Manager.json")?;

    // Profiles.json
    let profiles_json = r#"{
  "ProfileIndex": 0,
  "ProfilesList": [
    {
      "Name": "Default",
      "Filename": "Default.json"
    }
  ]
}"#;
    let profiles_path = profiles_dir.join("Profiles.json");
    std::fs::write(&profiles_path, profiles_json).context("Failed to write Profiles.json")?;

    // Default.json — the main profile with mods, patches, codes
    let mut enabled_mods = String::new();
    for (i, mod_entry) in selected_mods.iter().enumerate() {
        if i > 0 {
            enabled_mods.push_str(",\n");
        }
        enabled_mods.push_str(&format!("    \"{}\"", mod_dir_name(mod_entry.name)));
    }

    let default_json = format!(
        r#"{{
  "Graphics": {{
    "SelectedScreen": 0,
    "Enable43ResolutionRatio": false,
    "EnablePauseOnInactive": true,
    "EnableKeepResolutionRatio": false,
    "EnableResizableWindow": false,
    "FillModeBackground": 2,
    "FillModeFMV": 1,
    "ModeTextureFiltering": 0,
    "ModeUIFiltering": 0,
    "EnableUIScaling": true,
    "EnableForcedMipmapping": true,
    "EnableForcedTextureFilter": true,
    "ShowMouseInFullscreen": false,
    "DisableBorderImage": false,
    "Anisotropic": 16,
    "RenderBackend": 1
  }},
  "Controller": {{
    "EnabledInputMod": true
  }},
  "Sound": {{
    "EnableGameMusic": true,
    "EnableGameSound": true,
    "EnableGameSound3D": true,
    "EnableBassMusic": true,
    "EnableBassSFX": true,
    "GameMusicVolume": 100,
    "GameSoundVolume": 100,
    "SEVolume": 100
  }},
  "TestSpawn": {{
    "UseCharacter": false,
    "UseLevel": true,
    "UseEvent": false,
    "UseGameMode": false,
    "UseSave": false,
    "LevelIndex": 1,
    "ActIndex": 0,
    "CharacterIndex": 0,
    "EventIndex": -1,
    "GameModeIndex": 4,
    "SaveIndex": -1,
    "GameTextLanguage": 0,
    "GameVoiceLanguage": 0,
    "UseManual": false,
    "UsePosition": false,
    "XPosition": 0,
    "YPosition": 0,
    "ZPosition": 0,
    "Rotation": 0
  }},
  "DebugSettings": {{
    "EnableDebugConsole": false,
    "EnableDebugScreen": false,
    "EnableDebugFile": false,
    "EnableDebugCrashLog": true,
    "EnableShowConsole": null
  }},
  "EnabledMods": [
{}
  ],
  "EnabledGamePatches": [
    "HRTFSound",
    "KeepCamSettings",
    "FixVertexColorRendering",
    "MaterialColorFix",
    "NodeLimit",
    "FOVFix",
    "SkyChaseResolutionFix",
    "Chaos2CrashFix",
    "ChunkSpecularFix",
    "E102NGonFix",
    "ChaoPanelFix",
    "PixelOffSetFix",
    "LightFix",
    "KillGBIX",
    "DisableCDCheck",
    "ExtendedSaveSupport",
    "CrashGuard",
    "XInputFix"
  ],
  "EnabledCodes": [
    "Can Always Skip Credits",
    "Egg Carrier Ocean Music",
    "Use Tornado 2 Health Bar in Sky Chase Act 2",
    "Invert Right Stick Y Axis in First Person"
  ]
}}"#,
        enabled_mods
    );
    let default_path = profiles_dir.join("Default.json");
    std::fs::write(&default_path, &default_json).context("Failed to write Default.json")?;

    // The SADX Mod Loader DLL reads its active profile from mods/.modloader/profiles/,
    // not from SAManager/. Write the same profile there so mods load at game startup
    // without requiring the user to open SA Mod Manager and click "Save & Play" first.
    let loader_profiles_dir = game_path.join("mods").join(".modloader").join("profiles");
    std::fs::create_dir_all(&loader_profiles_dir)?;
    std::fs::write(loader_profiles_dir.join("Default.json"), &default_json)
        .context("Failed to write mod loader Default.json")?;
    std::fs::write(loader_profiles_dir.join("Profiles.json"), profiles_json)
        .context("Failed to write mod loader Profiles.json")?;

    // samanager.txt tells the mod loader where the game directory is (Wine path).
    let samanager_txt_path = game_path.join("mods").join(".modloader").join("samanager.txt");
    std::fs::write(
        &samanager_txt_path,
        format!("{}\\\n", game_dir_wine),
    )
    .context("Failed to write samanager.txt")?;

    tracing::info!(
        "Configured SA Mod Manager at {}",
        sa_manager_dir.display()
    );

    configure_game(game_path)?;

    Ok(())
}

/// Create a default `sonic.ini` with recommended game settings.
pub fn configure_game(game_path: &Path) -> Result<()> {
    // Detect the system directory (can be System or system on Linux)
    let system_dir = if game_path.join("System").is_dir() {
        game_path.join("System")
    } else if game_path.join("system").is_dir() {
        game_path.join("system")
    } else {
        game_path.join("System")
    };
    let ini_path = system_dir.join("sonic.ini");

    let content = "[sonicDX]\n\
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

    std::fs::write(&ini_path, content).context("Failed to write sonic.ini")?;
    tracing::info!("Configured SADX game settings at {}", ini_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_recommended_mods_count() {
        assert_eq!(RECOMMENDED_MODS.len(), 19);
    }

    #[test]
    fn test_mod_sources_valid() {
        for m in RECOMMENDED_MODS {
            match &m.source {
                ModSource::GameBanana { file_id } => {
                    assert!(*file_id > 0, "Mod '{}' has zero file_id", m.name);
                }
                ModSource::DirectUrl { url } => {
                    assert!(
                        url.starts_with("https://"),
                        "Mod '{}' has invalid URL: {}",
                        m.name,
                        url
                    );
                }
            }
        }
    }

    #[test]
    fn test_mod_sources_unique() {
        let sources: HashSet<String> = RECOMMENDED_MODS
            .iter()
            .map(|m| super::super::common::resolve_download_url(&m.source))
            .collect();
        assert_eq!(
            sources.len(),
            RECOMMENDED_MODS.len(),
            "Duplicate sources in RECOMMENDED_MODS"
        );
    }

    #[test]
    fn test_mod_names_unique() {
        let names: HashSet<&str> = RECOMMENDED_MODS.iter().map(|m| m.name).collect();
        assert_eq!(
            names.len(),
            RECOMMENDED_MODS.len(),
            "Duplicate mod names in RECOMMENDED_MODS"
        );
    }

    #[test]
    fn test_mod_entries_have_names_and_descriptions() {
        for m in RECOMMENDED_MODS {
            assert!(!m.name.is_empty(), "Mod has empty name");
            assert!(
                !m.description.is_empty(),
                "Mod '{}' has empty description",
                m.name
            );
        }
    }

    #[test]
    fn test_mod_names_safe_for_filesystem() {
        for m in RECOMMENDED_MODS {
            assert!(
                !m.name.contains('/'),
                "Mod name '{}' contains '/'",
                m.name
            );
            assert!(
                !m.name.contains('\\'),
                "Mod name '{}' contains '\\'",
                m.name
            );
            assert!(
                !m.name.contains('\0'),
                "Mod name '{}' contains null byte",
                m.name
            );
        }
    }

    #[test]
    fn test_sadx_mod_loader_url_valid() {
        assert!(SADX_MOD_LOADER_URL.starts_with("https://"));
        assert!(SADX_MOD_LOADER_URL.ends_with(".7z"));
    }

    #[test]
    fn test_dcmods_base_url_valid() {
        assert!(DCMODS_BASE.starts_with("https://"));
        assert!(DCMODS_BASE.ends_with('/'));
    }

    #[test]
    fn test_dcmods_urls_use_correct_base() {
        for m in RECOMMENDED_MODS {
            if let ModSource::DirectUrl { url } = &m.source {
                if url.contains("dcmods.unreliable.network") {
                    assert!(
                        url.starts_with(DCMODS_BASE),
                        "Mod '{}' dcmods URL doesn't start with DCMODS_BASE: {}",
                        m.name,
                        url
                    );
                }
            }
        }
    }

    // --- mod_dir_name() tests ---

    #[test]
    fn test_mod_dir_name_mappings() {
        assert_eq!(mod_dir_name("SADX: Fixed Edition"), "SADXFE");
        assert_eq!(mod_dir_name("Dreamcast Conversion"), "DreamcastConversion");
        assert_eq!(mod_dir_name("Dreamcast Characters Pack"), "SA1_Chars");
        assert_eq!(mod_dir_name("Lantern Engine"), "sadx-dc-lighting");
        assert_eq!(mod_dir_name("Sound Overhaul"), "SoundOverhaul");
        assert_eq!(mod_dir_name("Dreamcast DLC"), "DLC");
        assert_eq!(mod_dir_name("ADX Audio"), "ADXAudio");
        assert_eq!(mod_dir_name("Super Sonic"), "sadx-super-sonic");
        assert_eq!(mod_dir_name("Frame Limit"), "sadx-frame-limit");
        assert_eq!(mod_dir_name("Idle Chatter"), "idle-chatter");
        assert_eq!(mod_dir_name("Pause Hide"), "pause-hide");
        assert_eq!(mod_dir_name("Onion Blur"), "sadx-onion-blur");
        assert_eq!(mod_dir_name("Smooth Camera"), "smooth-cam");
        assert_eq!(mod_dir_name("SADX Style Water"), "sadx-style-water");
        assert_eq!(mod_dir_name("Steam Achievements"), "SteamAchievements");
        assert_eq!(mod_dir_name("HD GUI 2"), "HD_DCStyle");
        assert_eq!(mod_dir_name("SADX Launcher"), "AppLauncher");
        assert_eq!(mod_dir_name("Icon Data"), "icondata");
        assert_eq!(mod_dir_name("Time of Day"), "TrainDaytime");
    }

    #[test]
    fn test_mod_dir_name_unknown_returns_input() {
        assert_eq!(mod_dir_name("Unknown Mod"), "Unknown Mod");
        assert_eq!(mod_dir_name(""), "");
        assert_eq!(mod_dir_name("Some Random Name"), "Some Random Name");
    }

    // --- configure_mod_loader() tests ---

    fn make_game_dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        // configure_mod_loader calls configure_game which needs System dir
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();
        tmp
    }

    #[test]
    fn test_configure_mod_loader_creates_all_files() {
        let tmp = make_game_dir();
        let mods: Vec<&ModEntry> = RECOMMENDED_MODS.iter().take(2).collect();
        configure_mod_loader(tmp.path(), &mods).unwrap();

        assert!(tmp.path().join("SAManager/Manager.json").is_file());
        assert!(tmp.path().join("SAManager/SADX/profiles/Profiles.json").is_file());
        assert!(tmp.path().join("SAManager/SADX/profiles/Default.json").is_file());
        assert!(tmp.path().join("mods/.modloader/profiles/Default.json").is_file());
        assert!(tmp.path().join("mods/.modloader/profiles/Profiles.json").is_file());
        assert!(tmp.path().join("mods/.modloader/samanager.txt").is_file());
    }

    #[test]
    fn test_configure_mod_loader_manager_json() {
        let tmp = make_game_dir();
        configure_mod_loader(tmp.path(), &[]).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();

        // Verify key fields are present in the JSON text
        assert!(content.contains("\"SettingsVersion\": 3"));
        assert!(content.contains("\"Name\": \"Sonic Adventure DX\""));
        assert!(content.contains("\"Executable\": \"sonic.exe\""));

        // Wine path should start with Z: and use backslashes
        let wine_path = format!("Z:{}", tmp.path().to_string_lossy().replace('/', "\\"));
        assert!(
            content.contains(&wine_path),
            "Manager.json should contain Wine path: {wine_path}"
        );
    }

    #[test]
    fn test_configure_mod_loader_default_json_mods() {
        let tmp = make_game_dir();
        let mods: Vec<&ModEntry> = RECOMMENDED_MODS.iter().take(3).collect();
        configure_mod_loader(tmp.path(), &mods).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        let enabled = parsed["EnabledMods"].as_array().unwrap();
        assert_eq!(enabled.len(), 3);
        assert_eq!(enabled[0], mod_dir_name(RECOMMENDED_MODS[0].name));
        assert_eq!(enabled[1], mod_dir_name(RECOMMENDED_MODS[1].name));
        assert_eq!(enabled[2], mod_dir_name(RECOMMENDED_MODS[2].name));

        // Patches and codes should be present
        assert!(!parsed["EnabledGamePatches"].as_array().unwrap().is_empty());
        assert!(!parsed["EnabledCodes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_configure_mod_loader_wine_path() {
        let tmp = make_game_dir();
        configure_mod_loader(tmp.path(), &[]).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("SAManager/Manager.json")).unwrap();
        let expected = format!(
            "\"Directory\": \"Z:{}\"",
            tmp.path().to_string_lossy().replace('/', "\\")
        );
        assert!(
            content.contains(&expected),
            "Manager.json should contain correct Wine path directory entry"
        );
    }

    #[test]
    fn test_configure_mod_loader_empty_selection() {
        let tmp = make_game_dir();
        configure_mod_loader(tmp.path(), &[]).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("SAManager/SADX/profiles/Default.json"))
                .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let enabled = parsed["EnabledMods"].as_array().unwrap();
        assert!(enabled.is_empty());
    }

    #[test]
    fn test_configure_mod_loader_samanager_txt() {
        let tmp = make_game_dir();
        configure_mod_loader(tmp.path(), &[]).unwrap();

        let content =
            std::fs::read_to_string(tmp.path().join("mods/.modloader/samanager.txt")).unwrap();
        let wine_path = format!("Z:{}", tmp.path().to_string_lossy().replace('/', "\\"));
        // Should have trailing backslash and newline
        assert_eq!(content, format!("{}\\\n", wine_path));
    }

    #[test]
    fn test_configure_mod_loader_profiles_match() {
        let tmp = make_game_dir();
        let mods: Vec<&ModEntry> = RECOMMENDED_MODS.iter().take(2).collect();
        configure_mod_loader(tmp.path(), &mods).unwrap();

        // Default.json in SAManager and .modloader should be identical
        let sam_default = std::fs::read_to_string(
            tmp.path().join("SAManager/SADX/profiles/Default.json"),
        )
        .unwrap();
        let loader_default = std::fs::read_to_string(
            tmp.path().join("mods/.modloader/profiles/Default.json"),
        )
        .unwrap();
        assert_eq!(sam_default, loader_default);

        // Profiles.json should also match
        let sam_profiles = std::fs::read_to_string(
            tmp.path().join("SAManager/SADX/profiles/Profiles.json"),
        )
        .unwrap();
        let loader_profiles = std::fs::read_to_string(
            tmp.path().join("mods/.modloader/profiles/Profiles.json"),
        )
        .unwrap();
        assert_eq!(sam_profiles, loader_profiles);
    }

    // --- configure_game() tests ---

    #[test]
    fn test_configure_game_writes_sonic_ini() {
        let tmp = make_game_dir();
        configure_game(tmp.path()).unwrap();

        let ini = std::fs::read_to_string(tmp.path().join("System/sonic.ini")).unwrap();
        assert!(ini.contains("[sonicDX]"));
        assert!(ini.contains("framerate=1"));
        assert!(ini.contains("bgmv=100"));
        assert!(ini.contains("voicev=100"));
    }

    #[test]
    fn test_configure_game_case_insensitive_system_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // Create lowercase "system" instead of "System"
        std::fs::create_dir_all(tmp.path().join("system")).unwrap();
        configure_game(tmp.path()).unwrap();

        assert!(tmp.path().join("system/sonic.ini").is_file());
    }

    // --- move_dir_contents() tests ---

    #[test]
    fn test_move_dir_contents_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        std::fs::write(src.join("a.txt"), "hello").unwrap();
        std::fs::write(src.join("b.txt"), "world").unwrap();

        move_dir_contents(&src, &dst).unwrap();

        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "hello");
        assert_eq!(std::fs::read_to_string(dst.join("b.txt")).unwrap(), "world");
    }

    #[test]
    fn test_move_dir_contents_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::create_dir_all(dst.join("sub")).unwrap();

        std::fs::write(src.join("sub/new.txt"), "new").unwrap();
        std::fs::write(dst.join("sub/existing.txt"), "existing").unwrap();

        move_dir_contents(&src, &dst).unwrap();

        // Both files should exist in destination
        assert_eq!(std::fs::read_to_string(dst.join("sub/new.txt")).unwrap(), "new");
        assert_eq!(
            std::fs::read_to_string(dst.join("sub/existing.txt")).unwrap(),
            "existing"
        );
    }

    #[test]
    fn test_move_dir_contents_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        std::fs::write(src.join("file.txt"), "new content").unwrap();
        std::fs::write(dst.join("file.txt"), "old content").unwrap();

        move_dir_contents(&src, &dst).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst.join("file.txt")).unwrap(),
            "new content"
        );
    }

    #[test]
    fn test_move_dir_contents_file_replaces_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        // dst has a directory named "item"
        std::fs::create_dir_all(dst.join("item")).unwrap();
        std::fs::write(dst.join("item/inner.txt"), "inner").unwrap();

        // src has a file named "item"
        std::fs::write(src.join("item"), "I am a file").unwrap();

        move_dir_contents(&src, &dst).unwrap();

        // "item" should now be a file, not a directory
        assert!(dst.join("item").is_file());
        assert_eq!(
            std::fs::read_to_string(dst.join("item")).unwrap(),
            "I am a file"
        );
    }

    // --- convert_steam_to_2004() skip detection tests ---

    #[test]
    fn test_convert_skips_if_chrmodels_orig_exists() {
        let tmp = make_game_dir();
        std::fs::write(tmp.path().join("System/CHRMODELS_orig.dll"), "dummy").unwrap();

        // Should return Ok without needing hpatchz or downloads
        convert_steam_to_2004(tmp.path(), None).unwrap();
    }

    #[test]
    fn test_convert_skips_if_sadxmodloader_exists() {
        let tmp = make_game_dir();
        std::fs::write(tmp.path().join("SADXModLoader.dll"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }

    #[test]
    fn test_convert_skips_if_sonic_exe_exists() {
        let tmp = make_game_dir();
        std::fs::write(tmp.path().join("sonic.exe"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }
}
