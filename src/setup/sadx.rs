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
    // Skip if already converted (CHRMODELS_orig.dll exists from a previous run)
    let chrmodels_orig = game_path.join("System").join("CHRMODELS_orig.dll");
    if chrmodels_orig.exists() {
        tracing::info!("Game appears already converted (CHRMODELS_orig.dll exists), skipping");
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
    // Canonicalize to resolve symlinks, which is important for host-mounted drives
    let game_canonical = game_path.canonicalize().context("Failed to canonicalize game path")?;
    let game_str = game_canonical
        .to_str()
        .context("Game path is not valid UTF-8")?
        .trim_end_matches('/'); // Remove trailing slash for hpatchz

    let patch_str = patch_file
        .to_str()
        .context("Patch file path is not valid UTF-8")?;

    tracing::info!("Applying Steam-to-2004 patch to {}", game_str);

    let output = std::process::Command::new("hpatchz")
        .arg("-f")
        .arg(game_str)
        .arg(patch_str)
        .arg(game_str)
        .output()
        .context("Failed to run hpatchz — is HDiffPatch installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("Steam-to-2004 conversion failed:\n{stdout}\n{stderr}");
    }

    tracing::info!("Steam-to-2004 conversion complete");
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
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("SADXModLoader.7z");

    download::download_file(SADX_MOD_LOADER_URL, &archive_path, progress)?;

    // Extract into mods/.modloader/
    let modloader_dir = game_path.join("mods").join(".modloader");
    std::fs::create_dir_all(&modloader_dir)?;
    archive::extract(&archive_path, &modloader_dir)?;

    // Backup original System/CHRMODELS.dll if not already backed up
    let system_dir = game_path.join("System");
    let chrmodels = system_dir.join("CHRMODELS.dll");
    let chrmodels_orig = system_dir.join("CHRMODELS_orig.dll");
    if chrmodels.is_file() && !chrmodels_orig.exists() {
        std::fs::rename(&chrmodels, &chrmodels_orig)
            .context("Failed to backup System/CHRMODELS.dll")?;
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

/// Create a default `SADXModLoader.ini` with recommended patches and load order.
pub fn configure_mod_loader(game_path: &Path, selected_mods: &[&ModEntry]) -> Result<()> {
    let mods_dir = game_path.join("mods");
    let ini_path = mods_dir.join("SADXModLoader.ini");

    let mut content = String::from("[SADXModLoader]\n");
    content.push_str("ShowConsole=0\n");
    content.push_str("CheckForUpdates=1\n");
    content.push_str("UpdateHelper=1\n\n");

    content.push_str("[ModOrder]\n");
    for (i, mod_entry) in selected_mods.iter().enumerate() {
        // Assume the mod directory name is a sanitized version of the mod name
        // Most dcmods archives extract to a folder named after the mod
        let mod_dir = match mod_entry.name {
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
            _ => mod_entry.name,
        };
        content.push_str(&format!("Mod{}={}\n", i + 1, mod_dir));
    }

    content.push_str("\n[Patches]\n");
    let patches = [
        ("HRDirect3D9", 1),
        ("SoftShadows", 1),
        ("LightBlur", 1),
        ("DisableScreenSaver", 1),
        ("SCFix", 1),
        ("FovFix", 1),
        ("ChaosPuddleFix", 1),
        ("ChunkSideFix", 1),
        ("E-102Fix", 1),
        ("FogFix", 1),
        ("SkyboxFix", 1),
        ("MaterialFix", 1),
        ("NodeFix", 1),
        ("EffectFix", 1),
        ("UIFix", 1),
        ("ExtendedSave", 1),
        ("GBIXFix", 1),
        ("CameraFix", 1),
    ];
    for (name, val) in patches {
        content.push_str(&format!("{}={}\n", name, val));
    }

    content.push_str("\n[Codes]\n");
    let codes = [
        ("Sonic/Metal Sonic: Remove Spin Dash lighting", 1),
        ("Sonic: Improved homing attack", 1),
        ("Tails: High speed flight", 1),
        ("Knuckles: Improved climbing", 1),
    ];
    for (name, val) in codes {
        content.push_str(&format!("{}={}\n", name, val));
    }

    std::fs::write(&ini_path, content).context("Failed to write SADXModLoader.ini")?;
    tracing::info!("Configured SADX Mod Loader at {}", ini_path.display());

    configure_game(game_path)?;

    Ok(())
}

/// Create a default `sonic.ini` with recommended game settings.
pub fn configure_game(game_path: &Path) -> Result<()> {
    let system_dir = game_path.join("System");
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
}
