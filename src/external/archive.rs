use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const ARCHIVE_PROGRAM: &str = "7zz";

/// Extract an archive using 7z (supports .7z, .zip, .rar, .tar.*, etc.).
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory {}", dest.display()))?;

    let dest_arg = format!("-o{}", dest.display());
    let program = resolve_archive_program();
    let output = std::process::Command::new(&program)
        .arg("x")
        .arg("-y")
        .arg(&dest_arg)
        .arg(archive)
        .output()
        .with_context(|| format!("Failed to run {}. Is 7-Zip installed?", program.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "Archive extraction failed for {} with {}:\n{}\n{}",
            archive.display(),
            program.display(),
            stdout,
            stderr,
        );
    }

    Ok(())
}

fn resolve_archive_program() -> PathBuf {
    if let Some(program) = std::env::var_os("ADVENTURE_MODS_7ZZ") {
        return PathBuf::from(program);
    }

    resolve_archive_program_with_search_path(std::env::var_os("PATH").as_deref())
}

fn resolve_archive_program_with_search_path(search_path: Option<&OsStr>) -> PathBuf {
    find_program_in_search_path(ARCHIVE_PROGRAM, search_path)
        .unwrap_or_else(|| PathBuf::from(ARCHIVE_PROGRAM))
}

#[cfg(test)]
fn resolve_program_with_search_path(program: &str, search_path: Option<&OsStr>) -> PathBuf {
    let program_path = Path::new(program);
    if program_path.is_absolute() || program.contains(std::path::MAIN_SEPARATOR) {
        return program_path.to_path_buf();
    }

    find_program_in_search_path(program, search_path).unwrap_or_else(|| program_path.to_path_buf())
}

fn find_program_in_search_path(program: &str, search_path: Option<&OsStr>) -> Option<PathBuf> {
    search_path
        .into_iter()
        .flat_map(std::env::split_paths)
        .find_map(|dir| {
            let candidate = dir.join(program);
            candidate
                .is_file()
                .then(|| candidate.canonicalize().unwrap_or(candidate))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn assert_manifest_installs_7zz(manifest: &str) {
        assert!(manifest.contains("\"type\": \"file\""));
        assert!(manifest.contains("tar xf 7zip.tar.xz"));
        assert!(manifest.contains("install -Dm755 7zz /app/bin/7zz"));
    }

    fn assert_appimage_build_installs_7zz(script: &str) {
        assert!(script.contains("install -Dm755 \"$BUILD_DIR/tmp/7zz\" \"$APPDIR/usr/bin/7zz\""));
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
    fn resolve_archive_program_uses_7zz_when_present() {
        let temp = tempfile::tempdir().unwrap();
        let bin_dir = temp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let seven_zz = bin_dir.join("7zz");
        std::fs::write(&seven_zz, b"#!/bin/sh\nexit 0\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&seven_zz).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&seven_zz, permissions).unwrap();
        }

        let search_path = std::env::join_paths([&bin_dir]).unwrap();
        assert_eq!(
            resolve_archive_program_with_search_path(Some(search_path.as_os_str())),
            seven_zz.canonicalize().unwrap()
        );
    }

    #[test]
    fn resolve_archive_program_does_not_fall_back_to_7z() {
        let temp = tempfile::tempdir().unwrap();
        let bin_dir = temp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();

        let seven_z = bin_dir.join("7z");
        std::fs::write(&seven_z, b"#!/bin/sh\nexit 0\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&seven_z).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&seven_z, permissions).unwrap();
        }

        let search_path = std::env::join_paths([&bin_dir]).unwrap();
        assert_eq!(
            resolve_archive_program_with_search_path(Some(search_path.as_os_str())),
            PathBuf::from("7zz")
        );
    }

    #[test]
    fn resolve_archive_program_uses_override_path() {
        let _lock = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("ADVENTURE_MODS_7ZZ", "/tmp/fake-7zz");
        }

        assert_eq!(resolve_archive_program(), PathBuf::from("/tmp/fake-7zz"));

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_7ZZ");
        }
    }

    #[test]
    fn flatpak_manifest_installs_7zz() {
        let manifest = include_str!("../../build-aux/io.github.astrovm.AdventureMods.json");
        assert_manifest_installs_7zz(manifest);
    }

    #[test]
    fn flatpak_devel_manifest_installs_7zz() {
        let manifest = include_str!("../../build-aux/io.github.astrovm.AdventureMods.Devel.json");
        assert_manifest_installs_7zz(manifest);
    }

    #[test]
    fn appimage_build_installs_7zz() {
        let script = include_str!("../../build-aux/appimage/build-appimage.sh");
        assert_appimage_build_installs_7zz(script);
    }
}
