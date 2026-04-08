use std::path::Path;

use anyhow::{Context, Result};

use super::download;
use super::proton;

/// .NET Desktop Runtime 8.0 x64 offline installer.
///
/// Use the stable aka.ms redirect so Microsoft can rotate the underlying build
/// without breaking downloads when old patch-specific URLs expire.
const DOTNET_DESKTOP_8_URL: &str = "https://aka.ms/dotnet/8.0/windowsdesktop-runtime-win-x64.exe";

fn dotnet_desktop_8_url() -> String {
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

/// Check whether .NET Desktop Runtime 8 is already installed in the prefix.
pub fn is_dotnet_installed(prefix: &Path) -> bool {
    prefix
        .join("drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App")
        .is_dir()
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

    if !is_dotnet_installed(&prefix) {
        tracing::info!("Installing .NET Desktop Runtime 8...");
        let dotnet_path = installer_dir.join("windowsdesktop-runtime-8-win-x64.exe");
        let dotnet_url = dotnet_desktop_8_url();
        download::download_file(&dotnet_url, &dotnet_path, None)?;

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
                anyhow::bail!(".NET Desktop Runtime 8 installation failed (code {code}): {stderr}");
            }
        }
        tracing::info!(".NET Desktop Runtime 8 installed");
        let _ = std::fs::remove_file(&dotnet_path);
    } else {
        tracing::info!(".NET Desktop Runtime 8 already installed, skipping");
    }

    let _ = std::fs::remove_dir(&installer_dir);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_is_dotnet_installed_true() {
        let tmp = tempfile::tempdir().unwrap();
        let dotnet_path = tmp
            .path()
            .join("drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App");
        std::fs::create_dir_all(&dotnet_path).unwrap();

        assert!(is_dotnet_installed(tmp.path()));
    }

    #[test]
    fn test_is_dotnet_installed_false() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_dotnet_installed(tmp.path()));
    }

    #[test]
    fn test_is_success_or_reboot_code() {
        assert!(is_success_or_reboot_code(0));
        assert!(is_success_or_reboot_code(3010));
        assert!(is_success_or_reboot_code(3010 & 0xff));
        assert!(!is_success_or_reboot_code(1603));
        assert!(!is_success_or_reboot_code(-1));
    }

    #[test]
    fn test_installer_staging_dir_uses_compatdata() {
        let tmp = tempfile::tempdir().unwrap();
        let compatdata = tmp.path().join("compatdata/213610");

        let dir = installer_staging_dir(&compatdata).unwrap();

        assert_eq!(dir, compatdata.join("adventure-mods-installers"));
        assert!(dir.is_dir());
    }

    #[test]
    fn test_dotnet_url_uses_override() {
        let _lock = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var(
                "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
                "http://127.0.0.1:4010/dotnet.exe",
            );
        }

        assert_eq!(dotnet_desktop_8_url(), "http://127.0.0.1:4010/dotnet.exe");

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_URL_DOTNET_DESKTOP_8");
        }
    }
}
