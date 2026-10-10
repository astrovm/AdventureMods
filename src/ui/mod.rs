pub mod app;
pub mod dialogs;
pub mod gamepad;
pub mod images;
pub mod motion;
pub mod progress;
pub mod setup;
pub mod theme;
pub mod welcome;
pub mod widgets;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::Receiver;

pub const DEFAULT_WIDTH: f32 = 1088.0;
pub const DEFAULT_HEIGHT: f32 = 816.0;

/// Opens a URI; replaced in tests so links are checked, never launched.
pub type UriOpener = Rc<dyn Fn(&str)>;

/// Asks for a folder, starting at the given one. The answer arrives later:
/// the chosen folder, or `None` when the dialog was cancelled.
pub type FolderPicker = Rc<dyn Fn(&Path) -> Receiver<Option<PathBuf>>>;

/// Open `uri` with the default handler, such as Steam for `steam://` links.
pub fn launch_uri(uri: &str) {
    let target = uri.to_owned();
    std::thread::spawn(move || report_launch_failure(&target, open_with("xdg-open", &target)));
}

fn open_with(program: &str, uri: &str) -> std::io::Result<()> {
    let status = std::process::Command::new(program).arg(uri).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "{program} exited with {status}"
        )))
    }
}

/// Log why `target` could not be opened; there is no one else to tell.
fn report_launch_failure(target: &str, result: std::io::Result<()>) {
    if let Err(err) = result {
        tracing::warn!("Could not open {target}: {err}");
    }
}

/// Ask for a Steam library folder with the desktop's file chooser. Inside
/// Flatpak this goes through the portal, which grants the sandbox access.
pub fn pick_library_folder(initial_folder: &Path) -> Receiver<Option<PathBuf>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let initial_folder = initial_folder.to_path_buf();
    std::thread::spawn(move || {
        let folder = rfd::FileDialog::new()
            .set_title("Grant access to a Steam library")
            .set_directory(&initial_folder)
            .pick_folder();
        let _ = tx.send(folder);
    });
    rx
}

pub(crate) fn catch_ui_panic(label: &'static str, action: impl FnOnce()) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)).map_err(|payload| {
        let message = panic_message(payload.as_ref());
        tracing::error!("UI callback panicked in {label}: {message}");
        message
    })
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn catch_ui_panic_reports_panics_without_unwinding() {
        let result = super::catch_ui_panic("test callback", || panic!("boom"));

        assert_eq!(result, Err("boom".to_string()));
    }

    #[test]
    fn catch_ui_panic_returns_ok_for_successful_callbacks() {
        assert_eq!(super::catch_ui_panic("test callback", || {}), Ok(()));
    }

    #[test]
    fn catch_ui_panic_reports_owned_and_opaque_payloads() {
        let (owned, logs) = crate::test_log::capture_logs(|| {
            {
                // Formatting a runtime value makes an owned `String` payload.
                let what = String::from("load");
                super::catch_ui_panic("owned callback", || panic!("{what} failed"))
            }
        });
        assert_eq!(owned, Err("load failed".to_string()));
        assert!(logs.contains("UI callback panicked in owned callback: load failed"));

        let opaque = super::catch_ui_panic("opaque callback", || std::panic::panic_any(7));
        assert_eq!(opaque, Err("unknown panic payload".to_string()));
    }

    #[test]
    fn opening_reports_failed_and_missing_handlers() {
        assert!(super::open_with("true", "steam://validate/71250").is_ok());
        let failed = super::open_with("false", "steam://validate/71250").unwrap_err();
        assert!(
            failed.to_string().starts_with("false exited with"),
            "{failed}"
        );
        assert!(super::open_with("/nonexistent/xdg-open", "x").is_err());
    }

    #[test]
    fn only_failed_launches_are_logged() {
        let ((), logs) = crate::test_log::capture_logs(|| {
            super::report_launch_failure("steam://validate/71250", Ok(()));
        });
        assert_eq!(logs, "");

        let ((), logs) = crate::test_log::capture_logs(|| {
            super::report_launch_failure(
                "steam://validate/71250",
                Err(std::io::Error::other("no handler")),
            );
        });
        assert!(logs.contains("Could not open steam://validate/71250: no handler"));
    }

    #[test]
    fn launching_runs_xdg_open_in_the_background() {
        let _env = crate::test_env::lock();
        let bin = tempfile::tempdir().unwrap();
        let marker = bin.path().join("opened");
        let script = bin.path().join("xdg-open");
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", marker.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![bin.path().to_path_buf()];
        paths.extend(std::env::split_paths(&path));
        unsafe { std::env::set_var("PATH", std::env::join_paths(paths).unwrap()) };

        super::launch_uri("steam://rungameid/213610");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::fs::read_to_string(&marker).ok().as_deref() != Some("steam://rungameid/213610") {
            assert!(std::time::Instant::now() < deadline, "xdg-open never ran");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        unsafe { std::env::set_var("PATH", path) };
    }

    #[test]
    fn the_folder_picker_answers_none_without_a_portal_or_zenity() {
        use std::os::unix::fs::PermissionsExt;

        let _env = crate::test_env::lock();
        let previous_bus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS");
        let previous_path = std::env::var_os("PATH").unwrap_or_default();
        // No portal, and a zenity that is cancelled at once: no dialog opens.
        let bin = tempfile::tempdir().unwrap();
        let zenity = bin.path().join("zenity");
        std::fs::write(
            &zenity,
            "#!/bin/sh
exit 1
",
        )
        .unwrap();
        std::fs::set_permissions(&zenity, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut paths = vec![bin.path().to_path_buf()];
        paths.extend(std::env::split_paths(&previous_path));
        unsafe {
            std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/bus");
            std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        }

        let answer = super::pick_library_folder(std::path::Path::new("/tmp"))
            .recv_timeout(std::time::Duration::from_secs(30));

        unsafe { std::env::set_var("PATH", previous_path) };
        crate::test_env::set_var("DBUS_SESSION_BUS_ADDRESS", previous_bus);
        assert_eq!(answer, Ok(None));
    }
}
