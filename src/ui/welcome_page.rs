use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::steam::game::{Game, GameKind};
use crate::steam::library::{DetectionResult, InaccessibleGame};
use crate::ui::game_card::{AdventureModsGameCard, GameInstallOption};

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
            let mut inaccessible_names = Vec::new();
            for game in &result.inaccessible {
                let name = game.kind.name();
                if !inaccessible_names.contains(&name) {
                    inaccessible_names.push(name);
                }
            }
            let alert = gtk::Label::builder()
                .label(format!(
                    "Some Steam libraries still need access: {}.",
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
                GameCardState::Detected | GameCardState::Inaccessible => {
                    card.set_install_options(card_spec.kind, &card_spec.install_options);
                    let card_clone = card.clone();
                    let nav_view_clone = nav_view.clone();
                    let obj = self.clone();
                    card.connect_setup_clicked(move || {
                        let Some(option) = card_clone.selected_install_option() else {
                            return;
                        };

                        match option {
                            GameInstallOption::Detected(path) => {
                                let game = Game {
                                    kind: card_spec.kind,
                                    path,
                                };
                                let setup_page = crate::ui::setup_page::AdventureModsSetupPage::new(
                                    game.clone(),
                                );
                                let nav_page = adw::NavigationPage::builder()
                                    .title(game.kind.name())
                                    .child(&setup_page)
                                    .build();
                                nav_view_clone.push(&nav_page);
                            }
                            GameInstallOption::Inaccessible(path) => {
                                obj.request_library_access(path);
                            }
                        }
                    });
                }
                GameCardState::Missing => {
                    card.set_missing(card_spec.kind);
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
                        if !selected_library_matches(&expected_library, &path) {
                            if let Some(window) = obj.root().and_then(|root| {
                                root.downcast::<crate::window::AdventureModsWindow>().ok()
                            }) {
                                window.show_status_message(
                                    &format!(
                                        "Selected folder does not match the required Steam library: {}",
                                        expected_library.display()
                                    ),
                                    true,
                                );
                            }
                            return;
                        }

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

fn selected_library_matches(
    expected_library: &std::path::Path,
    selected_library: &std::path::Path,
) -> bool {
    let expected = expected_library
        .canonicalize()
        .unwrap_or_else(|_| expected_library.to_path_buf());
    let selected = selected_library
        .canonicalize()
        .unwrap_or_else(|_| selected_library.to_path_buf());
    expected == selected
}

#[derive(Clone, Debug)]
struct GameCardSpec {
    kind: GameKind,
    state: GameCardState,
    install_options: Vec<GameInstallOption>,
}

#[derive(Clone, Debug)]
enum GameCardState {
    Detected,
    Missing,
    Inaccessible,
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

        let detected_total = kind_games.len();
        let total = detected_total + kind_inaccessible.len();

        if total == 0 {
            cards.push(GameCardSpec {
                kind,
                state: GameCardState::Missing,
                install_options: Vec::new(),
            });
            continue;
        }

        let mut install_options: Vec<GameInstallOption> = kind_games
            .iter()
            .map(|game| GameInstallOption::detected(game.path.clone()))
            .collect();
        install_options.extend(
            kind_inaccessible
                .iter()
                .map(|game| GameInstallOption::inaccessible(game.library_path.clone())),
        );

        let state = if !kind_games.is_empty() {
            GameCardState::Detected
        } else {
            GameCardState::Inaccessible
        };

        cards.push(GameCardSpec {
            kind,
            state,
            install_options,
        });
    }

    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::steam::game::Game;
    use crate::steam::library::InaccessibleGame;
    use crate::ui::test_util::init_resource_overlay;

    #[test]
    fn selected_library_matches_accepts_same_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("steam-library");
        std::fs::create_dir_all(&path).unwrap();

        assert!(selected_library_matches(&path, &path));
    }

    #[test]
    fn selected_library_matches_rejects_different_path() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = tmp.path().join("expected");
        let selected = tmp.path().join("selected");
        std::fs::create_dir_all(&expected).unwrap();
        std::fs::create_dir_all(&selected).unwrap();

        assert!(!selected_library_matches(&expected, &selected));
    }

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
        assert!(matches!(cards[1].state, GameCardState::Detected));
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

        assert!(matches!(cards[0].state, GameCardState::Inaccessible));
        assert!(matches!(cards[1].state, GameCardState::Missing));
    }

    #[test]
    fn build_game_cards_shows_detected_and_inaccessible_cards_for_mixed_installs() {
        let result = DetectionResult {
            games: vec![
                Game {
                    kind: GameKind::SADX,
                    path: "/games/sadx-1".into(),
                },
                Game {
                    kind: GameKind::SADX,
                    path: "/games/sadx-2".into(),
                },
            ],
            inaccessible: vec![
                InaccessibleGame {
                    kind: GameKind::SADX,
                    library_path: "/mnt/steam-1".into(),
                },
                InaccessibleGame {
                    kind: GameKind::SADX,
                    library_path: "/mnt/steam-2".into(),
                },
            ],
        };

        let cards = build_game_cards(&result);

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].install_options.len(), 4);
        assert!(matches!(cards[0].state, GameCardState::Detected));
        assert!(matches!(cards[1].state, GameCardState::Missing));
    }

    #[gtk::test]
    fn detection_result_alert_does_not_claim_hidden_cards_are_visible() {
        init_resource_overlay();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        let nav_view = adw::NavigationView::new();
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

        page.set_detection_result(result, nav_view);

        let alert = page
            .imp()
            .alerts_box
            .first_child()
            .and_downcast::<gtk::Label>()
            .unwrap();

        assert_eq!(
            alert.label().as_str(),
            "Some Steam libraries still need access: Sonic Adventure DX."
        );
    }

    #[gtk::test]
    fn detection_result_alert_deduplicates_game_names() {
        init_resource_overlay();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        let nav_view = adw::NavigationView::new();
        let result = DetectionResult {
            games: vec![],
            inaccessible: vec![
                InaccessibleGame {
                    kind: GameKind::SADX,
                    library_path: "/mnt/steam-1".into(),
                },
                InaccessibleGame {
                    kind: GameKind::SADX,
                    library_path: "/mnt/steam-2".into(),
                },
                InaccessibleGame {
                    kind: GameKind::SA2,
                    library_path: "/mnt/steam-3".into(),
                },
            ],
        };

        page.set_detection_result(result, nav_view);

        let alert = page
            .imp()
            .alerts_box
            .first_child()
            .and_downcast::<gtk::Label>()
            .unwrap();

        assert_eq!(
            alert.label().as_str(),
            "Some Steam libraries still need access: Sonic Adventure DX, Sonic Adventure 2."
        );
    }

    #[gtk::test]
    fn detection_result_uses_horizontal_box_layout_for_cards() {
        init_resource_overlay();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        let nav_view = adw::NavigationView::new();
        let result = DetectionResult {
            games: vec![
                Game {
                    kind: GameKind::SADX,
                    path: "/games/sadx-1".into(),
                },
                Game {
                    kind: GameKind::SADX,
                    path: "/games/sadx-2".into(),
                },
                Game {
                    kind: GameKind::SA2,
                    path: "/games/sa2".into(),
                },
            ],
            inaccessible: vec![InaccessibleGame {
                kind: GameKind::SADX,
                library_path: "/mnt/steam".into(),
            }],
        };

        page.set_detection_result(result, nav_view);

        assert!(page.imp().games_row.first_child().is_some());
    }
}
