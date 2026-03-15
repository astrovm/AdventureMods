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
        ModSource::GameBanana { file_id } => format!("https://gamebanana.com/mmdl/{file_id}"),
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

    // SA2 has a Launcher.exe — replace it with the mod manager so Steam
    // launches the mod manager instead. SADX uses sonic.exe directly and
    // has no launcher, so we just leave SAModManager.exe in place.
    let launcher = game_path.join("Launcher.exe");
    if launcher.is_file() {
        let launcher_bak = game_path.join("Launcher.exe.bak");
        if !launcher_bak.exists() {
            std::fs::rename(&launcher, &launcher_bak)
                .context("Failed to backup Launcher.exe")?;
        }
        std::fs::rename(&dest_exe, &launcher)
            .context("Failed to install mod manager as Launcher.exe")?;
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
            "https://gamebanana.com/mmdl/1388911"
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

    #[test]
    fn test_recommended_mods_for_game_returns_correct_lists() {
        let sadx_mods = recommended_mods_for_game(GameKind::SADX);
        let sa2_mods = recommended_mods_for_game(GameKind::SA2);
        assert!(!sadx_mods.is_empty());
        assert!(!sa2_mods.is_empty());
        assert_ne!(sadx_mods.len(), sa2_mods.len());
    }
}
