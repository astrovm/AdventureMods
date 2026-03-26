use std::path::Path;

use anyhow::{Context, Result};

use super::download;
use super::proton;

/// VC++ 2015-2022 Redistributable x64 — stable Microsoft redirect URL.
const VCREDIST_URL: &str = "https://aka.ms/vs/17/release/vc_redist.x64.exe";

/// .NET Desktop Runtime 8.0 x64 offline installer.
///
/// This URL points to a specific patch version and should be updated when new
/// patches are released. Using the offline installer avoids network dependencies
/// during Wine execution.
const DOTNET_DESKTOP_8_URL: &str = "https://download.visualstudio.microsoft.com/download/pr/27bcdd70-ce64-4049-ba24-2b1971545497/5e58f0e5e0b8b33825c3caef1fae00a4/windowsdesktop-runtime-8.0.14-win-x64.exe";

fn is_success_or_reboot_code(code: i32) -> bool {
    code == 0 || code == 3010
}

/// Check whether the VC++ runtime is already installed in the prefix.
pub fn is_vcrun_installed(prefix: &Path) -> bool {
    prefix
        .join("drive_c/windows/system32/vcruntime140.dll")
        .is_file()
}

/// Check whether .NET Desktop Runtime 8 is already installed in the prefix.
pub fn is_dotnet_installed(prefix: &Path) -> bool {
    prefix
        .join("drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App")
        .is_dir()
}

/// Download and install VC++ 2022 and .NET Desktop Runtime 8 into the game's
/// Proton prefix using the game's own Proton/Wine installation.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_runtimes(game_path: &Path, app_id: u32) -> Result<()> {
    let env = proton::proton_env(game_path, app_id)?;
    let prefix = std::path::PathBuf::from(&env["WINEPREFIX"]);

    if !prefix.is_dir() {
        anyhow::bail!(
            "Proton prefix not found at {}. Launch the game from Steam at least once first.",
            prefix.display()
        );
    }

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;

    // Install VC++ first — .NET installer may depend on it.
    if !is_vcrun_installed(&prefix) {
        tracing::info!("Installing VC++ 2022 Redistributable...");
        let vcredist_path = temp_dir.path().join("vc_redist.x64.exe");
        download::download_file(VCREDIST_URL, &vcredist_path, None)?;

        let output = proton::run_in_prefix(
            game_path,
            app_id,
            &vcredist_path,
            &["/install", "/quiet", "/norestart"],
        )?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            // VC++ installer may return 3010 (reboot required) which is success.
            if !is_success_or_reboot_code(code) {
                anyhow::bail!("VC++ 2022 installation failed (code {code}): {stderr}");
            }
        }
        tracing::info!("VC++ 2022 Redistributable installed");
    } else {
        tracing::info!("VC++ 2022 already installed, skipping");
    }

    if !is_dotnet_installed(&prefix) {
        tracing::info!("Installing .NET Desktop Runtime 8...");
        let dotnet_path = temp_dir.path().join("windowsdesktop-runtime-8-win-x64.exe");
        download::download_file(DOTNET_DESKTOP_8_URL, &dotnet_path, None)?;

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
    } else {
        tracing::info!(".NET Desktop Runtime 8 already installed, skipping");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_vcrun_installed_true() {
        let tmp = tempfile::tempdir().unwrap();
        let dll_path = tmp.path().join("drive_c/windows/system32");
        std::fs::create_dir_all(&dll_path).unwrap();
        std::fs::write(dll_path.join("vcruntime140.dll"), "").unwrap();

        assert!(is_vcrun_installed(tmp.path()));
    }

    #[test]
    fn test_is_vcrun_installed_false() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_vcrun_installed(tmp.path()));
    }

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
        assert!(!is_success_or_reboot_code(1603));
        assert!(!is_success_or_reboot_code(-1));
    }
}
