use std::path::Path;

use adventure_mods::application::AdventureModsApplication;
use adventure_mods::config;
use glib::ExitCode;
use gtk::prelude::*;
use gtk::{gio, glib};

const GRESOURCE_NAME: &str = "adventure-mods.gresource";

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
    if let Err(e) = load_resources() {
        eprintln!("Warning: failed to load GResources: {e}");
    }
}

fn load_resources() -> Result<(), glib::Error> {
    let pkgdatadir = std::env::var("ADVENTURE_MODS_PKGDATADIR")
        .unwrap_or_else(|_| config::PKGDATADIR.to_string());
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let res = load_resource_bundle(
        Path::new(&pkgdatadir),
        exe_dir.as_deref(),
        Path::new("data"),
    )?;
    gio::resources_register(&res);
    Ok(())
}

/// Load the resource bundle from the package data directory, falling back to
/// the install prefix of the executable and then to the source data directory.
fn load_resource_bundle(
    pkgdatadir: &Path,
    exe_dir: Option<&Path>,
    source_data_dir: &Path,
) -> Result<gio::Resource, glib::Error> {
    gio::Resource::load(pkgdatadir.join(GRESOURCE_NAME))
        .or_else(|_| {
            // Fallback: look relative to the executable (covers AppImage and
            // local installs where the env var isn't set).
            if let Some(dir) = exe_dir {
                // Binary at <prefix>/bin/, gresource at <prefix>/share/adventure-mods/
                let path = dir.join("../share/adventure-mods").join(GRESOURCE_NAME);
                gio::Resource::load(path)
            } else {
                Err(glib::Error::new(gio::IOErrorEnum::NotFound, "no exe dir"))
            }
        })
        .or_else(|_| gio::Resource::load(source_data_dir.join(GRESOURCE_NAME)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap()
    }

    fn fake_gui() -> ExitCode {
        ExitCode::from(42)
    }

    /// Point `ADVENTURE_MODS_PKGDATADIR` at `dir` while running `test`.
    fn with_package_data_dir<T>(dir: &Path, test: impl FnOnce() -> T) -> T {
        let _guard = env_lock();
        unsafe { std::env::set_var("ADVENTURE_MODS_PKGDATADIR", dir) };
        let result = test();
        unsafe { std::env::remove_var("ADVENTURE_MODS_PKGDATADIR") };
        result
    }

    /// Compile a one-file resource bundle named like the app's into `dir`.
    /// The file `<prefix>/marker.txt` holds `prefix`.
    fn write_resource_bundle(dir: &Path, prefix: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("marker.txt"), prefix).unwrap();
        std::fs::write(
            source.path().join("bundle.gresource.xml"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  \
                 <gresource prefix=\"{prefix}\">\n    <file>marker.txt</file>\n  \
                 </gresource>\n</gresources>\n"
            ),
        )
        .unwrap();
        let status = std::process::Command::new("glib-compile-resources")
            .arg("--sourcedir")
            .arg(source.path())
            .arg("--target")
            .arg(dir.join(GRESOURCE_NAME))
            .arg(source.path().join("bundle.gresource.xml"))
            .status()
            .unwrap();
        assert!(status.success(), "glib-compile-resources failed");
    }

    fn marker(resource: &gio::Resource, prefix: &str) -> String {
        let bytes = resource
            .lookup_data(
                &format!("{prefix}/marker.txt"),
                gio::ResourceLookupFlags::NONE,
            )
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
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
    fn resource_bundle_prefers_the_package_data_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let pkgdatadir = tmp.path().join("pkgdata");
        let prefix = tmp.path().join("prefix");
        let source = tmp.path().join("source");
        write_resource_bundle(&pkgdatadir, "/test/pkgdata");
        write_resource_bundle(&prefix.join("share/adventure-mods"), "/test/prefix");
        write_resource_bundle(&source, "/test/source");

        let resource =
            load_resource_bundle(&pkgdatadir, Some(&prefix.join("bin")), &source).unwrap();

        assert_eq!(marker(&resource, "/test/pkgdata"), "/test/pkgdata");
    }

    #[test]
    fn resource_bundle_falls_back_to_the_executable_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let prefix = tmp.path().join("prefix");
        let source = tmp.path().join("source");
        write_resource_bundle(&prefix.join("share/adventure-mods"), "/test/prefix");
        write_resource_bundle(&source, "/test/source");
        std::fs::create_dir_all(prefix.join("bin")).unwrap();

        let resource = load_resource_bundle(
            &tmp.path().join("missing"),
            Some(&prefix.join("bin")),
            &source,
        )
        .unwrap();

        assert_eq!(marker(&resource, "/test/prefix"), "/test/prefix");
    }

    #[test]
    fn resource_bundle_falls_back_to_the_source_data_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        write_resource_bundle(&source, "/test/source");

        let without_exe_dir =
            load_resource_bundle(&tmp.path().join("missing"), None, &source).unwrap();
        assert_eq!(marker(&without_exe_dir, "/test/source"), "/test/source");

        std::fs::create_dir_all(tmp.path().join("bin")).unwrap();
        let without_prefix_bundle = load_resource_bundle(
            &tmp.path().join("missing"),
            Some(&tmp.path().join("bin")),
            &source,
        )
        .unwrap();
        assert_eq!(
            marker(&without_prefix_bundle, "/test/source"),
            "/test/source"
        );
    }

    #[test]
    fn resource_bundle_reports_the_last_missing_location() {
        let tmp = tempfile::tempdir().unwrap();

        let error = load_resource_bundle(
            &tmp.path().join("missing"),
            None,
            &tmp.path().join("missing-source"),
        )
        .unwrap_err();

        assert!(
            error.message().contains("missing-source"),
            "error was: {error}"
        );
    }

    #[test]
    fn gui_initialization_registers_resources_from_the_package_data_directory() {
        let tmp = tempfile::tempdir().unwrap();
        write_resource_bundle(tmp.path(), "/test/gui-initialization");

        with_package_data_dir(tmp.path(), initialize_gui);

        let registered = gio::resources_lookup_data(
            "/test/gui-initialization/marker.txt",
            gio::ResourceLookupFlags::NONE,
        )
        .unwrap();
        assert_eq!(&registered[..], b"/test/gui-initialization");
        assert_eq!(glib::application_name().as_deref(), Some(config::APP_NAME));
    }

    #[test]
    fn gui_initialization_continues_without_a_resource_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing");

        // The source data directory holds no compiled bundle either.
        let error = with_package_data_dir(&missing, load_resources).unwrap_err();
        assert!(
            error.message().contains(GRESOURCE_NAME),
            "error was: {error}"
        );

        with_package_data_dir(&missing, initialize_gui);
        assert_eq!(glib::application_name().as_deref(), Some(config::APP_NAME));
    }
}
