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

    fn assert_manifest_installs_7zz(manifest: &str) {
        assert!(manifest.contains("\"type\": \"file\""));
        assert!(manifest.contains("tar xf 7zip.tar.xz"));
        assert!(manifest.contains("install -Dm755 7zz /app/bin/7zz"));
        assert!(manifest.contains("7z2604-linux-x64.tar.xz"));
        assert!(manifest.contains("7z2604-linux-arm64.tar.xz"));
        assert!(manifest.contains("\"only-arches\": [\"x86_64\"]"));
        assert!(manifest.contains("\"only-arches\": [\"aarch64\"]"));
    }

    fn assert_manifest_installs_hpatchz(manifest: &str) {
        assert!(manifest.contains("install -Dm755 hpatchz /app/bin/hpatchz"));
        assert!(manifest.contains("hdiffpatch_v5.1.3_bin_linux64.zip"));
        assert!(manifest.contains("hdiffpatch_v5.1.3_bin_linux_arm64.zip"));
    }

    fn assert_appimage_build_installs_7zz(script: &str) {
        assert!(script.contains("install -Dm755 \"$BUILD_DIR/tmp/7zz\" \"$APPDIR/usr/bin/7zz\""));
        assert!(script.contains("7z2604-linux-${SEVENZIP_ARCH}.tar.xz"));
        assert!(script.contains("SEVENZIP_ARCH=\"x64\""));
        assert!(script.contains("SEVENZIP_ARCH=\"arm64\""));
    }

    fn assert_appimage_build_installs_hpatchz(script: &str) {
        assert!(script.contains("hdiffpatch_v5.1.3_bin_${HPATCHZ_ARCH}.zip"));
        assert!(script.contains("\"${HPATCHZ_ARCH}/hpatchz\""));
        assert!(script.contains("HPATCHZ_ARCH=\"linux64\""));
        assert!(script.contains("HPATCHZ_ARCH=\"linux_arm64\""));
    }

    fn assert_appimage_build_uses_arch_specific_linuxdeploy(script: &str) {
        assert!(script.contains("linuxdeploy-${LINUXDEPLOY_ARCH}.AppImage"));
        assert!(script.contains("LINUXDEPLOY_ARCH=\"x86_64\""));
        assert!(script.contains("LINUXDEPLOY_ARCH=\"aarch64\""));
        assert!(script.contains("LDAI_UPDATE_INFORMATION="));
        assert!(script.contains("AdventureMods-v*-${APPIMAGE_ARCH}.AppImage.zsync"));
        assert!(script.contains("AdventureMods-v${version}-${APPIMAGE_ARCH}.AppImage"));
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
    fn resolve_program_with_search_path_keeps_bare_name_when_not_found() {
        let temp = tempfile::tempdir().unwrap();
        let search_path = std::env::join_paths([temp.path()]).unwrap();

        assert_eq!(
            resolve_program_with_search_path("7z", Some(search_path.as_os_str())),
            PathBuf::from("7z")
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
        let _lock = crate::test_env::lock();
        unsafe {
            std::env::set_var("ADVENTURE_MODS_7ZZ", "/tmp/fake-7zz");
        }

        assert_eq!(resolve_archive_program(), PathBuf::from("/tmp/fake-7zz"));

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_7ZZ");
        }
    }

    fn write_executable(path: &Path, contents: &str) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, contents).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Extract with `ADVENTURE_MODS_7ZZ` pointing at `program`. Callers hold
    /// the environment lock.
    fn extract_with_override(program: &Path, archive: &Path, dest: &Path) -> Result<()> {
        unsafe { std::env::set_var("ADVENTURE_MODS_7ZZ", program) };
        let result = extract(archive, dest);
        unsafe { std::env::remove_var("ADVENTURE_MODS_7ZZ") };
        result
    }

    #[test]
    fn extract_finds_7zz_on_path_without_override() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let bin_dir = temp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        // Records its arguments so the test can check how 7zz was invoked.
        write_executable(
            &bin_dir.join("7zz"),
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"${3#-o}/args\"\n",
        );
        let archive = temp.path().join("mod.7z");
        let dest = temp.path().join("out/nested");

        let path = std::env::var_os("PATH").expect("tests run with PATH set");
        unsafe { std::env::set_var("PATH", &bin_dir) };
        let result = extract(&archive, &dest);
        unsafe { std::env::set_var("PATH", path) };

        result.unwrap();
        let args = std::fs::read_to_string(dest.join("args")).unwrap();
        assert_eq!(
            args,
            format!("x\n-y\n-o{}\n{}\n", dest.display(), archive.display())
        );
    }

    #[test]
    fn extract_reports_missing_archive_program() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing-7zz");

        let error = extract_with_override(
            &missing,
            &temp.path().join("mod.7z"),
            &temp.path().join("out"),
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("Failed to run {}. Is 7-Zip installed?", missing.display())
        );
    }

    #[test]
    fn extract_reports_archive_program_failure_output() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let fake_7zz = temp.path().join("7zz");
        write_executable(
            &fake_7zz,
            "#!/bin/sh\necho 'Scanning archive'\necho 'Unexpected end of archive' >&2\nexit 2\n",
        );
        let archive = temp.path().join("broken.7z");

        let error =
            extract_with_override(&fake_7zz, &archive, &temp.path().join("out")).unwrap_err();

        let message = error.to_string();
        assert!(message.starts_with(&format!(
            "Archive extraction failed for {} with {}:",
            archive.display(),
            fake_7zz.display()
        )));
        assert!(message.contains("Scanning archive"));
        assert!(message.contains("Unexpected end of archive"));
    }

    #[test]
    fn extract_reports_destination_that_cannot_be_created() {
        let temp = tempfile::tempdir().unwrap();
        let blocker = temp.path().join("file");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let dest = blocker.join("out");

        let error = extract(&temp.path().join("mod.7z"), &dest).unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("Failed to create directory {}", dest.display())
        );
    }

    #[test]
    fn flatpak_manifests_use_current_runtime_and_shared_helpers() {
        let production = include_str!("../../build-aux/io.github.astrovm.AdventureMods.json");
        let development =
            include_str!("../../build-aux/io.github.astrovm.AdventureMods.Devel.json");

        for manifest in [production, development] {
            assert!(manifest.contains("\"runtime-version\": \"26.08\""));
            assert!(manifest.contains("\"flatpak/7zip.json\""));
            assert!(manifest.contains("\"flatpak/hdiffpatch.json\""));
        }

        assert_manifest_installs_7zz(include_str!("../../build-aux/flatpak/7zip.json"));
        assert_manifest_installs_hpatchz(include_str!("../../build-aux/flatpak/hdiffpatch.json"));
    }

    #[test]
    fn appimage_build_installs_7zz() {
        let script = include_str!("../../build-aux/appimage/build-appimage.sh");
        assert_appimage_build_installs_7zz(script);
        assert_appimage_build_installs_hpatchz(script);
        assert_appimage_build_uses_arch_specific_linuxdeploy(script);
    }
}
