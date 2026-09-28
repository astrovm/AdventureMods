use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::path_display::display_path;

#[cfg(test)]
use crate::steam::game::Game;
use crate::steam::game::GameKind;

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/game_card.ui")]
    pub struct AdventureModsGameCard {
        #[template_child]
        pub cover_picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub status_row: TemplateChild<gtk::Box>,
        #[template_child]
        pub status_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub badge_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub install_selector: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub details_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub setup_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub secondary_button: TemplateChild<gtk::Button>,
        pub(super) install_options: RefCell<Vec<super::GameInstallOption>>,
        pub(super) kind: std::cell::Cell<Option<GameKind>>,
        pub(super) steam_repair: std::cell::Cell<bool>,
        pub setup_callback: RefCell<Option<Box<dyn Fn()>>>,
        pub secondary_callback: RefCell<Option<Box<dyn Fn()>>>,
    }

    impl std::fmt::Debug for AdventureModsGameCard {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AdventureModsGameCard").finish()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsGameCard {
        const NAME: &'static str = "AdventureModsGameCard";
        type Type = super::AdventureModsGameCard;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AdventureModsGameCard {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj().clone();
            let gesture = gtk::GestureClick::new();
            gesture.set_button(1);
            gesture.connect_released(move |_, _, _, _| {
                let _ = crate::ui::catch_ui_panic("game card click", || {
                    let imp = obj.imp();
                    if imp.install_selector.is_visible() {
                        return;
                    }
                    if let Some(ref cb) = *imp.setup_callback.borrow() {
                        cb();
                    }
                });
            });
            self.obj().add_controller(gesture);

            let obj = self.obj().clone();
            self.setup_button.connect_clicked(move |_| {
                let _ = crate::ui::catch_ui_panic("game card setup button", || {
                    let imp = obj.imp();
                    if let Some(ref cb) = *imp.setup_callback.borrow() {
                        cb();
                    }
                });
            });

            let obj = self.obj().clone();
            self.install_selector.connect_selected_notify(move |_| {
                let _ = crate::ui::catch_ui_panic("game card install selector", || {
                    obj.update_selected_install_option();
                });
            });

            let obj = self.obj().clone();
            self.secondary_button.connect_clicked(move |_| {
                let _ = crate::ui::catch_ui_panic("game card secondary button", || {
                    let imp = obj.imp();
                    if let Some(ref cb) = *imp.secondary_callback.borrow() {
                        cb();
                    }
                });
            });
        }
    }

    impl WidgetImpl for AdventureModsGameCard {}
    impl BoxImpl for AdventureModsGameCard {}
}

glib::wrapper! {
    pub struct AdventureModsGameCard(ObjectSubclass<imp::AdventureModsGameCard>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// Resource path of a game's Steam header image.
pub(crate) fn cover_resource(kind: GameKind) -> &'static str {
    match kind {
        GameKind::SADX => "/io/github/astrovm/AdventureMods/resources/covers/sadx.jpg",
        GameKind::SA2 => "/io/github/astrovm/AdventureMods/resources/covers/sa2.jpg",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GameInstallOption {
    Detected(std::path::PathBuf),
    Inaccessible(std::path::PathBuf),
}

impl GameInstallOption {
    pub(crate) fn detected(path: std::path::PathBuf) -> Self {
        Self::Detected(path)
    }

    pub(crate) fn inaccessible(path: std::path::PathBuf) -> Self {
        Self::Inaccessible(path)
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        match self {
            Self::Detected(path) | Self::Inaccessible(path) => path,
        }
    }

    fn is_accessible(&self) -> bool {
        matches!(self, Self::Detected(_))
    }

    fn selector_label(&self) -> String {
        if self.is_accessible() {
            display_path(self.path())
        } else {
            format!("{} (Needs access)", display_path(self.path()))
        }
    }
}

impl Default for AdventureModsGameCard {
    fn default() -> Self {
        Self::new()
    }
}

impl AdventureModsGameCard {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    #[cfg(test)]
    pub fn set_detected(&self, game: &Game, installation_index: usize, installation_total: usize) {
        let imp = self.imp();
        let status_text = if installation_total > 1 {
            format!(
                "Multiple installs found, this is {} of {}",
                installation_index + 1,
                installation_total
            )
        } else {
            String::from("Installed in Steam and ready for setup")
        };

        let show_status = installation_total > 1;

        imp.title_label.set_label(game.kind.name());
        imp.badge_label.set_label("Ready to set up");
        imp.status_label.set_visible(show_status);
        imp.status_label.set_label(&status_text);
        imp.details_label.set_label(&display_path(&game.path));
        self.set_tooltip_text(Some(&display_path(&game.path)));
        imp.setup_button.set_visible(true);
        imp.secondary_button.set_visible(false);
        self.add_css_class("game-card-clickable");
        self.set_cursor_from_name(Some("pointer"));

        self.set_cover(game.kind);
        self.set_state_classes("installed", None);

        let self_imp = self.imp();
        self_imp.install_selector.set_visible(false);
        self_imp.install_options.replace(Vec::new());
        self_imp.setup_callback.replace(None);
        self_imp.secondary_callback.replace(None);
    }

    pub fn set_missing(&self, kind: GameKind) {
        let imp = self.imp();

        imp.title_label.set_label(kind.name());
        imp.badge_label.set_label("Not installed");
        imp.status_label.set_visible(true);
        imp.status_label
            .set_label("Install it in Steam, then scan again.");
        imp.details_label.set_label("");
        self.set_tooltip_text(None);
        imp.install_selector.set_visible(false);
        imp.setup_button.set_visible(false);
        imp.secondary_button.set_visible(false);
        self.remove_css_class("game-card-clickable");
        self.set_cursor_from_name(None);

        self.set_cover(kind);
        self.set_state_classes("missing", Some("game-card-missing"));

        imp.install_options.replace(Vec::new());
        imp.setup_callback.replace(None);
        imp.secondary_callback.replace(None);
    }

    pub(crate) fn set_install_options(
        &self,
        kind: GameKind,
        install_options: &[GameInstallOption],
    ) {
        let imp = self.imp();

        imp.title_label.set_label(kind.name());
        self.set_cover(kind);

        imp.kind.set(Some(kind));
        imp.install_options.replace(install_options.to_vec());

        let labels: Vec<String> = install_options
            .iter()
            .map(GameInstallOption::selector_label)
            .collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let model = gtk::StringList::new(&label_refs);
        imp.install_selector.set_model(Some(&model));
        imp.install_selector.set_visible(install_options.len() > 1);

        let default_index = install_options
            .iter()
            .position(GameInstallOption::is_accessible)
            .unwrap_or(0) as u32;
        imp.install_selector.set_selected(default_index);
        imp.setup_button.set_visible(true);
        imp.secondary_button.set_visible(false);

        imp.setup_callback.replace(None);
        imp.secondary_callback.replace(None);
        self.update_selected_install_option();
    }

    /// Whether the selected install waits for Steam to repair its files.
    pub(crate) fn needs_steam_repair(&self) -> bool {
        self.imp().steam_repair.get()
    }

    pub(crate) fn selected_install_option(&self) -> Option<GameInstallOption> {
        let imp = self.imp();
        let install_options = imp.install_options.borrow();
        install_options
            .get(imp.install_selector.selected() as usize)
            .cloned()
            .or_else(|| install_options.first().cloned())
    }

    fn update_selected_install_option(&self) {
        let imp = self.imp();
        let install_count = imp.install_options.borrow().len();
        let Some(option) = self.selected_install_option() else {
            return;
        };

        imp.details_label.set_label(&display_path(option.path()));
        self.set_tooltip_text(Some(&display_path(option.path())));
        let modded = option.is_accessible()
            && imp
                .kind
                .get()
                .is_some_and(|kind| crate::setup::restore::is_modded(option.path(), kind));
        let steam_repair =
            option.is_accessible() && crate::setup::restore::needs_steam_repair(option.path());
        imp.steam_repair.set(steam_repair);
        imp.secondary_button.set_label("Restore");
        imp.secondary_button
            .set_tooltip_text(Some("Undo the setup so Steam starts the original game"));
        imp.secondary_button.set_visible(modded);

        if install_count > 1 {
            self.remove_css_class("game-card-clickable");
            self.set_cursor_from_name(None);
        } else {
            self.add_css_class("game-card-clickable");
            self.set_cursor_from_name(Some("pointer"));
        }

        if option.is_accessible() {
            imp.status_label.set_visible(install_count > 1);
            if install_count > 1 {
                imp.status_label
                    .set_label("Choose which install to set up.");
            }
            if steam_repair {
                // Restored, but Steam still has to put back converted files.
                imp.badge_label.set_label("Needs Steam repair");
                imp.status_label.set_visible(true);
                imp.status_label
                    .set_label("Let Steam verify the game files to finish restoring it.");
                imp.setup_button.set_label("Verify in Steam");
                imp.secondary_button.set_label("Set Up");
                imp.secondary_button
                    .set_tooltip_text(Some("Set up again once Steam has verified the files"));
                imp.secondary_button.set_visible(true);
                self.set_state_classes("repair", None);
            } else if modded {
                imp.badge_label.set_label("Mods installed");
                imp.setup_button.set_label("Change Mods");
                self.set_state_classes("modded", None);
            } else {
                imp.badge_label.set_label("Ready to set up");
                imp.setup_button.set_label("Set Up");
                self.set_state_classes("installed", None);
            }
        } else {
            imp.badge_label.set_label("Needs access");
            imp.status_label.set_visible(true);
            imp.status_label
                .set_label("Allow access to this Steam library to set it up.");
            imp.setup_button.set_label("Grant Access");
            self.set_state_classes("inaccessible", Some("game-card-inaccessible"));
        }
    }

    fn set_cover(&self, kind: GameKind) {
        self.imp()
            .cover_picture
            .set_resource(Some(cover_resource(kind)));
    }

    fn set_state_classes(&self, status_suffix: &str, extra_card_class: Option<&str>) {
        let imp = self.imp();

        for state in ["installed", "modded", "repair", "missing", "inaccessible"] {
            self.remove_css_class(&format!("game-card-{state}"));
            imp.status_row
                .remove_css_class(&format!("game-card-status-{state}"));
        }

        // A colored dot and short label, so the state reads at a glance.
        imp.status_icon.set_icon_name(Some("media-record-symbolic"));
        imp.status_row
            .add_css_class(&format!("game-card-status-{status_suffix}"));
        if let Some(card_class) = extra_card_class {
            self.add_css_class(card_class);
        }
    }

    pub fn connect_setup_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.imp().setup_callback.replace(Some(Box::new(callback)));
    }

    pub fn connect_secondary_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.imp()
            .secondary_callback
            .replace(Some(Box::new(callback)));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;

    use super::*;
    use crate::steam::game::Game;
    use crate::ui::test_util::init_resource_overlay;

    #[gtk::test]
    fn detected_cards_keep_setup_button_visible() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();
        let game = Game {
            kind: GameKind::SADX,
            path: PathBuf::from("/games/sadx"),
        };

        card.set_detected(&game, 0, 1);

        assert!(card.imp().setup_button.is_visible());
    }

    #[gtk::test]
    fn detected_single_install_cards_hide_status() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();
        let game = Game {
            kind: GameKind::SADX,
            path: PathBuf::from("/games/sadx"),
        };

        card.set_detected(&game, 0, 1);

        assert!(!card.imp().status_label.is_visible());
    }

    #[gtk::test]
    fn detected_cards_show_duplicate_install_status() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();
        let game = Game {
            kind: GameKind::SADX,
            path: PathBuf::from("/games/sadx"),
        };

        card.set_detected(&game, 1, 2);

        assert!(card.imp().status_label.is_visible());
        assert_eq!(
            card.imp().status_label.label().as_str(),
            "Multiple installs found, this is 2 of 2"
        );
    }

    #[gtk::test]
    fn missing_cards_hide_setup_button() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();

        card.set_missing(GameKind::SA2);

        assert!(!card.imp().setup_button.is_visible());
    }

    #[gtk::test]
    fn cards_show_install_selector_for_multiple_paths() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();

        card.set_install_options(
            GameKind::SA2,
            &[
                GameInstallOption::detected(PathBuf::from("/games/sa2-a")),
                GameInstallOption::inaccessible(PathBuf::from("/mnt/steam")),
            ],
        );

        assert!(card.imp().install_selector.is_visible());
    }

    #[gtk::test]
    fn cards_with_install_selector_are_not_marked_clickable() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();

        card.set_install_options(
            GameKind::SA2,
            &[
                GameInstallOption::detected(PathBuf::from("/games/sa2-a")),
                GameInstallOption::inaccessible(PathBuf::from("/mnt/steam")),
            ],
        );

        assert!(!card.has_css_class("game-card-clickable"));
    }

    #[gtk::test]
    fn cards_dispatch_callbacks_and_update_selected_install_state() {
        init_resource_overlay();

        let card = AdventureModsGameCard::default();
        assert!(format!("{:?}", card).starts_with("AdventureModsGameCard"));
        assert_eq!(format!("{:?}", card.imp()), "AdventureModsGameCard");
        assert!(card.selected_install_option().is_none());

        card.set_install_options(
            GameKind::SA2,
            &[GameInstallOption::detected(PathBuf::from("/games/sa2"))],
        );
        let setup_calls = Rc::new(Cell::new(0));
        let setup_calls_clone = setup_calls.clone();
        card.connect_setup_clicked(move || {
            setup_calls_clone.set(setup_calls_clone.get() + 1);
        });
        card.imp().setup_button.emit_clicked();
        assert_eq!(setup_calls.get(), 1);

        let secondary_calls = Rc::new(Cell::new(0));
        let secondary_calls_clone = secondary_calls.clone();
        card.connect_secondary_clicked(move || {
            secondary_calls_clone.set(secondary_calls_clone.get() + 1);
        });
        card.imp().secondary_button.set_visible(true);
        card.imp().secondary_button.emit_clicked();
        assert_eq!(secondary_calls.get(), 1);

        card.set_install_options(
            GameKind::SA2,
            &[
                GameInstallOption::detected(PathBuf::from("/games/sa2")),
                GameInstallOption::inaccessible(PathBuf::from("/mnt/steam")),
            ],
        );
        card.imp().install_selector.set_selected(1);
        assert!(matches!(
            card.selected_install_option(),
            Some(GameInstallOption::Inaccessible(_))
        ));
        assert_eq!(card.imp().badge_label.label().as_str(), "Needs access");

        assert!(card.has_css_class("game-card-inaccessible"));
        assert!(
            card.imp()
                .status_row
                .has_css_class("game-card-status-inaccessible")
        );

        card.imp().install_selector.set_selected(0);
        assert_eq!(card.imp().badge_label.label().as_str(), "Ready to set up");
        assert!(!card.has_css_class("game-card-inaccessible"));
        assert!(
            card.imp()
                .status_row
                .has_css_class("game-card-status-installed")
        );

        card.set_install_options(GameKind::SA2, &[]);
        card.set_state_classes("custom", None);
        assert!(
            card.imp()
                .status_row
                .has_css_class("game-card-status-custom")
        );
        assert_eq!(
            card.imp().status_icon.icon_name().as_deref(),
            Some("media-record-symbolic")
        );
    }

    fn click_card(card: &AdventureModsGameCard) {
        let gesture = card
            .observe_controllers()
            .item(0)
            .and_downcast::<gtk::GestureClick>()
            .unwrap();
        gesture.emit_by_name::<()>("released", &[&1i32, &0f64, &0f64]);
    }

    #[gtk::test]
    fn clicking_a_card_with_an_install_selector_does_not_start_setup() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();
        card.set_install_options(
            GameKind::SADX,
            &[
                GameInstallOption::detected(PathBuf::from("/games/sadx-a")),
                GameInstallOption::detected(PathBuf::from("/games/sadx-b")),
            ],
        );
        let setup_calls = Rc::new(Cell::new(0));
        card.connect_setup_clicked({
            let setup_calls = setup_calls.clone();
            move || setup_calls.set(setup_calls.get() + 1)
        });

        // The click is meant for the selector; only the button starts setup.
        click_card(&card);
        assert_eq!(setup_calls.get(), 0);

        card.imp().setup_button.emit_clicked();
        assert_eq!(setup_calls.get(), 1);
    }

    #[gtk::test]
    fn refreshing_a_card_drops_callbacks_for_the_previous_install() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();
        let options = [GameInstallOption::detected(PathBuf::from("/games/sa2"))];
        card.set_install_options(GameKind::SA2, &options);
        let calls = Rc::new(Cell::new(0));
        card.connect_setup_clicked({
            let calls = calls.clone();
            move || calls.set(calls.get() + 1)
        });
        card.connect_secondary_clicked({
            let calls = calls.clone();
            move || calls.set(calls.get() + 1)
        });
        click_card(&card);
        card.imp().secondary_button.set_visible(true);
        card.imp().secondary_button.emit_clicked();
        assert_eq!(calls.get(), 2);

        card.set_install_options(GameKind::SA2, &options);
        card.imp().secondary_button.set_visible(true);
        click_card(&card);
        card.imp().setup_button.emit_clicked();
        card.imp().secondary_button.emit_clicked();

        assert_eq!(calls.get(), 2);
        assert!(card.has_css_class("game-card-clickable"));
    }
}
