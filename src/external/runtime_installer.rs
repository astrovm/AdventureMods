use std::path::Path;

use anyhow::{Context, Result};

use super::download;
use super::proton;

/// .NET Desktop Runtime major SA Mod Manager needs. The latest release
/// (1.3.7) targets net8.0 and does not roll forward to newer majors.
const DOTNET_DESKTOP_MAJOR: u32 = 8;

/// Latest .NET Desktop Runtime 8 x64 offline installer.
///
/// The aka.ms channel link always points at the newest 8.0.x build, so
/// patches arrive without changing this URL.
const DOTNET_DESKTOP_8_URL: &str = "https://aka.ms/dotnet/8.0/windowsdesktop-runtime-win-x64.exe";

fn dotnet_desktop_url() -> String {
    std::env::var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_8")
        .unwrap_or_else(|_| DOTNET_DESKTOP_8_URL.to_string())
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

/// Majors of .NET Desktop Runtime installed in the prefix.
fn installed_dotnet_majors(prefix: &Path) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(windows_desktop_app_dir(prefix)) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            // Version folders look like "8.0.20", "10.0.12", etc.
            entry
                .file_name()
                .to_str()?
                .split('.')
                .next()?
                .parse::<u32>()
                .ok()
        })
        .collect()
}

/// Check whether the .NET Desktop Runtime SA Mod Manager needs is installed.
/// Other majors do not count: .NET does not run an app on a newer major.
pub fn is_dotnet_installed(prefix: &Path) -> bool {
    installed_dotnet_majors(prefix).contains(&DOTNET_DESKTOP_MAJOR)
}

/// Download and install .NET Desktop Runtime 8 into the game's
/// Proton prefix using the game's own Proton/Wine installation.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_runtimes(game_path: &Path, app_id: u32) -> Result<()> {
    proton::ensure_prefix_ready(game_path, app_id)?;

    let env = proton::proton_env(game_path, app_id)?;
    let compat_data = std::path::PathBuf::from(&env["STEAM_COMPAT_DATA_PATH"]);
    let prefix = std::path::PathBuf::from(&env["WINEPREFIX"]);
    let installer_dir = installer_staging_dir(&compat_data)?;

    if !prefix.is_dir() {
        anyhow::bail!(
            "Proton prefix not found at {}. Launch the game from Steam at least once first.",
            prefix.display()
        );
    }

    if is_dotnet_installed(&prefix) {
        tracing::info!(".NET Desktop Runtime {DOTNET_DESKTOP_MAJOR} already installed, skipping");
    } else {
        let major = DOTNET_DESKTOP_MAJOR;
        tracing::info!("Installing .NET Desktop Runtime {major}...");
        let dotnet_path = installer_dir.join(format!("windowsdesktop-runtime-{major}-win-x64.exe"));
        download::download_file(&dotnet_desktop_url(), &dotnet_path, None)?;

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
    fn is_dotnet_installed_needs_8() {
        let tmp = tempfile::tempdir().unwrap();
        // .NET 10 alone is what stopped SA Mod Manager 1.3.7 from starting.
        add_runtime(tmp.path(), "10.0.12");
        add_runtime(tmp.path(), "11.0.0");
        assert!(!is_dotnet_installed(tmp.path()));

        add_runtime(tmp.path(), "8.0.20");
        assert!(is_dotnet_installed(tmp.path()));
    }

    #[test]
    fn dotnet_url_tracks_the_latest_8_patch() {
        let _lock = crate::test_env::lock();
        assert_eq!(
            dotnet_desktop_url(),
            "https://aka.ms/dotnet/8.0/windowsdesktop-runtime-win-x64.exe"
        );
    }

    #[test]
    fn is_dotnet_installed_false_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_dotnet_installed(tmp.path()));
    }

    #[test]
    fn is_dotnet_installed_ignores_files_and_invalid_version_names() {
        let tmp = tempfile::tempdir().unwrap();
        let desktop_app = windows_desktop_app_dir(tmp.path());
        std::fs::create_dir_all(&desktop_app).unwrap();
        std::fs::write(desktop_app.join("8.0.0-file"), b"not a directory").unwrap();
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
    fn install_runtimes_skips_when_dotnet_is_already_present() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let game_path = steam_root.join("steamapps/common/Sonic Adventure 2");
        let proton_dir = steam_root.join("steamapps/common/Proton 10.0");
        let compatdata = steam_root.join("steamapps/compatdata/213610");
        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(proton_dir.join("files/bin")).unwrap();
        std::fs::write(proton_dir.join("files/bin/wine64"), b"").unwrap();
        std::fs::create_dir_all(
            compatdata
                .join("pfx/drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App/8.0.0"),
        )
        .unwrap();
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

        install_runtimes(&game_path, 213610).unwrap();
        assert!(!compatdata.join("adventure-mods-installers").exists());
    }

    #[test]
    fn dotnet_url_uses_override() {
        let _lock = crate::test_env::lock();
        unsafe {
            std::env::set_var(
                "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
                "http://127.0.0.1:4010/dotnet.exe",
            );
        }

        assert_eq!(dotnet_desktop_url(), "http://127.0.0.1:4010/dotnet.exe");

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_8");
        }
    }
}
