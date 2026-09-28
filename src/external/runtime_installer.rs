use std::path::Path;

use anyhow::{Context, Result};

use super::download;
use super::proton;

/// .NET Desktop Runtime majors installed into the prefix. SA Mod Manager
/// 1.3.7 targets .NET 8 (it will not roll forward to 10) and needs it to
/// start, while its update check shows a warning on every start until .NET
/// 10 is installed, which its next release will need anyway.
const DOTNET_DESKTOP_MAJORS: [u32; 2] = [8, 10];

/// .NET Desktop Runtime x64 offline installer for `major`.
///
/// Use the stable aka.ms redirect so Microsoft can rotate the underlying build
/// without breaking downloads when old patch-specific URLs expire.
fn dotnet_desktop_url(major: u32) -> String {
    std::env::var(format!("ADVENTURE_MODS_URL_DOTNET_DESKTOP_{major}")).unwrap_or_else(|_| {
        format!("https://aka.ms/dotnet/{major}.0/windowsdesktop-runtime-win-x64.exe")
    })
}

fn installer_staging_dir(compat_data: &Path) -> Result<std::path::PathBuf> {
    let dir = compat_data.join("adventure-mods-installers");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create installer staging dir {}", dir.display()))?;
    Ok(dir)
}

fn is_success_or_reboot_code(code: i32) -> bool {
    // Wine on Linux truncates Windows exit codes to the low 8 bits,
    // so 3010 (reboot required) arrives as 194.
    code == 0 || code == 3010 || code == (3010 & 0xff)
}

/// Path to the Windows Desktop shared framework inside a Proton/Wine prefix.
pub fn windows_desktop_app_dir(prefix: &Path) -> std::path::PathBuf {
    prefix.join("drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App")
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

/// Download and install the .NET Desktop Runtimes into the game's
/// Proton prefix using the game's own Proton/Wine installation.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_runtimes(game_path: &Path, app_id: u32) -> Result<()> {
    proton::ensure_prefix_ready(game_path, app_id)?;

    let env = proton::proton_env(game_path, app_id)?;
    let compat_data = std::path::PathBuf::from(&env["STEAM_COMPAT_DATA_PATH"]);
    let prefix = std::path::PathBuf::from(&env["WINEPREFIX"]);
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
        let dotnet_path = installer_dir.join(format!("windowsdesktop-runtime-{major}-win-x64.exe"));
        download::download_file(&dotnet_desktop_url(major), &dotnet_path, None)?;

        let output = proton::run_in_prefix(
            game_path,
            app_id,
            &dotnet_path,
            &["/install", "/quiet", "/norestart"],
        )?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            if !is_success_or_reboot_code(code) {
                anyhow::bail!(
                    ".NET Desktop Runtime {major} installation failed (code {code}): {stderr}"
                );
            }
        }
        tracing::info!(".NET Desktop Runtime {major} installed");
        let _ = std::fs::remove_file(&dotnet_path);
    }

    let _ = std::fs::remove_dir(&installer_dir);

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
    fn dotnet_urls_follow_the_major_and_allow_overrides() {
        let _lock = crate::test_env::lock();
        assert_eq!(
            dotnet_desktop_url(8),
            "https://aka.ms/dotnet/8.0/windowsdesktop-runtime-win-x64.exe"
        );
        unsafe { std::env::set_var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_10", "http://local/10") };
        assert_eq!(dotnet_desktop_url(10), "http://local/10");
        unsafe { std::env::remove_var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_10") };
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
    fn is_success_or_reboot_code_accepts_success_and_reboot() {
        assert!(is_success_or_reboot_code(0));
        assert!(is_success_or_reboot_code(3010));
        assert!(is_success_or_reboot_code(3010 & 0xff));
        assert!(!is_success_or_reboot_code(1603));
        assert!(!is_success_or_reboot_code(-1));
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
    fn fake_sa2_install(steam_root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
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

    /// Serve a fake installer for every .NET major and point the download
    /// overrides at it. Returns the request log.
    fn serve_installers() -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let (base, log) = crate::external::test_http::serve(|_| {
            crate::external::test_http::Reply::ok("MZ fake installer")
        });
        for major in DOTNET_DESKTOP_MAJORS {
            unsafe {
                std::env::set_var(
                    format!("ADVENTURE_MODS_URL_DOTNET_DESKTOP_{major}"),
                    format!("{base}/{major}.exe"),
                );
            }
        }
        log
    }

    fn clear_installer_overrides() {
        for major in DOTNET_DESKTOP_MAJORS {
            unsafe { std::env::remove_var(format!("ADVENTURE_MODS_URL_DOTNET_DESKTOP_{major}")) };
        }
    }

    #[test]
    fn install_runtimes_accepts_reboot_code_and_reports_failed_installer() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let (game_path, compatdata) = fake_sa2_install(tmp.path());
        // Proton's launcher: the .NET 8 installer asks for a reboot (3010,
        // truncated by Wine); .NET 10 succeeds unless a `fail` marker exists.
        let proton_dir = tmp.path().join("steamapps/common/Proton 10.0");
        let launcher = proton_dir.join("proton");
        std::fs::write(
            &launcher,
            "#!/bin/sh\ncase \"$2\" in *-8-*) exit 194;; esac\nif [ -e \"${0%/*}/fail\" ]; then echo \"installer crashed: $*\" >&2; exit 1; fi\n",
        )
        .unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
        let installers = compatdata.join("adventure-mods-installers");

        let _lock = crate::test_env::lock();
        let log = serve_installers();
        let installed = install_runtimes(&game_path, 213610);
        let cleaned_up = !installers.exists();
        std::fs::write(proton_dir.join("fail"), b"").unwrap();
        let result = install_runtimes(&game_path, 213610);
        clear_installer_overrides();

        installed.unwrap();
        assert!(cleaned_up);
        let failed = installers.join("windowsdesktop-runtime-10-win-x64.exe");
        assert_eq!(
            result.unwrap_err().to_string(),
            format!(
                ".NET Desktop Runtime 10 installation failed (code 1): installer crashed: runinprefix {} /install /quiet /norestart\n",
                failed.display()
            )
        );
        assert_eq!(
            *log.lock().unwrap(),
            vec!["GET /8.exe", "GET /10.exe", "GET /8.exe", "GET /10.exe"]
        );
        // The accepted .NET 8 installer is cleaned up; the failed one is kept.
        assert!(
            !installers
                .join("windowsdesktop-runtime-8-win-x64.exe")
                .exists()
        );
        assert!(failed.is_file());
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
    fn install_runtimes_reports_wine_that_cannot_run() {
        let tmp = tempfile::tempdir().unwrap();
        let (game_path, _) = fake_sa2_install(tmp.path());
        let wine = tmp
            .path()
            .join("steamapps/common/Proton 10.0/files/bin/wine64");

        let _lock = crate::test_env::lock();
        serve_installers();
        let result = install_runtimes(&game_path, 213610);
        clear_installer_overrides();

        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Could not run host command {}", wine.display())
        );
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

    #[test]
    fn dotnet_url_uses_override() {
        let _lock = crate::test_env::lock();
        unsafe {
            std::env::set_var(
                "ADVENTURE_MODS_URL_DOTNET_DESKTOP_10",
                "http://127.0.0.1:4010/dotnet.exe",
            );
        }

        assert_eq!(dotnet_desktop_url(10), "http://127.0.0.1:4010/dotnet.exe");

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_10");
        }
    }
}
