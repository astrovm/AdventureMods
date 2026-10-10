use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::*;
use crate::steam::library::InaccessibleGame;

// Fake detection reads the granted library folders: a library holding an
// `sa2` folder has SA2 installed, and one holding `locked` has SADX in a
// library the app still needs access to.
fn fake_detect(extra: &[PathBuf]) -> DetectionResult {
    let mut result = DetectionResult::default();
    for library in extra {
        if library.join("sa2").is_dir() {
            result.games.push(Game {
                kind: GameKind::SA2,
                path: library.join("sa2"),
            });
        }
        if library.join("locked").is_dir() {
            result.inaccessible.push(InaccessibleGame {
                kind: GameKind::SADX,
                library_path: library.join("locked"),
            });
        }
    }
    result
}

fn exploding_detect(_: &[PathBuf]) -> DetectionResult {
    panic!("vdf exploded")
}

fn fake_restore(path: &Path, _kind: GameKind) -> anyhow::Result<RestoreReport> {
    if path.join("fail-restore").exists() {
        anyhow::bail!("launcher missing");
    }
    std::fs::remove_dir_all(path.join("mods/.modloader"))?;
    Ok(RestoreReport {
        changes: Vec::new(),
        needs_steam_verify: path.join("verify").exists(),
    })
}

struct Fixture {
    _dir: tempfile::TempDir,
    library: PathBuf,
    config: PathBuf,
    opened: Rc<RefCell<Vec<String>>>,
    folder: Rc<RefCell<Option<PathBuf>>>,
}

impl Fixture {
    fn new(library_contents: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        for folder in library_contents {
            std::fs::create_dir_all(library.join(folder)).unwrap();
        }
        let config = dir.path().join("config");
        let mut settings = Settings::load_from(&config, &|| None);
        config::save_extra_library_paths(Some(&mut settings), std::slice::from_ref(&library));
        Self {
            _dir: dir,
            library,
            config,
            opened: Rc::new(RefCell::new(Vec::new())),
            folder: Rc::new(RefCell::new(None)),
        }
    }

    fn services(&self) -> Services {
        let opened = self.opened.clone();
        let folder = self.folder.clone();
        Services {
            open_uri: Rc::new(move |uri| opened.borrow_mut().push(uri.to_owned())),
            pick_folder: Rc::new(move |_| {
                let (tx, rx) = std::sync::mpsc::channel();
                tx.send(folder.borrow().clone()).unwrap();
                rx
            }),
            detect_games: fake_detect,
            restore_game: fake_restore,
            setup: crate::ui::setup::tests::work(),
        }
    }

    fn settings(&self) -> Settings {
        Settings::load_from(&self.config, &|| None)
    }

    fn app(&self) -> AdventureModsApp {
        AdventureModsApp::new(self.services(), Some(self.settings()))
    }

    fn harness(&self) -> Harness<'static, AdventureModsApp> {
        harness(self.app())
    }
}

fn harness(app: AdventureModsApp) -> Harness<'static, AdventureModsApp> {
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(|ui, app: &mut AdventureModsApp| app.show(ui), app)
}

/// Step frames until `done` holds, failing after a while.
fn run_until(
    harness: &mut Harness<'_, AdventureModsApp>,
    what: &str,
    done: impl Fn(&AdventureModsApp) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done(harness.state()) {
        assert!(Instant::now() < deadline, "timed out: {what}");
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.run_steps(2);
}

fn scanned(app: &AdventureModsApp) -> bool {
    !app.scanning() && app.welcome.has_result()
}

fn press(harness: &mut Harness<'_, AdventureModsApp>, label: &str) {
    harness.get_by_label(label).click();
    harness.run_steps(4);
}

#[test]
fn the_app_scans_on_start_and_shows_each_game() {
    let fixture = Fixture::new(&["sa2"]);
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);
    // The game that is there starts selected.
    harness.get_by_label("SONIC ADVENTURE 2");
    harness.get_by_label("Set Up");
    harness.get_by_label("ADVENTURE MODS");

    // Scanning again from the header keeps the cards.
    press(&mut harness, "Scan Again");
    run_until(&mut harness, "rescan", scanned);
    harness.get_by_label("Set Up");
}

#[test]
fn failed_scans_show_an_error_banner_until_one_works() {
    let fixture = Fixture::new(&["sa2"]);
    let mut services = fixture.services();
    services.detect_games = exploding_detect;
    let capture = crate::test_log::LogCapture::start();
    let mut harness = harness(AdventureModsApp::new(services, None));

    run_until(&mut harness, "failed scan", |app| app.status.is_some());
    harness.get_by_label("Failed to detect Steam libraries: spawn error: vdf exploded");
    assert!(capture.contents().contains("Failed to detect games"));

    harness.state_mut().services.detect_games = fake_detect;
    harness.state_mut().extra_library_paths = vec![fixture.library.clone()];
    press(&mut harness, "Scan Again");
    run_until(&mut harness, "scan", scanned);
    assert!(harness.state().status.is_none());
}

#[test]
fn only_the_newest_scan_is_shown() {
    let fixture = Fixture::new(&["sa2"]);
    let mut app = fixture.app();
    app.detect_games();
    let stale = app.latest_scan - 1;

    app.apply_scan(stale, Err(anyhow::anyhow!("stale failure")));
    assert!(app.status.is_none());
    app.apply_scan(app.latest_scan, Ok(DetectionResult::default()));
    assert!(app.welcome.has_result());

    // Scan ids wrap instead of overflowing.
    app.latest_scan = u64::MAX;
    app.detect_games();
    assert_eq!(app.latest_scan, 0);
}

#[test]
fn granting_access_remembers_the_library_and_scans_again() {
    let fixture = Fixture::new(&["locked"]);
    let locked = fixture.library.join("locked");
    std::fs::create_dir_all(locked.join("steamapps")).unwrap();
    *fixture.folder.borrow_mut() = Some(locked.clone());
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);
    harness.get_by_label_contains("needs access to the Steam library with Sonic Adventure DX");

    press(&mut harness, "Grant Access");
    run_until(&mut harness, "rescan", |app| {
        scanned(app) && app.extra_library_paths.len() == 2
    });
    assert_eq!(
        config::load_extra_library_paths(Some(&fixture.settings())),
        vec![fixture.library.clone(), locked.clone()]
    );

    // Granting the same library again saves nothing new.
    harness.state_mut().library_access_granted(locked);
    assert_eq!(harness.state().extra_library_paths.len(), 2);
}

#[test]
fn choosing_the_wrong_folder_or_cancelling_grants_nothing() {
    let fixture = Fixture::new(&["locked"]);
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);
    let capture = crate::test_log::LogCapture::start();

    // Cancelled.
    press(&mut harness, "Grant Access");
    assert!(
        capture
            .contents()
            .contains("Library access dialog cancelled")
    );

    // Not a Steam library.
    *fixture.folder.borrow_mut() = Some(fixture.library.clone());
    press(&mut harness, "Grant Access");
    harness.get_by_label_contains("That folder is not the requested Steam library.");
    assert_eq!(harness.state().extra_library_paths.len(), 1);
}

#[test]
fn restoring_a_modded_game_asks_first() {
    let fixture = Fixture::new(&["sa2/mods/.modloader"]);
    let game = fixture.library.join("sa2");
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);
    harness.get_by_label("Change Mods");

    // Cancelling the question changes nothing.
    press(&mut harness, "Restore");
    harness.get_by_label("Restore the original Sonic Adventure 2?");
    press(&mut harness, "Cancel");
    assert!(game.join("mods/.modloader").exists());

    // Cancel has focus; the dialog's Restore is next to it.
    press(&mut harness, "Restore");
    harness.key_press(egui::Key::Tab);
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    run_until(&mut harness, "restored", |app| {
        app.restoring.is_none() && !game.join("mods/.modloader").exists()
    });
    run_until(&mut harness, "rescan", scanned);
    harness.get_by_label("Sonic Adventure 2 was restored.");
    harness.get_by_label("Set Up");
    // Nothing was converted, so there is nothing for Steam to verify.
    assert!(harness.state().dialog.is_none());
}

#[test]
fn restoring_reports_back_and_offers_verification() {
    let fixture = Fixture::new(&["sa2/mods/.modloader"]);
    let game = fixture.library.join("sa2");
    std::fs::write(game.join("verify"), "").unwrap();
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);

    harness
        .state_mut()
        .restore_game(GameKind::SA2, game.clone());
    run_until(&mut harness, "restored", |app| app.restoring.is_none());
    harness.get_by_label("Sonic Adventure 2 was restored.");
    harness.get_by_label("Finish in Steam");
    press(&mut harness, "Verify in Steam");
    assert_eq!(
        *fixture.opened.borrow(),
        vec!["steam://validate/213610".to_owned()]
    );

    // Toasts go away after a few seconds.
    harness.run_steps(300);
    assert!(harness.state().toasts.is_empty());
}

#[test]
fn a_failed_restore_is_shown_in_the_banner() {
    let fixture = Fixture::new(&["sa2"]);
    let game = fixture.library.join("sa2");
    std::fs::write(game.join("fail-restore"), "").unwrap();
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);

    harness.state_mut().dialog = Some(Dialog::ConfirmRestore(
        GameKind::SA2,
        game.clone(),
        restore_dialog(GameKind::SA2),
    ));
    harness.run_steps(4);
    harness.key_press(egui::Key::Tab);
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    run_until(&mut harness, "restore failed", |app| app.status.is_some());
    harness.get_by_label("Could not restore Sonic Adventure 2: launcher missing");
}

#[test]
fn games_waiting_for_a_steam_repair_open_steam() {
    let fixture = Fixture::new(&["sa2"]);
    std::fs::write(fixture.library.join("sa2/.adventure-mods-steam-repair"), "").unwrap();
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);

    press(&mut harness, "Verify in Steam");
    assert_eq!(
        *fixture.opened.borrow(),
        vec!["steam://validate/213610".to_owned()]
    );

    // Later closes the verification offer without opening Steam.
    harness.state_mut().dialog = Some(Dialog::SteamVerify(
        GameKind::SA2,
        steam_verify_dialog(GameKind::SA2),
    ));
    harness.run_steps(4);
    press(&mut harness, "Later");
    assert!(harness.state().dialog.is_none());
    assert_eq!(fixture.opened.borrow().len(), 1);
}

#[test]
fn setup_opens_from_a_card_and_returns_with_b() {
    let fixture = Fixture::new(&["sa2"]);
    std::fs::write(fixture.library.join("sa2/proton-ready"), "").unwrap();
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);

    press(&mut harness, "Set Up");
    harness.get_by_label("Set Up Proton");
    harness.get_by_label("Sonic Adventure 2 · Step 1 of 4");
    press(&mut harness, "Continue");
    harness.get_by_label("Choose Mods");

    // The header's back button, then B (Escape), back to the games.
    press(&mut harness, "Back");
    harness.get_by_label("Set Up Proton");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().setup.is_none());
    run_until(&mut harness, "rescan", scanned);
}

#[test]
fn setup_saves_languages_and_opens_steam_through_the_app() {
    let fixture = Fixture::new(&["sa2"]);
    let game = fixture.library.join("sa2");
    std::fs::write(game.join("proton-ready"), "").unwrap();
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);
    harness.state_mut().open_setup(Game {
        kind: GameKind::SA2,
        path: game,
    });
    harness.run_steps(2);

    let setup = harness.state_mut().setup.as_mut().unwrap();
    setup.on_next();
    setup.on_next();
    harness.run_steps(2);
    press(&mut harness, "Voices");
    press(&mut harness, "English");
    // B closes the open picker, not setup.
    press(&mut harness, "Subtitles");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(4);
    assert!(harness.state().setup.is_some());
    press(&mut harness, "Install");
    let saved = config::load_language_selection(Some(&fixture.settings()), GameKind::SA2);
    assert_eq!(saved.voice, crate::setup::config::VoiceLanguage::English);

    run_until(&mut harness, "finished", |app| {
        app.setup
            .as_ref()
            .is_some_and(|setup| setup.title().0 == "Sonic Adventure 2")
    });
    // X on a controller plays the game; Start leaves setup.
    harness.state_mut().pad_presses.push(PadButton::Action);
    harness.run_steps(2);
    assert_eq!(
        *fixture.opened.borrow(),
        vec!["steam://rungameid/213610".to_owned()]
    );
    harness.state_mut().pad_presses.push(PadButton::Start);
    harness.run_steps(2);
    assert!(harness.state().setup.is_none());
}

#[test]
fn the_about_dialog_links_to_the_issue_tracker() {
    let fixture = Fixture::new(&[]);
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);

    press(&mut harness, "About");
    harness.get_by_label(&format!("Adventure Mods {}", env!("CARGO_PKG_VERSION")));
    press(&mut harness, "Report an Issue");
    assert_eq!(*fixture.opened.borrow(), vec![ISSUES_URL.to_owned()]);

    press(&mut harness, "About");
    press(&mut harness, "Close");
    assert!(harness.state().dialog.is_none());
    assert_eq!(fixture.opened.borrow().len(), 1);
}

#[test]
fn controllers_navigate_and_show_button_hints() {
    let fixture = Fixture::new(&["sa2"]);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = fixture.app();
    app.set_gamepad(Gamepad::fake(rx, true));
    let mut harness = harness(app);
    run_until(&mut harness, "scan", scanned);
    harness.get_by_label("Scan again");

    // Y scans again; it is ignored while a dialog is open. eframe feeds the
    // controller from its input hook, which the harness does not run.
    let feed = |harness: &mut Harness<'_, AdventureModsApp>| {
        harness
            .state_mut()
            .feed_gamepad(&mut egui::RawInput::default());
    };
    tx.send(PadButton::Refresh).unwrap();
    feed(&mut harness);
    harness.step();
    assert!(harness.state().scanning());
    run_until(&mut harness, "rescan", scanned);
    harness.state_mut().dialog = Some(Dialog::About(about_dialog()));
    tx.send(PadButton::Refresh).unwrap();
    feed(&mut harness);
    harness.run_steps(2);
    assert!(!harness.state().scanning());
    press(&mut harness, "Close");

    // A direction starts keyboard-style navigation on the first card; A presses it.
    let mut input = egui::RawInput::default();
    tx.send(PadButton::Down).unwrap();
    tx.send(PadButton::Confirm).unwrap();
    harness.state_mut().feed_gamepad(&mut input);
    assert_eq!(input.events.len(), 4);
    assert!(harness.state().navigating);
    harness.key_press(egui::Key::ArrowDown);
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert!(harness.state().setup.is_some());
    harness.get_by_label("Other action");
}

#[test]
fn pointer_input_ends_keyboard_navigation() {
    let fixture = Fixture::new(&["sa2"]);
    let mut harness = fixture.harness();
    run_until(&mut harness, "scan", scanned);

    harness.key_press(egui::Key::Tab);
    harness.run_steps(2);
    assert!(harness.state().navigating);
    harness.hover_at(egui::pos2(10.0, 10.0));
    harness.run_steps(2);
    assert!(!harness.state().navigating);

    // Inside setup, keyboard navigation focuses setup's main button.
    harness.state_mut().open_setup(Game {
        kind: GameKind::SA2,
        path: fixture.library.join("sa2"),
    });
    harness.key_press(egui::Key::Tab);
    harness.run_steps(2);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
}

#[test]
fn the_window_size_is_saved_unless_maximized() {
    let fixture = Fixture::new(&[]);
    let mut app = fixture.app();
    assert_eq!(
        app.window_state(),
        WindowState {
            size: Vec2::new(super::super::DEFAULT_WIDTH, super::super::DEFAULT_HEIGHT),
            maximized: false,
            saved: false,
        }
    );

    app.window = WindowState {
        size: Vec2::new(1111.0, 777.0),
        maximized: false,
        saved: true,
    };
    app.save_window_state();
    assert_eq!(
        WindowState::load(Some(&fixture.settings())),
        app.window_state()
    );

    // A maximized window keeps the size it had before.
    app.window = WindowState {
        size: Vec2::new(3000.0, 2000.0),
        maximized: true,
        saved: true,
    };
    app.save_window_state();
    let saved = WindowState::load(Some(&fixture.settings()));
    assert_eq!(saved.size, Vec2::new(1111.0, 777.0));
    assert!(saved.maximized);

    // Without settings nothing is saved.
    let mut without = AdventureModsApp::new(fixture.services(), None);
    without.save_window_state();
}

fn inner_size_commands(harness: &Harness<'_, AdventureModsApp>) -> Vec<Vec2> {
    harness.output().viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::ViewportCommand::InnerSize(size) => Some(*size),
            _ => None,
        })
        .collect()
}

/// A harness whose window is `window` on a `monitor`, in points. It's laid
/// out at the layout's size, so the UI isn't zoomed.
fn harness_on_monitor(
    app: AdventureModsApp,
    monitor: Vec2,
    window: Vec2,
) -> Harness<'static, AdventureModsApp> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(
            super::super::LAYOUT_WIDTH,
            super::super::LAYOUT_HEIGHT,
        ))
        .build_ui_state(|ui, app: &mut AdventureModsApp| app.show(ui), app);
    let viewport = harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap();
    viewport.monitor_size = Some(monitor);
    viewport.inner_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, window));
    harness
}

#[test]
fn a_window_bigger_than_its_monitor_shrinks_to_fit() {
    let fixture = Fixture::new(&[]);
    let mut app = fixture.app();
    app.window.saved = true;
    let mut harness = harness_on_monitor(app, Vec2::new(1280.0, 720.0), Vec2::new(1152.0, 864.0));
    harness.step();
    assert_eq!(
        inner_size_commands(&harness),
        vec![Vec2::new(1152.0, 720.0)]
    );

    // It only checks once, so a later resize is left alone.
    harness.step();
    assert!(inner_size_commands(&harness).is_empty());
}

#[test]
fn the_first_window_is_sized_for_its_monitor() {
    let fixture = Fixture::new(&[]);
    let default = Vec2::new(super::super::DEFAULT_WIDTH, super::super::DEFAULT_HEIGHT);
    let first_size = |monitor: Vec2, window: Vec2| {
        let mut harness = harness_on_monitor(fixture.app(), monitor, window);
        harness.step();
        inner_size_commands(&harness)
    };

    // A 1440p screen opens a bigger window.
    assert_eq!(
        first_size(Vec2::new(2560.0, 1440.0), default),
        vec![Vec2::new(1280.0, 960.0)]
    );
    // A 1080p screen already has the right size.
    assert!(first_size(Vec2::new(1920.0, 1080.0), default).is_empty());

    // A saved size is kept.
    let mut app = fixture.app();
    app.window.saved = true;
    let mut harness = harness_on_monitor(app, Vec2::new(2560.0, 1440.0), default);
    harness.step();
    assert!(inner_size_commands(&harness).is_empty());

    // So is a maximized window.
    let mut app = fixture.app();
    app.window.maximized = true;
    let mut harness = harness_on_monitor(app, Vec2::new(2560.0, 1440.0), default);
    harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .maximized = Some(true);
    harness.step();
    assert!(inner_size_commands(&harness).is_empty());
}

#[test]
fn first_window_sizes_follow_the_monitor() {
    assert_eq!(
        first_window_size(Vec2::new(2560.0, 1440.0)),
        Vec2::new(1280.0, 960.0)
    );
    assert_eq!(
        first_window_size(Vec2::new(3840.0, 2160.0)),
        Vec2::new(1920.0, 1440.0)
    );
    // Small screens get the default size.
    assert_eq!(
        first_window_size(Vec2::new(1366.0, 768.0)),
        Vec2::new(960.0, 720.0)
    );
    // Never bigger than the monitor.
    assert_eq!(
        first_window_size(Vec2::new(800.0, 600.0)),
        Vec2::new(800.0, 600.0)
    );
}

#[test]
fn the_ui_zooms_with_the_window_to_keep_the_layout() {
    assert_eq!(zoom_for_window(Vec2::new(1088.0, 816.0)), 1.0);
    // Big windows zoom in, in steps of 2.5% rounded down.
    assert_eq!(zoom_for_window(Vec2::new(1920.0, 1080.0)), 1.3);
    assert_eq!(zoom_for_window(Vec2::new(1280.0, 960.0)), 1.175);
    assert_eq!(zoom_for_window(Vec2::new(7680.0, 4320.0)), 2.0);
    assert_eq!(zoom_for_window(Vec2::new(960.0, 720.0)), 0.875);
    // The Steam Deck is a little short.
    assert_eq!(zoom_for_window(Vec2::new(1280.0, 800.0)), 0.975);
    // A window a hair smaller than a step stays on the step below.
    assert_eq!(zoom_for_window(Vec2::new(1087.0, 816.0)), 0.975);
    // Text stays readable in tiny windows.
    assert_eq!(zoom_for_window(Vec2::new(400.0, 300.0)), 0.8);

    // A Steam Deck sized window zooms out within the same frame, and stays.
    let ctx = egui::Context::default();
    let frame = |size: Vec2| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        let mut sizes = Vec::new();
        ctx.run_ui(input, |ui| {
            fit_zoom_to_window(ui.ctx());
            sizes.push(ui.ctx().content_rect().size());
        })
        .textures_delta
        .clear();
        sizes
    };
    let deck = Vec2::new(1280.0, 800.0);
    let zoom = 0.975;
    // The first layout is thrown away and redone at the new zoom.
    let sizes = frame(deck);
    assert_eq!(sizes.len(), 2);
    assert_eq!(ctx.zoom_factor(), zoom);
    assert!((sizes[1] - deck / zoom).length() < 0.5, "{sizes:?}");
    // Then it's steady: one layout per frame.
    assert_eq!(frame(deck / zoom).len(), 1);
    assert_eq!(ctx.zoom_factor(), zoom);
}

#[test]
fn a_window_that_fits_its_monitor_keeps_its_size() {
    assert_eq!(
        fit_to_monitor(Vec2::new(1152.0, 864.0), Vec2::new(2048.0, 1152.0)),
        None
    );
    assert_eq!(
        fit_to_monitor(Vec2::new(1152.0, 864.0), Vec2::new(1024.0, 768.0)),
        Some(Vec2::new(1024.0, 768.0))
    );
}

#[test]
fn the_window_state_follows_the_viewport() {
    let fixture = Fixture::new(&[]);
    let mut harness = fixture.harness();
    harness.run_steps(2);
    assert_eq!(
        harness.state().window_state().size,
        Vec2::new(super::super::DEFAULT_WIDTH, super::super::DEFAULT_HEIGHT)
    );

    // F11 toggles fullscreen; the harness ignores the request.
    harness.key_press(egui::Key::F11);
    harness.run_steps(2);
}

#[test]
fn development_builds_get_a_badge() {
    assert_eq!(profile_badge("development"), "DEVEL");
    assert_eq!(profile_badge("default"), "");
    let _ = Services::real();
}

#[test]
fn the_eframe_app_quits_after_one_frame_when_asked() {
    let _env = crate::test_env::lock();
    let fixture = Fixture::new(&[]);
    unsafe { std::env::set_var("ADVENTURE_MODS_QUIT_AFTER_FIRST_FRAME", "1") };
    let app = EframeApp::new(fixture.app());
    unsafe { std::env::remove_var("ADVENTURE_MODS_QUIT_AFTER_FIRST_FRAME") };
    assert!(app.quit_after_first_frame);
    assert!(!EframeApp::new(fixture.app()).quit_after_first_frame);
}
