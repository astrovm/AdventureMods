//! Start the real window. CI runs the tests under Xvfb, so a display exists.

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Run the app with a fresh home, killing it if it never exits.
fn run_app(home: &Path, configure: impl FnOnce(&mut Command)) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_adventure-mods"));
    command
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("ADVENTURE_MODS_QUIT_AFTER_FIRST_FRAME", "1")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&mut command);
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("the app did not quit after its first frame");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn the_window_opens_draws_and_saves_its_size() {
    let home = tempfile::tempdir().unwrap();

    let output = run_app(home.path(), |_| {});

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let settings = home
        .path()
        .join("config/io.github.astrovm.AdventureMods/settings.json");
    let saved = std::fs::read_to_string(settings).unwrap();
    assert!(saved.contains("window-width"), "{saved}");
}

#[test]
fn without_a_display_the_app_explains_why_it_cannot_start() {
    let home = tempfile::tempdir().unwrap();

    let output = run_app(home.path(), |command| {
        command.env_remove("DISPLAY");
    });

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Could not open the window"), "{stderr}");
}
