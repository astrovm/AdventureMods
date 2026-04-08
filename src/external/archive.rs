use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Extract an archive using 7z (supports .7z, .zip, .rar, .tar.*, etc.).
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory {}", dest.display()))?;

    let dest_arg = format!("-o{}", dest.display());
    let output = std::process::Command::new(resolve_program("7z"))
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

fn resolve_program(program: &str) -> PathBuf {
    resolve_program_with_search_path(program, std::env::var_os("PATH").as_deref())
}

fn resolve_program_with_search_path(program: &str, search_path: Option<&OsStr>) -> PathBuf {
    let program_path = Path::new(program);
    if program_path.is_absolute() || program.contains(std::path::MAIN_SEPARATOR) {
        return program_path.to_path_buf();
    }

    search_path
        .into_iter()
        .flat_map(std::env::split_paths)
        .find_map(|dir| {
            let candidate = dir.join(program);
            candidate
                .is_file()
                .then(|| candidate.canonicalize().unwrap_or(candidate))
        })
        .unwrap_or_else(|| program_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_manifest_installs_7z_shared_object(manifest: &str) {
        assert!(manifest.contains("install -Dm755 bin/7z /app/bin/7z"));
        assert!(manifest.contains("install -Dm755 bin/7z.so /app/bin/7z.so"));
    }

    fn assert_appimage_build_installs_7z_shared_object(script: &str) {
        assert!(script.contains(
            "install -Dm755 \"$BUILD_DIR/tmp/p7zip-17.05/bin/7z\" \"$APPDIR/usr/bin/7z\""
        ));
        assert!(script.contains(
            "install -Dm755 \"$BUILD_DIR/tmp/p7zip-17.05/bin/7z.so\" \"$APPDIR/usr/bin/7z.so\""
        ));
    }

    #[test]
    fn resolve_program_with_search_path_returns_absolute_match() {
        let temp = tempfile::tempdir().unwrap();
        let bin_dir = temp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let fake_7z = bin_dir.join("7z");
        std::fs::write(&fake_7z, b"#!/bin/sh\nexit 0\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&fake_7z).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&fake_7z, permissions).unwrap();
        }

        let search_path = std::env::join_paths([&bin_dir]).unwrap();
        assert_eq!(
            resolve_program_with_search_path("7z", Some(search_path.as_os_str())),
            fake_7z.canonicalize().unwrap()
        );
    }

    #[test]
    fn resolve_program_with_search_path_keeps_absolute_program() {
        let absolute = Path::new("/app/bin/7z");
        assert_eq!(
            resolve_program_with_search_path(absolute.to_str().unwrap(), None),
            absolute
        );
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

    #[test]
    fn appimage_build_installs_7z_shared_object() {
        let script = include_str!("../../build-aux/appimage/build-appimage.sh");
        assert_appimage_build_installs_7z_shared_object(script);
    }
}
