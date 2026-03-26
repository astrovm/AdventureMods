mod application;
mod config;
mod external;
mod setup;
mod steam;
mod ui;
mod window;

use application::AdventureModsApplication;
use glib::ExitCode;
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> ExitCode {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt::init();

    glib::set_application_name(config::APP_NAME);

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

    let app = AdventureModsApplication::new();
    app.run()
}
