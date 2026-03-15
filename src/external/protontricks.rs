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

    let cmd = if use_flatpak { "flatpak" } else { "protontricks" };
    flatpak::host_command(cmd, &refs)
        .await
        .context("protontricks command failed")
}

/// Build argument vector for a `protontricks run` invocation.
fn run_args(app_id: u32, args: &[&str], use_flatpak: bool) -> Vec<String> {
    let app_id_str = app_id.to_string();
    if use_flatpak {
        let mut all_args = vec![
            "run".to_string(),
            PROTONTRICKS_FLATPAK.to_string(),
            app_id_str,
        ];
        all_args.extend(args.iter().map(|a| a.to_string()));
        all_args
    } else {
        let mut all_args = vec![app_id_str];
        all_args.extend(args.iter().map(|a| a.to_string()));
        all_args
    }
}

/// Install .NET runtimes via protontricks (dotnet48 and dotnetdesktop8).
pub async fn install_dotnet(app_id: u32) -> Result<()> {
    // Run quietly to avoid forcing the user to click through multiple installers
    let output = run(app_id, &["-q", "dotnet48", "dotnetdesktop8"]).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to install .NET: {stderr}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_args_flatpak() {
        let args = run_args(71250, &["dotnet48"], true);
        assert_eq!(
            args,
            vec!["run", PROTONTRICKS_FLATPAK, "71250", "dotnet48"]
        );
    }

    #[test]
    fn test_run_args_system() {
        let args = run_args(71250, &["dotnet48"], false);
        assert_eq!(args, vec!["71250", "dotnet48"]);
    }

    #[test]
    fn test_protontricks_flatpak_id() {
        assert_eq!(PROTONTRICKS_FLATPAK, "com.github.Matoking.protontricks");
    }
}
