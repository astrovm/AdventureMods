use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download};

#[cfg(test)]
use super::common::ModSource;
pub use super::sadx_catalog::{PRESETS, RECOMMENDED_MODS};

/// Direct URL for the Steam-to-2004 conversion tools archive.
const STEAM_TOOLS_URL: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/steam_tools.7z";

fn steam_tools_url() -> String {
    super::common::env_or_default("ADVENTURE_MODS_URL_SADX_STEAM_TOOLS", STEAM_TOOLS_URL)
}

fn hpatchz_program() -> std::path::PathBuf {
    std::env::var_os("ADVENTURE_MODS_HPATCHZ")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("hpatchz"))
}

/// Base URL for mods hosted on dcmods.unreliable.network.
#[cfg(test)]
const DCMODS_BASE: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/";

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
    // Skip if already converted. Check multiple markers since previous setups
    // (including the official Windows installer) leave different traces.
    // Use case-insensitive lookup so this works whether Steam extracted to
    // system/ or System/ on a case-sensitive Linux filesystem.
    if super::common::sadx_data_dir(game_path)
        .and_then(|dir| super::common::find_file_icase(&dir, "CHRMODELS_orig.dll"))
        .is_some()
    {
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

    let steam_tools_url = steam_tools_url();
    download::download_file(&steam_tools_url, &archive_path, progress)?;

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

    let hpatchz = hpatchz_program();
    let output = std::process::Command::new(&hpatchz)
        .arg("-f")
        .arg(&game_str)
        .arg(&patch_str)
        .arg(&out_str)
        .output()
        .with_context(|| {
            format!(
                "Failed to run {}. Is HDiffPatch installed?",
                hpatchz.display()
            )
        })?;

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

    // hpatchz writes a complete output tree rather than patching in place.
    tracing::info!("Patch applied successfully to temp dir, moving files back...");

    super::common::move_dir_contents(&out_dir, game_path)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    crate::recommended_mods_tests!(29);

    #[test]
    fn test_dcmods_base_url_valid() {
        assert!(DCMODS_BASE.starts_with("https://"));
        assert!(DCMODS_BASE.ends_with('/'));
    }

    #[test]
    fn test_dcmods_urls_use_correct_base() {
        for m in RECOMMENDED_MODS {
            if let ModSource::DirectUrl { url } = &m.source
                && url.contains("dcmods.unreliable.network")
            {
                assert!(
                    url.starts_with(DCMODS_BASE),
                    "Mod '{}' dcmods URL doesn't start with DCMODS_BASE: {}",
                    m.name,
                    url
                );
            }
        }
    }

    #[test]
    fn test_sonic_new_tricks_uses_sadx_image_set() {
        let new_tricks = RECOMMENDED_MODS
            .iter()
            .find(|m| m.name == "Sonic: New Tricks")
            .expect("Sonic: New Tricks entry missing");

        assert_eq!(new_tricks.pictures.len(), 8);
        for picture in new_tricks.pictures {
            assert!(
                picture.starts_with(
                    "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_"
                ),
                "Unexpected SADX New Tricks picture path: {}",
                picture
            );
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

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

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

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

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

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

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

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

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
        std::fs::create_dir_all(tmp.path().join("system")).unwrap();
        std::fs::write(tmp.path().join("system/CHRMODELS_orig.dll"), "dummy").unwrap();

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
