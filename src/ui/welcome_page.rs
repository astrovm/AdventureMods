use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::steam::game::Game;
use crate::steam::library::DetectionResult;
use crate::ui::game_card::AdventureModsGameCard;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/welcome_page.ui")]
    pub struct AdventureModsWelcomePage {
        #[template_child]
        pub games_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsWelcomePage {
        const NAME: &'static str = "AdventureModsWelcomePage";
        type Type = super::AdventureModsWelcomePage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            AdventureModsGameCard::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AdventureModsWelcomePage {
        fn constructed(&self) {
            self.parent_constructed();
            self.status_page.set_icon_name(Some(crate::config::APP_ID));
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            use std::sync::OnceLock;
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS
                .get_or_init(|| vec![glib::subclass::Signal::builder("refresh").action().build()])
        }
    }
    impl WidgetImpl for AdventureModsWelcomePage {}
    impl BinImpl for AdventureModsWelcomePage {}
}

glib::wrapper! {
    pub struct AdventureModsWelcomePage(ObjectSubclass<imp::AdventureModsWelcomePage>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl AdventureModsWelcomePage {
    pub fn set_detection_result(&self, result: DetectionResult, nav_view: adw::NavigationView) {
        let games_box = &self.imp().games_box;

        while let Some(child) = games_box.first_child() {
            games_box.remove(&child);
        }

        if !result.inaccessible.is_empty() {
            let inaccessible_names: Vec<&str> =
                result.inaccessible.iter().map(|g| g.kind.name()).collect();
            let label = gtk::Label::builder()
                .label(format!(
                    "The following games are installed but their Steam library is inaccessible:\n{}\n\nThe partition may not be mounted.",
                    inaccessible_names.join(", ")
                ))
                .justify(gtk::Justification::Center)
                .wrap(true)
                .build();
            label.add_css_class("warning");
            games_box.append(&label);

            let refresh_button = gtk::Button::builder()
                .label("Refresh")
                .halign(gtk::Align::Center)
                .margin_top(12)
                .build();
            let obj = self.clone();
            refresh_button.connect_clicked(move |_| {
                obj.emit_by_name::<()>("refresh", &[]);
            });
            games_box.append(&refresh_button);

            if !result.games.is_empty() {
                let separator = gtk::Separator::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .margin_top(18)
                    .margin_bottom(18)
                    .build();
                games_box.append(&separator);
            }
        }

        if result.games.is_empty() {
            if result.inaccessible.is_empty() {
                let label = gtk::Label::builder()
                    .label(
                        "No Sonic Adventure games found.\n\nInstall Sonic Adventure DX or Sonic Adventure 2 via Steam, then restart this app.",
                    )
                    .justify(gtk::Justification::Center)
                    .build();
                games_box.append(&label);
            }
            return;
        }

        for game in result.games {
            let card = AdventureModsGameCard::new(&game);

            let game_clone = game.clone();
            let nav_view_clone = nav_view.clone();
            card.connect_setup_clicked(move || {
                let setup_page =
                    crate::ui::setup_page::AdventureModsSetupPage::new(game_clone.clone());
                let nav_page = adw::NavigationPage::builder()
                    .title(game_clone.kind.name())
                    .child(&setup_page)
                    .build();
                nav_view_clone.push(&nav_page);
            });

            games_box.append(&card);
        }
    }

    pub fn set_games(&self, games: Vec<Game>, nav_view: adw::NavigationView) {
        self.set_detection_result(
            DetectionResult {
                games,
                inaccessible: Vec::new(),
            },
            nav_view,
        );
    }
}
