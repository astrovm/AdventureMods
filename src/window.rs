use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::steam;
use crate::ui::setup_page::AdventureModsSetupPage;
use crate::ui::welcome_page::AdventureModsWelcomePage;
use crate::ui::{WIZARD_DEFAULT_HEIGHT, WIZARD_DEFAULT_WIDTH};

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/window.ui")]
    pub struct AdventureModsWindow {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub refresh_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub welcome_page: TemplateChild<AdventureModsWelcomePage>,
        pub extra_library_paths: RefCell<Vec<std::path::PathBuf>>,
        pub settings: RefCell<Option<gio::Settings>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsWindow {
        const NAME: &'static str = "AdventureModsWindow";
        type Type = super::AdventureModsWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            AdventureModsWelcomePage::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AdventureModsWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            if crate::config::PROFILE == "development" {
                obj.add_css_class("devel");
            }

            obj.setup_settings();
            obj.setup_header_actions();
            obj.setup_welcome_page_signals();
            obj.detect_games();
        }
    }

    impl WidgetImpl for AdventureModsWindow {}
    impl WindowImpl for AdventureModsWindow {}
    impl ApplicationWindowImpl for AdventureModsWindow {}
    impl AdwApplicationWindowImpl for AdventureModsWindow {}
}

glib::wrapper! {
    pub struct AdventureModsWindow(ObjectSubclass<imp::AdventureModsWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl AdventureModsWindow {
    pub fn new(app: &impl IsA<gtk::Application>) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn setup_welcome_page_signals(&self) {
        let welcome_page = self.imp().welcome_page.clone();

        welcome_page.connect_local("library-access-granted", true, {
            let obj = self.clone();
            move |args| {
                let Ok(path) = args[1].get::<String>() else {
                    return None;
                };

                let path_buf = std::path::PathBuf::from(path);
                let mut extra_paths = obj.imp().extra_library_paths.borrow_mut();
                if !extra_paths.iter().any(|existing| existing == &path_buf) {
                    extra_paths.push(path_buf);
                    obj.save_extra_library_paths();
                }

                obj.detect_games();
                None
            }
        });
    }

    fn setup_header_actions(&self) {
        let refresh_button = self.imp().refresh_button.clone();
        let obj = self.clone();
        refresh_button.connect_clicked(move |_| {
            obj.detect_games();
        });

        // Show/hide refresh button based on current navigation page
        let nav_view = self.imp().navigation_view.clone();
        let refresh_button_clone = refresh_button.clone();
        nav_view.connect_visible_page_notify(move |nav| {
            let is_welcome = nav
                .visible_page()
                .map(|page| page.tag() == Some("welcome".into()))
                .unwrap_or(false);
            refresh_button_clone.set_visible(is_welcome);
        });
    }

    fn setup_settings(&self) {
        let schema_source = gio::SettingsSchemaSource::default();
        let has_schema =
            schema_source.is_some_and(|s| s.lookup(crate::config::APP_ID, true).is_some());

        if !has_schema {
            tracing::warn!(
                "GSettings schema '{}' not found, using default window size",
                crate::config::APP_ID
            );
            self.set_default_size(WIZARD_DEFAULT_WIDTH, WIZARD_DEFAULT_HEIGHT);
            return;
        }

        let settings = gio::Settings::new(crate::config::APP_ID);
        self.load_extra_library_paths(&settings);
        self.imp().settings.replace(Some(settings.clone()));

        self.set_default_size(settings.int("window-width"), settings.int("window-height"));
        self.set_maximized(settings.boolean("window-maximized"));

        self.connect_close_request(move |window| {
            let _ = settings.set_int("window-width", window.default_size().0);
            let _ = settings.set_int("window-height", window.default_size().1);
            let _ = settings.set_boolean("window-maximized", window.is_maximized());
            glib::Propagation::Proceed
        });
    }

    fn load_extra_library_paths(&self, settings: &gio::Settings) {
        let paths = settings
            .strv("extra-library-paths")
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        self.imp().extra_library_paths.replace(paths);
    }

    fn save_extra_library_paths(&self) {
        let Some(settings) = self.imp().settings.borrow().clone() else {
            return;
        };

        let extra_paths = self.imp().extra_library_paths.borrow();
        let path_strings: Vec<String> = extra_paths
            .iter()
            .filter_map(|path| path.to_str().map(String::from))
            .collect();

        let refs: Vec<&str> = path_strings.iter().map(String::as_str).collect();
        let _ = settings.set_strv("extra-library-paths", refs);
    }

    fn detect_games(&self) {
        let imp = self.imp();
        let welcome_page = imp.welcome_page.clone();
        let nav_view = imp.navigation_view.clone();
        let extra_library_paths = imp.extra_library_paths.borrow().clone();

        glib::spawn_future_local(async move {
            let result = match gio::spawn_blocking(move || {
                steam::library::detect_games_with_extra_libraries(&extra_library_paths)
            })
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    tracing::error!("Failed to detect games: {err:?}");
                    return;
                }
            };

            welcome_page.set_detection_result(result, nav_view);
        });
    }

    pub fn navigation_view(&self) -> &adw::NavigationView {
        &self.imp().navigation_view
    }

    pub fn push_setup_page(&self, game: steam::game::Game) {
        let setup_page = AdventureModsSetupPage::new(game);
        let nav_page = adw::NavigationPage::builder()
            .title("Setup")
            .child(&setup_page)
            .build();
        self.imp().navigation_view.push(&nav_page);
    }
}
