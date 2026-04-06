use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::steam::game::{Game, GameKind};
use crate::steam::library::{DetectionResult, InaccessibleGame};
use crate::ui::game_card::AdventureModsGameCard;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/welcome_page.ui")]
    pub struct AdventureModsWelcomePage {
        #[template_child]
        pub alerts_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub games_row: TemplateChild<gtk::Box>,
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
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            use std::sync::OnceLock;
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("library-access-granted")
                        .param_types([String::static_type()])
                        .build(),
                ]
            })
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
        let alerts_box = &self.imp().alerts_box;
        let games_row = &self.imp().games_row;

        while let Some(child) = alerts_box.first_child() {
            alerts_box.remove(&child);
        }

        while let Some(child) = games_row.first_child() {
            games_row.remove(&child);
        }

        if !result.inaccessible.is_empty() {
            let inaccessible_names: Vec<&str> =
                result.inaccessible.iter().map(|g| g.kind.name()).collect();
            let alert = gtk::Label::builder()
                .label(format!(
                    "Some Steam libraries still need access: {}. Those games stay visible below, and you can grant access directly from their cards.",
                    inaccessible_names.join(", ")
                ))
                .wrap(true)
                .justify(gtk::Justification::Center)
                .build();
            alert.add_css_class("welcome-alert");
            alerts_box.append(&alert);
        }

        alerts_box.set_visible(alerts_box.first_child().is_some());

        for card_spec in build_game_cards(&result) {
            let card = AdventureModsGameCard::new();

            match &card_spec.state {
                GameCardState::Detected(game) => {
                    card.set_detected(
                        game,
                        card_spec.installation_index,
                        card_spec.installation_total,
                    );
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
                }
                GameCardState::Missing => {
                    card.set_missing(card_spec.kind);
                }
                GameCardState::Inaccessible(inaccessible) => {
                    card.set_inaccessible(card_spec.kind, &inaccessible.library_path);
                    let obj = self.clone();
                    let expected_library = inaccessible.library_path.clone();
                    card.connect_secondary_clicked(move || {
                        obj.request_library_access(expected_library.clone());
                    });
                }
            }

            games_row.append(&card);
        }
    }

    fn request_library_access(&self, expected_library: std::path::PathBuf) {
        let Some(window) = self.root().and_downcast::<gtk::Window>() else {
            tracing::warn!("Could not find parent window for library access dialog");
            return;
        };

        let dialog = gtk::FileDialog::builder()
            .title("Grant access to a Steam library")
            .modal(true)
            .accept_label("Grant Access")
            .build();

        if expected_library.exists() {
            let folder = gio::File::for_path(&expected_library);
            dialog.set_initial_folder(Some(&folder));
        }

        let obj = self.clone();
        glib::spawn_future_local(async move {
            match dialog.select_folder_future(Some(&window)).await {
                Ok(folder) => {
                    if let Some(path) = folder.path() {
                        let selected = path.to_string_lossy().to_string();
                        obj.emit_by_name::<()>("library-access-granted", &[&selected]);
                    }
                }
                Err(err) => {
                    tracing::info!("Library access dialog cancelled or failed: {err}");
                }
            }
        });
    }
}

#[derive(Clone, Debug)]
struct GameCardSpec {
    kind: GameKind,
    state: GameCardState,
    installation_index: usize,
    installation_total: usize,
}

#[derive(Clone, Debug)]
enum GameCardState {
    Detected(Game),
    Missing,
    Inaccessible(InaccessibleGame),
}

fn build_game_cards(result: &DetectionResult) -> Vec<GameCardSpec> {
    let mut cards = Vec::new();

    for kind in [GameKind::SADX, GameKind::SA2] {
        let kind_games: Vec<&Game> = result
            .games
            .iter()
            .filter(|game| game.kind == kind)
            .collect();

        let kind_inaccessible: Vec<&InaccessibleGame> = result
            .inaccessible
            .iter()
            .filter(|game| game.kind == kind)
            .collect();

        let total = kind_games.len() + kind_inaccessible.len();

        if total == 0 {
            cards.push(GameCardSpec {
                kind,
                state: GameCardState::Missing,
                installation_index: 0,
                installation_total: 1,
            });
            continue;
        }

        let mut index = 0;
        for game in &kind_games {
            cards.push(GameCardSpec {
                kind,
                state: GameCardState::Detected((*game).clone()),
                installation_index: index,
                installation_total: total,
            });
            index += 1;
        }

        for inaccessible in &kind_inaccessible {
            cards.push(GameCardSpec {
                kind,
                state: GameCardState::Inaccessible((*inaccessible).clone()),
                installation_index: index,
                installation_total: total,
            });
            index += 1;
        }
    }

    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::steam::game::Game;
    use crate::steam::library::InaccessibleGame;

    #[test]
    fn build_game_cards_always_includes_missing_games() {
        let result = DetectionResult {
            games: vec![Game {
                kind: GameKind::SA2,
                path: "/games/sa2".into(),
            }],
            inaccessible: vec![],
        };

        let cards = build_game_cards(&result);

        assert_eq!(cards.len(), 2);
        assert!(matches!(cards[0].state, GameCardState::Missing));
        assert!(matches!(cards[1].state, GameCardState::Detected(_)));
    }

    #[test]
    fn build_game_cards_marks_inaccessible_games() {
        let result = DetectionResult {
            games: vec![],
            inaccessible: vec![InaccessibleGame {
                kind: GameKind::SADX,
                library_path: "/mnt/steam".into(),
            }],
        };

        let cards = build_game_cards(&result);

        assert!(matches!(cards[0].state, GameCardState::Inaccessible(_)));
        assert!(matches!(cards[1].state, GameCardState::Missing));
    }

    #[test]
    fn build_game_cards_includes_both_detected_and_inaccessible() {
        let result = DetectionResult {
            games: vec![Game {
                kind: GameKind::SADX,
                path: "/games/sadx".into(),
            }],
            inaccessible: vec![InaccessibleGame {
                kind: GameKind::SADX,
                library_path: "/mnt/steam".into(),
            }],
        };

        let cards = build_game_cards(&result);

        assert_eq!(cards.len(), 3);
        assert!(matches!(cards[0].state, GameCardState::Detected(_)));
        assert_eq!(cards[0].installation_total, 2);
        assert!(matches!(cards[1].state, GameCardState::Inaccessible(_)));
        assert_eq!(cards[1].installation_total, 2);
        assert!(matches!(cards[2].state, GameCardState::Missing));
    }
}
