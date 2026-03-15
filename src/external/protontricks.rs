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
    let app_id_str = app_id.to_string();

    // Try Flatpak protontricks first
    if flatpak::is_flatpak_installed(PROTONTRICKS_FLATPAK).await {
        let mut all_args = vec!["run", PROTONTRICKS_FLATPAK, &app_id_str];
        all_args.extend_from_slice(args);
        return flatpak::host_command("flatpak", &all_args)
            .await
            .context("protontricks via Flatpak failed");
    }

    // Fall back to system protontricks
    let mut all_args = vec![app_id_str.as_str()];
    all_args.extend_from_slice(args);
    flatpak::host_command("protontricks", &all_args)
        .await
        .context("protontricks command failed")
}

/// Launch a program inside the game's Wine prefix using protontricks-launch.
pub async fn launch(app_id: u32, exe_path: &str) -> Result<std::process::Output> {
    let app_id_str = app_id.to_string();

    if flatpak::is_flatpak_installed(PROTONTRICKS_FLATPAK).await {
        let cmd = format!("protontricks-launch --appid {app_id_str} '{exe_path}'");
        let args = vec![
            "run",
            PROTONTRICKS_FLATPAK,
            "-c",
            &cmd,
            &app_id_str,
        ];
        return flatpak::host_command("flatpak", &args)
            .await
            .context("protontricks-launch via Flatpak failed");
    }

    flatpak::host_command(
        "protontricks-launch",
        &["--appid", &app_id_str, exe_path],
    )
    .await
    .context("protontricks-launch failed")
}

/// Install .NET runtime via protontricks (dotnet48).
pub async fn install_dotnet(app_id: u32) -> Result<()> {
    let output = run(app_id, &["dotnet48"]).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to install .NET: {stderr}");
    }
    Ok(())
}
