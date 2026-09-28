use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::path_display::display_path;
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{DetectionResult, InaccessibleGame, resolve_granted_steam_library};
use crate::ui::game_card::{AdventureModsGameCard, GameInstallOption};

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/welcome_page.ui")]
    pub struct AdventureModsWelcomePage {
        #[template_child]
        pub alerts_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub games_row: TemplateChild<adw::WrapBox>,
        pub(crate) open_uri: crate::ui::UriOpenerSlot,
        pub(crate) pick_folder: super::FolderPickerSlot,
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
                    // Emitted with a status message after a game was restored.
                    glib::subclass::Signal::builder("game-restored")
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
            let alert = gtk::Box::builder().spacing(12).build();
            alert.add_css_class("welcome-alert");
            alert.append(
                &gtk::Image::builder()
                    .icon_name("folder-open-symbolic")
                    .valign(gtk::Align::Start)
                    .build(),
            );
            alert.append(
                &gtk::Label::builder()
                    .label(format!(
                        "Adventure Mods needs access to the Steam library with {}. Use Grant Access on the game below.",
                        inaccessible_names.join(" and ")
                    ))
                    .wrap(true)
                    .xalign(0.0)
                    .hexpand(true)
                    .build(),
            );
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
                    let restore_card = card.clone();
                    let restore_page = self.clone();
                    let kind = card_spec.kind;
                    let secondary_nav_view = nav_view.clone();
                    card.connect_secondary_clicked(move || {
                        let Some(GameInstallOption::Detected(path)) =
                            restore_card.selected_install_option()
                        else {
                            return;
                        };
                        // Waiting for a Steam repair, the secondary action sets
                        // the game up again instead of restoring it.
                        if restore_card.needs_steam_repair() {
                            open_setup(&secondary_nav_view, Game { kind, path });
                        } else {
                            restore_page.confirm_restore(kind, path);
                        }
                    });
                    card.connect_setup_clicked(move || {
                        obj.activate_card(&card_clone, kind, &nav_view_clone);
                    });
                }
                GameCardState::Missing => {
                    card.set_missing(card_spec.kind);
                }
            }

            games_row.append(&card);
        }
    }

    /// Run the primary action for the install selected on `card`.
    fn activate_card(
        &self,
        card: &AdventureModsGameCard,
        kind: GameKind,
        nav_view: &adw::NavigationView,
    ) {
        let Some(option) = card.selected_install_option() else {
            return;
        };

        match option {
            GameInstallOption::Detected(_) if card.needs_steam_repair() => {
                self.verify_in_steam(kind);
            }
            GameInstallOption::Detected(path) => {
                open_setup(nav_view, Game { kind, path });
            }
            GameInstallOption::Inaccessible(path) => {
                self.request_library_access(path);
            }
        }
    }

    fn request_library_access(&self, expected_library: std::path::PathBuf) {
        let Some(window) = self.root().and_downcast::<gtk::Window>() else {
            tracing::warn!("Could not find parent window for library access dialog");
            return;
        };

        let choice = self.imp().pick_folder.pick(&window, &expected_library);
        let obj = self.clone();
        glib::spawn_future_local(async move {
            match choice.await {
                Ok(folder) => {
                    let Some(path) = folder.path() else {
                        obj.show_library_access_error(&format!(
                            "Could not read the selected folder. Please choose {}.",
                            display_path(&expected_library)
                        ));
                        return;
                    };

                    let Some(resolved) = resolve_granted_steam_library(&path, &expected_library)
                    else {
                        tracing::warn!(
                            selected = %path.display(),
                            expected = %expected_library.display(),
                            "Granted folder is not a usable Steam library"
                        );
                        obj.show_library_access_error(&format!(
                            "That folder is not the requested Steam library. Select {} (it must contain a steamapps folder).",
                            display_path(&expected_library)
                        ));
                        return;
                    };

                    tracing::info!(
                        selected = %path.display(),
                        expected = %expected_library.display(),
                        resolved = %resolved.display(),
                        "Granted Steam library access"
                    );
                    let selected = resolved.to_string_lossy().to_string();
                    obj.emit_by_name::<()>("library-access-granted", &[&selected]);
                }
                Err(err) => {
                    tracing::info!("Library access dialog cancelled or failed: {err}");
                }
            }
        });
    }

    /// Ask Steam to verify and repair the game's files.
    fn verify_in_steam(&self, kind: GameKind) {
        let window = self.root().and_downcast::<gtk::Window>();
        self.imp().open_uri.open(
            window.as_ref(),
            &crate::setup::restore::steam_verify_uri(kind),
        );
    }

    fn confirm_restore(&self, kind: GameKind, path: std::path::PathBuf) {
        self.restore_dialog(kind, path).present(Some(self));
    }

    fn restore_dialog(&self, kind: GameKind, path: std::path::PathBuf) -> adw::AlertDialog {
        let dialog = adw::AlertDialog::builder()
            .heading(format!("Restore the original {}?", kind.name()))
            .body(
                "This puts back the game's original launcher and removes the mod loader, \
                 so Steam starts the unmodded game. Downloaded mods stay in the mods folder \
                 and are reused if you set the game up again.",
            )
            .close_response("cancel")
            .default_response("cancel")
            .build();
        dialog.add_responses(&[("cancel", "Cancel"), ("restore", "Restore")]);
        dialog.set_response_appearance("restore", adw::ResponseAppearance::Destructive);

        let obj = self.clone();
        dialog.connect_response(Some("restore"), move |_, _| {
            let _ = crate::ui::catch_ui_panic("restore original game", || {
                obj.restore_game(kind, path.clone());
            });
        });
        dialog
    }

    fn restore_game(&self, kind: GameKind, path: std::path::PathBuf) {
        let obj = self.clone();
        glib::spawn_future_local(async move {
            let result = crate::blocking::flatten_spawn_result(
                gio::spawn_blocking(move || {
                    crate::setup::restore::restore_original_game(&path, kind)
                })
                .await,
            );
            match result {
                Ok(report) => {
                    let message = format!("{} was restored.", kind.name());
                    obj.emit_by_name::<()>("game-restored", &[&message]);
                    if report.needs_steam_verify {
                        obj.offer_steam_verify(kind);
                    }
                }
                Err(err) => {
                    tracing::error!("Failed to restore {}: {err:#}", kind.name());
                    obj.show_library_access_error(&format!(
                        "Could not restore {}: {err}",
                        kind.name()
                    ));
                }
            }
        });
    }

    /// The SADX 2004 conversion rewrote game files; Steam can put them back.
    fn offer_steam_verify(&self, kind: GameKind) {
        let obj = self.clone();
        steam_verify_dialog(kind, move |_, _| obj.verify_in_steam(kind)).present(Some(self));
    }
}

/// A folder the user is choosing, or why no folder was chosen.
pub(crate) type FolderChoice =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<gio::File, glib::Error>>>>;

/// Asks for a folder; replaced in tests so no file dialog waits on a user.
pub(crate) type FolderPicker = std::rc::Rc<dyn Fn(&gtk::Window, &std::path::Path) -> FolderChoice>;

/// How a page asks for a folder: the file dialog unless a test swapped it.
#[derive(Default)]
pub(crate) struct FolderPickerSlot(std::cell::RefCell<Option<FolderPicker>>);

impl std::fmt::Debug for FolderPickerSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FolderPickerSlot")
    }
}

impl FolderPickerSlot {
    fn pick(&self, window: &gtk::Window, initial_folder: &std::path::Path) -> FolderChoice {
        let picker = self.0.borrow().clone();
        match picker {
            Some(pick) => pick(window, initial_folder),
            None => pick_library_folder(window, initial_folder),
        }
    }

    #[cfg(test)]
    pub(crate) fn replace(&self, picker: FolderPicker) {
        self.0.replace(Some(picker));
    }
}

/// Ask for a Steam library folder with the file dialog.
fn pick_library_folder(window: &gtk::Window, initial_folder: &std::path::Path) -> FolderChoice {
    let dialog = gtk::FileDialog::builder()
        .title("Grant access to a Steam library")
        .modal(true)
        .accept_label("Grant Access")
        .build();

    // The host file chooser can see this path even when the sandbox cannot.
    dialog.set_initial_folder(Some(&gio::File::for_path(initial_folder)));
    dialog.select_folder_future(Some(window))
}

/// Offers to open Steam's file verification for `kind` through `open_uri`.
fn steam_verify_dialog(
    kind: GameKind,
    open_uri: impl Fn(Option<&gtk::Window>, &str) + 'static,
) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::builder()
        .heading("Finish in Steam")
        .body(format!(
            "Setup converted {} to the 2004 version. Let Steam verify the game files to \
             get the original Steam version back.",
            kind.name()
        ))
        .close_response("later")
        .default_response("verify")
        .build();
    dialog.add_responses(&[("later", "Later"), ("verify", "Verify in Steam")]);
    dialog.set_response_appearance("verify", adw::ResponseAppearance::Suggested);
    dialog.connect_response(Some("verify"), move |dialog, _| {
        let uri = crate::setup::restore::steam_verify_uri(kind);
        let window = dialog.root().and_downcast::<gtk::Window>();
        open_uri(window.as_ref(), &uri);
    });
    dialog
}

impl AdventureModsWelcomePage {
    fn show_library_access_error(&self, message: &str) {
        if let Some(window) = self
            .root()
            .and_then(|root| root.downcast::<crate::window::AdventureModsWindow>().ok())
        {
            window.show_status_message(message, true);
        }
    }
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

fn open_setup(nav_view: &adw::NavigationView, game: Game) {
    let setup_page = crate::ui::setup_page::AdventureModsSetupPage::new(game);
    nav_view.push(&setup_page.navigation_page());
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

        assert_eq!(
            alert_text(&page),
            "Adventure Mods needs access to the Steam library with Sonic Adventure DX. Use Grant Access on the game below."
        );
    }

    fn alert_text(page: &AdventureModsWelcomePage) -> String {
        page.imp()
            .alerts_box
            .first_child()
            .and_then(|alert| alert.last_child())
            .and_downcast::<gtk::Label>()
            .unwrap()
            .label()
            .to_string()
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

        assert_eq!(
            alert_text(&page),
            "Adventure Mods needs access to the Steam library with Sonic Adventure DX and Sonic Adventure 2. Use Grant Access on the game below."
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

    fn respond(dialog: &adw::AlertDialog, response: &str) {
        let signal =
            glib::subclass::SignalId::lookup("response", adw::AlertDialog::static_type()).unwrap();
        dialog.emit_with_details::<()>(signal, glib::Quark::from_str(response), &[&response]);
    }

    #[gtk::test]
    fn restored_converted_game_waits_for_steam_repair() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let game_path = tmp.path().to_path_buf();
        std::fs::write(game_path.join("Sonic Adventure DX.exe"), "game").unwrap();
        std::fs::write(game_path.join(".adventure-mods-steam-repair"), "").unwrap();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        // Never launch the real URI: it makes Steam verify the user's game.
        let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
        page.imp().open_uri.replace(std::rc::Rc::new({
            let opened = opened.clone();
            move |_: Option<&gtk::Window>, uri: &str| {
                opened.replace(Some(uri.to_owned()));
            }
        }));
        let nav_view = adw::NavigationView::new();
        page.set_detection_result(
            DetectionResult {
                games: vec![Game {
                    kind: GameKind::SADX,
                    path: game_path.clone(),
                }],
                inaccessible: vec![],
            },
            nav_view.clone(),
        );

        let card = page
            .imp()
            .games_row
            .first_child()
            .and_downcast::<AdventureModsGameCard>()
            .unwrap();
        assert!(card.needs_steam_repair());
        assert_eq!(
            card.imp().badge_label.label().as_str(),
            "Needs Steam repair"
        );
        assert_eq!(card.imp().setup_button.label().unwrap(), "Verify in Steam");
        assert_eq!(card.imp().secondary_button.label().unwrap(), "Set Up");
        assert!(card.imp().secondary_button.is_visible());

        card.imp().setup_button.emit_clicked();
        assert_eq!(opened.borrow().as_deref(), Some("steam://validate/71250"));

        // Once Steam has verified, setting up again is one click away.
        card.imp().secondary_button.emit_clicked();
        assert_eq!(
            nav_view
                .visible_page()
                .and_then(|page| page.tag())
                .as_deref(),
            Some("setup")
        );

        // Finishing the offered verification also goes through the page.
        opened.replace(None);
        page.verify_in_steam(GameKind::SA2);
        assert_eq!(opened.borrow().as_deref(), Some("steam://validate/213610"));
    }

    #[gtk::test]
    fn modded_games_offer_restore_and_restoring_reports_back() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let game_path = tmp.path().to_path_buf();
        std::fs::write(game_path.join("sonic2app.exe"), "game").unwrap();
        std::fs::write(game_path.join("Launcher.exe"), "manager").unwrap();
        std::fs::write(game_path.join("Launcher.exe.bak"), "launcher").unwrap();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        let window = gtk::Window::new();
        window.set_child(Some(&page));
        let restored = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
        let restored_for_signal = restored.clone();
        page.connect_local("game-restored", false, move |args| {
            restored_for_signal.replace(args[1].get::<String>().ok());
            None
        });
        page.set_detection_result(
            DetectionResult {
                games: vec![Game {
                    kind: GameKind::SA2,
                    path: game_path.clone(),
                }],
                inaccessible: vec![],
            },
            adw::NavigationView::new(),
        );

        // Cards are ordered SADX, SA2.
        let card = page
            .imp()
            .games_row
            .last_child()
            .and_downcast::<AdventureModsGameCard>()
            .unwrap();
        assert!(card.imp().secondary_button.get_visible());
        assert_eq!(card.imp().secondary_button.label().unwrap(), "Restore");
        assert_eq!(card.imp().setup_button.label().unwrap(), "Change Mods");
        assert_eq!(card.imp().badge_label.label().as_str(), "Mods installed");

        card.imp().secondary_button.emit_clicked();
        page.restore_game(GameKind::SA2, game_path.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while restored.borrow().is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "restore did not finish"
            );
            glib::MainContext::default().iteration(false);
        }

        assert_eq!(
            restored.borrow().as_deref(),
            Some("Sonic Adventure 2 was restored.")
        );
        assert_eq!(
            std::fs::read_to_string(game_path.join("Launcher.exe")).unwrap(),
            "launcher"
        );

        // Confirming the dialog restores; a restore that fails is reported.
        std::fs::write(game_path.join("Launcher.exe.bak"), "launcher").unwrap();
        std::fs::remove_file(game_path.join("Launcher.exe")).unwrap();
        std::fs::create_dir_all(game_path.join("Launcher.exe/blocker")).unwrap();
        restored.replace(None);
        let dialog = page.restore_dialog(GameKind::SA2, game_path.clone());
        respond(&dialog, "restore");

        page.offer_steam_verify(GameKind::SADX);
        // Never launch the real URI: it makes Steam verify the user's game.
        let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
        let verify = super::steam_verify_dialog(GameKind::SADX, {
            let opened = opened.clone();
            move |_, uri| {
                opened.replace(Some(uri.to_owned()));
            }
        });
        respond(&verify, "verify");
        assert_eq!(opened.borrow().as_deref(), Some("steam://validate/71250"));
        for _ in 0..100 {
            glib::MainContext::default().iteration(false);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(restored.borrow().is_none());
        assert!(game_path.join("Launcher.exe.bak").exists());
    }

    #[gtk::test]
    fn detection_cards_open_setup_and_handle_inaccessible_libraries() {
        init_resource_overlay();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        let nav_view = adw::NavigationView::new();
        page.set_detection_result(
            DetectionResult {
                games: vec![Game {
                    kind: GameKind::SADX,
                    path: "/games/sadx".into(),
                }],
                inaccessible: vec![],
            },
            nav_view.clone(),
        );

        let detected_card = page
            .imp()
            .games_row
            .first_child()
            .and_downcast::<AdventureModsGameCard>()
            .unwrap();
        let controllers = detected_card.observe_controllers();
        let gesture = controllers
            .item(0)
            .unwrap()
            .downcast::<gtk::GestureClick>()
            .unwrap();
        gesture.emit_by_name::<()>("released", &[&1i32, &0f64, &0f64]);
        assert_eq!(
            nav_view.visible_page().unwrap().title().as_str(),
            GameKind::SADX.name()
        );

        page.set_detection_result(
            DetectionResult {
                games: vec![],
                inaccessible: vec![InaccessibleGame {
                    kind: GameKind::SADX,
                    library_path: "/mnt/steam".into(),
                }],
            },
            nav_view,
        );
        let inaccessible_card = page
            .imp()
            .games_row
            .first_child()
            .and_downcast::<AdventureModsGameCard>()
            .unwrap();
        let controllers = inaccessible_card.observe_controllers();
        let gesture = controllers
            .item(0)
            .unwrap()
            .downcast::<gtk::GestureClick>()
            .unwrap();
        gesture.emit_by_name::<()>("released", &[&1i32, &0f64, &0f64]);

        page.set_detection_result(
            DetectionResult {
                games: vec![],
                inaccessible: vec![],
            },
            adw::NavigationView::new(),
        );
    }

    /// Run the main loop until `done` holds, failing after a few seconds.
    fn wait_until(what: &str, done: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !done() {
            assert!(std::time::Instant::now() < deadline, "timed out: {what}");
            glib::MainContext::default().iteration(false);
        }
    }

    type NextChoice = std::rc::Rc<std::cell::RefCell<Option<Result<gio::File, glib::Error>>>>;

    /// Answer folder requests with whatever the test queued, recording where
    /// each request started.
    fn fake_folder_picker(
        page: &AdventureModsWelcomePage,
    ) -> (
        NextChoice,
        std::rc::Rc<std::cell::RefCell<Vec<std::path::PathBuf>>>,
    ) {
        let next: NextChoice = Default::default();
        let initial_folders = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        page.imp().pick_folder.replace(std::rc::Rc::new({
            let next = next.clone();
            let initial_folders = initial_folders.clone();
            move |_: &gtk::Window, initial: &std::path::Path| -> FolderChoice {
                initial_folders.borrow_mut().push(initial.to_path_buf());
                let choice = next.take().expect("a queued folder choice");
                Box::pin(async move { choice })
            }
        }));
        (next, initial_folders)
    }

    #[gtk::test]
    fn library_access_checks_the_chosen_folder_before_granting_it() {
        init_resource_overlay();

        let app = gtk::Application::new(
            Some("io.github.astrovm.AdventureMods.WelcomeTests"),
            gio::ApplicationFlags::NON_UNIQUE,
        );
        let window = crate::window::AdventureModsWindow::new(&app);
        let page = window.welcome_page();
        let status = || window.imp().status_label.label().to_string();
        // The first scan clears the status banner, so let it finish first.
        wait_until("initial scan", || {
            window.imp().refresh_button.is_sensitive()
        });

        let (next, initial_folders) = fake_folder_picker(&page);
        let granted = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        page.connect_local("library-access-granted", false, {
            let granted = granted.clone();
            move |args| {
                granted.borrow_mut().push(args[1].get::<String>().unwrap());
                None
            }
        });
        let library = tempfile::tempdir().unwrap();
        std::fs::create_dir(library.path().join("steamapps")).unwrap();
        let expected = library.path().to_path_buf();
        let capture = crate::test_log::LogCapture::start();

        // Cancelling the dialog is not an error worth showing.
        next.replace(Some(Err(glib::Error::new(
            gtk::DialogError::Dismissed,
            "Dismissed by user",
        ))));
        page.request_library_access(expected.clone());
        wait_until("cancel is logged", || {
            capture
                .contents()
                .contains("Library access dialog cancelled or failed: Dismissed by user")
        });
        assert_eq!(status(), "");
        assert_eq!(*initial_folders.borrow(), vec![expected.clone()]);

        next.replace(Some(Ok(gio::File::for_uri(
            "https://example.com/SteamLibrary",
        ))));
        page.request_library_access(expected.clone());
        let unreadable = format!(
            "Could not read the selected folder. Please choose {}.",
            display_path(&expected)
        );
        wait_until("unreadable folder is reported", || status() == unreadable);

        let unrelated = tempfile::tempdir().unwrap();
        next.replace(Some(Ok(gio::File::for_path(unrelated.path()))));
        page.request_library_access(expected.clone());
        let wrong_folder = format!(
            "That folder is not the requested Steam library. Select {} (it must contain a steamapps folder).",
            display_path(&expected)
        );
        wait_until("wrong folder is reported", || status() == wrong_folder);
        assert!(
            capture
                .contents()
                .contains("Granted folder is not a usable Steam library")
        );
        assert!(granted.borrow().is_empty());

        // Picking the library's steamapps folder grants the library itself.
        next.replace(Some(Ok(gio::File::for_path(expected.join("steamapps")))));
        page.request_library_access(expected.clone());
        wait_until("library is granted", || !granted.borrow().is_empty());
        assert_eq!(
            *granted.borrow(),
            vec![expected.to_string_lossy().to_string()]
        );
        assert!(capture.contents().contains("Granted Steam library access"));
        assert_eq!(
            window.imp().extra_library_paths.borrow().as_slice(),
            &[expected]
        );
        assert_eq!(initial_folders.borrow().len(), 4);
    }

    #[gtk::test]
    fn default_folder_picker_opens_a_modal_dialog_that_can_be_dismissed() {
        init_resource_overlay();
        crate::ui::test_util::use_memory_gsettings_backend();

        let window = gtk::Window::new();
        window.present();
        let library = tempfile::tempdir().unwrap();
        let choice = FolderPickerSlot::default().pick(&window, library.path());
        let outcome = std::rc::Rc::new(std::cell::RefCell::new(None));
        glib::spawn_future_local({
            let outcome = outcome.clone();
            async move {
                outcome.replace(Some(choice.await));
            }
        });

        let find_dialog = || {
            gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
                .find(|toplevel| {
                    toplevel.title().as_deref() == Some("Grant access to a Steam library")
                })
        };
        wait_until("file dialog opens", || find_dialog().is_some());
        let dialog = find_dialog().unwrap();
        assert!(dialog.is_modal());
        assert_eq!(dialog.transient_for().as_ref(), Some(&window));

        dialog.close();
        wait_until("dialog reports back", || outcome.borrow().is_some());
        let err = outcome.take().unwrap().unwrap_err();
        assert!(err.matches(gtk::DialogError::Dismissed), "{err}");
    }

    #[gtk::test]
    fn secondary_action_only_restores_installs_the_app_can_read() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let game_path = tmp.path().to_path_buf();
        std::fs::write(game_path.join("sonic2app.exe"), "game").unwrap();
        std::fs::write(game_path.join("Launcher.exe"), "manager").unwrap();
        std::fs::write(game_path.join("Launcher.exe.bak"), "launcher").unwrap();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        assert!(format!("{:?}", page.imp()).contains("pick_folder: FolderPickerSlot"));
        let window = adw::Window::new();
        window.set_content(Some(&page));
        page.set_detection_result(
            DetectionResult {
                games: vec![Game {
                    kind: GameKind::SA2,
                    path: game_path,
                }],
                inaccessible: vec![InaccessibleGame {
                    kind: GameKind::SA2,
                    library_path: "/mnt/steam".into(),
                }],
            },
            adw::NavigationView::new(),
        );
        let card = page
            .imp()
            .games_row
            .last_child()
            .and_downcast::<AdventureModsGameCard>()
            .unwrap();

        card.imp().install_selector.set_selected(1);
        card.imp().secondary_button.emit_clicked();
        assert!(window.visible_dialog().is_none());

        card.imp().install_selector.set_selected(0);
        card.imp().secondary_button.emit_clicked();
        let dialog = window
            .visible_dialog()
            .and_downcast::<adw::AlertDialog>()
            .unwrap();
        assert_eq!(
            dialog.heading().as_deref(),
            Some("Restore the original Sonic Adventure 2?")
        );
    }

    #[gtk::test]
    fn activating_a_card_without_installs_does_nothing() {
        init_resource_overlay();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        let (_, initial_folders) = fake_folder_picker(&page);
        let nav_view = adw::NavigationView::new();

        page.activate_card(&AdventureModsGameCard::new(), GameKind::SADX, &nav_view);

        assert!(nav_view.visible_page().is_none());
        assert!(initial_folders.borrow().is_empty());
    }

    #[gtk::test]
    fn restoring_a_converted_game_offers_to_verify_it_in_steam() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let game_path = tmp.path().to_path_buf();
        std::fs::write(game_path.join("Sonic Adventure DX.exe"), "game").unwrap();
        std::fs::write(game_path.join(".adventure-mods-steam-repair"), "").unwrap();

        let page: AdventureModsWelcomePage = glib::Object::builder().build();
        let window = adw::Window::new();
        window.set_content(Some(&page));
        // Never launch the real URI: it makes Steam verify the user's game.
        let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
        page.imp().open_uri.replace(std::rc::Rc::new({
            let opened = opened.clone();
            move |_: Option<&gtk::Window>, uri: &str| {
                opened.replace(Some(uri.to_owned()));
            }
        }));

        page.restore_game(GameKind::SADX, game_path);
        wait_until("verify is offered", || window.visible_dialog().is_some());
        let dialog = window
            .visible_dialog()
            .and_downcast::<adw::AlertDialog>()
            .unwrap();
        assert_eq!(dialog.heading().as_deref(), Some("Finish in Steam"));

        respond(&dialog, "verify");
        assert_eq!(opened.borrow().as_deref(), Some("steam://validate/71250"));
    }
}
