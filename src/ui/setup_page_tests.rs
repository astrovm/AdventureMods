use adw::prelude::*;
use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::glib;

use super::AdventureModsSetupPage;
use super::{InstallView, ModPreviewPage, remote_mod_download_size, resolution_or_fallback};
use super::{
    MOD_PREVIEW_CACHE_LIMIT, ModDownloadEstimate, ModPreview, ProgressDisplay, ProgressMsg,
    ProgressSamples, ProgressState, apply_install_progress, completed_mod_fraction,
    download_size_text, drain_progress_updates, estimate_mod_downloads, format_download_bytes_text,
    format_download_size, format_step_download_text, fraction_needs_update, initial_preview_index,
    load_preview_texture, mod_download_finished_text, mod_download_fraction,
    mod_download_progress_update, mod_download_start_text, publish_mod_bytes, publish_step_bytes,
    spawn_progress_receiver, subtitle_language_index, subtitle_language_labels,
    voice_language_index, voice_language_labels,
};
use crate::setup::config::{SubtitleLanguage, VoiceLanguage};
use crate::setup::steps::StepId;
use crate::setup::{common, pipeline, steps};
use crate::steam::game::Game;
use crate::steam::game::GameKind;
use crate::test_log::LogCapture;
use crate::ui::test_util::init_resource_overlay;

#[test]
fn initial_preview_prefers_first_selected_mod() {
    assert_eq!(initial_preview_index(5, &[3, 1]), Some(3));
}

#[test]
fn initial_preview_falls_back_to_first_mod_when_none_selected() {
    assert_eq!(initial_preview_index(5, &[]), Some(0));
}

#[test]
fn initial_preview_skips_out_of_range_selection() {
    assert_eq!(initial_preview_index(2, &[4, 1]), Some(1));
}

#[test]
fn initial_preview_is_none_when_no_mods_exist() {
    assert_eq!(initial_preview_index(0, &[0]), None);
}

#[test]
fn initial_preview_is_none_when_no_mods_and_no_selection() {
    assert_eq!(initial_preview_index(0, &[]), None);
}

#[test]
fn initial_preview_falls_back_to_zero_when_all_selections_out_of_range() {
    // All selected indices are out of range, but mods exist
    assert_eq!(initial_preview_index(3, &[5, 10, 99]), Some(0));
}

#[test]
fn sadx_subtitle_options_include_expected_values() {
    assert_eq!(
        subtitle_language_labels(GameKind::SADX),
        vec!["日本語", "English", "Français", "Español", "Deutsch"]
    );
}

#[test]
fn sa2_subtitle_options_include_expected_values() {
    assert_eq!(
        subtitle_language_labels(GameKind::SA2),
        vec![
            "English",
            "Deutsch",
            "Español",
            "Français",
            "Italiano",
            "日本語"
        ]
    );
}

#[test]
fn voice_options_include_expected_values() {
    assert_eq!(voice_language_labels(), vec!["日本語", "English"]);
}

#[test]
fn mod_download_fraction_starts_at_completed_items() {
    assert_eq!(mod_download_fraction(0, 2, 0, Some(100)), 0.0);
    assert_eq!(mod_download_fraction(1, 2, 0, Some(100)), 0.5);
}

#[test]
fn mod_download_fraction_reaches_completion_at_end_of_current_item() {
    assert_eq!(mod_download_fraction(1, 2, 100, Some(100)), 1.0);
}

#[test]
fn mod_download_progress_pulses_when_total_size_is_unknown() {
    let update = mod_download_progress_update(1, 4, 1_048_576, None);

    assert_eq!(update.fraction, completed_mod_fraction(1, 4));
    assert!(update.pulse);
    assert_eq!(update.text, "Downloading mods - 1.0 MB");
}

#[test]
fn mod_download_progress_uses_generic_label_for_aggregate_bytes() {
    let update = mod_download_progress_update(1, 4, 1_048_576, Some(2_097_152));

    assert_eq!(update.text, "Downloading mods - 1.0 / 2.0 MB");
}

#[test]
fn draining_progress_updates_keeps_the_latest_value() {
    let (sender, receiver) = async_channel::bounded(4);
    sender
        .try_send(ProgressMsg::Bytes {
            downloaded: 1_048_576,
            total: Some(4_194_304),
            status: "Downloading...".to_string(),
        })
        .unwrap();
    sender
        .try_send(ProgressMsg::Bytes {
            downloaded: 2_097_152,
            total: Some(4_194_304),
            status: "Downloading...".to_string(),
        })
        .unwrap();

    let mut state = ProgressState::default();
    drain_progress_updates(&receiver, &mut state);

    let display = state.display.unwrap();
    assert_eq!(display.fraction, Some(0.5));
    assert_eq!(display.text, "Downloading... - 2.0 / 4.0 MB");
}

#[test]
fn progress_samples_keep_latest_bytes_per_mod() {
    let samples = ProgressSamples::default();
    samples.set_mod_bytes("Alpha", 1_048_576, Some(2_097_152), 2);
    samples.set_mod_bytes("Beta", 524_288, Some(2_097_152), 2);
    samples.set_mod_bytes("Alpha", 1_572_864, Some(2_097_152), 2);

    let mut state = ProgressState::default();
    samples.apply_to(&mut state);

    assert_eq!(
        state.active_downloads.get("Alpha"),
        Some(&(1_572_864, Some(2_097_152)))
    );
    assert_eq!(
        state.active_downloads.get("Beta"),
        Some(&(524_288, Some(2_097_152)))
    );
    let display = state.display.unwrap();
    assert_eq!(display.text, "Downloading mods - 2.0 / 4.0 MB");
    assert!(!display.pulse);
}

#[test]
fn progress_samples_overwrite_step_bytes() {
    let samples = ProgressSamples::default();
    samples.set_step_bytes(1_048_576, Some(4_194_304), "Downloading...");
    samples.set_step_bytes(2_097_152, Some(4_194_304), "Downloading...");

    let mut state = ProgressState::default();
    samples.apply_to(&mut state);

    let display = state.display.unwrap();
    assert_eq!(display.fraction, Some(0.5));
    assert_eq!(display.text, "Downloading... - 2.0 / 4.0 MB");
}

#[test]
fn determinate_progress_refreshes_after_pulse_mode() {
    let previous = ProgressDisplay {
        fraction: Some(0.25),
        pulse: true,
        text: "Downloading mods - 1.0 MB".to_string(),
    };

    assert!(fraction_needs_update(Some(&previous), 0.25));
}

#[test]
fn mod_download_start_text_is_uniform() {
    assert_eq!(
        mod_download_start_text("Render Fix"),
        "Starting Render Fix..."
    );
}

#[test]
fn mod_download_finished_text_is_uniform() {
    assert_eq!(
        mod_download_finished_text("Render Fix", 1, 4),
        "Installed Render Fix (1/4)"
    );
}

#[test]
fn step_download_text_uses_dash_separator() {
    assert_eq!(
        format_step_download_text("Downloading...", 1_048_576, Some(2_097_152)),
        "Downloading... - 1.0 / 2.0 MB"
    );
}

#[test]
fn progress_state_handles_every_message_kind() {
    let mut state = ProgressState::default();

    state.apply(ProgressMsg::Refresh);
    assert!(state.display.is_none());

    state.apply(ProgressMsg::Bytes {
        downloaded: 1_048_576,
        total: Some(0),
        status: String::new(),
    });
    assert_eq!(
        state.display,
        Some(ProgressDisplay {
            fraction: None,
            pulse: false,
            text: "1.0 / 0.0 MB".to_string(),
        })
    );

    state.apply(ProgressMsg::Bytes {
        downloaded: 1_048_576,
        total: None,
        status: "Downloading".to_string(),
    });
    assert!(state.display.as_ref().unwrap().pulse);

    state.apply(ProgressMsg::ModInstall {
        mod_name: "Alpha".to_string(),
        total: 2,
    });
    assert_eq!(state.display.as_ref().unwrap().text, "Starting Alpha...");

    state
        .active_downloads
        .insert("Alpha".to_string(), (1, None));
    state.apply(ProgressMsg::ModFinished {
        mod_name: "Alpha".to_string(),
        completed: 1,
        total: 2,
    });
    assert!(state.active_downloads.is_empty());

    state
        .active_downloads
        .insert("Beta".to_string(), (1, Some(2)));
    state.apply(ProgressMsg::Configuring { total: 2 });
    assert!(state.active_downloads.is_empty());
    assert_eq!(state.display.as_ref().unwrap().fraction, Some(1.0));
}

#[test]
fn progress_samples_publish_latest_values_and_can_be_cleared() {
    let samples = ProgressSamples::default();
    let (sender, receiver) = async_channel::bounded(8);

    publish_step_bytes(&sender, &samples, 1_048_576, Some(2_097_152), "Step");
    publish_mod_bytes(&sender, &samples, "Alpha", 2, 1_048_576, Some(2_097_152));
    publish_mod_bytes(&sender, &samples, "Beta", 2, 1_048_576, None);

    assert!(receiver.try_recv().is_ok());
    let mut state = ProgressState::default();
    samples.apply_to(&mut state);
    assert_eq!(state.active_downloads.len(), 2);
    assert!(state.display.as_ref().unwrap().pulse);

    samples.clear_mod("Alpha");
    samples.clear();
    let mut cleared = ProgressState::default();
    samples.apply_to(&mut cleared);
    assert!(cleared.display.is_none());
}

#[test]
fn progress_format_helpers_cover_empty_and_zero_cases() {
    assert_eq!(format_download_bytes_text(1_048_576, None), "1.0 MB");
    assert_eq!(
        format_step_download_text("", 1_048_576, Some(2_097_152)),
        "1.0 / 2.0 MB"
    );
    assert_eq!(completed_mod_fraction(0, 0), 0.0);
    assert_eq!(mod_download_fraction(1, 0, 1, Some(100)), 0.0);
    assert_eq!(mod_download_fraction(1, 2, 1, Some(0)), 0.5);
    assert_eq!(
        subtitle_language_index(GameKind::SADX, SubtitleLanguage::Italian),
        0
    );
    assert_eq!(
        subtitle_language_index(GameKind::SA2, SubtitleLanguage::Italian),
        4
    );
    assert_eq!(voice_language_index(VoiceLanguage::Japanese), 0);
    assert_eq!(voice_language_index(VoiceLanguage::English), 1);
    assert!(fraction_needs_update(None, 0.5));
    assert!(!fraction_needs_update(
        Some(&ProgressDisplay {
            fraction: Some(0.5),
            pulse: false,
            text: String::new(),
        }),
        0.5005
    ));
    assert!(fraction_needs_update(
        Some(&ProgressDisplay {
            fraction: None,
            pulse: false,
            text: String::new(),
        }),
        0.5
    ));
}

#[gtk::test]
fn progress_render_updates_pulse_and_determinate_bars() {
    init_resource_overlay();

    let progress_bar = gtk::ProgressBar::new();
    let mut previous = None;
    let mut state = ProgressState {
        display: Some(ProgressDisplay {
            fraction: None,
            pulse: true,
            text: "Working".to_string(),
        }),
        ..ProgressState::default()
    };
    state.render(&progress_bar, &mut previous);
    assert!(previous.is_some());

    state.display = Some(ProgressDisplay {
        fraction: Some(0.75),
        pulse: false,
        text: "Done".to_string(),
    });
    state.render(&progress_bar, &mut previous);
    assert_eq!(progress_bar.fraction(), 0.75);

    state.display = None;
    state.render(&progress_bar, &mut previous);
}

#[test]
fn install_progress_helper_publishes_each_event_and_honors_cancel() {
    let (sender, receiver) = async_channel::bounded(8);
    let samples = ProgressSamples::default();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    apply_install_progress(
        pipeline::InstallProgress::Started { mod_name: "Alpha" },
        &cancel,
        &sender,
        &samples,
        2,
    )
    .unwrap();
    apply_install_progress(
        pipeline::InstallProgress::DownloadingMod {
            mod_name: "Alpha",
            downloaded: 10,
            total_bytes: Some(20),
        },
        &cancel,
        &sender,
        &samples,
        2,
    )
    .unwrap();
    apply_install_progress(
        pipeline::InstallProgress::Finished {
            mod_name: "Alpha",
            completed: 1,
            total: 2,
        },
        &cancel,
        &sender,
        &samples,
        2,
    )
    .unwrap();
    apply_install_progress(
        pipeline::InstallProgress::GeneratingConfig,
        &cancel,
        &sender,
        &samples,
        2,
    )
    .unwrap();

    let mut state = ProgressState::default();
    drain_progress_updates(&receiver, &mut state);
    assert_eq!(state.completed_mods, 1);
    assert_eq!(state.display.unwrap().fraction, Some(1.0));

    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    for progress in [
        pipeline::InstallProgress::Started { mod_name: "Beta" },
        pipeline::InstallProgress::DownloadingMod {
            mod_name: "Beta",
            downloaded: 1,
            total_bytes: None,
        },
        pipeline::InstallProgress::Finished {
            mod_name: "Beta",
            completed: 2,
            total: 2,
        },
    ] {
        assert!(apply_install_progress(progress, &cancel, &sender, &samples, 2).is_err());
    }

    // Config generation is deliberately allowed through after cancellation.
    apply_install_progress(
        pipeline::InstallProgress::GeneratingConfig,
        &cancel,
        &sender,
        &samples,
        2,
    )
    .unwrap();
}

#[gtk::test]
fn progress_receiver_drains_messages_and_samples() {
    init_resource_overlay();

    let (sender, receiver) = async_channel::bounded(4);
    let samples = std::sync::Arc::new(ProgressSamples::default());
    let progress_bar = gtk::ProgressBar::new();
    spawn_progress_receiver(progress_bar.clone(), receiver, samples.clone());
    sender
        .send_blocking(ProgressMsg::Bytes {
            downloaded: 1,
            total: Some(2),
            status: "First".to_string(),
        })
        .unwrap();
    samples.set_step_bytes(1_048_576, Some(2_097_152), "Latest");
    sender.send_blocking(ProgressMsg::Refresh).unwrap();
    drop(sender);

    for _ in 0..100 {
        while glib::MainContext::default().iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert!(progress_bar.text().is_some());
}

#[test]
fn download_size_text_summarizes_the_selection() {
    let estimate = |size, installed| ModDownloadEstimate { size, installed };
    let estimates = [
        estimate(Some(150_000_000), false),
        estimate(Some(1_000_000_000), false),
        estimate(None, false),
        estimate(None, true),
        estimate(None, true),
    ];

    assert_eq!(
        download_size_text(Some(&estimates), &[]),
        "No mods selected"
    );
    assert_eq!(download_size_text(None, &[0]), "Checking download size…");
    assert_eq!(
        download_size_text(Some(&estimates), &[0]),
        "150 MB to download"
    );
    assert_eq!(
        download_size_text(Some(&estimates), &[1, 3]),
        "1.0 GB to download, 1 already installed"
    );
    assert_eq!(
        download_size_text(Some(&estimates), &[0, 2, 3, 4]),
        "At least 150 MB to download, 2 already installed"
    );
    assert_eq!(
        download_size_text(Some(&estimates), &[2]),
        "Download size unknown"
    );
    assert_eq!(
        download_size_text(Some(&estimates), &[3, 4]),
        "Everything selected is already installed"
    );
    assert_eq!(format_download_size(1), "1 MB");
}

#[test]
fn estimate_mod_downloads_skips_installed_mods() {
    let tmp = tempfile::tempdir().unwrap();
    let mods = common::recommended_mods_for_game(GameKind::SA2);
    let installed = mods[1].dir_name.unwrap();
    std::fs::create_dir_all(tmp.path().join("mods").join(installed)).unwrap();
    std::fs::write(
        tmp.path().join("mods").join(installed).join("mod.ini"),
        "[mod]",
    )
    .unwrap();

    let estimates = estimate_mod_downloads(tmp.path(), mods, |_| Some(42));

    assert_eq!(estimates.len(), mods.len());
    assert_eq!(
        estimates[1],
        ModDownloadEstimate {
            size: None,
            installed: true
        }
    );
    assert_eq!(
        estimates[0],
        ModDownloadEstimate {
            size: Some(42),
            installed: false
        }
    );
}

#[gtk::test]
fn mod_preview_decodes_screenshots_off_the_ui_thread() {
    init_resource_overlay();

    let carousel = adw::Carousel::new();
    let preview = ModPreview::new(
        GameKind::SADX,
        &gtk::Label::new(None),
        &carousel,
        &gtk::Frame::new(None),
        &gtk::Label::new(None),
        &gtk::FlowBox::new(),
    );
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    let (index, mod_entry) = mods
        .iter()
        .enumerate()
        .find(|(_, entry)| !entry.pictures.is_empty())
        .expect("a SADX mod with screenshots");

    preview.show_entry(Some(index), Some(mod_entry));
    let pages = preview.state.borrow().pages[&index].clone();
    assert_eq!(pages.len(), mod_entry.pictures.len());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while pages.iter().any(|page| page.picture.paintable().is_none()) {
        assert!(
            std::time::Instant::now() < deadline,
            "screenshots did not load"
        );
        glib::MainContext::default().iteration(false);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    // Showing another mod and coming back reuses the decoded textures.
    preview.show_entry(None, None);
    preview.show_entry(Some(index), Some(mod_entry));
    let cached = preview.state.borrow().pages[&index].clone();
    assert!(cached[0].picture.paintable().is_some());
    assert_eq!(cached[0].picture, pages[0].picture);
}

#[gtk::test]
fn mod_preview_cache_is_bounded() {
    init_resource_overlay();

    let preview = ModPreview::new(
        GameKind::SADX,
        &gtk::Label::new(None),
        &adw::Carousel::new(),
        &gtk::Frame::new(None),
        &gtk::Label::new(None),
        &gtk::FlowBox::new(),
    );
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    assert!(mods.len() > MOD_PREVIEW_CACHE_LIMIT + 1);

    for (index, mod_entry) in mods.iter().enumerate().take(MOD_PREVIEW_CACHE_LIMIT + 1) {
        preview.show_entry(Some(index), Some(mod_entry));
    }
    // Revisit the oldest so it becomes the most recently used entry.
    preview.show_entry(Some(1), mods.get(1));
    preview.show_entry(Some(0), mods.first());

    let state = preview.state.borrow();
    assert_eq!(state.pages.len(), MOD_PREVIEW_CACHE_LIMIT);
    assert_eq!(state.recent.len(), MOD_PREVIEW_CACHE_LIMIT);
    assert!(state.pages.contains_key(&0));
    assert!(state.pages.contains_key(&1));
    assert!(!state.pages.contains_key(&2));
    assert_eq!(state.recent.back(), Some(&0));
}

#[gtk::test]
fn load_preview_texture_reports_missing_and_invalid_resources() {
    init_resource_overlay();

    assert!(load_preview_texture("/io/github/astrovm/AdventureMods/missing.jpg").is_err());
    assert!(
        load_preview_texture("/io/github/astrovm/AdventureMods/resources/ui/window.ui").is_err()
    );
}

#[gtk::test]
fn mod_preview_handles_empty_cached_and_hover_states() {
    init_resource_overlay();

    let title = gtk::Label::new(None);
    let carousel = adw::Carousel::new();
    let carousel_frame = gtk::Frame::new(None);
    let description = gtk::Label::new(None);
    let links = gtk::FlowBox::new();
    let preview = ModPreview::new(
        GameKind::SADX,
        &title,
        &carousel,
        &carousel_frame,
        &description,
        &links,
    );
    let mods = common::recommended_mods_for_game(GameKind::SADX);

    preview.show_entry(Some(0), mods.first());
    assert!(!carousel_frame.is_visible() || carousel.first_child().is_some());
    assert_eq!(title.label().as_str(), mods[0].name);

    preview.show_entry(Some(1), mods.get(1));
    preview.show_entry(Some(0), mods.first());
    preview.show_entry(None, None);
    assert_eq!(title.label().as_str(), "");
    assert!(!carousel_frame.is_visible());

    preview.queue_hover(0);
    preview.cancel_hover();
    assert!(preview.hover_source.borrow().is_none());
    preview.queue_hover(0);
    std::thread::sleep(std::time::Duration::from_millis(70));
    while glib::MainContext::default().iteration(false) {}
    assert_eq!(preview.state.borrow().current_index, Some(0));
}

/// Every widget of type `T` under `root`, depth first.
fn descendants<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Vec<T> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Ok(matching) = widget.clone().downcast::<T>() {
            found.push(matching);
        }
        found.extend(descendants::<T>(&widget));
        child = widget.next_sibling();
    }
    found
}

/// The two-column box of the mod selection step.
fn mod_selection_main_box(page: &AdventureModsSetupPage) -> gtk::Box {
    page.imp()
        .content_box
        .borrow()
        .first_child()
        .and_downcast::<adw::BreakpointBin>()
        .and_then(|bin| bin.child())
        .and_downcast::<gtk::Box>()
        .unwrap()
}

#[gtk::test]
fn setup_page_renders_info_language_and_download_controls() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let all_steps = page.imp().all_steps.borrow().clone();

    let steam_step = all_steps
        .iter()
        .find(|step| step.id == StepId::SteamConfig)
        .unwrap()
        .clone();
    let steam_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.render_step(&steam_step, false, &steam_content);
    assert_eq!(
        page.imp().next_button.label().as_deref(),
        Some("Check Again")
    );

    let language_step = all_steps
        .iter()
        .find(|step| step.id == StepId::LanguageOptions)
        .unwrap()
        .clone();
    let language_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.render_step(&language_step, false, &language_content);

    let combos = descendants::<adw::ComboRow>(language_content.upcast_ref());
    let [subtitle_row, voice_row] = combos.as_slice() else {
        panic!("expected subtitle and voice rows, got {}", combos.len());
    };
    subtitle_row.set_selected(1);
    voice_row.set_selected(0);
    voice_row.set_selected(1);
    assert_eq!(
        page.imp().language_selection.borrow().unwrap().voice,
        VoiceLanguage::English
    );
    assert_eq!(page.imp().next_button.label().as_deref(), Some("Install"));

    page.imp().game.replace(None);
    page.imp().language_selection.replace(None);
    let fallback_language_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.render_step(&language_step, false, &fallback_language_content);
    assert_eq!(
        descendants::<adw::ComboRow>(fallback_language_content.upcast_ref()).len(),
        2
    );

    let complete_step = all_steps
        .iter()
        .find(|step| step.id == StepId::Complete)
        .unwrap()
        .clone();
    let complete_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.render_step(&complete_step, true, &complete_content);
    assert_eq!(page.imp().next_button.label().as_deref(), Some("Done"));
    assert!(page.imp().secondary_button.is_visible());
    assert_eq!(
        page.imp().secondary_button.label().as_deref(),
        Some("Play in Steam")
    );
    // Without a game there is nothing to launch.
    page.imp().secondary_button.emit_clicked();

    page.imp().game.replace(Some(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    }));
    // Never launch the real URI: it would start the user's game.
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    page.imp().open_uri.replace(std::rc::Rc::new({
        let opened = opened.clone();
        move |_: Option<&gtk::Window>, uri: &str| {
            opened.replace(Some(uri.to_owned()));
        }
    }));
    page.imp().secondary_button.emit_clicked();
    assert_eq!(opened.borrow().as_deref(), Some("steam://rungameid/213610"));
    assert!(descendants::<gtk::Picture>(complete_content.upcast_ref()).is_empty());
    let complete_with_game = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.render_step(&complete_step, true, &complete_with_game);
    assert_eq!(
        descendants::<gtk::Picture>(complete_with_game.upcast_ref()).len(),
        1
    );

    let download_step = all_steps
        .iter()
        .find(|step| step.id == StepId::DownloadMods)
        .unwrap()
        .clone();
    let download_index = all_steps
        .iter()
        .position(|step| step.id == StepId::DownloadMods)
        .unwrap();
    page.imp().current_step.set(download_index);
    let download_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.render_step(&download_step, false, &download_content);
    let view = page.imp().install_view.borrow().clone().unwrap();
    assert_eq!(view.task_state(download_index).as_deref(), Some("running"));
    assert!(!page.imp().next_button.is_visible());
    assert_eq!(
        page.imp().secondary_button.label().as_deref(),
        Some("Cancel")
    );
    page.imp().secondary_button.emit_clicked();
    assert_eq!(
        page.imp().secondary_button.label().as_deref(),
        Some("Cancelling…")
    );
    assert!(!page.imp().secondary_button.is_sensitive());
    // The poll keeps waiting while the task is still running.
    page.imp().task_running.set(true);
    std::thread::sleep(std::time::Duration::from_millis(60));
    while glib::MainContext::default().iteration(false) {}
    page.imp().task_running.set(false);
    std::thread::sleep(std::time::Duration::from_millis(60));
    while glib::MainContext::default().iteration(false) {}
    // Cancelling returns to the last screen that asked something.
    assert_eq!(
        page.current_step().map(|step| step.id),
        Some(StepId::LanguageOptions)
    );
}

#[gtk::test]
fn no_game_auto_steps_complete_without_running_external_work() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    page.imp().game.replace(None);
    page.imp().all_steps.replace(vec![
        steps::SetupStep {
            id: StepId::Dotnet,
            title: "Runtime",
            description: "Runtime",
            kind: steps::StepKind::Auto,
        },
        steps::SetupStep {
            id: StepId::Complete,
            title: "Complete",
            description: "Complete",
            kind: steps::StepKind::Info,
        },
    ]);
    page.imp().current_step.set(0);

    page.run_auto_step(StepId::Dotnet);
    while glib::MainContext::default().iteration(false) {}
    assert_eq!(page.imp().current_step.get(), 1);

    page.run_auto_step(StepId::ConvertSteam);
    while glib::MainContext::default().iteration(false) {}
    assert_eq!(page.imp().current_step.get(), 1);
}

#[gtk::test]
fn install_screen_walks_every_task_and_reports_failures() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let window = gtk::Window::builder()
        .default_width(800)
        .default_height(600)
        .child(&page)
        .build();
    window.present();
    let index_of = |id| {
        page.imp()
            .all_steps
            .borrow()
            .iter()
            .position(|step: &steps::SetupStep| step.id == id)
            .unwrap()
    };
    let (languages, dotnet, manager, mods) = (
        index_of(StepId::LanguageOptions),
        index_of(StepId::Dotnet),
        index_of(StepId::InstallModManager),
        index_of(StepId::DownloadMods),
    );

    // Screens count the Proton check only when it was needed, and all work
    // steps as one install screen.
    page.imp().current_step.set(languages);
    page.show_current_step();
    assert_eq!(page.imp().window_title.title(), "Languages");
    assert_eq!(
        page.imp().window_title.subtitle(),
        "Sonic Adventure 2 · Step 3 of 4"
    );
    page.imp().steam_check_needed.set(false);
    page.show_current_step();
    assert_eq!(
        page.imp().window_title.subtitle(),
        "Sonic Adventure 2 · Step 2 of 3"
    );

    // Without a game the work steps finish without touching anything real.
    page.imp().game.replace(None);
    page.imp().current_step.set(dotnet);
    page.show_current_step();
    let view = page.imp().install_view.borrow().clone().unwrap();
    assert_eq!(page.imp().window_title.title(), "Installing");
    assert_eq!(view.task_state(dotnet).as_deref(), Some("running"));
    assert_eq!(view.task_state(mods).as_deref(), Some("pending"));
    assert!(!page.imp().next_button.is_visible());

    // The runtime finishes and the same checklist moves on to the next task.
    while glib::MainContext::default().iteration(false) {}
    assert_eq!(page.imp().current_step.get(), manager);
    let same_view = page.imp().install_view.borrow().clone().unwrap();
    assert_eq!(same_view.root, view.root);
    assert_eq!(view.task_state(dotnet).as_deref(), Some("done"));
    assert_eq!(view.task_state(manager).as_deref(), Some("running"));

    // A failure marks the task and offers to try again.
    page.show_error("network down");
    assert_eq!(view.task_state(manager).as_deref(), Some("failed"));
    assert!(view.error_box.is_visible());
    assert_eq!(view.error_label.label(), "network down");
    assert!(page.imp().next_button.is_visible());
    assert_eq!(page.imp().next_button.label().as_deref(), Some("Try Again"));

    // Back leaves the install screen for the last question.
    page.imp().back_button.emit_clicked();
    assert_eq!(page.imp().current_step.get(), languages);
    assert!(page.imp().install_view.borrow().is_none());

    // Errors outside the install screen replace the step with a status page.
    page.show_error("oops");
    let status = descendants::<adw::StatusPage>(page.imp().content_box.borrow().upcast_ref());
    assert_eq!(status[0].description().as_deref(), Some("oops"));

    // With nothing but work left, going back leaves setup.
    page.imp().all_steps.replace(vec![steps::SetupStep {
        id: StepId::Dotnet,
        title: "Runtime",
        description: "Runtime",
        kind: steps::StepKind::Auto,
    }]);
    page.imp().current_step.set(0);
    page.go_back_to_choices();
    window.close();
}

#[gtk::test]
fn install_screen_downloads_mods_in_the_background() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let all_steps = page.imp().all_steps.borrow().clone();
    let mods = all_steps
        .iter()
        .position(|step| step.id == StepId::DownloadMods)
        .unwrap();
    let view = super::InstallView::new(&all_steps);
    let mods_row_subtitle = || {
        view.tasks
            .iter()
            .find(|task| task.step == mods)
            .and_then(|task| task.row.subtitle())
            .unwrap()
            .to_string()
    };

    // Nothing selected, nothing to download.
    page.imp().selected_mods.replace(Vec::new());
    page.start_prefetch(&view);
    assert!(page.imp().prefetch.borrow().is_none());

    page.imp().selected_mods.replace(vec![0, 1]);
    page.start_prefetch(&view);
    assert!(page.imp().prefetch.borrow().is_some());
    assert_eq!(mods_row_subtitle(), "Downloading in the background");
    // The note stays while the task waits and goes once it runs.
    view.show_step(&all_steps, 0);
    assert_eq!(mods_row_subtitle(), "Downloading in the background");
    view.show_step(&all_steps, mods);
    assert_ne!(mods_row_subtitle(), "Downloading in the background");

    // A running prefetch is kept; a cancelled one is replaced.
    let first = page.prefetch_done().unwrap();
    page.start_prefetch(&view);
    assert!(page.prefetch_done().unwrap().same_channel(&first));
    page.cancel_prefetch();
    page.start_prefetch(&view);
    let second = page.prefetch_done().unwrap();
    assert!(!second.same_channel(&first));
    glib::MainContext::default().block_on(async {
        assert!(first.recv().await.is_err());
        assert!(second.recv().await.is_err());
    });

    let run_mods_step = |cancelled: bool| {
        page.run_download_step(
            StepId::DownloadMods,
            gtk::ProgressBar::new(),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(cancelled)),
        );
        for _ in 0..200 {
            while glib::MainContext::default().iteration(false) {}
            if !page.imp().task_running.get() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!page.imp().task_running.get());
    };
    page.imp().selected_mods.replace(Vec::new());
    page.imp().current_step.set(mods);
    let manager_config = tmp.path().join("SAManager/Manager.json");

    // Cancelling while waiting for the downloads installs nothing.
    run_mods_step(true);
    assert!(!manager_config.exists());

    // Otherwise the mods step waits for the prefetch, then installs.
    run_mods_step(false);
    assert!(manager_config.exists());

    // Leaving setup stops the downloads.
    page.go_back_to_welcome();
    assert!(
        page.imp()
            .prefetch
            .borrow()
            .as_ref()
            .unwrap()
            .cancel
            .load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[gtk::test]
fn steam_step_says_when_proton_is_ready() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    page.imp()
        .steam_config_status
        .replace(Some(common::SteamConfigStatus {
            message: "Proton 10.0 is set up.".to_owned(),
            ready: true,
        }));
    page.imp().current_step.set(0);
    page.show_current_step();

    let status = descendants::<adw::StatusPage>(page.imp().content_box.borrow().upcast_ref());
    assert_eq!(status[0].title(), "Proton Is Ready");
    assert_eq!(page.imp().next_button.label().as_deref(), Some("Continue"));
}

#[gtk::test]
fn setup_page_navigation_handles_errors_and_backtracking() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let info = |id| steps::SetupStep {
        id,
        title: "Info",
        description: "Info",
        kind: steps::StepKind::Info,
    };
    let auto = steps::SetupStep {
        id: StepId::Dotnet,
        title: "Auto",
        description: "Auto",
        kind: steps::StepKind::Auto,
    };

    page.imp().all_steps.replace(vec![info(StepId::Complete)]);
    page.imp().step_busy.set(true);
    page.on_next_clicked();
    page.imp().step_busy.set(false);
    page.imp().is_error.set(true);
    page.on_next_clicked();
    page.imp().current_step.set(usize::MAX);
    page.imp().is_error.set(false);
    page.on_next_clicked();
    assert!(page.imp().is_error.get());

    page.imp()
        .all_steps
        .replace(vec![info(StepId::SteamConfig), info(StepId::Complete)]);
    page.imp().current_step.set(0);
    page.imp().is_error.set(false);
    page.on_next_clicked();
    assert_eq!(
        page.imp().next_button.label().as_deref(),
        Some("Check Again")
    );

    page.imp()
        .all_steps
        .replace(vec![info(StepId::LanguageOptions), info(StepId::Complete)]);
    page.imp().current_step.set(0);
    page.imp().game.replace(None);
    page.on_next_clicked();
    assert_eq!(page.imp().current_step.get(), 1);

    page.imp()
        .all_steps
        .replace(vec![auto.clone(), info(StepId::Complete)]);
    page.imp().current_step.set(1);
    page.imp().game.replace(Some(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    }));
    page.on_back_clicked();

    page.imp().all_steps.replace(vec![
        info(StepId::SteamConfig),
        auto,
        info(StepId::Complete),
    ]);
    page.imp().current_step.set(2);
    page.on_back_clicked();
    assert_eq!(page.imp().current_step.get(), 0);

    page.imp()
        .all_steps
        .replace(vec![info(StepId::Complete), info(StepId::Complete)]);
    page.imp().current_step.set(3);
    page.on_back_clicked();
    assert!(page.imp().is_error.get());

    page.imp().all_steps.replace(Vec::new());
    page.imp().current_step.set(1);
    page.on_back_clicked();
    assert!(page.imp().is_error.get());

    page.imp().all_steps.replace(vec![info(StepId::Complete)]);
    page.imp().current_step.set(0);
    page.imp().cancel_flag.replace(Some(std::sync::Arc::new(
        std::sync::atomic::AtomicBool::new(false),
    )));
    page.show_current_step();
    assert!(page.imp().cancel_flag.borrow().is_none());
    assert!(page.get_resolution().0 > 0);
    page.imp().game.replace(None);
    page.imp().language_selection.replace(None);
    assert_eq!(
        page.current_language_selection().voice,
        VoiceLanguage::Japanese
    );
    page.persist_language_selection();
    assert_eq!(page.skip_completed_steps(4), 4);

    page.imp().game.replace(Some(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    }));
    page.imp().all_steps.replace(Vec::new());
    assert_eq!(page.skip_completed_steps(4), 4);

    page.imp().all_steps.replace(vec![
        info(StepId::Complete),
        info(StepId::Complete),
        info(StepId::Complete),
    ]);
    page.imp().current_step.set(2);
    page.imp().is_error.set(false);
    page.on_next_clicked();

    page.imp().current_step.set(usize::MAX);
    page.apply_step_chrome();
    page.render_current_step_content();
    page.show_current_step();

    page.imp().all_steps.replace(vec![
        info(StepId::Complete),
        info(StepId::Complete),
        info(StepId::Complete),
    ]);
    page.imp().current_step.set(2);
    page.imp().is_error.set(false);
    page.on_back_clicked();

    let nav = adw::NavigationView::new();
    let welcome = adw::NavigationPage::builder()
        .title("Welcome")
        .child(&gtk::Label::new(Some("Welcome")))
        .build();
    nav.push(&welcome);
    let nav_page = adw::NavigationPage::builder().child(&page).build();
    nav.push(&nav_page);
    page.go_back_to_welcome();
    assert_eq!(nav.visible_page().unwrap().title().as_str(), "Welcome");
}

#[gtk::test]
fn mapped_setup_page_uses_fade_transition() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let window = gtk::Window::builder()
        .default_width(800)
        .default_height(600)
        .child(&page)
        .build();
    window.present();
    while glib::MainContext::default().iteration(false) {}

    // Forward slides left, back slides right; old steps leave the stack.
    let stack = page.imp().content_stack.get();
    page.imp().current_step.set(2);
    page.show_current_step();
    assert_eq!(stack.transition_type(), gtk::StackTransitionType::SlideLeft);
    page.imp().current_step.set(1);
    page.show_current_step();
    assert_eq!(
        stack.transition_type(),
        gtk::StackTransitionType::SlideRight
    );
    assert!(stack.observe_children().n_items() <= 2);
    assert_eq!(
        stack.visible_child().as_ref(),
        Some(page.imp().content_box.borrow().upcast_ref())
    );
    window.close();
}

#[gtk::test]
fn set_step_busy_disables_back_button() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });

    assert!(page.imp().back_button.is_sensitive());
    page.set_step_busy(true);
    assert!(!page.imp().back_button.is_sensitive());
    page.set_step_busy(false);
    assert!(page.imp().back_button.is_sensitive());
}

#[gtk::test]
fn setup_button_panic_handlers_show_recoverable_errors() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    assert!(format!("{:?}", page).starts_with("AdventureModsSetupPage"));

    let all_steps_guard = page.imp().all_steps.borrow_mut();
    page.imp().next_button.emit_clicked();
    drop(all_steps_guard);
    assert_eq!(page.imp().next_button.label().as_deref(), Some("Try Again"));
    assert!(page.imp().is_error.get());

    page.imp().is_error.set(false);
    page.imp().current_step.set(1);
    let all_steps_guard = page.imp().all_steps.borrow_mut();
    page.imp().back_button.emit_clicked();
    drop(all_steps_guard);
    assert_eq!(page.imp().next_button.label().as_deref(), Some("Try Again"));
}

#[gtk::test]
fn setup_page_covers_selector_callbacks_and_motion_handlers() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: tmp.path().to_path_buf(),
    });
    let select_mods_index = page
        .imp()
        .all_steps
        .borrow()
        .iter()
        .position(|step| step.id == StepId::SelectMods)
        .unwrap();
    page.imp().current_step.set(select_mods_index);
    page.show_current_step();

    let main_box = mod_selection_main_box(&page);
    let presets = descendants::<adw::ComboRow>(main_box.upcast_ref());
    let preset_description = descendants::<gtk::Label>(main_box.upcast_ref())
        .into_iter()
        .find(|label| label.label() == common::presets_for_game(GameKind::SADX)[0].description)
        .unwrap();
    presets[0].set_selected(1);
    assert_eq!(
        preset_description.label(),
        common::presets_for_game(GameKind::SADX)[1].description
    );

    let list_box = descendants::<gtk::ListBox>(main_box.upcast_ref())
        .into_iter()
        .find(|list| list.selection_mode() == gtk::SelectionMode::Single)
        .unwrap();
    let row = list_box.row_at_index(0).unwrap();
    let check = descendants::<gtk::CheckButton>(row.upcast_ref())
        .into_iter()
        .next()
        .unwrap();
    check.set_active(false);
    check.set_active(true);

    let controllers = row.observe_controllers();
    for index in 0..controllers.n_items() {
        if let Some(controller) = controllers.item(index)
            && let Ok(motion) = controller.downcast::<gtk::EventControllerMotion>()
        {
            motion.emit_by_name::<()>("enter", &[&0.0f64, &0.0f64]);
            motion.emit_by_name::<()>("leave", &[]);
        }
    }
    while glib::MainContext::default().iteration(false) {}
    assert!(!page.imp().selected_mods.borrow().is_empty());
}

#[gtk::test]
fn run_download_steps_cover_conversion_manager_and_empty_mods() {
    init_resource_overlay();

    let sadx_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(sadx_dir.path().join("system")).unwrap();
    std::fs::write(sadx_dir.path().join("system/CHRMODELS_orig.dll"), b"orig").unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: sadx_dir.path().to_path_buf(),
    });
    page.imp().all_steps.replace(Vec::new());

    page.run_download_step(
        StepId::ConvertSteam,
        gtk::ProgressBar::new(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    for _ in 0..100 {
        while glib::MainContext::default().iteration(false) {}
        if !page.imp().task_running.get() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    std::fs::write(
        sadx_dir.path().join("Sonic Adventure DX.exe.bak"),
        b"backup",
    )
    .unwrap();
    std::fs::create_dir_all(sadx_dir.path().join("mods/.modloader")).unwrap();
    std::fs::write(
        sadx_dir.path().join("mods/.modloader/SADXModLoader.dll"),
        b"loader",
    )
    .unwrap();
    page.run_download_step(
        StepId::InstallModManager,
        gtk::ProgressBar::new(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    for _ in 0..100 {
        while glib::MainContext::default().iteration(false) {}
        if !page.imp().task_running.get() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    let sa2_dir = tempfile::tempdir().unwrap();
    let sa2_page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: sa2_dir.path().to_path_buf(),
    });
    sa2_page.imp().all_steps.replace(Vec::new());
    sa2_page.run_download_step(
        StepId::DownloadMods,
        gtk::ProgressBar::new(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    for _ in 0..100 {
        while glib::MainContext::default().iteration(false) {}
        if !sa2_page.imp().task_running.get() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!sa2_page.imp().task_running.get());
}

#[gtk::test]
fn selecting_mod_row_updates_preview_title() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let select_mods_index = page
        .imp()
        .all_steps
        .borrow()
        .iter()
        .position(|step| step.id == StepId::SelectMods)
        .unwrap();

    page.imp().current_step.set(select_mods_index);
    page.show_current_step();

    let main_box = mod_selection_main_box(&page);
    while glib::MainContext::default().iteration(false) {}
    let left_box = main_box
        .first_child()
        .unwrap()
        .downcast::<gtk::Box>()
        .unwrap();
    let scrolled = left_box
        .first_child()
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    let viewport = scrolled
        .child()
        .unwrap()
        .downcast::<gtk::Viewport>()
        .unwrap();
    let list_box = viewport
        .child()
        .unwrap()
        .downcast::<gtk::ListBox>()
        .unwrap();
    let preview_box = main_box
        .last_child()
        .unwrap()
        .downcast::<gtk::Box>()
        .unwrap();
    let preview_title = preview_box
        .first_child()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();

    let row = list_box.row_at_index(1).unwrap();
    list_box.select_row(Some(&row));
    while glib::MainContext::default().iteration(false) {}

    assert_eq!(
        preview_title.label().as_str(),
        "Retranslated Story -COMPLETE-"
    );
}

#[gtk::test]
fn mod_selection_columns_keep_width_when_preview_content_changes() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: tmp.path().to_path_buf(),
    });
    let select_mods_index = page
        .imp()
        .all_steps
        .borrow()
        .iter()
        .position(|step| step.id == StepId::SelectMods)
        .unwrap();

    page.imp().current_step.set(select_mods_index);
    page.show_current_step();

    let window = gtk::Window::builder()
        .default_width(1100)
        .default_height(820)
        .child(&page)
        .build();
    window.present();
    while glib::MainContext::default().iteration(false) {}

    let main_box = mod_selection_main_box(&page);
    let left_box = main_box
        .first_child()
        .unwrap()
        .downcast::<gtk::Box>()
        .unwrap();
    let scrolled = left_box
        .last_child()
        .and_then(|size_label| size_label.prev_sibling())
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    let viewport = scrolled
        .child()
        .unwrap()
        .downcast::<gtk::Viewport>()
        .unwrap();
    let list_box = viewport
        .child()
        .unwrap()
        .downcast::<gtk::ListBox>()
        .unwrap();
    let preview_box = main_box
        .last_child()
        .unwrap()
        .downcast::<gtk::Box>()
        .unwrap();

    let row = list_box.row_at_index(8).unwrap();
    list_box.select_row(Some(&row));
    while glib::MainContext::default().iteration(false) {}

    let initial_left_width = left_box.width();
    let initial_preview_width = preview_box.width();
    assert!(initial_left_width > 0);
    assert!(initial_preview_width > 0);

    for row_index in [9, 10, 11] {
        let row = list_box.row_at_index(row_index).unwrap();
        list_box.select_row(Some(&row));
        while glib::MainContext::default().iteration(false) {}

        assert_eq!(left_box.width(), initial_left_width);
        assert_eq!(preview_box.width(), initial_preview_width);
    }
}

#[gtk::test]
fn mod_selection_keeps_two_columns_in_narrow_window() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: tmp.path().to_path_buf(),
    });
    let select_mods_index = page
        .imp()
        .all_steps
        .borrow()
        .iter()
        .position(|step| step.id == StepId::SelectMods)
        .unwrap();

    page.imp().current_step.set(select_mods_index);
    page.show_current_step();

    let window = gtk::Window::builder()
        .default_width(700)
        .default_height(820)
        .child(&page)
        .build();
    window.present();
    while glib::MainContext::default().iteration(false) {}

    let main_box = mod_selection_main_box(&page);

    assert_eq!(main_box.orientation(), gtk::Orientation::Horizontal);
    assert!(main_box.is_homogeneous());
}

#[gtk::test]
fn run_download_step_clears_task_running_when_game_is_missing() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let progress_bar = gtk::ProgressBar::new();
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    page.imp().game.replace(None);
    page.run_download_step(StepId::DownloadMods, progress_bar, cancel_flag);
    while glib::MainContext::default().iteration(false) {}

    assert!(!page.imp().task_running.get());
}

#[gtk::test]
fn run_download_step_marks_task_running_before_main_loop_spins() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: tmp.path().to_path_buf(),
    });
    let progress_bar = gtk::ProgressBar::new();
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    page.run_download_step(StepId::Complete, progress_bar, cancel_flag);

    assert!(page.imp().task_running.get());
}

/// Spin the main loop until `done` holds, failing after a few seconds.
fn run_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done() {
        assert!(std::time::Instant::now() < deadline, "timed out: {what}");
        while glib::MainContext::default().iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn sa2_page(path: &std::path::Path) -> AdventureModsSetupPage {
    AdventureModsSetupPage::new(Game {
        kind: GameKind::SA2,
        path: path.to_path_buf(),
    })
}

fn step_index(page: &AdventureModsSetupPage, id: StepId) -> usize {
    page.imp()
        .all_steps
        .borrow()
        .iter()
        .position(|step| step.id == id)
        .unwrap()
}

fn info_step(id: StepId) -> steps::SetupStep {
    steps::SetupStep {
        id,
        title: "Info",
        description: "Info description",
        kind: steps::StepKind::Info,
    }
}

/// Put `page` on top of a navigation view whose first page is "Welcome".
fn inside_navigation(page: &AdventureModsSetupPage) -> adw::NavigationView {
    let nav = adw::NavigationView::new();
    nav.push(
        &adw::NavigationPage::builder()
            .title("Welcome")
            .child(&gtk::Label::new(Some("Welcome")))
            .build(),
    );
    nav.push(&page.navigation_page());
    nav
}

fn visible_title(nav: &adw::NavigationView) -> String {
    nav.visible_page().unwrap().title().to_string()
}

fn status_description(page: &AdventureModsSetupPage) -> String {
    descendants::<adw::StatusPage>(page.imp().content_box.borrow().upcast_ref())[0]
        .description()
        .unwrap()
        .to_string()
}

#[gtk::test]
fn navigation_page_is_named_after_the_game() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    assert_eq!(format!("{:?}", page.imp()), "AdventureModsSetupPage");

    let nav_page = page.navigation_page();
    assert_eq!(nav_page.tag().as_deref(), Some("setup"));
    assert_eq!(nav_page.title(), "Sonic Adventure 2");

    let other = sa2_page(tmp.path());
    other.imp().game.replace(None);
    assert_eq!(other.navigation_page().title(), "Setup");
}

#[gtk::test]
fn header_buttons_run_their_actions_and_recover_from_panics() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    assert_eq!(
        page.current_step().map(|step| step.id),
        Some(StepId::SteamConfig)
    );

    // Proton is missing, so Continue checks again instead of moving on.
    page.imp().next_button.emit_clicked();
    assert!(!page.imp().is_error.get());
    assert_eq!(page.imp().current_step.get(), 0);
    assert_eq!(
        page.imp().next_button.label().as_deref(),
        Some("Check Again")
    );

    page.set_secondary_action(Some("Explode"), Some(std::rc::Rc::new(|| panic!("boom"))));
    page.imp().secondary_button.emit_clicked();
    assert!(page.imp().is_error.get());
    assert_eq!(
        status_description(&page),
        "Something went wrong. Please try again."
    );
    assert!(!page.imp().secondary_button.is_visible());
}

#[gtk::test]
fn progress_render_skips_unchanged_values() {
    init_resource_overlay();

    let progress_bar = gtk::ProgressBar::new();
    let mut previous = None;
    let mut state = ProgressState {
        display: Some(ProgressDisplay {
            fraction: Some(0.5),
            pulse: false,
            text: "Half".to_string(),
        }),
        ..ProgressState::default()
    };
    state.render(&progress_bar, &mut previous);
    assert_eq!(progress_bar.fraction(), 0.5);

    // The same display again leaves the bar alone.
    progress_bar.set_fraction(0.2);
    progress_bar.set_text(Some("Changed elsewhere"));
    state.render(&progress_bar, &mut previous);
    assert_eq!(progress_bar.fraction(), 0.2);
    assert_eq!(progress_bar.text().as_deref(), Some("Changed elsewhere"));

    // A known size of zero bytes keeps the fraction but shows the new text.
    state.display = Some(ProgressDisplay {
        fraction: None,
        pulse: false,
        text: "Empty".to_string(),
    });
    state.render(&progress_bar, &mut previous);
    assert_eq!(progress_bar.fraction(), 0.2);
    assert_eq!(progress_bar.text().as_deref(), Some("Empty"));
}

#[gtk::test]
fn progress_receiver_throttles_back_to_back_updates() {
    init_resource_overlay();

    let (sender, receiver) = async_channel::bounded(4);
    let progress_bar = gtk::ProgressBar::new();
    spawn_progress_receiver(
        progress_bar.clone(),
        receiver,
        std::sync::Arc::new(ProgressSamples::default()),
    );
    let bytes = |status: &str| ProgressMsg::Bytes {
        downloaded: 1_048_576,
        total: Some(2_097_152),
        status: status.to_string(),
    };

    let started = std::time::Instant::now();
    sender.send_blocking(bytes("First")).unwrap();
    run_until("first update", || {
        progress_bar.text().as_deref() == Some("First - 1.0 / 2.0 MB")
    });
    sender.send_blocking(bytes("Second")).unwrap();
    run_until("second update", || {
        progress_bar.text().as_deref() == Some("Second - 1.0 / 2.0 MB")
    });

    // The second render waited for the frame interval after the first.
    assert!(started.elapsed() >= super::PROGRESS_UPDATE_INTERVAL);

    // An update after a quiet spell renders right away.
    std::thread::sleep(super::PROGRESS_UPDATE_INTERVAL * 2);
    let quiet = std::time::Instant::now();
    sender.send_blocking(bytes("Third")).unwrap();
    run_until("third update", || {
        progress_bar.text().as_deref() == Some("Third - 1.0 / 2.0 MB")
    });
    assert!(quiet.elapsed() < std::time::Duration::from_secs(5));
}

#[gtk::test]
fn mod_preview_logs_screenshots_that_fail_to_load() {
    init_resource_overlay();
    let logs = LogCapture::start();

    let preview = ModPreview::new(
        GameKind::SADX,
        &gtk::Label::new(None),
        &adw::Carousel::new(),
        &gtk::Frame::new(None),
        &gtk::Label::new(None),
        &gtk::FlowBox::new(),
    );
    let picture = gtk::Picture::new();
    let page = ModPreviewPage {
        widget: picture.clone().upcast(),
        picture: picture.clone(),
        resource: "/io/github/astrovm/AdventureMods/missing.jpg",
    };
    preview.load_textures(&[page]);

    run_until("preview load failure", || {
        logs.contents().contains("Failed to load mod preview")
    });
    assert!(
        logs.contents()
            .contains("/io/github/astrovm/AdventureMods/missing.jpg")
    );
    assert!(picture.paintable().is_none());
}

#[test]
fn remote_download_size_is_unknown_when_the_lookup_fails() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();
    let logs = LogCapture::start();
    let (base, requests) = serve(|_| Reply::ok("down").status("500 Internal Server Error"));
    let mod_entry = common::recommended_mods_for_game(GameKind::SA2)
        .iter()
        .find(|entry| matches!(entry.source, common::ModSource::GameBananaItem { .. }))
        .unwrap();

    unsafe { std::env::set_var("ADVENTURE_MODS_GAMEBANANA_API_BASE", format!("{base}/api?")) };
    let size = remote_mod_download_size(mod_entry);
    unsafe { std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE") };

    assert_eq!(size, None);
    assert!(!requests.lock().unwrap().is_empty());
    assert!(
        logs.contents()
            .contains(&format!("No download size for {}", mod_entry.name))
    );
}

/// Show the mod list of a fresh SA2 page whose size lookups use `size_of`.
fn mod_list_page(
    path: &std::path::Path,
    size_of: Option<super::DownloadSizeFn>,
) -> AdventureModsSetupPage {
    let page = sa2_page(path);
    page.imp().download_size_of.set(size_of);
    page.imp()
        .current_step
        .set(step_index(&page, StepId::SelectMods));
    page.show_current_step();
    page
}

fn download_size_label(page: &AdventureModsSetupPage) -> String {
    page.imp()
        .download_size_label
        .borrow()
        .as_ref()
        .unwrap()
        .label()
        .to_string()
}

#[gtk::test]
fn mod_list_shows_the_download_size_once_estimated() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = mod_list_page(tmp.path(), Some(|_| Some(5_000_000)));
    assert_eq!(download_size_label(&page), "Checking download size…");
    run_until("download estimates", || {
        page.imp().download_estimates.borrow().is_some()
    });
    let selected = page.imp().selected_mods.borrow().len() as u64;
    assert!(selected > 0);
    assert_eq!(
        download_size_label(&page),
        format!("{} MB to download", 5 * selected)
    );

    // Sizes are only looked up once per page.
    page.imp().download_estimates.replace(None);
    page.request_download_estimates();
    while glib::MainContext::default().iteration(false) {}
    assert!(page.imp().download_estimates.borrow().is_none());

    // Without a game there is nothing to look up.
    let no_game = sa2_page(tmp.path());
    no_game.imp().game.replace(None);
    no_game.request_download_estimates();
    while glib::MainContext::default().iteration(false) {}
    assert!(no_game.imp().download_estimates_requested.get());
    assert!(no_game.imp().download_estimates.borrow().is_none());
    // Before the mod list exists there is no label to refresh.
    no_game.update_download_size_label();
    assert!(no_game.imp().download_size_label.borrow().is_none());
}

#[gtk::test]
fn mod_list_survives_a_failed_download_estimate() {
    init_resource_overlay();
    let logs = LogCapture::start();

    let tmp = tempfile::tempdir().unwrap();
    let page = mod_list_page(tmp.path(), Some(|_| panic!("size lookup exploded")));
    run_until("download estimates", || {
        page.imp().download_estimates.borrow().is_some()
    });

    assert!(
        page.imp()
            .download_estimates
            .borrow()
            .as_ref()
            .unwrap()
            .is_empty()
    );
    assert!(logs.contents().contains("Failed to estimate mod downloads"));
    assert_ne!(download_size_label(&page), "Checking download size…");
}

#[gtk::test]
fn mod_list_without_a_game_is_empty() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    page.imp().game.replace(None);
    page.imp()
        .current_step
        .set(step_index(&page, StepId::SelectMods));
    page.show_current_step();

    let main_box = mod_selection_main_box(&page);
    assert!(descendants::<gtk::CheckButton>(main_box.upcast_ref()).is_empty());
    assert!(page.imp().selected_mods.borrow().is_empty());
    assert_eq!(download_size_label(&page), "No mods selected");
}

/// Records the prefetched mods next to the game instead of downloading them.
fn record_prefetch(
    game_path: &std::path::Path,
    selected: &[&common::ModEntry],
    _cancelled: &std::sync::atomic::AtomicBool,
) {
    let names: Vec<_> = selected.iter().map(|entry| entry.name).collect();
    std::fs::write(game_path.join("prefetched"), names.join("\n")).unwrap();
}

#[gtk::test]
fn prefetch_downloads_the_selected_mods() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    let view = InstallView::new(&page.imp().all_steps.borrow());
    page.imp().selected_mods.replace(vec![0, 2]);

    // Without a game there is nothing to download.
    let game = page.imp().game.replace(None);
    page.start_prefetch(&view);
    assert!(page.imp().prefetch.borrow().is_none());
    page.imp().game.replace(game);

    page.imp().prefetch_archives.set(Some(record_prefetch));
    page.start_prefetch(&view);
    let done = page.prefetch_done().unwrap();
    glib::MainContext::default().block_on(async {
        assert!(done.recv().await.is_err());
    });

    let mods = common::recommended_mods_for_game(GameKind::SA2);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("prefetched")).unwrap(),
        format!("{}\n{}", mods[0].name, mods[2].name)
    );

    // A flow without a mods step has no row to annotate.
    let other = tempfile::tempdir().unwrap();
    let page = sa2_page(other.path());
    page.imp().prefetch_archives.set(Some(record_prefetch));
    page.imp().selected_mods.replace(vec![1]);
    page.imp()
        .all_steps
        .replace(vec![info_step(StepId::Complete)]);
    let view = InstallView::new(&page.imp().all_steps.borrow());
    page.start_prefetch(&view);
    let done = page.prefetch_done().unwrap();
    glib::MainContext::default().block_on(async {
        assert!(done.recv().await.is_err());
    });
    assert!(view.tasks.is_empty());
    assert_eq!(
        std::fs::read_to_string(other.path().join("prefetched")).unwrap(),
        mods[1].name
    );
}

#[gtk::test]
fn install_view_ignores_steps_without_a_task() {
    init_resource_overlay();

    let all_steps = steps::steps_for_game(GameKind::SA2);
    let view = InstallView::new(&all_steps);
    // The first step asks about Proton; it is not an install task.
    assert!(!all_steps[0].kind.is_work());
    let subtitles = || {
        view.tasks
            .iter()
            .map(|task| task.row.subtitle().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    let before = subtitles();

    view.set_pending_note(0, "Waiting");
    view.show_error(0, "Lost the connection");

    assert_eq!(subtitles(), before);
    assert!(
        view.tasks
            .iter()
            .all(|task| view.task_state(task.step).as_deref() == Some("pending"))
    );
    assert!(view.error_box.is_visible());
    assert_eq!(view.error_label.label(), "Lost the connection");
}

#[gtk::test]
fn rerendering_a_step_replaces_its_content() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    let content_box = page.imp().content_box.borrow().clone();
    let first = content_box.first_child().unwrap();

    page.render_current_step_content();

    let replacement = content_box.first_child().unwrap();
    assert_ne!(replacement, first);
    assert!(replacement.next_sibling().is_none());
    assert!(first.parent().is_none());
}

#[gtk::test]
fn steam_step_without_a_game_shows_the_step_description() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    page.imp().game.replace(None);
    page.imp().steam_config_status.replace(None);
    let step = page.current_step().unwrap();
    assert_eq!(step.id, StepId::SteamConfig);

    page.render_current_step_content();

    // Status pages take markup, so the text arrives escaped.
    assert_eq!(
        status_description(&page),
        glib::markup_escape_text(step.description).as_str()
    );
    assert_eq!(
        page.imp().next_button.label().as_deref(),
        Some("Check Again")
    );
}

#[gtk::test]
fn cancelling_waits_for_the_running_task() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    let mods = step_index(&page, StepId::DownloadMods);
    page.imp().current_step.set(mods);
    page.imp().task_running.set(true);
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    page.cancel_download(&flag);
    assert!(flag.load(std::sync::atomic::Ordering::Relaxed));

    // The poll fires but keeps waiting while the task still runs.
    std::thread::sleep(std::time::Duration::from_millis(80));
    while glib::MainContext::default().iteration(false) {}
    assert_eq!(page.imp().current_step.get(), mods);
    assert!(page.imp().poll_source.borrow().is_some());

    page.imp().task_running.set(false);
    run_until("cancel poll", || page.imp().poll_source.borrow().is_none());
    assert_eq!(
        page.current_step().map(|step| step.id),
        Some(StepId::LanguageOptions)
    );
}

#[gtk::test]
fn mod_checkboxes_handle_presets_and_repeated_toggles() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: tmp.path().to_path_buf(),
    });
    page.imp()
        .current_step
        .set(step_index(&page, StepId::SelectMods));
    page.show_current_step();
    let presets = common::presets_for_game(GameKind::SADX);
    let main_box = mod_selection_main_box(&page);
    let preset_row = descendants::<adw::ComboRow>(main_box.upcast_ref())
        .into_iter()
        .next()
        .unwrap();
    let preset_description = descendants::<gtk::Label>(main_box.upcast_ref())
        .into_iter()
        .find(|label| label.label() == presets[0].description)
        .unwrap();
    let checks = descendants::<gtk::CheckButton>(main_box.upcast_ref());

    // An entry without a matching preset changes nothing.
    let mut names: Vec<_> = presets.iter().map(|preset| preset.name).collect();
    names.push("Custom");
    preset_row.set_model(Some(&gtk::StringList::new(&names)));
    let selected_before = page.imp().selected_mods.borrow().clone();
    preset_row.set_selected(presets.len() as u32);
    assert_eq!(preset_row.selected(), presets.len() as u32);
    assert_eq!(preset_description.label(), presets[0].description);
    assert_eq!(*page.imp().selected_mods.borrow(), selected_before);

    // Toggling a mod on that is already selected does not add it twice.
    checks[0].set_active(false);
    page.imp().selected_mods.borrow_mut().push(0);
    checks[0].set_active(true);
    assert_eq!(
        page.imp()
            .selected_mods
            .borrow()
            .iter()
            .filter(|&&index| index == 0)
            .count(),
        1
    );

    // Without a game a preset has no mods to pick.
    let active_before: Vec<_> = checks.iter().map(|check| check.is_active()).collect();
    page.imp().game.replace(None);
    preset_row.set_selected(1);
    assert_eq!(preset_description.label(), presets[1].description);
    assert!(page.imp().selected_mods.borrow().is_empty());
    let active_after: Vec<_> = checks.iter().map(|check| check.is_active()).collect();
    assert_eq!(active_after, active_before);
}

#[gtk::test]
fn focusing_a_mod_checkbox_previews_that_mod() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: tmp.path().to_path_buf(),
    });
    page.imp()
        .current_step
        .set(step_index(&page, StepId::SelectMods));
    page.show_current_step();
    let window = gtk::Window::builder()
        .default_width(1100)
        .default_height(820)
        .child(&page)
        .build();
    window.present();
    while glib::MainContext::default().iteration(false) {}

    let main_box = mod_selection_main_box(&page);
    let preview_title = main_box
        .last_child()
        .and_then(|preview_box| preview_box.first_child())
        .and_downcast::<gtk::Label>()
        .unwrap();
    let checks = descendants::<gtk::CheckButton>(main_box.upcast_ref());
    let mods = common::recommended_mods_for_game(GameKind::SADX);
    assert_ne!(preview_title.label(), mods[3].name);

    assert!(checks[3].grab_focus());
    run_until("focus preview", || checks[3].has_focus());
    assert_eq!(preview_title.label(), mods[3].name);
    window.close();
}

#[gtk::test]
fn runtime_install_failure_is_shown_on_the_install_screen() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let game_path = tmp.path().join("steamapps/common/Sonic Adventure 2");
    std::fs::create_dir_all(&game_path).unwrap();
    let page = sa2_page(&game_path);
    let dotnet = step_index(&page, StepId::Dotnet);
    page.imp().current_step.set(dotnet);
    page.show_current_step();

    run_until("runtime failure", || page.imp().is_error.get());

    let view = page.imp().install_view.borrow().clone().unwrap();
    assert_eq!(view.task_state(dotnet).as_deref(), Some("failed"));
    assert!(view.error_label.label().contains("Proton prefix"));
    assert_eq!(page.imp().current_step.get(), dotnet);
    assert!(!page.imp().step_busy.get());
    assert_eq!(page.imp().next_button.label().as_deref(), Some("Try Again"));
}

#[gtk::test]
fn download_step_failures_report_progress_and_the_error() {
    use crate::external::test_http::{Reply, serve};
    use std::os::unix::fs::PermissionsExt;

    init_resource_overlay();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    let tmp = tempfile::tempdir().unwrap();
    let game_path = tmp.path().join("game");
    std::fs::create_dir_all(game_path.join("system")).unwrap();
    let fake_7zz = tmp.path().join("7zz");
    std::fs::write(&fake_7zz, "#!/bin/sh\necho 'not an archive' >&2\nexit 2\n").unwrap();
    std::fs::set_permissions(&fake_7zz, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (base, requests) = serve(|_| Reply::ok(vec![b'x'; 4096]));
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_URL_SADX_STEAM_TOOLS",
            format!("{base}/steam_tools.7z"),
        );
        std::env::set_var(
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            format!("{base}/SAModManager.zip"),
        );
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
    }

    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: game_path.clone(),
    });
    for id in [StepId::ConvertSteam, StepId::InstallModManager] {
        let index = step_index(&page, id);
        page.imp().current_step.set(index);
        page.show_current_step();
        run_until("download failure", || page.imp().is_error.get());

        let view = page.imp().install_view.borrow().clone().unwrap();
        assert_eq!(view.task_state(index).as_deref(), Some("failed"), "{id}");
        assert!(view.error_label.label().contains("not an archive"), "{id}");
        run_until("download progress", || {
            view.progress_bar
                .text()
                .is_some_and(|text| text.starts_with("Downloading..."))
        });
    }

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_URL_SADX_STEAM_TOOLS");
        std::env::remove_var("ADVENTURE_MODS_URL_SA_MOD_MANAGER");
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
    }
    assert_eq!(
        *requests.lock().unwrap(),
        ["GET /steam_tools.7z", "GET /SAModManager.zip"]
    );
}

/// Run the mods step for `selected`, cancelling it once the install started
/// when `cancel_midway` is set. Returns the page after the task finished.
fn run_mods_step(
    path: &std::path::Path,
    selected: Vec<usize>,
    cancel_midway: bool,
) -> AdventureModsSetupPage {
    let logs = LogCapture::start();
    let page = sa2_page(path);
    page.imp()
        .current_step
        .set(step_index(&page, StepId::DownloadMods));
    page.imp().selected_mods.replace(selected);
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    page.run_download_step(
        StepId::DownloadMods,
        gtk::ProgressBar::new(),
        cancel_flag.clone(),
    );
    // The resolution is read right before the install is handed to a worker,
    // whose result only comes back on a later main loop iteration.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !logs.contents().contains("Detected resolution") {
        assert!(
            std::time::Instant::now() < deadline,
            "install never started"
        );
        glib::MainContext::default().iteration(false);
    }
    if cancel_midway {
        cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    run_until("install end", || !page.imp().task_running.get());
    page
}

#[gtk::test]
fn mods_step_reports_install_errors() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    // The same mod twice is rejected before anything downloads.
    let page = run_mods_step(tmp.path(), vec![0, 0], false);

    assert!(page.imp().is_error.get());
    assert!(status_description(&page).contains("Duplicate mod install target"));
    assert_eq!(
        page.current_step().map(|step| step.id),
        Some(StepId::DownloadMods)
    );
}

#[gtk::test]
fn cancelled_mods_step_neither_errors_nor_moves_on() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let failed = run_mods_step(tmp.path(), vec![0, 0], true);
    assert!(!failed.imp().is_error.get());
    assert!(!failed.imp().step_busy.get());
    assert_eq!(
        failed.current_step().map(|step| step.id),
        Some(StepId::DownloadMods)
    );

    let finished = run_mods_step(tmp.path(), Vec::new(), true);
    assert!(tmp.path().join("SAManager/Manager.json").exists());
    assert!(!finished.imp().is_error.get());
    assert_eq!(
        finished.current_step().map(|step| step.id),
        Some(StepId::DownloadMods)
    );
}

#[gtk::test]
fn finished_mods_step_stops_the_cancel_poll_and_moves_on() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    page.imp()
        .current_step
        .set(step_index(&page, StepId::DownloadMods));
    page.imp().selected_mods.replace(Vec::new());
    let poll_fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let source = glib::timeout_add_local(std::time::Duration::from_secs(60), {
        let poll_fired = poll_fired.clone();
        move || {
            poll_fired.set(true);
            glib::ControlFlow::Break
        }
    });
    page.imp().poll_source.replace(Some(source));

    page.run_download_step(
        StepId::DownloadMods,
        gtk::ProgressBar::new(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    run_until("install end", || !page.imp().task_running.get());

    assert!(page.imp().poll_source.borrow().is_none());
    assert!(!poll_fired.get());
    assert_eq!(
        page.current_step().map(|step| step.id),
        Some(StepId::Complete)
    );
}

#[gtk::test]
fn advancing_skips_steps_that_are_already_done() {
    init_resource_overlay();
    let logs = LogCapture::start();

    let tmp = tempfile::tempdir().unwrap();
    // A game already converted to the 2004 layout.
    std::fs::write(tmp.path().join("sonic.exe"), b"exe").unwrap();
    let page = AdventureModsSetupPage::new(Game {
        kind: GameKind::SADX,
        path: tmp.path().to_path_buf(),
    });
    page.imp().all_steps.replace(vec![
        info_step(StepId::LanguageOptions),
        steps::SetupStep {
            id: StepId::ConvertSteam,
            title: "Convert",
            description: "Convert",
            kind: steps::StepKind::Download,
        },
        info_step(StepId::Complete),
    ]);
    page.imp().current_step.set(0);

    page.advance_step();

    assert_eq!(page.imp().current_step.get(), 2);
    assert!(logs.contents().contains(&format!(
        "Auto-skipping completed step: {}",
        StepId::ConvertSteam
    )));
}

#[gtk::test]
fn next_with_an_invalid_step_logs_and_shows_an_error() {
    init_resource_overlay();
    let logs = LogCapture::start();

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    page.imp().current_step.set(42);
    page.on_next_clicked();

    assert!(page.imp().is_error.get());
    assert!(
        logs.contents()
            .contains("Setup next button clicked with invalid step index 42")
    );
}

#[gtk::test]
fn continuing_from_languages_saves_the_choice() {
    init_resource_overlay();
    let _guard = crate::test_env::lock();

    let schema_dir = tempfile::tempdir().unwrap();
    let schema = include_str!("../../data/io.github.astrovm.AdventureMods.gschema.xml")
        .replace("@APP_ID_RAW@", crate::config::APP_ID)
        .replace("@APP_PATH_RAW@", "/io/github/astrovm/AdventureMods/");
    std::fs::write(
        schema_dir
            .path()
            .join(format!("{}.gschema.xml", crate::config::APP_ID)),
        schema,
    )
    .unwrap();
    let status = std::process::Command::new("glib-compile-schemas")
        .arg(schema_dir.path())
        .status()
        .unwrap();
    assert!(status.success(), "glib-compile-schemas failed");
    let previous: Vec<_> = ["GSETTINGS_SCHEMA_DIR", "GSETTINGS_BACKEND"]
        .into_iter()
        .map(|name| (name, std::env::var_os(name)))
        .collect();
    unsafe {
        std::env::set_var("GSETTINGS_SCHEMA_DIR", schema_dir.path());
        std::env::set_var("GSETTINGS_BACKEND", "memory");
    }

    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    let chosen = crate::setup::config::LanguageSelection {
        subtitle: SubtitleLanguage::Italian,
        voice: VoiceLanguage::English,
    };
    page.imp().language_selection.replace(Some(chosen));
    // GSettings reports changes through the thread-default context; keep them
    // off GTK's.
    let saved = glib::MainContext::new()
        .with_thread_default(|| {
            page.persist_language_selection();
            crate::setup::config::load_language_selection(
                crate::setup::config::app_settings().as_ref(),
                GameKind::SA2,
            )
        })
        .unwrap();

    for (name, value) in previous {
        match value {
            Some(value) => unsafe { std::env::set_var(name, value) },
            None => unsafe { std::env::remove_var(name) },
        }
    }
    assert_eq!(saved, chosen);
}

#[gtk::test]
fn back_button_edge_cases() {
    init_resource_overlay();

    let tmp = tempfile::tempdir().unwrap();

    // Busy steps ignore Back.
    let page = sa2_page(tmp.path());
    page.imp().all_steps.replace(vec![
        info_step(StepId::LanguageOptions),
        info_step(StepId::Complete),
    ]);
    page.imp().current_step.set(1);
    page.imp().step_busy.set(true);
    page.on_back_clicked();
    assert_eq!(page.imp().current_step.get(), 1);

    // Back from the first screen leaves setup.
    let page = sa2_page(tmp.path());
    let nav = inside_navigation(&page);
    assert_eq!(page.imp().current_step.get(), 0);
    page.imp().back_button.emit_clicked();
    assert_eq!(visible_title(&nav), "Welcome");

    // Without a game there is nothing to step back through.
    let logs = LogCapture::start();
    let page = sa2_page(tmp.path());
    let nav = inside_navigation(&page);
    page.imp().all_steps.replace(vec![
        info_step(StepId::LanguageOptions),
        info_step(StepId::Complete),
    ]);
    page.imp().current_step.set(1);
    page.imp().game.replace(None);
    page.on_back_clicked();
    assert_eq!(visible_title(&nav), "Welcome");
    assert!(
        logs.contents()
            .contains("Setup back button clicked without a selected game")
    );

    // Back skips over automatic steps to the last question.
    let page = sa2_page(tmp.path());
    page.imp().all_steps.replace(vec![
        info_step(StepId::LanguageOptions),
        steps::SetupStep {
            id: StepId::Dotnet,
            title: "Runtime",
            description: "Runtime",
            kind: steps::StepKind::Auto,
        },
        info_step(StepId::SelectMods),
    ]);
    page.imp().current_step.set(2);
    page.on_back_clicked();
    assert_eq!(page.imp().current_step.get(), 0);
}

#[gtk::test]
fn resolution_falls_back_to_1080p() {
    init_resource_overlay();
    let logs = LogCapture::start();

    assert_eq!(resolution_or_fallback(Some((2560, 1440))), (2560, 1440));
    assert!(logs.contents().is_empty());
    assert_eq!(resolution_or_fallback(None), (1920, 1080));
    assert!(
        logs.contents()
            .contains("Could not detect monitor resolution, using fallback 1920x1080")
    );

    // A page on screen reads the monitor its window is on.
    let tmp = tempfile::tempdir().unwrap();
    let page = sa2_page(tmp.path());
    let window = gtk::Window::builder().child(&page).build();
    window.present();
    while glib::MainContext::default().iteration(false) {}
    let (width, height) = page.get_resolution();
    assert!(width > 0 && height > 0);
    window.close();
}
