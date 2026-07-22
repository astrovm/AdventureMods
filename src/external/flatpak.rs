use std::path::Path;
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
    host_command_with_env(
        program,
        args,
        env,
        Path::new("/.flatpak-info").exists(),
        Path::new("flatpak-spawn"),
    )
}

fn host_command_with_env(
    program: &str,
    args: &[&str],
    env: &std::collections::HashMap<String, String>,
    in_flatpak: bool,
    flatpak_spawn: &Path,
) -> Result<Output> {
    if in_flatpak {
        let mut cmd = std::process::Command::new(flatpak_spawn);
        cmd.arg("--host");
        for (key, value) in env {
            cmd.arg(format!("--env={key}={value}"));
        }
        cmd.arg(program);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output().with_context(|| {
            format!(
                "Could not run host Proton command {program} through flatpak-spawn. Check that flatpak-spawn is installed and that Adventure Mods has permission to access org.freedesktop.Flatpak"
            )
        })
    } else {
        let mut cmd = std::process::Command::new(program);
        for (key, value) in env {
            cmd.env(key, value);
        }
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output().with_context(|| {
            format!(
                "Could not run host Proton command {program}. Check that the selected Proton version is installed and readable"
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;

    fn executable_script(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp_dir = tempfile::tempdir().unwrap();
        let script = temp_dir.path().join("flatpak-spawn");
        std::fs::write(&script, contents).unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();
        (temp_dir, script)
    }

    #[test]
    fn native_command_receives_environment() {
        let env = HashMap::from([("TEST_VALUE".to_owned(), "native".to_owned())]);
        let output = host_command_with_env(
            "/bin/sh",
            &["-c", "printf %s \"$TEST_VALUE\""],
            &env,
            false,
            Path::new("unused"),
        )
        .unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout, b"native");
    }

    #[test]
    fn flatpak_command_runs_host_proton_through_flatpak_spawn() {
        let (_temp_dir, flatpak_spawn) = executable_script("#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
        let env = HashMap::from([("WINEPREFIX".to_owned(), "/steam/pfx".to_owned())]);

        let output = host_command_with_env(
            "/steam/Proton/files/bin/wine64",
            &["installer.exe", "/quiet"],
            &env,
            true,
            &flatpak_spawn,
        )
        .unwrap();

        assert!(output.status.success());
        let arguments = String::from_utf8(output.stdout).unwrap();
        assert!(arguments.contains("--host\n"));
        assert!(arguments.contains("--env=WINEPREFIX=/steam/pfx\n"));
        assert!(arguments.contains("/steam/Proton/files/bin/wine64\n"));
        assert!(arguments.contains("installer.exe\n/quiet\n"));
    }

    #[test]
    fn missing_flatpak_spawn_has_actionable_error() {
        let error = host_command_with_env(
            "/steam/Proton/files/bin/wine64",
            &[],
            &HashMap::new(),
            true,
            Path::new("/definitely-missing/flatpak-spawn"),
        )
        .unwrap_err();

        let message = format!("{error:#}");
        assert!(message.contains("flatpak-spawn is installed"));
        assert!(message.contains("org.freedesktop.Flatpak"));
    }
}
