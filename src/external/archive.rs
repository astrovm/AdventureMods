use std::path::Path;

use anyhow::{Context, Result};
use gtk::gio;

/// Extract an archive using 7z (supports .7z, .zip, .rar, .tar.*, etc.).
pub async fn extract(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let archive = archive_path.to_path_buf();
    let dest = dest_dir.to_path_buf();

    gio::spawn_blocking(move || extract_sync(&archive, &dest))
        .await
        .map_err(|e| anyhow::anyhow!("spawn error: {e:?}"))?
}

fn extract_sync(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory {}", dest.display()))?;

    let dest_arg = format!("-o{}", dest.display());
    let output = std::process::Command::new("7z")
        .arg("x")
        .arg("-y")
        .arg(&dest_arg)
        .arg(archive)
        .output()
        .context("Failed to run 7z — is p7zip installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "Archive extraction failed for {}:\n{}\n{}",
            archive.display(),
            stdout,
            stderr,
        );
    }

    Ok(())
}
