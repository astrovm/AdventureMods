use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download};

/// GitHub release URL for SA Mod Manager (x64).
const SA_MOD_MANAGER_URL: &str =
    "https://github.com/X-Hax/SA-Mod-Manager/releases/latest/download/release_x64.zip";

/// Recommended SA2 mods with their GameBanana mmdl file IDs.
pub struct ModEntry {
    pub name: &'static str,
    pub file_id: u64,
    pub description: &'static str,
}

/// Download URL for a GameBanana mod file.
fn gamebanana_download_url(file_id: u64) -> String {
    format!("https://gamebanana.com/mmdl/{file_id}")
}

pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "SA2 Render Fix",
        file_id: 1388911,
        description: "Fixes various rendering issues",
    },
    ModEntry {
        name: "Cutscene Revamp",
        file_id: 440737,
        description: "Improved in-game cutscenes",
    },
    ModEntry {
        name: "Retranslated Story -COMPLETE-",
        file_id: 1388469,
        description: "Accurate retranslation of the story",
    },
    ModEntry {
        name: "HD GUI: SA2 Edition",
        file_id: 409120,
        description: "High-definition GUI textures",
    },
    ModEntry {
        name: "IMPRESSive",
        file_id: 1213103,
        description: "Visual enhancements and effects",
    },
    ModEntry {
        name: "Stage Atmosphere Tweaks",
        file_id: 884395,
        description: "Improved stage lighting and atmosphere",
    },
    ModEntry {
        name: "SA2 Volume Controls",
        file_id: 835829,
        description: "Adds proper volume control options",
    },
    ModEntry {
        name: "Mech Sound Improvement",
        file_id: 893090,
        description: "Better mech stage sound effects",
    },
    ModEntry {
        name: "SA2 Input Controls",
        file_id: 1255952,
        description: "Fixes input issues with modern controllers",
    },
    ModEntry {
        name: "Better Radar",
        file_id: 860716,
        description: "Improved treasure hunting radar",
    },
    ModEntry {
        name: "HedgePanel - Sonic + Shadow Tweaks",
        file_id: 454296,
        description: "Gameplay tweaks for Sonic and Shadow",
    },
    ModEntry {
        name: "Sonic: New Tricks",
        file_id: 915082,
        description: "Additional moves for Sonic",
    },
    ModEntry {
        name: "Retranslated Hints",
        file_id: 1388468,
        description: "Accurate retranslation of hint messages",
    },
];

/// Download and install SA Mod Manager into the game directory.
///
/// Downloads from GitHub, extracts, and replaces Launcher.exe with
/// SAModManager.exe (backing up the original).
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_mod_manager(
    game_path: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("SAModManager.zip");

    download::download_file(SA_MOD_MANAGER_URL, &archive_path, progress)?;

    // Extract to a temp subdirectory
    let extract_dir = temp_dir.path().join("extracted");
    archive::extract(&archive_path, &extract_dir)?;

    // Copy SAModManager.exe to game directory
    let manager_exe = extract_dir.join("SAModManager.exe");
    if !manager_exe.is_file() {
        anyhow::bail!("SAModManager.exe not found in release archive");
    }

    let dest_exe = game_path.join("SAModManager.exe");
    std::fs::copy(&manager_exe, &dest_exe)
        .context("Failed to copy SAModManager.exe to game directory")?;

    // Backup original Launcher.exe and replace with mod manager
    let launcher = game_path.join("Launcher.exe");
    let launcher_bak = game_path.join("Launcher.exe.bak");
    if launcher.is_file() && !launcher_bak.exists() {
        std::fs::rename(&launcher, &launcher_bak)
            .context("Failed to backup Launcher.exe")?;
    }
    std::fs::rename(&dest_exe, &launcher)
        .context("Failed to install mod manager as Launcher.exe")?;

    tracing::info!("SA Mod Manager installed to {}", game_path.display());
    Ok(())
}

/// Download and install a single mod into the game's mods directory.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_mod(
    game_path: &Path,
    mod_entry: &ModEntry,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let url = gamebanana_download_url(mod_entry.file_id);

    let mods_dir = game_path.join("mods");
    std::fs::create_dir_all(&mods_dir)?;

    let temp_dir = tempfile::tempdir()?;

    // Download — the mmdl endpoint redirects, and the filename comes from
    // the Content-Disposition header. We just save to a generic name.
    let archive_path = temp_dir.path().join("mod_download");
    download::download_file(&url, &archive_path, progress)?;

    // Extract directly into the mods directory
    archive::extract(&archive_path, &mods_dir)?;

    tracing::info!("Installed mod: {}", mod_entry.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_recommended_mods_count() {
        assert_eq!(RECOMMENDED_MODS.len(), 13);
    }

    #[test]
    fn test_mod_file_ids_nonzero() {
        for m in RECOMMENDED_MODS {
            assert!(m.file_id > 0, "Mod '{}' has zero file_id", m.name);
        }
    }

    #[test]
    fn test_mod_file_ids_unique() {
        let ids: HashSet<u64> = RECOMMENDED_MODS.iter().map(|m| m.file_id).collect();
        assert_eq!(
            ids.len(),
            RECOMMENDED_MODS.len(),
            "Duplicate file_ids in RECOMMENDED_MODS"
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
    fn test_gamebanana_download_url() {
        assert_eq!(
            gamebanana_download_url(1388911),
            "https://gamebanana.com/mmdl/1388911"
        );
    }
}
