use std::process::Output;

use anyhow::{Context, Result};

/// Run a command on the host with extra environment variables.
///
/// In a Flatpak sandbox, env vars are passed as `--env=KEY=VALUE` args to
/// `flatpak-spawn`. Outside the sandbox, they are set directly on the command.
pub fn host_command_with_env_sync(
    program: &str,
    args: &[&str],
    env: &std::collections::HashMap<String, String>,
) -> Result<Output> {
    let in_flatpak = std::path::Path::new("/.flatpak-info").exists();

    let output = if in_flatpak {
        let mut cmd = std::process::Command::new("flatpak-spawn");
        cmd.arg("--host");
        for (key, value) in env {
            cmd.arg(format!("--env={key}={value}"));
        }
        cmd.arg(program);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
            .with_context(|| format!("Failed to run flatpak-spawn --host {program}"))?
    } else {
        let mut cmd = std::process::Command::new(program);
        for (key, value) in env {
            cmd.env(key, value);
        }
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
            .with_context(|| format!("Failed to run {program}"))?
    };

    Ok(output)
}
