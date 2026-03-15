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

    glib::set_application_name("Adventure Mods");

    let res = gio::Resource::load(
        [config::PKGDATADIR, "adventure-mods.gresource"]
            .iter()
            .collect::<std::path::PathBuf>(),
    )
    .or_else(|_| gio::Resource::load("data/adventure-mods.gresource"))
    .ok();

    if let Some(res) = res {
        gio::resources_register(&res);
    }

    let app = AdventureModsApplication::new();
    app.run()
}
