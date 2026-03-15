use std::process::Output;

use anyhow::{Context, Result};

/// Run a command on the host via flatpak-spawn.
/// When running outside a Flatpak sandbox, runs the command directly.
pub async fn host_command(program: &str, args: &[&str]) -> Result<Output> {
    let prog = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    gio::spawn_blocking(move || host_command_sync(&prog, &args))
        .await
        .map_err(|e| anyhow::anyhow!("spawn error: {e:?}"))?
}

use gtk::gio;

fn host_command_sync(program: &str, args: &[String]) -> Result<Output> {
    let in_flatpak = std::path::Path::new("/.flatpak-info").exists();

    let output = if in_flatpak {
        let mut cmd = std::process::Command::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
            .with_context(|| format!("Failed to run flatpak-spawn --host {program}"))?
    } else {
        let mut cmd = std::process::Command::new(program);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
            .with_context(|| format!("Failed to run {program}"))?
    };

    Ok(output)
}

/// Check if a Flatpak app is installed on the host.
pub async fn is_flatpak_installed(app_id: &str) -> bool {
    let output = host_command("flatpak", &["info", "--show-ref", app_id]).await;
    output.is_ok_and(|o| o.status.success())
}

/// Install a Flatpak app from Flathub on the host.
pub async fn install_flatpak(app_id: &str) -> Result<()> {
    let output = host_command(
        "flatpak",
        &["install", "--user", "-y", "flathub", app_id],
    )
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to install {app_id}: {stderr}");
    }
    Ok(())
}

/// Launch a Flatpak app on the host (fire-and-forget).
pub async fn launch_flatpak(app_id: &str, args: &[&str]) -> Result<()> {
    let mut all_args = vec!["run", app_id];
    all_args.extend_from_slice(args);
    let output = host_command("flatpak", &all_args).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to launch {app_id}: {stderr}");
    }
    Ok(())
}
