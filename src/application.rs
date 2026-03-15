use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::window::AdventureModsWindow;

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
                    app.quit();
                })
                .build();

            let about_action = gio::ActionEntry::builder("about")
                .activate(|app: &super::AdventureModsApplication, _, _| {
                    let about = adw::AboutDialog::builder()
                        .application_name("Adventure Mods")
                        .application_icon(config::APP_ID)
                        .developer_name("astrovm")
                        .version(config::VERSION)
                        .developers(vec!["astrovm"])
                        .copyright("2026 astrovm")
                        .license_type(gtk::License::Gpl30)
                        .issue_url("https://github.com/astrovm/AdventureMods/issues")
                        .build();

                    let window = app.active_window().unwrap();
                    about.present(Some(&window));
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

impl AdventureModsApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("flags", gio::ApplicationFlags::default())
            .property("resource-base-path", "/io/github/astrovm/AdventureMods")
            .build()
    }
}
