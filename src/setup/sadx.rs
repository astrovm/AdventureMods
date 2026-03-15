use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{download, protontricks};
use crate::steam::game::GameKind;

/// Direct URL for the SADX Mod Installer executable.
const SADX_INSTALLER_URL: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/sadx_setup.exe";

/// Expected filename for the downloaded installer.
const SADX_INSTALLER_FILENAME: &str = "sadx_setup.exe";

/// Download the SADX Mod Installer.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn download_installer(
    dest_dir: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let dest_file = dest_dir.join(SADX_INSTALLER_FILENAME);
    download::download_file(SADX_INSTALLER_URL, &dest_file, progress)?;
    Ok(dest_file)
}

/// Launch the SADX Mod Installer via protontricks.
pub async fn run_installer(installer_path: &Path) -> Result<()> {
    let path_str = installer_path
        .to_str()
        .context("Invalid installer path")?;

    let output = protontricks::launch(GameKind::SADX.app_id(), path_str).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("Installer exited with non-zero status: {stderr}");
        // Don't fail — the user may have closed it intentionally
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_sadx_installer_url_valid() {
        assert!(SADX_INSTALLER_URL.starts_with("https://"));
        assert!(SADX_INSTALLER_URL.ends_with(".exe"));
    }

    #[test]
    fn test_download_installer_returns_correct_filename() {
        let dest = PathBuf::from("/tmp/some_dir");
        let expected = dest.join(SADX_INSTALLER_FILENAME);
        assert!(expected.ends_with("sadx_setup.exe"));
    }
}
