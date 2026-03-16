use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download, flatpak, protontricks};
use crate::steam::game::GameKind;

use super::{sa2, sadx};

const PROTONUP_QT_FLATPAK: &str = "net.davidotek.pupgui2";

/// GitHub release URL for SA Mod Manager (x64).
const SA_MOD_MANAGER_URL: &str =
    "https://github.com/X-Hax/SA-Mod-Manager/releases/latest/download/release_x64.zip";

/// Source for downloading a mod.
pub enum ModSource {
    GameBanana { file_id: u64 },
    DirectUrl { url: &'static str },
}

/// A recommended mod entry.
pub struct ModEntry {
    pub name: &'static str,
    pub source: ModSource,
    pub description: &'static str,
    pub before_image: Option<&'static str>,
    pub after_image: Option<&'static str>,
}

/// Resolve a `ModSource` to a download URL string.
pub fn resolve_download_url(source: &ModSource) -> String {
    match source {
        ModSource::GameBanana { file_id } => format!("https://gamebanana.com/dl/{file_id}"),
        ModSource::DirectUrl { url } => (*url).to_string(),
    }
}

/// Return the recommended mods list for a given game.
pub fn recommended_mods_for_game(kind: GameKind) -> &'static [ModEntry] {
    match kind {
        GameKind::SADX => sadx::RECOMMENDED_MODS,
        GameKind::SA2 => sa2::RECOMMENDED_MODS,
    }
}

/// Ensure protontricks is installed, installing it if needed.
pub async fn ensure_protontricks() -> Result<()> {
    if protontricks::is_available().await {
        tracing::info!("protontricks is available");
        return Ok(());
    }

    tracing::info!("Installing protontricks...");
    protontricks::install().await
}

/// Check if ProtonUp-Qt is available.
pub async fn is_protonup_available() -> bool {
    flatpak::is_flatpak_installed(PROTONUP_QT_FLATPAK).await
}

/// Install ProtonUp-Qt if not already installed.
pub async fn ensure_protonup() -> Result<()> {
    if is_protonup_available().await {
        return Ok(());
    }
    flatpak::install_flatpak(PROTONUP_QT_FLATPAK).await
}

/// Launch ProtonUp-Qt.
pub async fn launch_protonup() -> Result<()> {
    flatpak::launch_flatpak(PROTONUP_QT_FLATPAK, &[]).await
}

/// Install .NET Framework 4.8 for the given game's prefix.
pub async fn install_dotnet(app_id: u32) -> Result<()> {
    protontricks::install_dotnet(app_id).await
}

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

    // Replace the game's Steam launch executable with the mod manager so
    // Steam launches the mod manager, which then launches the real game exe.
    // SA2 uses Launcher.exe; SADX uses "Sonic Adventure DX.exe".
    let launcher = game_path.join("Launcher.exe");
    let sadx_exe = game_path.join("Sonic Adventure DX.exe");
    let steam_exe = if launcher.is_file() {
        Some(launcher)
    } else if sadx_exe.is_file() {
        Some(sadx_exe)
    } else {
        None
    };

    if let Some(steam_exe) = steam_exe {
        let bak = steam_exe.with_extension("exe.bak");
        if !bak.exists() {
            std::fs::rename(&steam_exe, &bak)
                .context(format!("Failed to backup {}", steam_exe.display()))?;
        }
        std::fs::rename(&dest_exe, &steam_exe).context(format!(
            "Failed to install mod manager as {}",
            steam_exe.display()
        ))?;
    }

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
    let url = resolve_download_url(&mod_entry.source);

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

    #[test]
    fn test_resolve_gamebanana_url() {
        let source = ModSource::GameBanana { file_id: 1388911 };
        assert_eq!(
            resolve_download_url(&source),
            "https://gamebanana.com/dl/1388911"
        );
    }

    #[test]
    fn test_resolve_direct_url() {
        let source = ModSource::DirectUrl {
            url: "https://example.com/mod.7z",
        };
        assert_eq!(
            resolve_download_url(&source),
            "https://example.com/mod.7z"
        );
    }

    #[test]
    fn test_sa_mod_manager_url_valid() {
        assert!(SA_MOD_MANAGER_URL.starts_with("https://github.com/"));
        assert!(SA_MOD_MANAGER_URL.contains("/releases/"));
        assert!(SA_MOD_MANAGER_URL.ends_with(".zip"));
    }

    #[test]
    fn test_install_mod_dir_construction() {
        let game_path = std::path::Path::new("/fake/game/dir");
        let mods_dir = game_path.join("mods");
        assert!(mods_dir.ends_with("mods"));
        assert_eq!(
            mods_dir,
            std::path::PathBuf::from("/fake/game/dir/mods")
        );
    }

    /// Helper: simulate the Steam exe replacement logic from `install_mod_manager`.
    /// Creates `SAModManager.exe` in the game dir and runs the replacement logic.
    fn run_exe_replacement(game_path: &std::path::Path) {
        // Create a fake SAModManager.exe (the "dest_exe" that install_mod_manager copies)
        let dest_exe = game_path.join("SAModManager.exe");
        std::fs::write(&dest_exe, b"mod_manager_content").unwrap();

        let launcher = game_path.join("Launcher.exe");
        let sadx_exe = game_path.join("Sonic Adventure DX.exe");
        let steam_exe = if launcher.is_file() {
            Some(launcher)
        } else if sadx_exe.is_file() {
            Some(sadx_exe)
        } else {
            None
        };

        if let Some(steam_exe) = steam_exe {
            let bak = steam_exe.with_extension("exe.bak");
            if !bak.exists() {
                std::fs::rename(&steam_exe, &bak).unwrap();
            }
            std::fs::rename(&dest_exe, &steam_exe).unwrap();
        }
    }

    #[test]
    fn test_exe_replacement_sa2_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        std::fs::write(game_path.join("Launcher.exe"), b"original_launcher").unwrap();

        run_exe_replacement(game_path);

        // Launcher.exe should now contain the mod manager
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe")).unwrap(),
            b"mod_manager_content"
        );
        // Original backed up
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
            b"original_launcher"
        );
        // SAModManager.exe should have been renamed away
        assert!(!game_path.join("SAModManager.exe").exists());
    }

    #[test]
    fn test_exe_replacement_sadx() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        std::fs::write(
            game_path.join("Sonic Adventure DX.exe"),
            b"original_sadx",
        )
        .unwrap();

        run_exe_replacement(game_path);

        // "Sonic Adventure DX.exe" should now contain the mod manager
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe")).unwrap(),
            b"mod_manager_content"
        );
        // Original backed up
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe.bak")).unwrap(),
            b"original_sadx"
        );
        assert!(!game_path.join("SAModManager.exe").exists());
    }

    #[test]
    fn test_exe_replacement_sadx_backup_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        // Simulate a prior backup already existing
        std::fs::write(
            game_path.join("Sonic Adventure DX.exe.bak"),
            b"first_backup",
        )
        .unwrap();
        std::fs::write(
            game_path.join("Sonic Adventure DX.exe"),
            b"already_replaced",
        )
        .unwrap();

        run_exe_replacement(game_path);

        // The original backup should be preserved (not overwritten)
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe.bak")).unwrap(),
            b"first_backup"
        );
    }

    #[test]
    fn test_exe_replacement_sa2_backup_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        std::fs::write(game_path.join("Launcher.exe.bak"), b"first_backup").unwrap();
        std::fs::write(game_path.join("Launcher.exe"), b"already_replaced").unwrap();

        run_exe_replacement(game_path);

        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
            b"first_backup"
        );
    }

    #[test]
    fn test_exe_replacement_no_steam_exe() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        // No Launcher.exe or Sonic Adventure DX.exe — mod manager stays as-is

        run_exe_replacement(game_path);

        // SAModManager.exe should remain in place
        assert_eq!(
            std::fs::read(game_path.join("SAModManager.exe")).unwrap(),
            b"mod_manager_content"
        );
        assert!(!game_path.join("Launcher.exe").exists());
        assert!(!game_path.join("Sonic Adventure DX.exe").exists());
    }

    #[test]
    fn test_exe_replacement_launcher_takes_priority_over_sadx() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        // Both exist — Launcher.exe should win (SA2 path)
        std::fs::write(game_path.join("Launcher.exe"), b"launcher").unwrap();
        std::fs::write(
            game_path.join("Sonic Adventure DX.exe"),
            b"sadx",
        )
        .unwrap();

        run_exe_replacement(game_path);

        // Launcher.exe replaced
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe")).unwrap(),
            b"mod_manager_content"
        );
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
            b"launcher"
        );
        // SADX exe untouched
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe")).unwrap(),
            b"sadx"
        );
    }

    #[test]
    fn test_recommended_mods_for_game_returns_correct_lists() {
        let sadx_mods = recommended_mods_for_game(GameKind::SADX);
        let sa2_mods = recommended_mods_for_game(GameKind::SA2);
        assert!(!sadx_mods.is_empty());
        assert!(!sa2_mods.is_empty());
        assert_ne!(sadx_mods.len(), sa2_mods.len());
    }
}
