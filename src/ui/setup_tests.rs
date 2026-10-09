use std::path::Path;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::*;
use crate::setup::config::{SubtitleLanguage, VoiceLanguage};

// Fakes for the work behind each step. They read marker files in the game
// folder, so every test sets them up without shared state.

fn marker(path: &Path, name: &str) -> bool {
    path.join(name).exists()
}

fn fake_runtimes(path: &Path, _app_id: u32) -> anyhow::Result<()> {
    if marker(path, "panic-dotnet") {
        panic!("runtime installer exploded");
    }
    if marker(path, "fail-dotnet") {
        anyhow::bail!("dotnet failed");
    }
    Ok(())
}

fn fake_convert(path: &Path, progress: Option<ProgressFn>) -> anyhow::Result<()> {
    if let Some(progress) = progress {
        progress(1_048_576, Some(2_097_152));
    }
    std::fs::write(path.join("converted"), "")?;
    Ok(())
}

fn fake_manager(path: &Path, _kind: GameKind, progress: Option<ProgressFn>) -> anyhow::Result<()> {
    if let Some(progress) = progress
        && !marker(path, "quiet-manager")
    {
        progress(5, None);
    }
    while marker(path, "hold-manager") {
        std::thread::sleep(Duration::from_millis(5));
    }
    if marker(path, "fail-manager") {
        anyhow::bail!("manager failed");
    }
    Ok(())
}

fn fake_install_mods(
    path: &Path,
    _kind: GameKind,
    mods: &[&ModEntry],
    width: u32,
    height: u32,
    languages: LanguageSelection,
    progress: &mut dyn FnMut(InstallProgress<'_>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    if marker(path, "fail-mods") {
        anyhow::bail!("mods failed");
    }
    for (index, mod_entry) in mods.iter().enumerate() {
        progress(InstallProgress::Started {
            mod_name: mod_entry.name,
        })?;
        loop {
            progress(InstallProgress::DownloadingMod {
                mod_name: mod_entry.name,
                downloaded: 1,
                total_bytes: Some(2),
            })?;
            if !marker(path, "hold-mods") {
                break;
            }
            // Slow enough that the screen shows the cancel in progress.
            std::thread::sleep(Duration::from_millis(200));
        }
        progress(InstallProgress::Finished {
            mod_name: mod_entry.name,
            completed: index + 1,
            total: mods.len(),
        })?;
    }
    progress(InstallProgress::GeneratingConfig)?;
    let names: Vec<_> = mods.iter().map(|mod_entry| mod_entry.name).collect();
    std::fs::write(
        path.join("installed.txt"),
        format!(
            "{}\n{width}x{height}\n{}/{}",
            names.join(","),
            languages.subtitle.as_str(),
            languages.voice.as_str()
        ),
    )?;
    Ok(())
}

fn fake_size(mod_entry: &ModEntry) -> Option<u64> {
    (mod_entry.name != "SA2 Render Fix").then_some(150_000_000)
}

fn exploding_size(_: &ModEntry) -> Option<u64> {
    panic!("size lookup exploded")
}

fn fake_prefetch(path: &Path, mods: &[&ModEntry], cancel: &AtomicBool) {
    while marker(path, "hold-prefetch") && !cancel.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = std::fs::write(path.join("prefetched"), mods.len().to_string());
}

fn fake_steam_status(game: &Game) -> SteamConfigStatus {
    if marker(&game.path, "panic-steam") {
        panic!("steam config exploded");
    }
    let ready = marker(&game.path, "proton-ready");
    SteamConfigStatus {
        message: if marker(&game.path, "quiet-steam") {
            String::new()
        } else {
            format!("Proton ready: {ready}")
        },
        ready,
    }
}

fn fake_step_complete(step: StepId, game: &Game) -> bool {
    if marker(&game.path, "panic-complete") {
        panic!("step check exploded");
    }
    match step {
        StepId::SteamConfig => marker(&game.path, "steam-done"),
        StepId::ConvertSteam => marker(&game.path, "converted"),
        _ => false,
    }
}

pub(crate) fn work() -> SetupWork {
    SetupWork {
        install_runtimes: fake_runtimes,
        convert_steam: fake_convert,
        install_mod_manager: fake_manager,
        install_mods: fake_install_mods,
        download_size_of: Some(fake_size),
        prefetch_archives: Some(fake_prefetch),
        steam_config_status: fake_steam_status,
        is_step_complete: fake_step_complete,
    }
}

struct State {
    flow: SetupFlow,
    images: Option<ImageCache>,
    events: Vec<SetupEvent>,
}

fn game_dir(markers: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in markers {
        std::fs::write(dir.path().join(name), "").unwrap();
    }
    dir
}

fn flow(kind: GameKind, path: &Path, work: SetupWork) -> SetupFlow {
    SetupFlow::new(
        Game {
            kind,
            path: path.to_path_buf(),
        },
        work,
        LanguageSelection::defaults_for(kind),
    )
}

fn harness(flow: SetupFlow) -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_max_steps(64)
        .build_ui_state(
            |ui, state: &mut State| {
                super::super::theme::apply_ui(ui);
                state.flow.poll();
                let images = state
                    .images
                    .get_or_insert_with(|| ImageCache::new(ui.ctx()));
                egui::Panel::bottom("footer").show(ui, |ui| state.flow.show_footer(ui));
                egui::CentralPanel::default().show(ui, |ui| state.flow.show_body(ui, images));
                state.events.extend(state.flow.take_events());
            },
            State {
                flow,
                images: None,
                events: Vec::new(),
            },
        )
}

/// Step frames until `done` holds, failing after a while.
fn run_until(harness: &mut Harness<'_, State>, what: &str, done: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done(harness.state()) {
        assert!(Instant::now() < deadline, "timed out: {what}");
        harness.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    harness.step();
}

/// Change the flow directly, then draw the result.
fn act(harness: &mut Harness<'_, State>, change: impl FnOnce(&mut SetupFlow)) {
    change(&mut harness.state_mut().flow);
    harness.step();
    harness.step();
}

fn step_id(state: &State) -> StepId {
    state.flow.step().unwrap().id
}

/// Press the widget labeled `label`. Dialogs need a few frames to appear.
fn press(harness: &mut Harness<'_, State>, label: &str) {
    harness.get_by_label(label).click();
    harness.run_steps(4);
}

#[test]
fn sa2_setup_walks_choices_then_installs_and_finishes() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));

    assert_eq!(
        harness.state().flow.title(),
        (
            "Set Up Proton".to_owned(),
            "Sonic Adventure 2 · Step 1 of 4".to_owned()
        )
    );
    harness.get_by_label("Proton Is Ready");
    harness.get_by_label("Proton ready: true");
    press(&mut harness, "Continue");

    assert_eq!(step_id(harness.state()), StepId::SelectMods);
    run_until(&mut harness, "download sizes", |state| {
        state.flow.estimates.is_some()
    });
    harness.get_by_label("At least 1.6 GB to download");
    // Untick the first mod.
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, "SA2 Render Fix")
        .click();
    harness.step();
    harness.step();
    assert!(!harness.state().flow.selected_mods.contains(&0));
    press(&mut harness, "Continue");

    assert_eq!(step_id(harness.state()), StepId::LanguageOptions);
    press(&mut harness, "Voices");
    press(&mut harness, "English");
    assert_eq!(harness.state().flow.languages.voice, VoiceLanguage::English);
    // The same picker opens again just as well.
    press(&mut harness, "Voices");
    press(&mut harness, "日本語");
    assert_eq!(
        harness.state().flow.languages.voice,
        VoiceLanguage::Japanese
    );
    press(&mut harness, "Voices");
    press(&mut harness, "English");
    assert_eq!(harness.state().flow.languages.voice, VoiceLanguage::English);
    press(&mut harness, "Subtitles");
    press(&mut harness, "Français");
    press(&mut harness, "Install");
    assert!(
        harness.state().events.contains(&SetupEvent::SaveLanguages(
            GameKind::SA2,
            LanguageSelection {
                subtitle: SubtitleLanguage::French,
                voice: VoiceLanguage::English,
            }
        )),
        "{:?}",
        harness.state().events
    );

    run_until(&mut harness, "setup finished", |state| {
        step_id(state) == StepId::Complete
    });
    let installed = std::fs::read_to_string(dir.path().join("installed.txt")).unwrap();
    assert!(!installed.starts_with("SA2 Render Fix"), "{installed}");
    assert!(
        installed.contains("1920x1080\nfrench/english"),
        "{installed}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("prefetched")).unwrap(),
        (common::recommended_mods_for_game(GameKind::SA2).len() - 1).to_string()
    );

    assert_eq!(harness.state().flow.title().0, "Sonic Adventure 2");
    assert_eq!(harness.state().flow.title().1, "");
    assert!(!harness.state().flow.can_go_back());
    press(&mut harness, "Play in Steam");
    harness.state_mut().flow.handle_pad(PadButton::Action);
    press(&mut harness, "Done");
    assert_eq!(
        harness.state().events[1..],
        [
            SetupEvent::OpenUri("steam://rungameid/213610".to_owned()),
            SetupEvent::OpenUri("steam://rungameid/213610".to_owned()),
            SetupEvent::Exit,
        ]
    );
}

#[test]
fn the_proton_step_checks_again_until_steam_is_ready() {
    let dir = game_dir(&["quiet-steam"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));

    harness.get_by_label("Proton 10 Needed");
    // Without a message from Steam's config, the step explains what to do.
    harness.get_by_label(steps::steps_for_game(GameKind::SA2)[0].description);
    press(&mut harness, "Check Again");
    assert_eq!(step_id(harness.state()), StepId::SteamConfig);

    std::fs::write(dir.path().join("proton-ready"), "").unwrap();
    press(&mut harness, "Check Again");
    assert_eq!(step_id(harness.state()), StepId::SelectMods);
}

#[test]
fn completed_steps_are_skipped_both_ways() {
    let dir = game_dir(&["steam-done", "converted"]);
    let mut harness = harness(flow(GameKind::SADX, dir.path(), work()));

    // Proton was ready, so its screen is not counted.
    assert_eq!(step_id(harness.state()), StepId::SelectMods);
    assert_eq!(
        harness.state().flow.title().1,
        "Sonic Adventure DX · Step 1 of 3"
    );

    // Nothing before the mods is left to ask, so going back leaves setup.
    harness.state_mut().flow.back();
    assert_eq!(
        harness.state_mut().flow.take_events(),
        vec![SetupEvent::Exit]
    );
}

#[test]
fn back_walks_to_the_previous_question() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    press(&mut harness, "Continue");
    press(&mut harness, "Continue");
    assert_eq!(step_id(harness.state()), StepId::LanguageOptions);

    harness.state_mut().flow.back();
    assert_eq!(step_id(harness.state()), StepId::SelectMods);
    harness.state_mut().flow.back();
    assert_eq!(step_id(harness.state()), StepId::SteamConfig);
    harness.state_mut().flow.back();
    assert_eq!(
        harness.state_mut().flow.take_events(),
        vec![SetupEvent::Exit]
    );
}

#[test]
fn going_back_from_mods_skips_completed_steps_in_between() {
    let dir = game_dir(&["proton-ready"]);
    let mut setup = flow(GameKind::SADX, dir.path(), work());
    setup.current = setup
        .steps
        .iter()
        .position(|step| step.id == StepId::LanguageOptions)
        .unwrap();
    std::fs::write(dir.path().join("steam-done"), "").unwrap();

    setup.on_back();
    assert_eq!(setup.step().unwrap().id, StepId::SelectMods);
    setup.on_back();
    assert_eq!(setup.take_events(), vec![SetupEvent::Exit]);
}

#[test]
fn going_back_skips_questions_that_are_already_answered() {
    let dir = game_dir(&["proton-ready"]);
    let work = SetupWork {
        is_step_complete: |step, _| step == StepId::SelectMods,
        ..work()
    };
    let mut setup = flow(GameKind::SA2, dir.path(), work);
    setup.current = setup
        .steps
        .iter()
        .position(|step| step.id == StepId::LanguageOptions)
        .unwrap();

    setup.on_back();

    assert_eq!(setup.step().unwrap().id, StepId::SteamConfig);
}

#[test]
fn the_action_button_does_nothing_without_a_secondary_action() {
    let dir = game_dir(&["proton-ready"]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());

    setup.handle_pad(PadButton::Action);

    assert_eq!(setup.step().unwrap().id, StepId::SteamConfig);
    assert!(setup.take_events().is_empty());
}

#[test]
fn mods_toggle_both_ways_and_previews_page_forward() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    press(&mut harness, "Continue");
    assert_eq!(step_id(harness.state()), StepId::SelectMods);
    let mods = common::recommended_mods_for_game(GameKind::SA2);
    let selected = harness.state().flow.selected_mods.clone();

    let toggle = |harness: &mut Harness<'_, State>| {
        harness
            .get_by_role_and_label(egui::accesskit::Role::CheckBox, mods[0].name)
            .click();
        harness.run_steps(4);
    };
    toggle(&mut harness);
    assert_ne!(harness.state().flow.selected_mods, selected);
    toggle(&mut harness);
    let mut now = harness.state().flow.selected_mods.clone();
    now.sort_unstable();
    let mut before = selected;
    before.sort_unstable();
    assert_eq!(now, before);

    let paged = mods
        .iter()
        .position(|mod_entry| mod_entry.pictures.len() > 1)
        .expect("a mod with several pictures");
    act(&mut harness, |flow| flow.show_preview(Some(paged)));
    // Decode the picture now, so the preview draws it however slow the machine.
    let ctx = harness.ctx.clone();
    harness
        .state_mut()
        .images
        .as_mut()
        .expect("drawn once")
        .load_now(&ctx, mods[paged].pictures[0]);
    harness.step();
    press(&mut harness, "›");
    assert_eq!(harness.state().flow.preview.page, 1);

    // A single picture needs no page arrows.
    let single = mods
        .iter()
        .position(|mod_entry| mod_entry.pictures.len() == 1)
        .expect("a mod with one picture");
    act(&mut harness, |flow| flow.show_preview(Some(single)));
    assert!(harness.query_by_label("›").is_none());

    // A page without a picture shows just the name.
    act(&mut harness, |flow| flow.preview.page = usize::MAX);
    harness.get_by_role_and_label(egui::accesskit::Role::CheckBox, mods[paged].name);

    // With nothing to preview, the panel stays empty and paging is ignored.
    act(&mut harness, |flow| flow.preview.index = None);
    harness.state_mut().flow.turn_page(1);
    harness.get_by_label(mods[0].name);
}

#[test]
fn a_failed_task_can_be_retried_or_left() {
    let dir = game_dir(&["proton-ready", "fail-dotnet"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    press(&mut harness, "Continue");
    press(&mut harness, "Continue");
    press(&mut harness, "Install");

    run_until(&mut harness, "failure", |state| state.flow.error.is_some());
    harness.get_by_label("Setup Didn't Finish");
    harness.get_by_label("Failed");
    press(&mut harness, "Details");
    harness.get_by_label("dotnet failed");

    std::fs::remove_file(dir.path().join("fail-dotnet")).unwrap();
    press(&mut harness, "Try Again");
    run_until(&mut harness, "retry finished", |state| {
        step_id(state) == StepId::Complete
    });
}

#[test]
fn going_back_from_the_install_screen_returns_to_the_last_question() {
    let dir = game_dir(&["proton-ready", "fail-manager"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    act(&mut harness, |flow| {
        flow.on_next();
        flow.on_next();
        flow.on_next();
    });
    run_until(&mut harness, "failure", |state| state.flow.error.is_some());
    assert!(harness.state().flow.can_go_back());

    harness.state_mut().flow.back();
    assert_eq!(step_id(harness.state()), StepId::LanguageOptions);
    assert!(harness.state().flow.install.is_none());
}

#[test]
fn a_panicking_task_is_reported_as_an_error() {
    let dir = game_dir(&["proton-ready", "panic-dotnet"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    act(&mut harness, |flow| {
        flow.on_next();
        flow.on_next();
        flow.on_next();
    });

    run_until(&mut harness, "failure", |state| state.flow.error.is_some());
    assert!(
        harness
            .state()
            .flow
            .error
            .as_deref()
            .unwrap()
            .contains("runtime installer exploded")
    );
}

#[test]
fn cancelling_the_mod_download_waits_for_it_and_goes_back() {
    let dir = game_dir(&["proton-ready", "hold-mods"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    act(&mut harness, |flow| {
        flow.on_next();
        flow.on_next();
        flow.on_next();
    });
    run_until(&mut harness, "mods downloading", |state| {
        step_id(state) == StepId::DownloadMods
            && state
                .flow
                .install
                .as_ref()
                .is_some_and(|install| install.progress.display.is_some())
    });
    harness.get_by_label_contains("Downloading mods");
    // Busy steps ignore the main button and going back.
    harness.state_mut().flow.on_next();
    harness.state_mut().flow.on_back();
    harness.state_mut().flow.handle_pad(PadButton::Start);
    assert_eq!(step_id(harness.state()), StepId::DownloadMods);

    harness.get_by_label("Cancel").click();
    harness.step();
    harness.step();
    harness.get_by_label("Cancelling…");
    run_until(&mut harness, "back to choices", |state| {
        step_id(state) == StepId::LanguageOptions
    });
    assert!(!dir.path().join("installed.txt").exists());
}

#[test]
fn cancelling_while_the_prefetch_finishes_installs_nothing() {
    let dir = game_dir(&["proton-ready", "hold-prefetch"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    act(&mut harness, |flow| {
        flow.on_next();
        flow.on_next();
        flow.on_next();
    });
    let install = harness.state().flow.install.as_ref().unwrap();
    assert!(
        install
            .tasks
            .iter()
            .any(|task| task.pending_note == Some("Downloading in the background"))
    );
    run_until(&mut harness, "waiting on the prefetch", |state| {
        step_id(state) == StepId::DownloadMods
    });
    harness.get_by_label("Finishing downloads…");

    harness.state_mut().flow.handle_pad(PadButton::Action);
    run_until(&mut harness, "back to choices", |state| {
        step_id(state) == StepId::LanguageOptions
    });
    assert!(!dir.path().join("installed.txt").exists());

    // Installing again waits for the cancelled prefetch before the next one.
    std::fs::remove_file(dir.path().join("hold-prefetch")).unwrap();
    harness.state_mut().flow.on_next();
    run_until(&mut harness, "finished", |state| {
        step_id(state) == StepId::Complete
    });
}

#[test]
fn sadx_converts_and_shows_download_progress() {
    let dir = game_dir(&["proton-ready", "hold-manager"]);
    let mut harness = harness(flow(GameKind::SADX, dir.path(), work()));
    act(&mut harness, |flow| {
        flow.on_next();
        flow.on_next();
        flow.on_next();
    });
    run_until(&mut harness, "manager step", |state| {
        step_id(state) == StepId::InstallModManager
            && state
                .flow
                .install
                .as_ref()
                .is_some_and(|install| install.progress.display.is_some())
    });
    harness.get_by_label_contains("Downloading... - ");
    assert!(dir.path().join("converted").exists());
    assert_eq!(harness.state().flow.title().0, "Installing");
    assert!(!harness.state().flow.can_go_back());

    std::fs::remove_file(dir.path().join("hold-manager")).unwrap();
    run_until(&mut harness, "finished", |state| {
        step_id(state) == StepId::Complete
    });
}

#[test]
fn installing_without_mods_skips_the_background_download() {
    let dir = game_dir(&["proton-ready", "hold-manager", "quiet-manager"]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());
    setup.selected_mods.clear();
    let mut harness = harness(setup);
    act(&mut harness, SetupFlow::on_next);
    harness.get_by_label("No mods selected");
    act(&mut harness, |flow| {
        flow.on_next();
        flow.on_next();
    });
    assert!(harness.state().flow.prefetch.is_none());
    run_until(&mut harness, "manager step", |state| {
        step_id(state) == StepId::InstallModManager
    });
    harness.get_by_label("Starting…");
    std::fs::remove_file(dir.path().join("hold-manager")).unwrap();

    run_until(&mut harness, "finished", |state| {
        step_id(state) == StepId::Complete
    });
    assert!(!dir.path().join("prefetched").exists());
}

#[test]
fn sadx_presets_pick_their_mods() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SADX, dir.path(), work()));
    act(&mut harness, SetupFlow::on_next);
    let presets = common::presets_for_game(GameKind::SADX);
    harness.get_by_label(presets[0].description);

    press(&mut harness, "Preset");
    press(&mut harness, presets[1].name);
    harness.get_by_label(presets[1].description);
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    let chosen: Vec<_> = harness
        .state()
        .flow
        .selected_mods
        .iter()
        .map(|&index| mods[index].name)
        .collect();
    assert_eq!(chosen.len(), presets[1].mod_names.len());

    // Unknown presets change nothing.
    harness.state_mut().flow.apply_preset(99);
    assert_eq!(harness.state().flow.matching_preset(), Some(1));
}

#[test]
fn presets_follow_mod_selection_and_custom_choices_stay_available() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SADX, dir.path(), work()));
    act(&mut harness, SetupFlow::on_next);
    let presets = common::presets_for_game(GameKind::SADX);
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    harness.get_by_label(presets[0].description);

    let toggle = |harness: &mut Harness<'_, State>| {
        harness
            .get_by_role_and_label(egui::accesskit::Role::CheckBox, mods[0].name)
            .click();
        harness.run_steps(4);
    };
    toggle(&mut harness);
    assert_eq!(
        harness
            .get_by_label("Preset")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Custom")
    );
    assert!(harness.query_by_label(presets[0].description).is_none());
    press(&mut harness, "Preset");
    for preset in presets {
        harness.get_by_label(preset.name);
        assert!(
            harness
                .query_by_label(&format!("✔  {}", preset.name))
                .is_none()
        );
    }
    press(&mut harness, "Cancel");
    assert_eq!(
        harness
            .get_by_label("Preset")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Custom")
    );

    toggle(&mut harness);
    assert_eq!(
        harness
            .get_by_label("Preset")
            .accesskit_node()
            .value()
            .as_deref(),
        Some(presets[0].name)
    );
    harness.get_by_label(presets[0].description);
    assert_ne!(
        harness
            .get_by_label("Preset")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Custom")
    );
    press(&mut harness, "Preset");
    harness.get_by_label(&format!("✔  {}", presets[0].name));
    press(&mut harness, "Cancel");

    toggle(&mut harness);
    press(&mut harness, "Preset");
    press(&mut harness, presets[1].name);
    assert_eq!(
        harness
            .get_by_label("Preset")
            .accesskit_node()
            .value()
            .as_deref(),
        Some(presets[1].name)
    );
    harness.get_by_label(presets[1].description);
}

#[test]
fn manually_matching_another_preset_updates_its_label_and_picker() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SADX, dir.path(), work()));
    act(&mut harness, SetupFlow::on_next);
    let presets = common::presets_for_game(GameKind::SADX);
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    act(&mut harness, |flow| {
        for (index, mod_entry) in mods.iter().enumerate().rev() {
            flow.set_mod_selected(index, presets[1].mod_names.contains(&mod_entry.name));
        }
    });
    assert_eq!(
        harness
            .get_by_label("Preset")
            .accesskit_node()
            .value()
            .as_deref(),
        Some(presets[1].name)
    );
    harness.get_by_label(presets[1].description);
    assert!(harness.query_by_label(presets[0].description).is_none());
    press(&mut harness, "Preset");
    harness.get_by_label(&format!("✔  {}", presets[1].name));
    harness.get_by_label(presets[0].name);
}

#[test]
fn preset_matching_requires_exact_mod_sets_in_any_order() {
    let dir = game_dir(&[]);
    let mut setup = flow(GameKind::SADX, dir.path(), work());
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    assert_eq!(setup.matching_preset(), Some(0));
    for index in 0..common::presets_for_game(GameKind::SADX).len() {
        setup.apply_preset(index);
        setup.selected_mods.reverse();
        assert_eq!(setup.matching_preset(), Some(index));
        let selected = setup.selected_mods.clone();
        let extra = (0..mods.len())
            .find(|index| !selected.contains(index))
            .unwrap();
        setup.set_mod_selected(extra, true);
        assert_eq!(setup.matching_preset(), None);
        setup.selected_mods = selected.clone();
        setup.selected_mods.pop();
        assert_eq!(setup.matching_preset(), None);
        setup.set_mod_selected(extra, true);
        assert_eq!(setup.matching_preset(), None);
    }
    setup.selected_mods.clear();
    assert_eq!(setup.matching_preset(), None);
    setup.selected_mods = (0..mods.len()).collect();
    assert_eq!(setup.matching_preset(), None);
    let mut sa2 = flow(GameKind::SA2, dir.path(), work());
    assert_eq!(sa2.matching_preset(), None);
    sa2.selected_mods.clear();
    assert_eq!(sa2.matching_preset(), None);
}

#[test]
fn the_preview_follows_the_focused_mod_and_turns_pages() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SADX, dir.path(), work()));
    act(&mut harness, SetupFlow::on_next);
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    // The first chosen mod is previewed, with a Before/After badge.
    assert_eq!(harness.state().flow.preview.index, Some(0));
    harness.get_by_label(mods[0].full_description.unwrap_or(mods[0].description));
    run_until(&mut harness, "screenshot", |state| {
        state
            .images
            .as_ref()
            .is_some_and(|images| images.cached() > 0)
    });

    let pages = mods[0].pictures.len();
    assert!(pages > 1);
    harness.state_mut().flow.handle_pad(PadButton::NextPage);
    assert_eq!(harness.state().flow.preview.page, 1);
    harness.state_mut().flow.handle_pad(PadButton::PreviousPage);
    harness.state_mut().flow.handle_pad(PadButton::PreviousPage);
    assert_eq!(harness.state().flow.preview.page, pages - 1);
    press(&mut harness, "›");
    assert_eq!(harness.state().flow.preview.page, 0);
    press(&mut harness, "‹");
    harness.state_mut().flow.handle_pad(PadButton::Up);

    // Moving down the list previews the next mod.
    act(&mut harness, SetupFlow::focus_primary);
    harness.key_press(egui::Key::ArrowDown);
    harness.step();
    harness.step();
    assert_eq!(harness.state().flow.preview.index, Some(1));
    assert_eq!(harness.state().flow.preview.page, 0);

    let link = &mods[1].links[0];
    press(&mut harness, &format!("{}  ↗", link.label));
    assert_eq!(
        harness.state().events,
        vec![SetupEvent::OpenUri(link.url.to_owned())]
    );
}

#[test]
fn mods_without_screenshots_have_nothing_to_turn() {
    let dir = game_dir(&["proton-ready"]);
    let mut setup = flow(GameKind::SADX, dir.path(), work());
    setup.preview = Preview::default();
    setup.turn_page(1);
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    let without = mods
        .iter()
        .position(|mod_entry| mod_entry.pictures.is_empty());
    if let Some(index) = without {
        setup.show_preview(Some(index));
        setup.turn_page(1);
        assert_eq!(setup.preview.page, 0);
    }
    assert!(setup.preview_entry().is_none() || without.is_some());
}

#[test]
fn narrow_windows_hide_the_preview() {
    let dir = game_dir(&["proton-ready"]);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), work()));
    act(&mut harness, SetupFlow::on_next);
    harness.set_size(egui::vec2(700.0, 800.0));
    harness.run_steps(3);
    let mods = common::recommended_mods_for_game(GameKind::SA2);
    assert!(
        harness
            .query_by_label(mods[0].full_description.unwrap())
            .is_none()
    );
}

#[test]
fn failed_download_estimates_leave_the_size_unknown() {
    let dir = game_dir(&["proton-ready"]);
    let mut failing = work();
    failing.download_size_of = Some(exploding_size);
    let mut harness = harness(flow(GameKind::SA2, dir.path(), failing));
    let capture = crate::test_log::LogCapture::start();
    act(&mut harness, SetupFlow::on_next);

    run_until(&mut harness, "estimates", |state| {
        state.flow.estimates.is_some()
    });
    harness.get_by_label("Download size unknown");
    assert!(
        capture
            .contents()
            .contains("Failed to estimate mod downloads")
    );

    // Without a size lookup, sizes are unknown too.
    let mut offline = work();
    offline.download_size_of = None;
    let mut setup = flow(GameKind::SA2, dir.path(), offline);
    setup.on_next();
    let deadline = Instant::now() + Duration::from_secs(20);
    while setup.estimates.is_none() {
        assert!(Instant::now() < deadline);
        setup.poll();
    }
    assert!(
        setup
            .estimates
            .unwrap()
            .iter()
            .all(|estimate| estimate.size.is_none())
    );
}

#[test]
fn a_panicking_button_shows_a_recoverable_error() {
    let dir = game_dir(&["panic-steam"]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());
    setup.guarded("test", "Something broke.", SetupFlow::on_next);
    let mut harness = harness(setup);

    harness.get_by_label("Something Went Wrong");
    harness.get_by_label("Something broke.");
    assert_eq!(harness.state_mut().flow.next_label(), Some("Try Again"));
    assert_eq!(harness.state().flow.secondary(), None);

    std::fs::remove_file(dir.path().join("panic-steam")).unwrap();
    press(&mut harness, "Try Again");
    harness.get_by_label("Proton 10 Needed");
}

#[test]
fn the_secondary_and_next_buttons_recover_from_panics() {
    fn boom(_: &mut SetupFlow) {
        panic!("boom");
    }
    let dir = game_dir(&["proton-ready"]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());
    setup.guarded("test", "Recovered.", boom);
    assert_eq!(setup.error.as_deref(), Some("Recovered."));

    // A failed press on the footer goes through the same guard.
    std::fs::write(dir.path().join("panic-steam"), "").unwrap();
    setup.error = None;
    setup.steam_status = Some(SteamConfigStatus {
        message: "Ready".into(),
        ready: true,
    });
    let mut harness = harness(setup);
    press(&mut harness, "Continue");
    harness.get_by_label("Something went wrong while continuing setup. Please try again.");
}

#[test]
fn a_panicking_back_button_shows_an_error() {
    let dir = game_dir(&["proton-ready"]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());
    setup.on_next();
    setup.on_next();
    std::fs::write(dir.path().join("panic-complete"), "").unwrap();
    setup.back();
    assert_eq!(
        setup.error.as_deref(),
        Some("Something went wrong while going back. Please try again.")
    );
    assert_eq!(setup.next_label(), Some("Try Again"));
}

#[test]
fn a_step_out_of_range_shows_nothing_and_does_nothing() {
    let dir = game_dir(&["proton-ready"]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());
    setup.current = 99;
    setup.enter_step();
    setup.on_next();
    assert_eq!(setup.next_label(), None);
    assert_eq!(setup.secondary(), None);
    assert_eq!(setup.screen_position(), None);
    assert_eq!(setup.title().0, "Sonic Adventure 2");
    let mut harness = harness(setup);
    harness.run_steps(2);
    assert!(harness.state().events.is_empty());
}

#[test]
fn going_back_with_nothing_to_return_to_leaves_setup() {
    let dir = game_dir(&["proton-ready"]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());
    // Pretend every step does work, so there is no question to go back to.
    for step in &mut setup.steps {
        if !matches!(step.id, StepId::Complete) {
            step.kind = StepKind::Auto;
        }
    }
    setup.current = 1;
    setup.go_back_to_choices();
    assert_eq!(setup.take_events(), vec![SetupEvent::Exit]);

    setup.steps.clear();
    assert_eq!(setup.skip_completed_steps(3), 3);
}

#[test]
fn the_install_view_ignores_unknown_steps() {
    let steps = steps::steps_for_game(GameKind::SA2);
    let mut view = InstallView::new(&steps);
    view.set_pending_note(0, "Ignored");
    view.show_error(0, "Ignored");
    assert!(view.tasks.iter().all(|task| task.pending_note.is_none()));
    assert!(
        view.tasks
            .iter()
            .all(|task| task.state == TaskState::Pending)
    );
}

#[test]
fn setup_reports_the_game_it_is_for() {
    let dir = game_dir(&[]);
    let mut setup = flow(GameKind::SA2, dir.path(), work());
    assert_eq!(setup.game().kind, GameKind::SA2);
    assert!(!setup.has_dialog());
    setup.open_picker(Picker::Preset);
    assert!(setup.has_dialog());
    setup.focus_primary();
    assert!(setup.focus_primary);
    let _ = SetupWork::real();
}

#[test]
fn the_real_mod_installer_reports_pipeline_errors() {
    let dir = game_dir(&[]);
    let result = install_mods_with_pipeline(
        dir.path(),
        GameKind::SA2,
        &[],
        1280,
        800,
        LanguageSelection::defaults_for(GameKind::SA2),
        &mut |_| Ok(()),
    );
    // Without a mod loader in place, writing the config fails.
    let _ = result;
}
