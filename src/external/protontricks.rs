use anyhow::{Context, Result};

use super::flatpak;

const PROTONTRICKS_FLATPAK: &str = "com.github.Matoking.protontricks";

/// Check if protontricks is available (Flatpak or system).
pub async fn is_available() -> bool {
    flatpak::is_flatpak_installed(PROTONTRICKS_FLATPAK).await
        || flatpak::host_command("protontricks", &["--version"])
            .await
            .is_ok_and(|o| o.status.success())
}

/// Install protontricks from Flathub.
pub async fn install() -> Result<()> {
    flatpak::install_flatpak(PROTONTRICKS_FLATPAK).await
}

/// Run a protontricks command for a given app ID.
pub async fn run(app_id: u32, args: &[&str]) -> Result<std::process::Output> {
    let use_flatpak = flatpak::is_flatpak_installed(PROTONTRICKS_FLATPAK).await;
    let built = run_args(app_id, args, use_flatpak);
    let refs: Vec<&str> = built.iter().map(|s| s.as_str()).collect();

    let cmd = if use_flatpak {
        "flatpak"
    } else {
        "protontricks"
    };
    flatpak::host_command(cmd, &refs)
        .await
        .context("protontricks command failed")
}

/// Build argument vector for a `protontricks run` invocation.
fn run_args(app_id: u32, args: &[&str], use_flatpak: bool) -> Vec<String> {
    let app_id_str = app_id.to_string();
    let mut all_args = vec![
        "run".to_string(),
        if use_flatpak {
            PROTONTRICKS_FLATPAK.to_string()
        } else {
            app_id_str.clone()
        },
    ];

    if use_flatpak {
        all_args.push(app_id_str);
    }

    all_args.extend(args.iter().map(|a| a.to_string()));
    all_args
}

/// Install runtimes via protontricks (.NET Desktop 8.0 + VC++ Redist).
pub async fn install_dotnet(app_id: u32) -> Result<()> {
    // Install components separately for better reliability
    let components = ["dotnetdesktop8", "vcrun2022"];

    for component in components {
        tracing::info!("Installing {} via protontricks...", component);
        let output = run(app_id, &["-q", component]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let code = output.status.code().unwrap_or(-1);

            // protontricks/winetricks often exit with non-zero codes for warnings
            // 1: General warning/error
            // 67: Often related to network/files but sometimes just noise
            let is_warning = stderr.contains("WARNING") || stderr.contains("fixme:");
            let started = stdout.contains("Executing") || stderr.contains("Executing");
            let already_installed =
                stdout.contains("already installed") || stderr.contains("already installed");

            if (code == 1 || code == 67) && (is_warning || started || already_installed) {
                tracing::warn!(
                    "protontricks exited with code {} for {} but appears to have executed or is already installed",
                    code,
                    component
                );
                continue;
            }

            anyhow::bail!(
                "Failed to install {}: {} (code {})",
                component,
                stderr,
                code
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_args_flatpak() {
        let args = run_args(71250, &["dotnetdesktop8"], true);
        assert_eq!(
            args,
            vec!["run", PROTONTRICKS_FLATPAK, "71250", "dotnetdesktop8"]
        );
    }

    #[test]
    fn test_run_args_system() {
        let args = run_args(71250, &["dotnetdesktop8"], false);
        assert_eq!(args, vec!["run", "71250", "dotnetdesktop8"]);
    }

    #[test]
    fn test_protontricks_flatpak_id() {
        assert_eq!(PROTONTRICKS_FLATPAK, "com.github.Matoking.protontricks");
    }
}
