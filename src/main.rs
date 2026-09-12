use adventure_mods::application::AdventureModsApplication;
use adventure_mods::config;
use glib::ExitCode;
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> ExitCode {
    run_application(std::env::args().collect(), run_gui)
}

fn run_application(args: Vec<String>, launch_gui: impl FnOnce() -> ExitCode) -> ExitCode {
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
    initialize_gui();

    let app = AdventureModsApplication::new();
    app.run()
}

fn initialize_gui() {
    // Install the ring TLS provider. If the CLI path already installed it,
    // install_default returns an error which we can safely ignore.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let _ = tracing_subscriber::fmt::try_init();

    glib::set_application_name(config::APP_NAME);
    load_resources();
}

fn load_resources() {
    let gresource_name = "adventure-mods.gresource";

    let pkgdatadir = std::env::var("ADVENTURE_MODS_PKGDATADIR")
        .unwrap_or_else(|_| config::PKGDATADIR.to_string());

    let res = gio::Resource::load(std::path::PathBuf::from(&pkgdatadir).join(gresource_name))
        .or_else(|_| {
            // Fallback: look relative to the executable (covers AppImage and
            // local installs where the env var isn't set).
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()));
            if let Some(dir) = exe_dir {
                // Binary at <prefix>/bin/, gresource at <prefix>/share/adventure-mods/
                let path = dir.join("../share/adventure-mods").join(gresource_name);
                gio::Resource::load(path)
            } else {
                Err(glib::Error::new(gio::IOErrorEnum::NotFound, "no exe dir"))
            }
        })
        .or_else(|_| gio::Resource::load(std::path::PathBuf::from("data").join(gresource_name)));

    match res {
        Ok(res) => gio::resources_register(&res),
        Err(e) => eprintln!("Warning: failed to load GResources: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn restore_package_data_dir(value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => unsafe { std::env::set_var("ADVENTURE_MODS_PKGDATADIR", value) },
            None => unsafe { std::env::remove_var("ADVENTURE_MODS_PKGDATADIR") },
        }
    }

    #[test]
    fn command_line_exit_codes_short_circuit_gui_startup() {
        assert_eq!(
            run_application(
                vec!["adventure-mods".to_string(), "--version".to_string()],
                || ExitCode::from(42),
            ),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_application(
                vec!["adventure-mods".to_string(), "--unknown".to_string()],
                || ExitCode::from(42),
            ),
            ExitCode::FAILURE
        );
        assert_eq!(
            run_application(vec!["adventure-mods".to_string()], || ExitCode::from(42)),
            ExitCode::from(42)
        );
    }

    #[test]
    fn gui_initialization_handles_missing_resource_bundle() {
        let previous = std::env::var_os("ADVENTURE_MODS_PKGDATADIR");
        unsafe {
            std::env::set_var(
                "ADVENTURE_MODS_PKGDATADIR",
                "/definitely/missing/adventure-mods-data",
            );
        }

        initialize_gui();

        restore_package_data_dir(Some(std::ffi::OsString::from("/previous/path")));
        initialize_gui();

        restore_package_data_dir(None);
        initialize_gui();
        restore_package_data_dir(previous);
    }
}
