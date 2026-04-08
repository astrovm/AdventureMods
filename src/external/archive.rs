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

#[cfg(test)]
mod tests {
    fn assert_manifest_installs_7z_shared_object(manifest: &str) {
        assert!(manifest.contains("install -Dm755 bin/7z /app/bin/7z"));
        assert!(manifest.contains("install -Dm755 bin/7z.so /app/bin/7z.so"));
    }

    #[test]
    fn flatpak_manifest_installs_7z_shared_object() {
        let manifest = include_str!("../../build-aux/io.github.astrovm.AdventureMods.json");
        assert_manifest_installs_7z_shared_object(manifest);
    }

    #[test]
    fn flatpak_devel_manifest_installs_7z_shared_object() {
        let manifest = include_str!("../../build-aux/io.github.astrovm.AdventureMods.Devel.json");
        assert_manifest_installs_7z_shared_object(manifest);
    }
}
