use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download, protontricks};
use crate::steam::game::GameKind;

/// GameBanana file ID for the SADX Mod Installer.
const SADX_INSTALLER_FILE_ID: u64 = 1035580;

/// Download the SADX Mod Installer.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn download_installer(
    dest_dir: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<std::path::PathBuf> {
    let (url, filename) = download::resolve_gamebanana_url(SADX_INSTALLER_FILE_ID)
        .context("Failed to resolve SADX installer download URL")?;

    let dest_file = dest_dir.join(&filename);
    download::download_file(&url, &dest_file, progress)?;

    // If it's an archive, extract it
    let ext = dest_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if matches!(ext, "7z" | "zip" | "rar") {
        let extract_dir = dest_dir.join("sadx_installer");
        archive::extract(&dest_file, &extract_dir)?;
        // Find the .exe in the extracted files
        if let Some(exe) = find_exe_in_dir(&extract_dir) {
            return Ok(exe);
        }
        anyhow::bail!("Could not find installer executable in extracted archive");
    }

    Ok(dest_file)
}

fn find_exe_in_dir(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("exe") {
                    return Some(path);
                }
            }
        }
        if path.is_dir() {
            if let Some(found) = find_exe_in_dir(&path) {
                return Some(found);
            }
        }
    }
    None
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

    #[test]
    fn test_find_exe_flat() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("setup.exe"), b"fake").unwrap();
        let result = find_exe_in_dir(tmp.path());
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("setup.exe"));
    }

    #[test]
    fn test_find_exe_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("installer.exe"), b"fake").unwrap();
        let result = find_exe_in_dir(tmp.path());
        assert!(result.is_some());
        assert!(result
            .unwrap()
            .to_str()
            .unwrap()
            .contains("installer.exe"));
    }

    #[test]
    fn test_find_exe_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Setup.EXE"), b"fake").unwrap();
        let result = find_exe_in_dir(tmp.path());
        assert!(result.is_some());
    }

    #[test]
    fn test_find_exe_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_exe_in_dir(tmp.path()).is_none());
    }
}
