use std::path::Path;

use anyhow::{Context, Result};

/// Extract an archive using 7z (supports .7z, .zip, .rar, .tar.*, etc.).
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory {}", dest.display()))?;

    let dest_arg = format!("-o{}", dest.display());
    let output = std::process::Command::new("7z")
        .arg("x")
        .arg("-y")
        .arg(&dest_arg)
        .arg(archive)
        .output()
        .context("Failed to run 7z. Is p7zip installed?")?;

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
