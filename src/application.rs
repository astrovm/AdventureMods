use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::window::AdventureModsWindow;

/// libadwaita loads `style.css` (and the app icons) from under this path.
const RESOURCE_BASE_PATH: &str = "/io/github/astrovm/AdventureMods";

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct AdventureModsApplication {}

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsApplication {
        const NAME: &'static str = "AdventureModsApplication";
        type Type = super::AdventureModsApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for AdventureModsApplication {}

    impl ApplicationImpl for AdventureModsApplication {
        fn activate(&self) {
            let app = self.obj();

            let window = if let Some(window) = app.active_window() {
                window
            } else {
                let window = AdventureModsWindow::new(&*app);
                window.upcast()
            };

            window.present();
        }

        fn startup(&self) {
            self.parent_startup();

            let app = self.obj();

            app.set_accels_for_action("window.close", &["<Control>w"]);
            app.set_accels_for_action("app.quit", &["<Control>q"]);

            let quit_action = gio::ActionEntry::builder("quit")
                .activate(|app: &super::AdventureModsApplication, _, _| {
                    let _ = crate::ui::catch_ui_panic("quit action", || {
                        app.quit();
                    });
                })
                .build();

            let about_action = gio::ActionEntry::builder("about")
                .activate(|app: &super::AdventureModsApplication, _, _| {
                    let _ = crate::ui::catch_ui_panic("about action", || {
                        let about = adw::AboutDialog::builder()
                            .application_name(config::APP_NAME)
                            .application_icon(config::APP_ID)
                            .developer_name("astrovm")
                            .version(env!("CARGO_PKG_VERSION"))
                            .developers(vec!["astrovm"])
                            .copyright("2026 astrovm")
                            .license_type(gtk::License::MitX11)
                            .issue_url("https://github.com/astrovm/AdventureMods/issues")
                            .build();

                        let Some(window) = app.active_window() else {
                            tracing::warn!("About action activated without an active window");
                            return;
                        };
                        about.present(Some(&window));
                    });
                })
                .build();

            app.add_action_entries([quit_action, about_action]);
        }
    }

    impl GtkApplicationImpl for AdventureModsApplication {}
    impl AdwApplicationImpl for AdventureModsApplication {}
}

glib::wrapper! {
    pub struct AdventureModsApplication(ObjectSubclass<imp::AdventureModsApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for AdventureModsApplication {
    fn default() -> Self {
        Self::new()
    }
}

impl AdventureModsApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("flags", gio::ApplicationFlags::default())
            .property("resource-base-path", RESOURCE_BASE_PATH)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use adw::prelude::*;
    use gtk::gio;

    use super::AdventureModsApplication;
    use crate::config;
    use crate::ui::test_util::init_resource_overlay;

    #[test]
    fn stylesheet_is_bundled_where_libadwaita_loads_it() {
        let out = tempfile::tempdir().unwrap();
        let bundle = out.path().join("app.gresource");
        let status = std::process::Command::new("glib-compile-resources")
            .arg(format!("--sourcedir={}/data", env!("CARGO_MANIFEST_DIR")))
            .arg(format!("--target={}", bundle.display()))
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/data/resources.gresource.xml"
            ))
            .status()
            .unwrap();
        assert!(status.success(), "glib-compile-resources failed");

        let resource = gio::Resource::load(&bundle).unwrap();
        let stylesheet = resource
            .lookup_data(
                &format!("{}/style.css", super::RESOURCE_BASE_PATH),
                gio::ResourceLookupFlags::NONE,
            )
            .expect("style.css must sit at the resource base path");
        assert_eq!(
            stylesheet.as_ref(),
            include_bytes!("../data/resources/style.css")
        );
    }

    #[gtk::test]
    fn application_registers_actions_and_presents_a_window() {
        init_resource_overlay();

        let app = AdventureModsApplication::new();
        assert_eq!(app.application_id().as_deref(), Some(config::APP_ID));

        app.register(None::<&gio::Cancellable>).unwrap();

        assert!(app.lookup_action("quit").is_some());
        assert!(app.lookup_action("about").is_some());
        assert_eq!(
            app.accels_for_action("app.quit"),
            vec!["<Control>q".to_string()]
        );

        app.activate_action("about", None);
        app.activate();
        while glib::MainContext::default().iteration(false) {}
        assert!(app.active_window().is_some());

        app.activate();
        app.activate_action("about", None);
        app.activate_action("quit", None);
    }

    #[test]
    fn default_application_uses_the_configured_application_id() {
        let app = AdventureModsApplication::default();

        assert_eq!(app.application_id().as_deref(), Some(config::APP_ID));
    }
}
