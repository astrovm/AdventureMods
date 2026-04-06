use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::steam::game::{Game, GameKind};

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/game_card.ui")]
    pub struct AdventureModsGameCard {
        #[template_child]
        pub cover_picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub status_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub badge_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub details_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub setup_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub secondary_button: TemplateChild<gtk::Button>,
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
                let imp = obj.imp();
                if let Some(ref cb) = *imp.setup_callback.borrow() {
                    cb();
                }
            });
            self.obj().add_controller(gesture);

            let obj = self.obj().clone();
            self.setup_button.connect_clicked(move |_| {
                let imp = obj.imp();
                if let Some(ref cb) = *imp.setup_callback.borrow() {
                    cb();
                }
            });

            let obj = self.obj().clone();
            self.secondary_button.connect_clicked(move |_| {
                let imp = obj.imp();
                if let Some(ref cb) = *imp.secondary_callback.borrow() {
                    cb();
                }
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

impl AdventureModsGameCard {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

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

        imp.title_label.set_label(game.kind.name());
        imp.badge_label.set_label("Ready");
        imp.status_label.set_visible(false);
        imp.status_label.set_label(&status_text);
        imp.details_label.set_visible(true);
        imp.details_label
            .set_label(&game.path.display().to_string());
        imp.setup_button.set_visible(true);
        imp.secondary_button.set_visible(false);
        self.add_css_class("game-card-clickable");
        self.set_cursor_from_name(Some("pointer"));

        self.set_cover(game.kind);
        self.set_state_classes("installed", None);

        let self_imp = self.imp();
        self_imp.setup_callback.replace(None);
        self_imp.secondary_callback.replace(None);
    }

    pub fn set_missing(&self, kind: GameKind) {
        let imp = self.imp();

        imp.title_label.set_label(kind.name());
        imp.badge_label.set_label("Not found");
        imp.status_label.set_visible(true);
        imp.status_label
            .set_label("Install it through Steam to enable setup.");
        imp.details_label.set_visible(false);
        imp.details_label.set_label(
            "If it is already installed, refresh after Steam finishes detecting the library.",
        );
        imp.setup_button.set_visible(false);
        imp.secondary_button.set_visible(false);
        self.remove_css_class("game-card-clickable");
        self.set_cursor_from_name(None);

        self.set_cover(kind);
        self.set_state_classes("missing", Some("game-card-missing"));

        imp.setup_callback.replace(None);
        imp.secondary_callback.replace(None);
    }

    pub fn set_inaccessible(&self, kind: GameKind, library_path: &std::path::Path) {
        let imp = self.imp();

        imp.title_label.set_label(kind.name());
        imp.badge_label.set_label("Needs access");
        imp.status_label.set_visible(true);
        imp.status_label
            .set_label("Grant access to this Steam library to continue.");
        imp.details_label.set_visible(true);
        imp.details_label
            .set_label(&format!("Steam library path: {}", library_path.display()));
        imp.setup_button.set_visible(false);
        imp.secondary_button.set_visible(true);
        imp.secondary_button.set_label("Grant Access");
        self.remove_css_class("game-card-clickable");
        self.set_cursor_from_name(None);

        self.set_cover(kind);
        self.set_state_classes("inaccessible", Some("game-card-inaccessible"));

        imp.setup_callback.replace(None);
        imp.secondary_callback.replace(None);
    }

    fn set_cover(&self, kind: GameKind) {
        let resource = match kind {
            GameKind::SADX => "/io/github/astrovm/AdventureMods/resources/covers/sadx.jpg",
            GameKind::SA2 => "/io/github/astrovm/AdventureMods/resources/covers/sa2.jpg",
        };
        self.imp().cover_picture.set_resource(Some(resource));
    }

    fn set_state_classes(&self, status_suffix: &str, extra_card_class: Option<&str>) {
        let imp = self.imp();

        for class_name in [
            "game-card-status-installed",
            "game-card-status-missing",
            "game-card-status-inaccessible",
            "game-card-badge-installed",
            "game-card-badge-missing",
            "game-card-badge-inaccessible",
            "game-card-status-icon-installed",
            "game-card-status-icon-missing",
            "game-card-status-icon-inaccessible",
            "game-card-missing",
            "game-card-inaccessible",
        ] {
            imp.status_label.remove_css_class(class_name);
            imp.badge_label.remove_css_class(class_name);
            imp.status_icon.remove_css_class(class_name);
            self.remove_css_class(class_name);
        }

        let status_class = format!("game-card-status-{status_suffix}");
        let badge_class = format!("game-card-badge-{status_suffix}");
        let icon_class = format!("game-card-status-icon-{status_suffix}");
        let icon_name = match status_suffix {
            "installed" => "emblem-ok-symbolic",
            "missing" => "dialog-warning-symbolic",
            "inaccessible" => "folder-open-symbolic",
            _ => "dialog-information-symbolic",
        };

        imp.status_icon.set_icon_name(Some(icon_name));
        imp.status_label.add_css_class(&status_class);
        imp.badge_label.add_css_class(&badge_class);
        imp.status_icon.add_css_class(&icon_class);
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
    use std::path::PathBuf;
    use std::sync::Once;

    use super::*;

    fn init_resource_overlay() {
        static INIT: Once = Once::new();

        INIT.call_once(|| unsafe {
            std::env::set_var(
                "G_RESOURCE_OVERLAYS",
                concat!(
                    "/io/github/astrovm/AdventureMods=",
                    env!("CARGO_MANIFEST_DIR"),
                    "/data"
                ),
            );
        });
    }

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
    fn missing_cards_hide_setup_button() {
        init_resource_overlay();

        let card = AdventureModsGameCard::new();

        card.set_missing(GameKind::SA2);

        assert!(!card.imp().setup_button.is_visible());
    }
}
