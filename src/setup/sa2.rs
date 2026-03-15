use std::path::Path;

use anyhow::{Context, Result};
use tempfile;

use crate::external::{archive, download};

/// SA Mod Manager GameBanana file ID.
const SA_MOD_MANAGER_FILE_ID: u64 = 1098414;

/// Recommended SA2 mods with their GameBanana file IDs.
pub struct ModEntry {
    pub name: &'static str,
    pub file_id: u64,
    pub description: &'static str,
}

pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "SA2 Input Controls Fix",
        file_id: 862057,
        description: "Fixes input issues with modern controllers",
    },
    ModEntry {
        name: "SA2 Volume Controls",
        file_id: 1098269,
        description: "Adds proper volume control options",
    },
    ModEntry {
        name: "SA2 Fixes",
        file_id: 1045498,
        description: "Various bug fixes and improvements",
    },
    ModEntry {
        name: "Character Select Plus",
        file_id: 1023625,
        description: "Enhanced character selection screen",
    },
    ModEntry {
        name: "SA2 Render Fix",
        file_id: 996892,
        description: "Fixes various rendering issues",
    },
    ModEntry {
        name: "SA2 Chao World Extended",
        file_id: 693344,
        description: "Extends Chao World with new features",
    },
    ModEntry {
        name: "SA2 HD GUI",
        file_id: 798498,
        description: "High-definition GUI textures",
    },
    ModEntry {
        name: "SA2 Battle Network",
        file_id: 1112698,
        description: "Online multiplayer support",
    },
    ModEntry {
        name: "Results Screen Enhancement",
        file_id: 982072,
        description: "Improved results screen",
    },
    ModEntry {
        name: "SA2 Camera",
        file_id: 862055,
        description: "Improved camera controls",
    },
    ModEntry {
        name: "SA2 Physics Swap",
        file_id: 862056,
        description: "Adventure-style physics option",
    },
    ModEntry {
        name: "SA2 Cutscene Revamp",
        file_id: 862058,
        description: "Improved in-game cutscenes",
    },
    ModEntry {
        name: "SA2 60FPS",
        file_id: 1100400,
        description: "Stable 60 FPS gameplay",
    },
];

/// Download and install SA Mod Manager into the game directory.
pub async fn install_mod_manager(
    game_path: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let (url, filename) = download::resolve_gamebanana_url(SA_MOD_MANAGER_FILE_ID)
        .await
        .context("Failed to resolve SA Mod Manager download URL")?;

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join(&filename);

    download::download_file(&url, &archive_path, progress).await?;

    // Extract to game directory
    archive::extract(&archive_path, game_path).await?;

    tracing::info!("SA Mod Manager installed to {}", game_path.display());
    Ok(())
}

/// Download and install a single mod into the game's mods directory.
pub async fn install_mod(
    game_path: &Path,
    mod_entry: &ModEntry,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let (url, filename) = download::resolve_gamebanana_url(mod_entry.file_id)
        .await
        .with_context(|| {
            format!(
                "Failed to resolve download URL for {}",
                mod_entry.name
            )
        })?;

    let mods_dir = game_path.join("mods");
    std::fs::create_dir_all(&mods_dir)?;

    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join(&filename);

    download::download_file(&url, &archive_path, progress).await?;

    // Extract into a folder named after the mod
    let mod_dir = mods_dir.join(mod_entry.name.replace(' ', "_"));
    archive::extract(&archive_path, &mod_dir).await?;

    tracing::info!("Installed mod: {}", mod_entry.name);
    Ok(())
}
