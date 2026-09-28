use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::blocking;
use crate::steam;
use crate::ui::setup_page::AdventureModsSetupPage;
use crate::ui::welcome_page::AdventureModsWelcomePage;
use crate::ui::{WIZARD_DEFAULT_HEIGHT, WIZARD_DEFAULT_WIDTH};

mod imp {
    use super::*;
    use std::cell::Cell;
    use std::cell::RefCell;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/window.ui")]
    pub struct AdventureModsWindow {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub refresh_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub refresh_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub refresh_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub refresh_spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub status_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub status_banner: TemplateChild<gtk::Box>,
        #[template_child]
        pub status_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub welcome_page: TemplateChild<AdventureModsWelcomePage>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        pub extra_library_paths: RefCell<Vec<std::path::PathBuf>>,
        pub latest_detection_request_id: Cell<u64>,
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

            obj.apply_profile_style(crate::config::PROFILE);
            obj.setup_settings();
            obj.setup_header_actions();
            obj.setup_welcome_page_signals();
            obj.setup_restore_signal();
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

    /// Development builds get the striped header, so they are easy to tell apart.
    fn apply_profile_style(&self, profile: &str) {
        if profile == "development" {
            self.add_css_class("devel");
        }
    }

    fn setup_welcome_page_signals(&self) {
        let welcome_page = self.imp().welcome_page.clone();

        welcome_page.connect_local("library-access-granted", true, {
            let obj = self.clone();
            move |args| {
                let _ = crate::ui::catch_ui_panic("library access granted signal", || {
                    let Ok(path) = args[1].get::<String>() else {
                        return;
                    };

                    let path_buf = std::path::PathBuf::from(path);
                    handle_library_access_granted(
                        &obj.imp().extra_library_paths,
                        path_buf,
                        || obj.save_extra_library_paths(),
                        || obj.detect_games(),
                    );
                });
                None
            }
        });
    }

    fn setup_restore_signal(&self) {
        let welcome_page = self.imp().welcome_page.clone();
        welcome_page.connect_local("game-restored", true, {
            let obj = self.clone();
            move |args| {
                let _ = crate::ui::catch_ui_panic("game restored signal", || {
                    let message = args[1].get::<String>().unwrap_or_default();
                    obj.detect_games();
                    obj.imp().toast_overlay.add_toast(adw::Toast::new(&message));
                });
                None
            }
        });
    }

    fn setup_header_actions(&self) {
        let refresh_button = self.imp().refresh_button.clone();
        let obj = self.clone();
        refresh_button.connect_clicked(move |_| {
            let _ = crate::ui::catch_ui_panic("refresh button", || {
                obj.detect_games();
            });
        });

        // Setup changes what each game card shows, so scan again on return.
        let obj = self.clone();
        self.imp().navigation_view.connect_popped(move |_, page| {
            if page.tag().as_deref() == Some("setup") {
                let _ = crate::ui::catch_ui_panic("setup page closed", || {
                    obj.detect_games();
                });
            }
        });
    }

    fn setup_settings(&self) {
        let Some(settings) = crate::setup::config::app_settings() else {
            tracing::warn!(
                "GSettings schema '{}' not found, using default window size",
                crate::config::APP_ID
            );
            self.set_default_size(WIZARD_DEFAULT_WIDTH, WIZARD_DEFAULT_HEIGHT);
            return;
        };

        self.load_extra_library_paths(&settings);
        self.imp().settings.replace(Some(settings.clone()));

        self.set_default_size(settings.int("window-width"), settings.int("window-height"));
        self.set_maximized(settings.boolean("window-maximized"));

        self.connect_close_request(move |window| {
            let _ = crate::ui::catch_ui_panic("window close request", || {
                if !window.is_maximized() {
                    let _ = settings.set_int("window-width", window.width());
                    let _ = settings.set_int("window-height", window.height());
                }
                let _ = settings.set_boolean("window-maximized", window.is_maximized());
            });
            glib::Propagation::Proceed
        });
    }

    fn load_extra_library_paths(&self, settings: &gio::Settings) {
        let paths = crate::setup::config::load_extra_library_paths(Some(settings));
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
        let extra_library_paths = imp.extra_library_paths.borrow().clone();
        let request_id = next_detection_request_id(imp.latest_detection_request_id.get());
        imp.latest_detection_request_id.set(request_id);
        let obj = self.clone();

        self.set_refresh_busy(true);

        glib::spawn_future_local(async move {
            let result = blocking::spawn_result(
                gio::spawn_blocking(move || {
                    steam::library::detect_games_with_extra_libraries(&extra_library_paths)
                })
                .await,
            );
            obj.apply_detection_result(request_id, result);
        });
    }

    /// Show the outcome of scan `request_id`, unless a newer scan replaced it.
    fn apply_detection_result(
        &self,
        request_id: u64,
        result: anyhow::Result<steam::library::DetectionResult>,
    ) {
        let imp = self.imp();
        if !should_apply_detection_result(imp.latest_detection_request_id.get(), request_id) {
            return;
        }

        match result {
            Ok(result) => {
                imp.welcome_page
                    .set_detection_result(result, imp.navigation_view.clone());
                self.set_refresh_busy(false);
                self.clear_status_message();
            }
            Err(err) => {
                tracing::error!("Failed to detect games: {err}");
                self.set_refresh_busy(false);
                self.show_status_message(&format!("Failed to detect Steam libraries: {err}"), true);
            }
        }
    }

    fn set_refresh_busy(&self, busy: bool) {
        let imp = self.imp();
        imp.refresh_button.set_sensitive(!busy);
        let child: &gtk::Widget = if busy {
            imp.refresh_spinner.upcast_ref()
        } else {
            imp.refresh_icon.upcast_ref()
        };
        imp.refresh_stack.set_visible_child(child);
    }

    pub(crate) fn show_status_message(&self, message: &str, is_error: bool) {
        let imp = self.imp();
        imp.status_label.set_label(message);
        imp.status_banner.remove_css_class("status-banner-error");
        imp.status_icon.set_icon_name(Some(if is_error {
            "dialog-warning-symbolic"
        } else {
            "dialog-information-symbolic"
        }));
        if is_error {
            imp.status_banner.add_css_class("status-banner-error");
        }
        imp.status_revealer.set_reveal_child(true);
    }

    fn clear_status_message(&self) {
        let imp = self.imp();
        imp.status_label.set_label("");
        imp.status_banner.remove_css_class("status-banner-error");
        imp.status_revealer.set_reveal_child(false);
    }

    pub fn navigation_view(&self) -> &adw::NavigationView {
        &self.imp().navigation_view
    }

    #[cfg(test)]
    pub(crate) fn welcome_page(&self) -> AdventureModsWelcomePage {
        self.imp().welcome_page.clone()
    }

    pub fn push_setup_page(&self, game: steam::game::Game) {
        let setup_page = AdventureModsSetupPage::new(game);
        self.imp()
            .navigation_view
            .push(&setup_page.navigation_page());
    }
}

fn add_extra_library_path(
    extra_paths: &mut Vec<std::path::PathBuf>,
    path: std::path::PathBuf,
) -> bool {
    if extra_paths.iter().any(|existing| existing == &path) {
        return false;
    }

    extra_paths.push(path);
    true
}

fn handle_library_access_granted(
    extra_paths: &std::cell::RefCell<Vec<std::path::PathBuf>>,
    path: std::path::PathBuf,
    save: impl FnOnce(),
    refresh: impl FnOnce(),
) {
    let added = {
        let mut extra_paths = extra_paths.borrow_mut();
        add_extra_library_path(&mut extra_paths, path)
    };

    if added {
        save();
    }

    refresh();
}

fn next_detection_request_id(current: u64) -> u64 {
    current.wrapping_add(1)
}

fn should_apply_detection_result(latest_request_id: u64, request_id: u64) -> bool {
    latest_request_id == request_id
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::process::Command as ProcessCommand;

    use adw::prelude::*;
    use adw::subclass::prelude::ObjectSubclassIsExt;
    use gio::prelude::SettingsExt;
    use gtk::gio;

    use super::{
        AdventureModsWindow, add_extra_library_path, handle_library_access_granted,
        next_detection_request_id, should_apply_detection_result,
    };
    use crate::steam::game::{Game, GameKind};
    use crate::ui::test_util::init_resource_overlay;

    fn test_application() -> gtk::Application {
        gtk::Application::new(
            Some("io.github.astrovm.AdventureMods.WindowTests"),
            gio::ApplicationFlags::NON_UNIQUE,
        )
    }

    fn with_test_settings<T>(test: impl FnOnce(&gio::Settings) -> T) -> T {
        let _env = crate::test_env::lock();
        with_test_settings_locked(test)
    }

    /// Put `value` back into `key`, or unset it when it was unset before.
    fn restore_env_var(key: &str, value: Option<String>) {
        match value {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    /// [`with_test_settings`] for callers already holding the environment lock.
    fn with_test_settings_locked<T>(test: impl FnOnce(&gio::Settings) -> T) -> T {
        let schema_dir = tempfile::tempdir().unwrap();
        let schema_path = schema_dir
            .path()
            .join(format!("{}.gschema.xml", crate::config::APP_ID));
        let schema = include_str!("../data/io.github.astrovm.AdventureMods.gschema.xml")
            .replace("@APP_ID_RAW@", crate::config::APP_ID)
            .replace("@APP_PATH_RAW@", "/io/github/astrovm/AdventureMods/");
        std::fs::write(&schema_path, schema).unwrap();
        assert!(
            ProcessCommand::new("glib-compile-schemas")
                .arg(schema_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // GIO reads GSETTINGS_SCHEMA_DIR into its default schema source once.
        // Load it now, so a window shown below cannot pin this temporary one.
        let _ = gio::SettingsSchemaSource::default();
        let previous_schema_dir = std::env::var("GSETTINGS_SCHEMA_DIR").ok();
        let previous_backend = std::env::var("GSETTINGS_BACKEND").ok();
        unsafe {
            std::env::set_var("GSETTINGS_SCHEMA_DIR", schema_dir.path());
            std::env::set_var("GSETTINGS_BACKEND", "memory");
        }

        let source =
            gio::SettingsSchemaSource::from_directory(schema_dir.path(), None, true).unwrap();
        let schema = source.lookup(crate::config::APP_ID, true).unwrap();
        let settings = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
        let result = test(&settings);

        restore_env_var("GSETTINGS_SCHEMA_DIR", previous_schema_dir);
        restore_env_var("GSETTINGS_BACKEND", previous_backend);

        result
    }

    #[test]
    fn adding_a_granted_library_path_reports_new_paths_only() {
        let mut paths = vec![PathBuf::from("/data/SteamLibrary")];

        assert!(!add_extra_library_path(
            &mut paths,
            PathBuf::from("/data/SteamLibrary")
        ));
        assert!(add_extra_library_path(
            &mut paths,
            PathBuf::from("/run/user/1000/doc/abc123/SteamLibrary")
        ));
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/data/SteamLibrary"),
                PathBuf::from("/run/user/1000/doc/abc123/SteamLibrary")
            ]
        );
    }

    #[test]
    fn granting_library_saves_before_refreshing() {
        let paths = RefCell::new(Vec::new());
        let events = RefCell::new(Vec::new());
        let granted_path = PathBuf::from("/run/user/1000/doc/abc123/SteamLibrary");
        let expected_path = granted_path.clone();

        handle_library_access_granted(
            &paths,
            granted_path,
            || {
                assert_eq!(*paths.borrow(), vec![expected_path]);
                events.borrow_mut().push("saved");
            },
            || events.borrow_mut().push("refreshed"),
        );

        assert_eq!(*events.borrow(), vec!["saved", "refreshed"]);
    }

    #[test]
    fn newer_detection_request_replaces_older_one() {
        let first = next_detection_request_id(0);
        let second = next_detection_request_id(first);

        assert!(!should_apply_detection_result(second, first));
        assert!(should_apply_detection_result(second, second));
    }

    #[test]
    fn detection_request_ids_wrap_safely() {
        assert_eq!(next_detection_request_id(u64::MAX), 0);
    }

    #[gtk::test]
    fn window_handles_status_messages_and_detection_refresh() {
        init_resource_overlay();

        let app = test_application();
        let window = AdventureModsWindow::new(&app);

        window.show_status_message("A warning", true);
        assert_eq!(window.imp().status_label.label().as_str(), "A warning");
        assert!(
            window
                .imp()
                .status_banner
                .has_css_class("status-banner-error")
        );
        assert!(window.imp().status_revealer.reveals_child());

        window.show_status_message("A note", false);
        assert!(
            !window
                .imp()
                .status_banner
                .has_css_class("status-banner-error")
        );

        window.clear_status_message();
        assert_eq!(window.imp().status_label.label().as_str(), "");
        assert!(!window.imp().status_revealer.reveals_child());

        while gtk::glib::MainContext::default().iteration(false) {}
    }

    #[gtk::test]
    fn window_pushes_setup_page_and_accepts_granted_library_signal() {
        init_resource_overlay();

        let app = test_application();
        let window = AdventureModsWindow::new(&app);
        let game_dir = tempfile::tempdir().unwrap();

        window.push_setup_page(Game {
            kind: GameKind::SA2,
            path: game_dir.path().to_path_buf(),
        });

        let navigation = window.navigation_view();
        let visible_page = navigation.visible_page().unwrap();
        assert_eq!(visible_page.title().as_str(), "Sonic Adventure 2");
        assert_eq!(visible_page.tag().as_deref(), Some("setup"));

        // Leaving setup scans again so the cards show what changed.
        let before = window.imp().latest_detection_request_id.get();
        navigation.pop();
        while gtk::glib::MainContext::default().iteration(false) {}
        assert!(window.imp().latest_detection_request_id.get() > before);
        window.push_setup_page(Game {
            kind: GameKind::SA2,
            path: game_dir.path().to_path_buf(),
        });

        let granted = "/run/user/1000/doc/test-id/SteamLibrary";
        window
            .imp()
            .welcome_page
            .emit_by_name::<()>("library-access-granted", &[&granted]);
        assert_eq!(
            window.imp().extra_library_paths.borrow().as_slice(),
            &[PathBuf::from(granted)]
        );

        while gtk::glib::MainContext::default().iteration(false) {}
    }

    #[gtk::test]
    fn window_header_refresh_and_navigation_callbacks_are_wired() {
        init_resource_overlay();

        let app = test_application();
        let window = AdventureModsWindow::new(&app);
        window.imp().refresh_button.emit_clicked();
        window
            .imp()
            .welcome_page
            .emit_by_name::<()>("game-restored", &[&"Sonic Adventure 2 was restored."]);

        let navigation = window.navigation_view();
        let page = adw::NavigationPage::builder()
            .tag("synthetic")
            .title("Synthetic")
            .child(&gtk::Label::new(Some("Synthetic")))
            .build();
        navigation.push(&page);
        while gtk::glib::MainContext::default().iteration(false) {}
        navigation.pop();
        while gtk::glib::MainContext::default().iteration(false) {}

        window.imp().settings.replace(None);
        window.save_extra_library_paths();
    }

    #[gtk::test]
    fn window_loads_and_saves_settings_when_schema_is_available() {
        init_resource_overlay();

        with_test_settings(|settings| {
            settings.set_int("window-width", 1111).unwrap();
            settings.set_int("window-height", 777).unwrap();
            settings.set_boolean("window-maximized", true).unwrap();
            settings
                .set_strv("extra-library-paths", ["/tmp/synthetic-steam"])
                .unwrap();

            let app = test_application();
            let window = AdventureModsWindow::new(&app);

            assert_eq!(window.default_width(), 1111);
            assert_eq!(window.default_height(), 777);
            assert!(window.is_maximized());
            assert_eq!(
                window.imp().extra_library_paths.borrow().as_slice(),
                &[PathBuf::from("/tmp/synthetic-steam")]
            );

            window
                .imp()
                .extra_library_paths
                .borrow_mut()
                .push(PathBuf::from("/tmp/synthetic-extra"));
            window.save_extra_library_paths();
            assert_eq!(
                settings.strv("extra-library-paths").as_slice(),
                [
                    "/tmp/synthetic-steam".to_string(),
                    "/tmp/synthetic-extra".to_string()
                ]
            );
        });
    }

    /// Run the main loop until `done` holds, failing after a few seconds.
    fn wait_until(what: &str, done: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !done() {
            assert!(std::time::Instant::now() < deadline, "timed out: {what}");
            gtk::glib::MainContext::default().iteration(false);
        }
    }

    #[test]
    fn granting_the_same_library_twice_saves_it_once() {
        let granted = PathBuf::from("/data/SteamLibrary");
        let paths = RefCell::new(Vec::new());
        let events = RefCell::new(Vec::new());

        for _ in 0..2 {
            handle_library_access_granted(
                &paths,
                granted.clone(),
                || events.borrow_mut().push("saved"),
                || events.borrow_mut().push("refreshed"),
            );
        }

        assert_eq!(*events.borrow(), vec!["saved", "refreshed", "refreshed"]);
        assert_eq!(*paths.borrow(), vec![granted]);
    }

    #[test]
    fn test_settings_restore_the_callers_gsettings_environment() {
        let _env = crate::test_env::lock();
        let previous_schema_dir = std::env::var("GSETTINGS_SCHEMA_DIR").ok();
        let previous_backend = std::env::var("GSETTINGS_BACKEND").ok();
        unsafe {
            std::env::set_var("GSETTINGS_SCHEMA_DIR", "/caller/schemas");
            std::env::set_var("GSETTINGS_BACKEND", "caller-backend");
        }

        let backend_inside =
            with_test_settings_locked(|_| std::env::var("GSETTINGS_BACKEND").unwrap());
        let restored = (
            std::env::var("GSETTINGS_SCHEMA_DIR").ok(),
            std::env::var("GSETTINGS_BACKEND").ok(),
        );
        restore_env_var("GSETTINGS_SCHEMA_DIR", previous_schema_dir);
        restore_env_var("GSETTINGS_BACKEND", previous_backend);

        assert_eq!(backend_inside, "memory");
        assert_eq!(
            restored,
            (
                Some("/caller/schemas".to_string()),
                Some("caller-backend".to_string())
            )
        );
    }

    #[gtk::test]
    fn only_development_builds_get_the_devel_style() {
        init_resource_overlay();

        let app = test_application();
        let window = AdventureModsWindow::new(&app);
        assert_eq!(
            window.has_css_class("devel"),
            crate::config::PROFILE == "development"
        );

        window.apply_profile_style("default");
        assert_eq!(
            window.has_css_class("devel"),
            crate::config::PROFILE == "development"
        );

        window.apply_profile_style("development");
        assert!(window.has_css_class("devel"));
    }

    #[gtk::test]
    fn granted_library_signal_without_a_path_is_ignored() {
        init_resource_overlay();

        let app = test_application();
        let window = AdventureModsWindow::new(&app);
        let before = window.imp().latest_detection_request_id.get();

        window
            .imp()
            .welcome_page
            .emit_by_name::<()>("library-access-granted", &[&None::<String>]);

        assert!(window.imp().extra_library_paths.borrow().is_empty());
        assert_eq!(window.imp().latest_detection_request_id.get(), before);
    }

    #[gtk::test]
    fn failed_detection_is_reported_unless_a_newer_scan_replaced_it() {
        init_resource_overlay();

        let app = test_application();
        let window = AdventureModsWindow::new(&app);
        let stale = window.imp().latest_detection_request_id.get();
        window.detect_games();
        let current = window.imp().latest_detection_request_id.get();
        assert!(!window.imp().refresh_button.is_sensitive());

        let capture = crate::test_log::LogCapture::start();
        window.apply_detection_result(stale, Err(anyhow::anyhow!("stale failure")));
        assert_eq!(window.imp().status_label.label().as_str(), "");
        assert!(!window.imp().refresh_button.is_sensitive());

        window.apply_detection_result(current, Err(anyhow::anyhow!("spawn error: boom")));
        assert_eq!(
            window.imp().status_label.label().as_str(),
            "Failed to detect Steam libraries: spawn error: boom"
        );
        assert!(
            window
                .imp()
                .status_banner
                .has_css_class("status-banner-error")
        );
        assert!(window.imp().refresh_button.is_sensitive());
        let logs = capture.contents();
        assert!(logs.contains("Failed to detect games: spawn error: boom"));
        assert!(!logs.contains("stale failure"));
    }

    #[gtk::test]
    fn closing_the_window_remembers_its_size_unless_maximized() {
        init_resource_overlay();

        with_test_settings(|settings| {
            settings.set_int("window-width", 1111).unwrap();
            settings.set_int("window-height", 777).unwrap();
            settings.set_boolean("window-maximized", false).unwrap();

            let app = test_application();
            let window = AdventureModsWindow::new(&app);
            window.set_default_size(900, 600);
            window.present();
            wait_until("window is mapped", || window.width() > 0);
            let size = (window.width(), window.height());
            assert!(!window.is_maximized());

            window.close();
            assert_eq!(
                (settings.int("window-width"), settings.int("window-height")),
                size
            );
            assert!(!settings.boolean("window-maximized"));

            // A maximized size is not the one to restore, so keep the last.
            settings.set_int("window-width", 1234).unwrap();
            settings.set_int("window-height", 567).unwrap();
            settings.set_boolean("window-maximized", true).unwrap();
            let maximized = AdventureModsWindow::new(&app);
            maximized.present();
            wait_until("window is mapped", || maximized.width() > 0);
            assert!(maximized.is_maximized());

            maximized.close();
            assert_eq!(
                (settings.int("window-width"), settings.int("window-height")),
                (1234, 567)
            );
            assert!(settings.boolean("window-maximized"));
        });
    }
}
