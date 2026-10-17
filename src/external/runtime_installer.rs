use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::archive;
use super::download;
use super::proton;

/// .NET Desktop Runtime majors installed into the prefix. SA Mod Manager
/// 1.3.7 targets .NET 8 (it will not roll forward to 10) and needs it to
/// start, while its update check shows a warning on every start until .NET
/// 10 is installed, which its next release will need anyway.
const DOTNET_DESKTOP_MAJORS: [u32; 2] = [8, 10];

/// Microsoft's zip packages for one .NET major, in install order: the base
/// runtime brings the host and Microsoft.NETCore.App, and the Windows Desktop
/// package adds WPF and Windows Forms on top of it. Each entry is the name
/// used by the download override and the package's file name.
const DOTNET_PACKAGES: [(&str, &str); 2] = [
    ("RUNTIME", "dotnet-runtime"),
    ("DESKTOP", "windowsdesktop-runtime"),
];

/// Versioned folders a .NET package unpacks. Windows Desktop goes last: its
/// folder is what marks the runtime as installed.
const DOTNET_VERSIONED_DIRS: [&str; 3] = [
    "host/fxr",
    "shared/Microsoft.NETCore.App",
    "shared/Microsoft.WindowsDesktop.App",
];

/// x64 zip of the .NET package `file_name` for `major`.
///
/// Use the stable aka.ms redirect so Microsoft can rotate the underlying build
/// without breaking downloads when old patch-specific URLs expire.
fn dotnet_zip_url(package: &str, file_name: &str, major: u32) -> String {
    std::env::var(format!("ADVENTURE_MODS_URL_DOTNET_{package}_{major}"))
        .unwrap_or_else(|_| format!("https://aka.ms/dotnet/{major}.0/{file_name}-win-x64.zip"))
}

fn installer_staging_dir(compat_data: &Path) -> Result<PathBuf> {
    let dir = compat_data.join("adventure-mods-installers");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create installer staging dir {}", dir.display()))?;
    Ok(dir)
}

/// Where .NET lives in a Proton/Wine prefix. .NET apps look here when no
/// other install is registered, so no installer or registry entry is needed.
fn dotnet_dir(prefix: &Path) -> PathBuf {
    prefix.join("drive_c/Program Files/dotnet")
}

/// Path to the Windows Desktop shared framework inside a Proton/Wine prefix.
pub fn windows_desktop_app_dir(prefix: &Path) -> PathBuf {
    dotnet_dir(prefix).join("shared/Microsoft.WindowsDesktop.App")
}

/// A .NET release version, such as 10.0.12. Preview suffixes are ignored.
type DotnetVersion = (u32, u32, u32);

fn parse_dotnet_version(text: &str) -> Option<DotnetVersion> {
    let release = text.split('-').next()?;
    let mut parts = release.split('.').map(|part| part.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// Versions of .NET Desktop Runtime installed in the prefix.
fn installed_dotnet_versions(prefix: &Path) -> Vec<DotnetVersion> {
    let Ok(entries) = std::fs::read_dir(windows_desktop_app_dir(prefix)) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        // Version folders look like "8.0.20", "10.0.12", etc.
        .filter_map(|entry| parse_dotnet_version(entry.file_name().to_str()?))
        .collect()
}

/// .NET Desktop Runtime majors SA Mod Manager needs that the prefix lacks.
fn missing_dotnet_majors(prefix: &Path) -> Vec<u32> {
    let installed = installed_dotnet_versions(prefix);
    DOTNET_DESKTOP_MAJORS
        .into_iter()
        .filter(|major| !installed.iter().any(|version| version.0 == *major))
        .collect()
}

/// Majors to install: those missing, plus those whose newest installed patch
/// is older than `latest` reports. When the latest version is unknown (for
/// example offline), an installed runtime is kept.
fn dotnet_majors_to_install(
    prefix: &Path,
    latest: impl Fn(u32) -> Option<DotnetVersion>,
) -> Vec<u32> {
    let installed = installed_dotnet_versions(prefix);
    DOTNET_DESKTOP_MAJORS
        .into_iter()
        .filter(|major| {
            let newest = installed
                .iter()
                .filter(|version| version.0 == *major)
                .max()
                .copied();
            match newest {
                None => true,
                Some(newest) => latest(*major).is_some_and(|latest| latest > newest),
            }
        })
        .collect()
}

/// Microsoft's release metadata for a .NET major, listing its latest patch.
fn dotnet_release_metadata_url(major: u32) -> String {
    std::env::var(format!("ADVENTURE_MODS_URL_DOTNET_RELEASES_{major}")).unwrap_or_else(|_| {
        format!(
            "https://builds.dotnet.microsoft.com/dotnet/release-metadata/{major}.0/releases.json"
        )
    })
}

/// The latest .NET Desktop Runtime version for `major`, from Microsoft.
fn latest_dotnet_version(major: u32) -> Result<DotnetVersion> {
    let url = dotnet_release_metadata_url(major);
    let body = download::block_on(async {
        download::client()
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch {url}"))?
            .error_for_status()
            .with_context(|| format!("Failed to fetch {url}"))?
            .text()
            .await
            .with_context(|| format!("Failed to read {url}"))
    })??;
    let metadata: serde_json::Value =
        serde_json::from_str(&body).with_context(|| format!("Failed to parse {url}"))?;
    metadata["releases"][0]["windowsdesktop"]["version"]
        .as_str()
        .and_then(parse_dotnet_version)
        .with_context(|| format!("No Windows Desktop version in {url}"))
}

/// Check whether every .NET Desktop Runtime SA Mod Manager needs is installed.
pub fn is_dotnet_installed(prefix: &Path) -> bool {
    missing_dotnet_majors(prefix).is_empty()
}

/// Download the .NET Desktop Runtimes and unpack them into the game's
/// Proton prefix. Nothing runs inside the prefix, so this needs no Wine.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_runtimes(game_path: &Path, app_id: u32) -> Result<()> {
    proton::ensure_prefix_ready(game_path, app_id)?;

    let compat_data = proton::compat_data_dir(game_path, app_id)?;
    let prefix = compat_data.join("pfx");
    // `ensure_prefix_ready` has already checked that this prefix exists.
    let installer_dir = installer_staging_dir(&compat_data)?;

    let latest = |major| {
        latest_dotnet_version(major)
            .inspect_err(|err| {
                tracing::warn!("Could not check .NET {major} for updates: {err:#}");
            })
            .ok()
    };
    for major in dotnet_majors_to_install(&prefix, latest) {
        tracing::info!("Installing the latest .NET Desktop Runtime {major}...");
        install_dotnet_major(major, &installer_dir, &dotnet_dir(&prefix))?;
        tracing::info!(".NET Desktop Runtime {major} installed");
    }

    let _ = std::fs::remove_dir(&installer_dir);

    Ok(())
}

/// Download and unpack every package of one .NET major into `dotnet`.
///
/// The packages are unpacked into a staging folder first and then moved into
/// place, so a failed download or extraction never leaves a half-unpacked
/// runtime that looks installed.
fn install_dotnet_major(major: u32, installer_dir: &Path, dotnet: &Path) -> Result<()> {
    let staging = installer_dir.join(format!("dotnet-{major}"));
    let _ = std::fs::remove_dir_all(&staging);
    for (package, file_name) in DOTNET_PACKAGES {
        let zip = installer_dir.join(format!("{file_name}-{major}-win-x64.zip"));
        download::download_file(&dotnet_zip_url(package, file_name, major), &zip, None)?;
        archive::extract(&zip, &staging)?;
        let _ = std::fs::remove_file(&zip);
    }
    move_dotnet_into_place(&staging, dotnet)
        .with_context(|| format!("Could not install .NET Desktop Runtime {major}"))?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

/// Move unpacked .NET folders from `staging` into `dotnet`.
///
/// Anything already there is kept. Version folders only ever appear whole,
/// moved in like this or written by Microsoft's installer, and .NET apps
/// start through the newest `host/fxr` folder, not the top-level `dotnet.exe`.
fn move_dotnet_into_place(staging: &Path, dotnet: &Path) -> Result<()> {
    for dir in DOTNET_VERSIONED_DIRS {
        let entries = std::fs::read_dir(staging.join(dir))
            .with_context(|| format!("The .NET download has no {dir} folder"))?;
        std::fs::create_dir_all(dotnet.join(dir))?;
        for entry in entries {
            let entry = entry?;
            move_if_new(&entry.path(), &dotnet.join(dir).join(entry.file_name()))?;
        }
    }
    for entry in std::fs::read_dir(staging)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            move_if_new(&entry.path(), &dotnet.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn move_if_new(from: &Path, to: &Path) -> Result<()> {
    if !to.exists() {
        std::fs::rename(from, to)
            .with_context(|| format!("Failed to move .NET into {}", to.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_runtime(prefix: &Path, version: &str) {
        std::fs::create_dir_all(windows_desktop_app_dir(prefix).join(version)).unwrap();
    }

    #[test]
    fn is_dotnet_installed_needs_both_8_and_10() {
        let tmp = tempfile::tempdir().unwrap();
        add_runtime(tmp.path(), "10.0.12");
        // Only 10 is what broke SA Mod Manager 1.3.7, which targets .NET 8.
        assert!(!is_dotnet_installed(tmp.path()));
        assert_eq!(missing_dotnet_majors(tmp.path()), vec![8]);

        add_runtime(tmp.path(), "8.0.20");
        assert!(is_dotnet_installed(tmp.path()));
        assert!(missing_dotnet_majors(tmp.path()).is_empty());
    }

    #[test]
    fn newer_majors_do_not_replace_8_or_10() {
        let tmp = tempfile::tempdir().unwrap();
        add_runtime(tmp.path(), "11.0.0");
        assert_eq!(missing_dotnet_majors(tmp.path()), vec![8, 10]);
    }

    #[test]
    fn is_dotnet_installed_false_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_dotnet_installed(tmp.path()));
    }

    #[test]
    fn is_dotnet_installed_false_for_only_8() {
        let tmp = tempfile::tempdir().unwrap();
        add_runtime(tmp.path(), "8.0.0");

        assert!(!is_dotnet_installed(tmp.path()));
    }

    #[test]
    fn is_dotnet_installed_ignores_files_and_invalid_version_names() {
        let tmp = tempfile::tempdir().unwrap();
        let desktop_app = windows_desktop_app_dir(tmp.path());
        std::fs::create_dir_all(&desktop_app).unwrap();
        std::fs::write(desktop_app.join("10.0.0-file"), b"not a directory").unwrap();
        std::fs::create_dir_all(desktop_app.join("runtime")).unwrap();

        assert!(!is_dotnet_installed(tmp.path()));
    }

    #[test]
    fn installer_staging_dir_uses_compatdata() {
        let tmp = tempfile::tempdir().unwrap();
        let compatdata = tmp.path().join("compatdata/213610");

        let dir = installer_staging_dir(&compatdata).unwrap();

        assert_eq!(dir, compatdata.join("adventure-mods-installers"));
        assert!(dir.is_dir());
    }

    #[test]
    fn installer_staging_dir_reports_blocked_compatdata() {
        let tmp = tempfile::tempdir().unwrap();
        let compatdata = tmp.path().join("compatdata");
        std::fs::write(&compatdata, b"not a directory").unwrap();

        let error = installer_staging_dir(&compatdata).unwrap_err();

        assert_eq!(
            error.to_string(),
            format!(
                "Failed to create installer staging dir {}",
                compatdata.join("adventure-mods-installers").display()
            )
        );
    }

    /// A Steam library holding SA2 with a ready Proton 10 prefix and an empty
    /// (not executable) Wine binary. Returns the game path and compatdata.
    fn fake_sa2_install(steam_root: &Path) -> (PathBuf, PathBuf) {
        let game_path = steam_root.join("steamapps/common/Sonic Adventure 2");
        let proton_dir = steam_root.join("steamapps/common/Proton 10.0");
        let compatdata = steam_root.join("steamapps/compatdata/213610");
        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(proton_dir.join("files/bin")).unwrap();
        std::fs::write(proton_dir.join("files/bin/wine64"), b"").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        std::fs::write(compatdata.join("version"), "10.1000-105\n").unwrap();
        std::fs::write(
            compatdata.join("config_info"),
            format!("{}\n", proton_dir.join("files").display()),
        )
        .unwrap();
        std::fs::create_dir_all(steam_root.join("config")).unwrap();
        std::fs::write(
            steam_root.join("config/config.vdf"),
            r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "CompatToolMapping"
                {
                    "213610" { "name" "proton_10" }
                }
            }
        }
    }
}"#,
        )
        .unwrap();
        (game_path, compatdata)
    }

    #[test]
    fn dotnet_urls_follow_the_package_and_major_and_allow_overrides() {
        let _lock = crate::test_env::lock();
        assert_eq!(
            dotnet_zip_url("RUNTIME", "dotnet-runtime", 8),
            "https://aka.ms/dotnet/8.0/dotnet-runtime-win-x64.zip"
        );
        assert_eq!(
            dotnet_zip_url("DESKTOP", "windowsdesktop-runtime", 10),
            "https://aka.ms/dotnet/10.0/windowsdesktop-runtime-win-x64.zip"
        );
        unsafe { std::env::set_var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_10", "http://local/10") };
        assert_eq!(
            dotnet_zip_url("DESKTOP", "windowsdesktop-runtime", 10),
            "http://local/10"
        );
        unsafe { std::env::remove_var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_10") };
    }

    /// A stand-in for 7zz that unpacks the layout of Microsoft's .NET zips.
    /// Each fake zip holds the version it unpacks; `broken` fails to extract
    /// and `empty` unpacks nothing.
    fn fake_dotnet_7zz(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let script = dir.join("fake-7zz");
        std::fs::write(
            &script,
            r#"#!/bin/sh
for arg in "$@"; do
    case "$arg" in
        -o*) dest="${arg#-o}" ;;
        x|-y) ;;
        *) archive="$arg" ;;
    esac
done
version=$(cat "$archive")
case "$version" in
    broken) echo "Cannot open the file as archive" >&2; exit 2 ;;
    empty) mkdir -p "$dest"; exit 0 ;;
esac
case "$archive" in
    */dotnet-runtime-*)
        mkdir -p "$dest/host/fxr/$version" "$dest/shared/Microsoft.NETCore.App/$version"
        echo "$version" > "$dest/dotnet.exe" ;;
    */windowsdesktop-runtime-*)
        mkdir -p "$dest/shared/Microsoft.WindowsDesktop.App/$version" ;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    /// Serve fake .NET zips and point the download overrides and 7zz at
    /// them. `body` gives each zip's content from its package and major.
    /// Returns the request log.
    fn serve_dotnet_zips(
        dir: &Path,
        body: fn(&str, u32) -> String,
    ) -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (base, log) = crate::external::test_http::serve(move |request| {
            let (package, major) = request.path[1..].split_once('/').unwrap();
            crate::external::test_http::Reply::ok(body(package, major.parse().unwrap()))
        });
        unsafe { std::env::set_var("ADVENTURE_MODS_7ZZ", fake_dotnet_7zz(dir)) };
        for major in DOTNET_DESKTOP_MAJORS {
            for (package, _) in DOTNET_PACKAGES {
                unsafe {
                    std::env::set_var(
                        format!("ADVENTURE_MODS_URL_DOTNET_{package}_{major}"),
                        format!("{base}/{package}/{major}"),
                    );
                }
            }
        }
        log
    }

    fn clear_dotnet_overrides() {
        unsafe { std::env::remove_var("ADVENTURE_MODS_7ZZ") };
        for major in DOTNET_DESKTOP_MAJORS {
            for name in ["RUNTIME", "DESKTOP"] {
                unsafe {
                    std::env::remove_var(format!("ADVENTURE_MODS_URL_DOTNET_{name}_{major}"))
                };
            }
        }
    }

    #[test]
    fn install_runtimes_unpacks_both_packages_for_each_major() {
        let tmp = tempfile::tempdir().unwrap();
        let (game_path, compatdata) = fake_sa2_install(tmp.path());
        let dotnet = compatdata.join("pfx/drive_c/Program Files/dotnet");

        let _lock = crate::test_env::lock();
        let log = serve_dotnet_zips(tmp.path(), |_, major| format!("{major}.0.5"));
        let result = install_runtimes(&game_path, 213610);
        clear_dotnet_overrides();

        result.unwrap();
        assert!(is_dotnet_installed(&compatdata.join("pfx")));
        for version in ["8.0.5", "10.0.5"] {
            for dir in DOTNET_VERSIONED_DIRS {
                assert!(dotnet.join(dir).join(version).is_dir(), "{dir}/{version}");
            }
        }
        // The first major's dotnet.exe is kept.
        assert_eq!(
            std::fs::read_to_string(dotnet.join("dotnet.exe")).unwrap(),
            "8.0.5\n"
        );
        assert!(!compatdata.join("adventure-mods-installers").exists());
        assert_eq!(
            *log.lock().unwrap(),
            vec![
                "GET /RUNTIME/8",
                "GET /DESKTOP/8",
                "GET /RUNTIME/10",
                "GET /DESKTOP/10",
            ]
        );
    }

    #[test]
    fn install_runtimes_leaves_a_failed_major_uninstalled() {
        let tmp = tempfile::tempdir().unwrap();
        let (game_path, compatdata) = fake_sa2_install(tmp.path());
        let prefix = compatdata.join("pfx");

        let _lock = crate::test_env::lock();
        serve_dotnet_zips(tmp.path(), |package, major| match (package, major) {
            ("DESKTOP", 10) => "broken".to_owned(),
            _ => format!("{major}.0.5"),
        });
        let result = install_runtimes(&game_path, 213610);
        clear_dotnet_overrides();

        let error = format!("{:#}", result.unwrap_err());
        assert!(
            error.starts_with("Archive extraction failed for "),
            "{error}"
        );
        assert!(error.contains("Cannot open the file as archive"), "{error}");
        // .NET 8 went in; .NET 10 never reached the prefix.
        assert_eq!(missing_dotnet_majors(&prefix), vec![10]);
        assert!(!dotnet_dir(&prefix).join("host/fxr/10.0.5").exists());
        // Its staging folder stays behind; the next run starts it over.
        assert!(
            compatdata
                .join("adventure-mods-installers/dotnet-10/host/fxr/10.0.5")
                .is_dir()
        );
    }

    #[test]
    fn install_runtimes_reports_a_download_missing_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let (game_path, compatdata) = fake_sa2_install(tmp.path());

        let _lock = crate::test_env::lock();
        serve_dotnet_zips(tmp.path(), |package, major| match package {
            "RUNTIME" => "empty".to_owned(),
            _ => format!("{major}.0.5"),
        });
        let result = install_runtimes(&game_path, 213610);
        clear_dotnet_overrides();

        assert_eq!(
            format!("{:#}", result.unwrap_err())
                .split(": No such file")
                .next()
                .unwrap(),
            "Could not install .NET Desktop Runtime 8: The .NET download has no host/fxr folder"
        );
        assert!(!is_dotnet_installed(&compatdata.join("pfx")));
    }

    #[test]
    fn move_dotnet_into_place_keeps_what_is_already_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let dotnet = tmp.path().join("dotnet");
        for dir in DOTNET_VERSIONED_DIRS {
            std::fs::create_dir_all(staging.join(dir).join("8.0.5")).unwrap();
        }
        std::fs::write(staging.join("dotnet.exe"), "new").unwrap();
        std::fs::write(staging.join("LICENSE.txt"), "license").unwrap();
        let installed = dotnet.join("shared/Microsoft.NETCore.App/8.0.5");
        std::fs::create_dir_all(&installed).unwrap();
        std::fs::write(installed.join("from-installer"), "").unwrap();
        std::fs::write(dotnet.join("dotnet.exe"), "old").unwrap();

        move_dotnet_into_place(&staging, &dotnet).unwrap();

        assert!(installed.join("from-installer").is_file());
        assert!(dotnet.join("host/fxr/8.0.5").is_dir());
        assert!(
            dotnet
                .join("shared/Microsoft.WindowsDesktop.App/8.0.5")
                .is_dir()
        );
        assert_eq!(
            std::fs::read_to_string(dotnet.join("dotnet.exe")).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::read_to_string(dotnet.join("LICENSE.txt")).unwrap(),
            "license"
        );
    }

    #[test]
    fn move_dotnet_into_place_reports_a_folder_it_cannot_write() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let dotnet = tmp.path().join("dotnet");
        for dir in DOTNET_VERSIONED_DIRS {
            std::fs::create_dir_all(staging.join(dir).join("8.0.5")).unwrap();
        }
        let fxr = dotnet.join("host/fxr");
        std::fs::create_dir_all(&fxr).unwrap();
        std::fs::set_permissions(&fxr, std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = move_dotnet_into_place(&staging, &dotnet);
        std::fs::set_permissions(&fxr, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Failed to move .NET into {}", fxr.join("8.0.5").display())
        );
    }

    #[test]
    fn install_runtimes_keeps_installed_dotnet_when_updates_cannot_be_checked() {
        let tmp = tempfile::tempdir().unwrap();
        let (game_path, compatdata) = fake_sa2_install(tmp.path());
        for version in ["10.0.0", "8.0.0"] {
            add_runtime(&compatdata.join("pfx"), version);
        }
        // Nothing listens here, so the release metadata cannot be fetched.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/releases.json", listener.local_addr().unwrap());
        drop(listener);

        let _lock = crate::test_env::lock();
        let _ = rustls::crypto::ring::default_provider().install_default();
        for major in DOTNET_DESKTOP_MAJORS {
            unsafe {
                std::env::set_var(format!("ADVENTURE_MODS_URL_DOTNET_RELEASES_{major}"), &url)
            };
        }
        let (result, logs) = crate::test_log::capture_logs(|| install_runtimes(&game_path, 213610));
        for major in DOTNET_DESKTOP_MAJORS {
            unsafe { std::env::remove_var(format!("ADVENTURE_MODS_URL_DOTNET_RELEASES_{major}")) };
        }

        result.unwrap();
        for major in DOTNET_DESKTOP_MAJORS {
            assert!(logs.contains(&format!(
                "Could not check .NET {major} for updates: Failed to fetch {url}"
            )));
        }
        assert!(!logs.contains("Installing"));
        assert!(!compatdata.join("adventure-mods-installers").exists());
    }

    #[test]
    fn install_runtimes_skips_when_dotnet_is_already_present() {
        let tmp = tempfile::tempdir().unwrap();
        let (game_path, compatdata) = fake_sa2_install(tmp.path());
        for version in ["10.0.0", "8.0.0"] {
            add_runtime(&compatdata.join("pfx"), version);
        }

        let _lock = crate::test_env::lock();
        let _ = rustls::crypto::ring::default_provider().install_default();
        // Microsoft reports 8.0.0 and 10.0.0 as latest, which is what is here.
        let (base, log) = crate::external::test_http::serve(|request| {
            let version = if request.path.contains("/8") {
                "8.0.0"
            } else {
                "10.0.0"
            };
            crate::external::test_http::Reply::ok(format!(
                r#"{{"releases":[{{"windowsdesktop":{{"version":"{version}"}}}}]}}"#
            ))
        });
        for major in [8, 10] {
            unsafe {
                std::env::set_var(
                    format!("ADVENTURE_MODS_URL_DOTNET_RELEASES_{major}"),
                    format!("{base}/{major}"),
                );
            }
        }

        install_runtimes(&game_path, 213610).unwrap();
        assert!(!compatdata.join("adventure-mods-installers").exists());
        assert_eq!(log.lock().unwrap().len(), 2);

        for major in [8, 10] {
            unsafe { std::env::remove_var(format!("ADVENTURE_MODS_URL_DOTNET_RELEASES_{major}")) };
        }
    }

    #[test]
    fn newer_patches_are_installed_and_unknown_latest_keeps_what_is_there() {
        let tmp = tempfile::tempdir().unwrap();
        add_runtime(tmp.path(), "8.0.31");
        add_runtime(tmp.path(), "10.0.10");
        add_runtime(tmp.path(), "10.0.9");

        let latest = |major| match major {
            8 => Some((8, 0, 31)),
            _ => Some((10, 0, 12)),
        };
        // 10.0.12 is newer than the newest installed 10.x; 8 is current.
        assert_eq!(dotnet_majors_to_install(tmp.path(), latest), vec![10]);
        // Offline: keep what is installed.
        assert!(dotnet_majors_to_install(tmp.path(), |_| None).is_empty());
        // A missing major is installed without asking.
        let empty = tempfile::tempdir().unwrap();
        add_runtime(empty.path(), "10.0.12");
        assert_eq!(dotnet_majors_to_install(empty.path(), |_| None), vec![8]);
    }

    #[test]
    fn dotnet_versions_parse_releases_and_previews() {
        assert_eq!(parse_dotnet_version("10.0.12"), Some((10, 0, 12)));
        assert_eq!(
            parse_dotnet_version("11.0.0-rc.1.25451.107"),
            Some((11, 0, 0))
        );
        assert_eq!(parse_dotnet_version("10.0"), None);
        assert_eq!(parse_dotnet_version("runtime"), None);
    }

    #[test]
    fn latest_dotnet_version_reads_microsoft_release_metadata() {
        let _lock = crate::test_env::lock();
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (base, _) = crate::external::test_http::serve(|request| {
            use crate::external::test_http::Reply;
            match request.path.as_str() {
                "/good" => Reply::ok(
                    r#"{"latest-release":"10.0.12","releases":[{"windowsdesktop":{"version":"10.0.12"}}]}"#,
                ),
                "/empty" => Reply::ok(r#"{"releases":[]}"#),
                "/html" => Reply::ok("<html>maintenance</html>"),
                "/truncated" => Reply::ok(r#"{"releases":"#).claim_length(100),
                _ => Reply::ok("").status("404 Not Found"),
            }
        });
        let latest_from = |path: &str| {
            unsafe {
                std::env::set_var(
                    "ADVENTURE_MODS_URL_DOTNET_RELEASES_10",
                    format!("{base}{path}"),
                )
            };
            latest_dotnet_version(10).map_err(|err| err.to_string())
        };
        assert_eq!(latest_from("/good"), Ok((10, 0, 12)));
        assert_eq!(
            latest_from("/empty"),
            Err(format!("No Windows Desktop version in {base}/empty"))
        );
        assert_eq!(
            latest_from("/html"),
            Err(format!("Failed to parse {base}/html"))
        );
        assert_eq!(
            latest_from("/truncated"),
            Err(format!("Failed to read {base}/truncated"))
        );
        assert_eq!(
            latest_from("/missing"),
            Err(format!("Failed to fetch {base}/missing"))
        );
        unsafe { std::env::remove_var("ADVENTURE_MODS_URL_DOTNET_RELEASES_10") };
        assert_eq!(
            dotnet_release_metadata_url(8),
            "https://builds.dotnet.microsoft.com/dotnet/release-metadata/8.0/releases.json"
        );
    }
}
