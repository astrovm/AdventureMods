use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download};

use super::common::{ModEntry, ModSource};

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
        dir_name: Some("ADXAudio"),
    },
    ModEntry {
        name: "SADX: Fixed Edition",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXFE.7z",
        },
        description: "Comprehensive bug fix and restoration mod",
        before_image: None,
        after_image: None,
        dir_name: Some("SADXFE"),
    },
    ModEntry {
        name: "Smooth Camera",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/smooth-cam.7z",
        },
        description: "Smoother camera movement",
        before_image: None,
        after_image: None,
        dir_name: Some("smooth-cam"),
    },
    ModEntry {
        name: "Pause Hide",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/pause-hide.7z",
        },
        description: "Hide HUD when pausing for screenshots",
        before_image: None,
        after_image: None,
        dir_name: Some("pause-hide"),
    },
    ModEntry {
        name: "Frame Limit",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-frame-limit.7z",
        },
        description: "Accurate frame rate limiter",
        before_image: None,
        after_image: None,
        dir_name: Some("sadx-frame-limit"),
    },
    ModEntry {
        name: "Onion Blur",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-onion-blur.7z",
        },
        description: "Dreamcast-style motion blur effect",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/onion_blur_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/onion_blur_after.jpg"),
        dir_name: Some("sadx-onion-blur"),
    },
    ModEntry {
        name: "Idle Chatter",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/idle-chatter.7z",
        },
        description: "Restores character idle voice lines",
        before_image: None,
        after_image: None,
        dir_name: Some("idle-chatter"),
    },
    ModEntry {
        name: "Steam Achievements",
        source: ModSource::DirectUrl {
            url: "https://mm.reimuhakurei.net/sadxmods/SteamAchievements.7z",
        },
        description: "Enables Steam achievements with mods",
        before_image: None,
        after_image: None,
        dir_name: Some("SteamAchievements"),
    },
    ModEntry {
        name: "Lantern Engine",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-dc-lighting.7z",
        },
        description: "Dreamcast-accurate lighting engine",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/lantern_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/lantern_after.jpg"),
        dir_name: Some("sadx-dc-lighting"),
    },
    ModEntry {
        name: "Dreamcast Conversion",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z",
        },
        description: "Restores Dreamcast visuals and features",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_conv_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_conv_after.jpg"),
        dir_name: Some("DreamcastConversion"),
    },
    ModEntry {
        name: "Dreamcast DLC",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DLCs.7z",
        },
        description: "Restored Dreamcast downloadable content",
        before_image: None,
        after_image: None,
        dir_name: Some("DLC"),
    },
    ModEntry {
        name: "SADX Style Water",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-style-water.7z",
        },
        description: "Improved water rendering",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/water_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/water_after.jpg"),
        dir_name: Some("sadx-style-water"),
    },
    ModEntry {
        name: "Sound Overhaul",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SoundOverhaul.7z",
        },
        description: "Restored and improved sound effects",
        before_image: None,
        after_image: None,
        dir_name: Some("SoundOverhaul"),
    },
    ModEntry {
        name: "Time of Day",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/TrainDaytime.7z",
        },
        description: "Dynamic time-of-day lighting changes",
        before_image: None,
        after_image: None,
        dir_name: Some("TrainDaytime"),
    },
    ModEntry {
        name: "Dreamcast Characters Pack",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SA1_Chars.7z",
        },
        description: "Original Dreamcast character models",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_chars_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/dc_chars_after.jpg"),
        dir_name: Some("SA1_Chars"),
    },
    ModEntry {
        name: "Super Sonic",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-super-sonic.7z",
        },
        description: "Playable Super Sonic in action stages",
        before_image: None,
        after_image: None,
        dir_name: Some("sadx-super-sonic"),
    },
    ModEntry {
        name: "HD GUI 2",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/HD_DCStyle.7z",
        },
        description: "High resolution GUI textures for menus, HUD and icons",
        before_image: Some("/io/github/astrovm/AdventureMods/resources/images/hd_gui_before.jpg"),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/hd_gui_after.jpg"),
        dir_name: Some("HD_DCStyle"),
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
/// - `SoundData/VOICE_JP` → `SoundData/voice_jp`
/// - `SoundData/VOICE_US` → `SoundData/voice_us`
/// - `SoundData/SE` → `SoundData/se`
/// - `SoundData/*/WMA` → `SoundData/*/wma`
fn normalize_case_for_patch(game_path: &Path) -> Result<()> {
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

    let sound_data = game_path.join("SoundData");
    if sound_data.is_dir() {
        rename_if_needed(&sound_data.join("VOICE_JP"), &sound_data.join("voice_jp"))?;
        rename_if_needed(&sound_data.join("VOICE_US"), &sound_data.join("voice_us"))?;
        rename_if_needed(&sound_data.join("SE"), &sound_data.join("se"))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_recommended_mods_count() {
        assert_eq!(RECOMMENDED_MODS.len(), 17);
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
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("System")).unwrap();
        std::fs::write(tmp.path().join("System/CHRMODELS_orig.dll"), "dummy").unwrap();

        // Should return Ok without needing hpatchz or downloads
        convert_steam_to_2004(tmp.path(), None).unwrap();
    }

    #[test]
    fn test_convert_skips_if_sadxmodloader_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("SADXModLoader.dll"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }

    #[test]
    fn test_convert_skips_if_sonic_exe_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("sonic.exe"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }
}
