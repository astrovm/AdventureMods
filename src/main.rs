use std::process::ExitCode;

use adventure_mods::config;
use adventure_mods::ui::app::{AdventureModsApp, EframeApp, Services, WindowState};
use adventure_mods::ui::gamepad::Gamepad;

fn main() -> ExitCode {
    run_application(std::env::args().collect(), run_gui)
}

fn run_application(args: Vec<String>, launch_gui: fn() -> ExitCode) -> ExitCode {
    match adventure_mods::cli::run_from_args(args) {
        Ok(true) => return ExitCode::SUCCESS,
        Ok(false) => {}
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    }

    launch_gui()
}

fn run_gui() -> ExitCode {
    // Install the ring TLS provider. If the CLI path already installed it,
    // install_default returns an error which we can safely ignore.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _ = tracing_subscriber::fmt::try_init();

    let settings = adventure_mods::setup::config::app_settings();
    let options = native_options(WindowState::load(settings.as_ref()), in_game_mode());
    let result = eframe::run_native(
        config::APP_NAME,
        options,
        Box::new(|creation| {
            // Before the first frame, so Rubik is there from the start.
            adventure_mods::ui::theme::apply(&creation.egui_ctx);
            let mut app = AdventureModsApp::new(Services::real(), settings);
            app.set_gamepad(Gamepad::spawn(&creation.egui_ctx));
            Ok(Box::new(EframeApp::new(app)))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Could not open the window: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Steam's Game Mode runs apps inside gamescope, where fullscreen fits best.
fn in_game_mode() -> bool {
    std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
        || std::env::var_os("SteamGamepadUI").is_some()
}

fn native_options(window: WindowState, fullscreen: bool) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(config::APP_NAME)
            .with_app_id(config::APP_ID)
            .with_icon(std::sync::Arc::new(
                adventure_mods::ui::images::window_icon(),
            ))
            .with_inner_size(window.size)
            // On Wayland with fractional scaling, eframe sees the monitor as
            // smaller than it is; the app checks the size itself instead.
            .with_clamp_size_to_monitor_size(false)
            .with_min_inner_size([360.0, 480.0])
            .with_maximized(window.maximized)
            .with_fullscreen(fullscreen),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_gui() -> ExitCode {
        ExitCode::from(42)
    }

    #[test]
    fn command_line_exit_codes_short_circuit_gui_startup() {
        assert_eq!(
            run_application(
                vec!["adventure-mods".to_string(), "--version".to_string()],
                fake_gui,
            ),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_application(
                vec!["adventure-mods".to_string(), "--unknown".to_string()],
                fake_gui,
            ),
            ExitCode::FAILURE
        );
        assert_eq!(
            run_application(vec!["adventure-mods".to_string()], fake_gui),
            ExitCode::from(42)
        );
    }

    #[test]
    fn the_window_opens_at_the_saved_size_and_fills_game_mode() {
        let window = WindowState {
            size: egui::vec2(1111.0, 777.0),
            maximized: true,
            saved: true,
        };

        let options = native_options(window, true);

        assert_eq!(options.viewport.inner_size, Some(egui::vec2(1111.0, 777.0)));
        assert_eq!(options.viewport.maximized, Some(true));
        assert_eq!(options.viewport.fullscreen, Some(true));
        assert_eq!(options.viewport.clamp_size_to_monitor_size, Some(false));
        assert_eq!(options.viewport.app_id.as_deref(), Some(config::APP_ID));
        let _ = in_game_mode();
    }
}
