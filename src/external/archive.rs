use std::path::Path;

use anyhow::{Context, Result};
use gtk::gio;

/// Extract an archive using 7z or unrar depending on the file extension.
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

    let ext = archive
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let output = match ext.as_str() {
        "rar" => std::process::Command::new("unrar")
            .arg("x")
            .arg("-o+")
            .arg(archive)
            .arg(dest)
            .output()
            .context("Failed to run unrar")?,
        _ => {
            // Use 7z for everything else (.7z, .zip, etc.)
            let dest_arg = format!("-o{}", dest.display());
            std::process::Command::new("7z")
                .arg("x")
                .arg("-y")
                .arg(&dest_arg)
                .arg(archive)
                .output()
                .context("Failed to run 7z")?
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Archive extraction failed for {}: {}",
            archive.display(),
            stderr
        );
    }

    Ok(())
}
