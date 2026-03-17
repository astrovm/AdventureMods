use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download};

use super::common::{ModEntry, ModSource};
use super::config;

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
        description: "Comprehensive bug fix mod for the PC port",
        before_image: None,
        after_image: None,
        dir_name: Some("SADXFE"),
    },
    ModEntry {
        name: "Smooth Camera",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/smooth-cam.7z",
        },
        description: "Smooth first-person camera (analog instead of 8-directional)",
        before_image: None,
        after_image: None,
        dir_name: Some("smooth-cam"),
    },
    ModEntry {
        name: "Pause Hide",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/pause-hide.7z",
        },
        description: "Press X+Y to hide the pause menu for screenshots",
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
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/onion_blur_before.jpg",
        ),
        after_image: Some("/io/github/astrovm/AdventureMods/resources/images/onion_blur_after.jpg"),
        dir_name: Some("sadx-onion-blur"),
    },
    ModEntry {
        name: "Idle Chatter",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/idle-chatter.7z",
        },
        description: "Press a button to hear character commentary about the current stage",
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
        description: "Restores the ocean wave effect in Emerald Coast",
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
        description: "Change the time of day by taking the train after beating the story",
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
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/super_sonic_showcase.jpg",
        ),
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
    ModEntry {
        name: "Sonic Adventure Retranslated",
        source: ModSource::GameBanana { file_id: 384650 },
        description: "Retranslation of the game script from the original Japanese",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/sa_retranslated_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("Sonic Adventure Retranslated"),
    },
    ModEntry {
        name: "HUD Plus",
        source: ModSource::GameBanana { file_id: 1309612 },
        description: "Various HUD improvements",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/hud_plus_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("sadx-hud-plus"),
    },
    ModEntry {
        name: "AI HD Textures",
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/AI_textures/releases/latest/download/AI_HD_Textures.7z",
        },
        description: "AI upscaled textures for both vanilla and Dreamcast Conversion assets",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/ai_hd_textures_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("AI_HD_Textures"),
    },
    ModEntry {
        name: "AI HD FMVs",
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/sadx-hd-videos/releases/latest/download/AI_HD_FMVs.7z",
        },
        description: "HD upscaled video cutscenes",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/ai_hd_fmvs_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("AI_HD_FMVs"),
    },
    ModEntry {
        name: "Active Mouths",
        source: ModSource::GameBanana { file_id: 622235 },
        description: "Adds character face animations outside cutscenes",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/active_mouths_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("Active Mouths"),
    },
    ModEntry {
        name: "New Tricks",
        source: ModSource::GameBanana { file_id: 1102800 },
        description: "Adds new character abilities like drop dash and air dash",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/new_tricks_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("sadx-new-tricks"),
    },
    ModEntry {
        name: "Better Tails AI",
        source: ModSource::GameBanana { file_id: 1148657 },
        description: "Makes Tails follow Sonic in all areas with improved AI behavior",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/better_tails_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("Better Tails AI"),
    },
    ModEntry {
        name: "DX Characters Refined",
        source: ModSource::GameBanana { file_id: 1498662 },
        description: "Various improvements for SADX character models",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/dx_chars_refined_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("DX Characters Refined"),
    },
    ModEntry {
        name: "Multiplayer",
        source: ModSource::GameBanana { file_id: 1046512 },
        description: "Split screen coop and battle multiplayer for trial stages",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/multiplayer_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("sadx-multiplayer"),
    },
    ModEntry {
        name: "Chao Gameplay",
        source: ModSource::GameBanana { file_id: 781777 },
        description: "Makes Chao follow you in action stages",
        before_image: Some(
            "/io/github/astrovm/AdventureMods/resources/images/chao_gameplay_showcase.jpg",
        ),
        after_image: None,
        dir_name: Some("sadx-chao-gameplay"),
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
    let system_dir = config::system_dir(game_path);

    // Skip if already converted. Check multiple markers since previous setups
    // (including the official Windows installer) leave different traces.
    let chrmodels_orig = system_dir.join("CHRMODELS_orig.dll");
    if chrmodels_orig.exists() {
        tracing::info!("Game appears already converted (CHRMODELS_orig.dll exists), skipping");
        return Ok(());
    }

    if game_path.join("mods/.modloader/SADXModLoader.dll").exists() {
        tracing::info!(
            "Game appears already converted (SADXModLoader.dll exists in .modloader), skipping"
        );
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

    let game_str = game_path
        .to_string_lossy()
        .trim_end_matches('/')
        .to_string();
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
            tracing::error!(
                "Source file mismatch detected. This usually happens if the game is already modded or corrupted."
            );
            anyhow::bail!(
                "Steam-to-2004 conversion failed. Your game installation might be modified or corrupted. Please verify game integrity in Steam and try again.\n\nDetails:\n{stderr}"
            );
        }

        anyhow::bail!("Steam-to-2004 conversion failed:\n{stdout}\n{stderr}");
    }

    // Success! Now we move the patched files back to the game directory.
    // hpatchz in directory mode produces a new directory with the patched files.
    // We want to merge/overwrite these into the game directory.
    tracing::info!("Patch applied successfully to temp dir, moving files back...");

    // Move files from out_dir to game_path
    move_dir_contents(&out_dir, game_path)?;

    tracing::info!("Steam-to-2004 conversion complete");
    Ok(())
}

/// Rename directories to match the casing the hpatchz patch expects.
///
/// The hpatchz directory patch was created on a case-insensitive Windows filesystem.
/// On case-sensitive Linux filesystems, hpatchz will fail to find "old" files to patch
/// if their casing doesn't exactly match the manifest (which is often lowercase).
/// Steam on Linux may extract directories with different casing (e.g. VOICE_JP instead of voice_jp).
fn normalize_case_for_patch(game_path: &Path) -> Result<()> {
    // List of known directory casing mismatches between Steam and the patch manifest.
    let renames = [
        ("SoundData/VOICE_JP", "SoundData/voice_jp"),
        ("SoundData/VOICE_US", "SoundData/voice_us"),
        ("SoundData/SE", "SoundData/se"),
        ("SoundData/voice_jp/WMA", "SoundData/voice_jp/wma"),
        ("SoundData/voice_us/WMA", "SoundData/voice_us/wma"),
    ];

    for (old, new) in renames {
        let old_path = game_path.join(old);
        let new_path = game_path.join(new);

        if old_path.is_dir() && !new_path.exists() {
            std::fs::rename(&old_path, &new_path).with_context(|| {
                format!(
                    "Failed to rename {} → {}",
                    old_path.display(),
                    new_path.display()
                )
            })?;
            tracing::info!("Renamed {} → {} for patch compatibility", old, new);
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
        assert_eq!(RECOMMENDED_MODS.len(), 27);
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
            assert!(!m.name.contains('/'), "Mod name '{}' contains '/'", m.name);
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
        assert_eq!(
            std::fs::read_to_string(dst.join("sub/new.txt")).unwrap(),
            "new"
        );
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
        let loader_dir = tmp.path().join("mods/.modloader");
        std::fs::create_dir_all(&loader_dir).unwrap();
        std::fs::write(loader_dir.join("SADXModLoader.dll"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }

    #[test]
    fn test_convert_skips_if_sonic_exe_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("sonic.exe"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }
}
