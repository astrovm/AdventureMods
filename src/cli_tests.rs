use std::io::Write;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use clap::Parser;
use gio::Settings;
use gio::prelude::{SettingsExt, SettingsExtManual};

use super::{
    Cli, CliOutput, Command, DetectArgs, Prompt, SetupArgs, TerminalPrompt,
    detect_games_with_saved_libraries, not_detected_error, parse_xrandr_resolution,
    persist_cli_language_selection, resolve_game_kind_rich, resolve_game_path_rich,
    resolve_mods_flag, resolve_setup_languages, resolve_setup_mods, resolve_setup_mods_rich,
    run_from_args_with_io, run_with_io, setup_is_fully_specified,
};
use crate::config::APP_ID;
use crate::setup::common;
use crate::setup::config::{
    LanguageSelection, SubtitleLanguage, VoiceLanguage, load_extra_library_paths,
};
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{DetectionResult, InaccessibleGame};
use crate::test_log::capture_logs;

fn with_test_settings<T>(test: impl FnOnce(&Settings) -> T) -> T {
    let _guard = crate::test_env::lock();
    let schema_dir = tempfile::tempdir().unwrap();
    let schema_path = schema_dir.path().join(format!("{APP_ID}.gschema.xml"));
    let schema = include_str!("../data/io.github.astrovm.AdventureMods.gschema.xml")
        .replace("@APP_ID_RAW@", APP_ID)
        .replace("@APP_PATH_RAW@", "/io/github/astrovm/AdventureMods/");
    std::fs::write(&schema_path, schema).unwrap();

    let status = ProcessCommand::new("glib-compile-schemas")
        .arg(schema_dir.path())
        .status()
        .unwrap();
    assert!(status.success(), "glib-compile-schemas failed");

    let previous_schema_dir = std::env::var("GSETTINGS_SCHEMA_DIR").ok();
    let previous_backend = std::env::var("GSETTINGS_BACKEND").ok();
    let previous_xdg_config = std::env::var("XDG_CONFIG_HOME").ok();
    let previous_xdg_data = std::env::var("XDG_DATA_HOME").ok();
    let config_home = tempfile::tempdir().unwrap();
    let data_home = tempfile::tempdir().unwrap();

    unsafe {
        std::env::set_var("GSETTINGS_SCHEMA_DIR", schema_dir.path());
        std::env::set_var("GSETTINGS_BACKEND", "memory");
        std::env::set_var("XDG_CONFIG_HOME", config_home.path());
        std::env::set_var("XDG_DATA_HOME", data_home.path());
    }

    let schema_source =
        gio::SettingsSchemaSource::from_directory(schema_dir.path(), None, true).unwrap();
    let schema = schema_source.lookup(APP_ID, true).unwrap();
    // GSettings delivers change notifications through the thread-default
    // context. Without a private one, writes acquire the global default
    // context and race with GTK initialization in `#[gtk::test]`s.
    let result = gio::glib::MainContext::new()
        .with_thread_default(|| {
            let settings = Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
            test(&settings)
        })
        .unwrap();

    match previous_schema_dir {
        Some(value) => unsafe { std::env::set_var("GSETTINGS_SCHEMA_DIR", value) },
        None => unsafe { std::env::remove_var("GSETTINGS_SCHEMA_DIR") },
    }
    match previous_backend {
        Some(value) => unsafe { std::env::set_var("GSETTINGS_BACKEND", value) },
        None => unsafe { std::env::remove_var("GSETTINGS_BACKEND") },
    }
    match previous_xdg_config {
        Some(value) => unsafe { std::env::set_var("XDG_CONFIG_HOME", value) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }
    match previous_xdg_data {
        Some(value) => unsafe { std::env::set_var("XDG_DATA_HOME", value) },
        None => unsafe { std::env::remove_var("XDG_DATA_HOME") },
    }

    result
}

struct MockPrompt {
    select_result: usize,
    multi_select_result: Vec<usize>,
    confirm_result: bool,
}

impl Prompt for MockPrompt {
    fn select(&self, _prompt: &str, _items: &[String], _default: usize) -> anyhow::Result<usize> {
        Ok(self.select_result)
    }

    fn multi_select(
        &self,
        _prompt: &str,
        _items: &[String],
        _defaults: &[bool],
    ) -> anyhow::Result<Vec<usize>> {
        Ok(self.multi_select_result.clone())
    }

    fn confirm(&self, _prompt: &str, _default: bool) -> anyhow::Result<bool> {
        Ok(self.confirm_result)
    }
}

fn empty_setup_args() -> SetupArgs {
    SetupArgs {
        game: None,
        mods: None,
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    }
}

#[test]
fn resolve_game_kind_rich_uses_game_flag() {
    let args = SetupArgs {
        game: Some("sa2".to_string()),
        mods: None,
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };
    let prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };

    let kind = resolve_game_kind_rich(&args, None, &prompt).unwrap();
    assert_eq!(kind, GameKind::SA2);
}

#[test]
fn resolve_game_kind_rich_prompts_when_only_a_game_path_is_given() {
    let mut args = empty_setup_args();
    args.game_path = Some(PathBuf::from("/games/sa2"));
    let prompt = MockPrompt {
        select_result: 1,
        multi_select_result: vec![],
        confirm_result: true,
    };

    assert_eq!(
        resolve_game_kind_rich(&args, None, &prompt).unwrap(),
        GameKind::SA2
    );
}

#[test]
fn resolve_game_kind_rich_uses_a_single_detected_game() {
    let args = empty_setup_args();
    let detected = DetectionResult {
        games: vec![Game {
            kind: GameKind::SADX,
            path: PathBuf::from("/games/sadx"),
        }],
        inaccessible: vec![],
    };
    let prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };

    assert_eq!(
        resolve_game_kind_rich(&args, Some(&detected), &prompt).unwrap(),
        GameKind::SADX
    );
}

#[test]
fn resolve_game_kind_rich_rejects_empty_detection() {
    let args = empty_setup_args();
    let detected = DetectionResult {
        games: vec![],
        inaccessible: vec![],
    };
    let prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };

    let error = resolve_game_kind_rich(&args, Some(&detected), &prompt).unwrap_err();
    assert!(error.to_string().contains("No supported games detected"));
}

#[test]
fn resolve_game_kind_rich_prompts_for_multiple_detected_games() {
    let args = empty_setup_args();
    let detected = DetectionResult {
        games: vec![
            Game {
                kind: GameKind::SADX,
                path: PathBuf::from("/games/sadx"),
            },
            Game {
                kind: GameKind::SA2,
                path: PathBuf::from("/games/sa2"),
            },
        ],
        inaccessible: vec![],
    };
    let prompt = MockPrompt {
        select_result: 1,
        multi_select_result: vec![],
        confirm_result: true,
    };

    assert_eq!(
        resolve_game_kind_rich(&args, Some(&detected), &prompt).unwrap(),
        GameKind::SA2
    );
}

#[test]
fn missing_game_error_names_inaccessible_libraries() {
    let result = DetectionResult {
        games: vec![],
        inaccessible: vec![InaccessibleGame {
            kind: GameKind::SADX,
            library_path: PathBuf::from("/data/SteamLibrary"),
        }],
    };

    let sadx = not_detected_error(GameKind::SADX, &result).to_string();
    assert!(sadx.contains("/data/SteamLibrary"), "{sadx}");
    assert!(sadx.contains("grant access"), "{sadx}");
    assert_eq!(
        not_detected_error(GameKind::SA2, &result).to_string(),
        "Sonic Adventure 2 was not detected. Pass --game-path."
    );
}

#[test]
fn saved_library_grants_are_read_from_settings() {
    assert!(load_extra_library_paths(None).is_empty());
    with_test_settings(|settings| {
        settings
            .set_strv(
                "extra-library-paths",
                ["/run/user/1000/doc/abc/SteamLibrary"],
            )
            .unwrap();
        assert_eq!(
            load_extra_library_paths(Some(settings)),
            vec![PathBuf::from("/run/user/1000/doc/abc/SteamLibrary")]
        );
    });
}

#[test]
fn saved_library_grants_join_the_default_steam_scan_only() {
    let library = tempfile::tempdir().unwrap();
    let game = library
        .path()
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&game).unwrap();
    std::fs::File::create(game.join("sonic2app.exe")).unwrap();
    let saved = [library.path().to_path_buf()];

    // Scan a home without Steam so only the saved grant can find the game.
    let default_scan = {
        let _guard = crate::test_env::lock();
        let home = tempfile::tempdir().unwrap();
        let previous_home = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", home.path()) };
        let result = detect_games_with_saved_libraries(
            &DetectArgs {
                libraryfolders_vdf: None,
                steam_libraries: vec![],
            },
            &saved,
        );
        match previous_home {
            Some(value) => unsafe { std::env::set_var("HOME", value) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        result.unwrap()
    };
    let paths: Vec<_> = default_scan.games.iter().map(|g| g.path.clone()).collect();
    assert_eq!(paths, vec![game]);

    // An explicit libraryfolders.vdf is used exactly as given.
    let vdf_path = library.path().join("libraryfolders.vdf");
    std::fs::write(&vdf_path, "\"libraryfolders\" {}").unwrap();
    let explicit = detect_games_with_saved_libraries(
        &DetectArgs {
            libraryfolders_vdf: Some(vdf_path),
            steam_libraries: vec![],
        },
        &saved,
    )
    .unwrap();
    assert!(explicit.games.is_empty());
}

#[test]
fn resolve_game_path_rich_handles_explicit_single_and_multiple_paths() {
    let temp = tempfile::tempdir().unwrap();
    let explicit_path = temp.path().join("Sonic Adventure 2");
    std::fs::create_dir_all(&explicit_path).unwrap();
    std::fs::File::create(explicit_path.join("sonic2app.exe")).unwrap();

    let mut explicit_args = empty_setup_args();
    explicit_args.game_path = Some(explicit_path.clone());
    let prompt = MockPrompt {
        select_result: 1,
        multi_select_result: vec![],
        confirm_result: true,
    };
    assert_eq!(
        resolve_game_path_rich(&explicit_args, GameKind::SA2, None, &prompt).unwrap(),
        explicit_path
    );

    let detected = DetectionResult {
        games: vec![
            Game {
                kind: GameKind::SA2,
                path: PathBuf::from("/games/sa2-one"),
            },
            Game {
                kind: GameKind::SADX,
                path: PathBuf::from("/games/sadx"),
            },
            Game {
                kind: GameKind::SA2,
                path: PathBuf::from("/games/sa2-two"),
            },
        ],
        inaccessible: vec![],
    };

    let mut detected_args = empty_setup_args();
    detected_args.game_path = None;
    let selected =
        resolve_game_path_rich(&detected_args, GameKind::SA2, Some(&detected), &prompt).unwrap();
    assert_eq!(selected, PathBuf::from("/games/sa2-two"));

    let empty = DetectionResult {
        games: vec![],
        inaccessible: vec![],
    };
    let error =
        resolve_game_path_rich(&detected_args, GameKind::SA2, Some(&empty), &prompt).unwrap_err();
    assert!(error.to_string().contains("was not detected"));

    let detected_library = tempfile::tempdir().unwrap();
    let detected_game = detected_library
        .path()
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&detected_game).unwrap();
    std::fs::File::create(detected_game.join("sonic2app.exe")).unwrap();
    let vdf_path = detected_library.path().join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" \"apps\" {{ \"213610\" \"0\" }} }} }}",
            detected_library.path().display()
        ),
    )
    .unwrap();
    let mut detected_from_disk = empty_setup_args();
    detected_from_disk.detect.libraryfolders_vdf = Some(vdf_path);
    assert_eq!(
        resolve_game_path_rich(&detected_from_disk, GameKind::SA2, None, &prompt).unwrap(),
        detected_game
    );
}

#[test]
fn resolve_setup_mods_rich_handles_presets_all_mods_and_invalid_manual_indices() {
    let args = empty_setup_args();
    let preset_prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };
    let preset_mods = resolve_setup_mods_rich(&args, GameKind::SADX, &preset_prompt).unwrap();
    assert!(!preset_mods.is_empty());

    let all_mods_prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };
    let all_mods = resolve_setup_mods_rich(&args, GameKind::SA2, &all_mods_prompt).unwrap();
    assert_eq!(
        all_mods.len(),
        common::recommended_mods_for_game(GameKind::SA2).len()
    );

    let invalid_manual_prompt = MockPrompt {
        select_result: 1,
        multi_select_result: vec![common::recommended_mods_for_game(GameKind::SA2).len()],
        confirm_result: true,
    };
    let error = match resolve_setup_mods_rich(&args, GameKind::SA2, &invalid_manual_prompt) {
        Ok(_) => panic!("expected invalid manual selection to fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("Invalid mod selection"));
}

#[test]
fn resolve_setup_mods_rejects_conflicting_mod_flags() {
    let mut args = empty_setup_args();
    args.mods = Some("sa2-render-fix".to_string());
    args.preset = Some("DX Enhanced".to_string());
    let error = match resolve_setup_mods(&args, GameKind::SA2) {
        Ok(_) => panic!("expected conflicting mod flags to fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("both --preset and --mods"));

    args.preset = None;
    args.all_mods = true;
    let error = match resolve_setup_mods(&args, GameKind::SA2) {
        Ok(_) => panic!("expected all-mods and mods to conflict"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("--all-mods with --mods"));
}

#[test]
fn resolve_mods_flag_rejects_empty_values_and_game_parser_normalizes_input() {
    let error = match resolve_mods_flag(GameKind::SA2, " , ") {
        Ok(_) => panic!("expected empty mod values to fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("at least one mod slug"));

    assert_eq!(super::parse_game_kind(" SADX ").unwrap(), GameKind::SADX);
    let error = super::parse_game_kind("sonic").unwrap_err();
    assert!(error.to_string().contains("Unknown game"));
}

#[test]
fn resolve_setup_mods_rich_with_all_mods_flag() {
    let args = SetupArgs {
        game: Some("sa2".to_string()),
        mods: None,
        preset: None,
        all_mods: true,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };
    let prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };

    let selected = resolve_setup_mods_rich(&args, GameKind::SA2, &prompt).unwrap();
    assert_eq!(
        selected.len(),
        common::recommended_mods_for_game(GameKind::SA2).len()
    );
}

#[test]
fn resolve_setup_mods_rich_hides_all_recommended_when_presets_exist() {
    let args = SetupArgs {
        game: Some("sadx".to_string()),
        mods: None,
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };
    let prompt = MockPrompt {
        select_result: 2,
        multi_select_result: vec![0],
        confirm_result: true,
    };

    let selected = resolve_setup_mods_rich(&args, GameKind::SADX, &prompt).unwrap();

    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].name,
        common::recommended_mods_for_game(GameKind::SADX)[0].name
    );
}

#[test]
fn resolve_setup_mods_rich_rejects_empty_manual_selection() {
    let args = SetupArgs {
        game: Some("sa2".to_string()),
        mods: None,
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };
    let prompt = MockPrompt {
        select_result: 1,
        multi_select_result: vec![],
        confirm_result: true,
    };

    let Err(error) = resolve_setup_mods_rich(&args, GameKind::SA2, &prompt) else {
        panic!("expected empty manual selection to fail");
    };

    assert!(error.to_string().contains("at least one mod"));
}

#[test]
fn cli_persists_selected_languages() {
    with_test_settings(|settings| {
        settings
            .set_string("sa2-subtitle-language", "english")
            .unwrap();
        settings
            .set_string("sa2-voice-language", "japanese")
            .unwrap();

        persist_cli_language_selection(
            GameKind::SA2,
            LanguageSelection {
                subtitle: SubtitleLanguage::Italian,
                voice: VoiceLanguage::English,
            },
        );

        assert_eq!(settings.string("sa2-subtitle-language"), "italian");
        assert_eq!(settings.string("sa2-voice-language"), "english");
    });
}

#[test]
fn resolve_setup_languages_prompts_when_no_flags() {
    with_test_settings(|settings| {
        settings
            .set_string("sadx-subtitle-language", "english")
            .unwrap();
        settings
            .set_string("sadx-voice-language", "japanese")
            .unwrap();

        let args = SetupArgs {
            game: None,
            mods: None,
            preset: None,
            all_mods: false,
            subtitle_language: None,
            voice_language: None,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        // supported_for(SADX) = [Japanese, English, French, Spanish, German]
        // index 1 = English for subtitle, index 1 = English for voice
        let prompt = MockPrompt {
            select_result: 1,
            multi_select_result: vec![],
            confirm_result: true,
        };

        let result = resolve_setup_languages(&args, GameKind::SADX, Some(&prompt)).unwrap();

        assert_eq!(result.subtitle, SubtitleLanguage::English);
        assert_eq!(result.voice, VoiceLanguage::English);
    });
}

#[test]
fn resolve_setup_languages_skips_prompts_when_flags_set() {
    with_test_settings(|_settings| {
        let args = SetupArgs {
            game: None,
            mods: None,
            preset: None,
            all_mods: false,
            subtitle_language: Some("spanish".to_string()),
            voice_language: Some("english".to_string()),
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        // select_result doesn't matter since flags bypass prompts
        let prompt = MockPrompt {
            select_result: 999,
            multi_select_result: vec![],
            confirm_result: true,
        };

        let result = resolve_setup_languages(&args, GameKind::SA2, Some(&prompt)).unwrap();

        assert_eq!(result.subtitle, SubtitleLanguage::Spanish);
        assert_eq!(result.voice, VoiceLanguage::English);
    });
}

#[test]
fn parses_detect_command() {
    let cli = Cli::parse_from(["adventure-mods", "detect"]);
    assert!(matches!(cli.command, Some(Command::Detect(_))));
}

#[test]
fn parses_list_mods_command() {
    let cli = Cli::parse_from(["adventure-mods", "list-mods", "--game", "sadx"]);

    match cli.command {
        Some(Command::ListMods { game }) => assert_eq!(game.as_str(), "sadx"),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_setup_command_with_mods_flag() {
    let cli = Cli::parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sa2",
        "--mods",
        "sa2-render-fix,hd-gui-sa2-edition",
        "--width",
        "1280",
        "--height",
        "720",
    ]);

    match cli.command {
        Some(Command::Setup(args)) => {
            assert_eq!(args.game.as_deref(), Some("sa2"));
            assert_eq!(
                args.mods.as_deref(),
                Some("sa2-render-fix,hd-gui-sa2-edition")
            );
            assert!(!args.all_mods);
            assert_eq!(args.width, Some(1280));
            assert_eq!(args.height, Some(720));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn resolve_mods_flag_accepts_slug_ids() {
    let selected = resolve_mods_flag(GameKind::SA2, "sa2-render-fix,hd-gui-sa2-edition").unwrap();

    let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();
    assert_eq!(names, vec!["SA2 Render Fix", "HD GUI: SA2 Edition"]);
}

#[test]
fn resolve_mods_flag_rejects_display_names() {
    let error = match resolve_mods_flag(GameKind::SADX, "Fixes, Adds, and Beta Restores") {
        Ok(_) => panic!("expected display name to be rejected"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("Unknown mod slug"));
}

#[test]
fn setup_args_accept_language_flags() {
    let cli = Cli::parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sa2",
        "--game-path",
        "/tmp/game",
        "--all-mods",
        "--subtitle-language",
        "japanese",
        "--voice-language",
        "japanese",
    ]);

    match cli.command {
        Some(Command::Setup(args)) => {
            assert_eq!(args.subtitle_language.as_deref(), Some("japanese"));
            assert_eq!(args.voice_language.as_deref(), Some("japanese"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_removed_non_interactive_flag() {
    let error = Cli::try_parse_from(["adventure-mods", "setup", "--non-interactive"]).unwrap_err();

    assert!(error.to_string().contains("--non-interactive"));
}

#[test]
fn rejects_zero_width_flag() {
    let error = Cli::try_parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sa2",
        "--mods",
        "sa2-render-fix",
        "--width",
        "0",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("--width"));
}

#[test]
fn rejects_zero_height_flag() {
    let error = Cli::try_parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sa2",
        "--mods",
        "sa2-render-fix",
        "--height",
        "0",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("--height"));
}

#[test]
fn setup_is_fully_specified_with_explicit_flags() {
    let args = SetupArgs {
        game: Some("sa2".to_string()),
        mods: Some("sa2-render-fix".to_string()),
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: Some(PathBuf::from("/tmp/sa2")),
        detect: Default::default(),
    };

    assert!(setup_is_fully_specified(&args));
}

#[test]
fn setup_is_not_fully_specified_without_mod_choice() {
    let args = SetupArgs {
        game: Some("sa2".to_string()),
        mods: None,
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: Some(PathBuf::from("/tmp/sa2")),
        detect: Default::default(),
    };

    assert!(!setup_is_fully_specified(&args));
}

#[test]
fn parses_no_color_flag() {
    let cli = Cli::parse_from(["adventure-mods", "--no-color", "detect"]);
    assert!(cli.no_color);
}

#[test]
fn run_from_args_initializes_runtime_for_cli_commands() {
    let mut initialized = false;
    let mut output = Vec::new();

    let handled = run_from_args_with_io(
        vec!["adventure-mods".to_string(), "detect".to_string()],
        || {
            initialized = true;
            Ok(())
        },
        &mut output,
        false,
    )
    .unwrap();

    assert!(handled);
    assert!(initialized);
}

#[test]
fn resolve_setup_mods_bails_without_mod_selection() {
    let args = SetupArgs {
        game: Some("sadx".to_string()),
        mods: None,
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };

    let Err(error) = resolve_setup_mods(&args, GameKind::SADX) else {
        panic!("expected error when no mod selection specified");
    };

    assert!(error.to_string().contains("--all-mods"));
    assert!(error.to_string().contains("list-mods"));
}

#[test]
fn resolve_setup_mods_returns_all_recommended_mods() {
    let args = SetupArgs {
        game: Some("sa2".to_string()),
        mods: None,
        preset: None,
        all_mods: true,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };

    let selected = resolve_setup_mods(&args, GameKind::SA2).unwrap();

    assert_eq!(
        selected.len(),
        common::recommended_mods_for_game(GameKind::SA2).len()
    );
}

#[test]
fn resolve_setup_mods_rejects_all_mods_with_preset() {
    let args = SetupArgs {
        game: Some("sadx".to_string()),
        mods: None,
        preset: Some("DX Enhanced".to_string()),
        all_mods: true,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };

    let Err(error) = resolve_setup_mods(&args, GameKind::SADX) else {
        panic!("expected incompatible all-mods selection to fail");
    };

    assert!(error.to_string().contains("Cannot use --all-mods"));
}

#[test]
fn resolve_setup_mods_accepts_single_mods_flag() {
    let args = SetupArgs {
        game: Some("sa2".to_string()),
        mods: Some("sa2-render-fix,hd-gui-sa2-edition".to_string()),
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };

    let selected = resolve_setup_mods(&args, GameKind::SA2).unwrap();
    let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();

    assert_eq!(names, vec!["SA2 Render Fix", "HD GUI: SA2 Edition"]);
}

#[test]
fn resolve_setup_mods_accepts_preset_flag() {
    let args = SetupArgs {
        game: Some("sadx".to_string()),
        mods: None,
        preset: Some("DX Enhanced".to_string()),
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: None,
        detect: Default::default(),
    };

    let selected = resolve_setup_mods(&args, GameKind::SADX).unwrap();

    assert!(!selected.is_empty());
    assert!(selected.iter().any(|m| m.name == "Dreamcast Conversion"));
}

#[test]
fn resolve_game_kind_requires_game_flag_when_game_path_given() {
    let args = SetupArgs {
        game: None,
        mods: Some("sa2-render-fix".to_string()),
        preset: None,
        all_mods: false,
        subtitle_language: None,
        voice_language: None,
        width: None,
        height: None,
        game_path: Some(PathBuf::from("/tmp/sa2")),
        detect: Default::default(),
    };

    let error = super::resolve_game_kind(&args).unwrap_err();
    assert!(error.to_string().contains("--game"));
}

#[test]
fn run_from_args_surfaces_unknown_subcommand_as_error() {
    let mut output = Vec::new();

    let result = run_from_args_with_io(
        vec!["adventure-mods".to_string(), "detcet".to_string()],
        || Ok(()),
        &mut output,
        false,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("detcet"));
}

#[test]
fn run_from_args_surfaces_unknown_top_level_flag_as_error() {
    let mut output = Vec::new();

    let result = run_from_args_with_io(
        vec!["adventure-mods".to_string(), "--bogus".to_string()],
        || Ok(()),
        &mut output,
        false,
    );

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("--bogus"));
}

#[test]
fn run_from_args_ignores_gui_flags_that_take_separate_values() {
    let mut output = Vec::new();
    let mut initialized = false;

    let handled = run_from_args_with_io(
        vec![
            "adventure-mods".to_string(),
            "--gtk-debug".to_string(),
            "interactive".to_string(),
        ],
        || {
            initialized = true;
            Ok(())
        },
        &mut output,
        false,
    )
    .unwrap();

    assert!(!handled);
    assert!(!initialized);
}

#[test]
fn run_from_args_help_does_not_initialize_runtime() {
    let mut output = Vec::new();
    let mut initialized = false;

    let handled = run_from_args_with_io(
        vec!["adventure-mods".to_string(), "--help".to_string()],
        || {
            initialized = true;
            Ok(())
        },
        &mut output,
        false,
    )
    .unwrap();

    assert!(handled);
    assert!(!initialized);
    assert!(String::from_utf8(output).unwrap().contains("Usage:"));
}

#[test]
fn run_from_args_version_does_not_initialize_runtime() {
    let mut output = Vec::new();
    let mut initialized = false;

    let handled = run_from_args_with_io(
        vec!["adventure-mods".to_string(), "--version".to_string()],
        || {
            initialized = true;
            Ok(())
        },
        &mut output,
        false,
    )
    .unwrap();

    assert!(handled);
    assert!(!initialized);
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn run_from_args_does_not_handle_no_color_without_subcommand() {
    let mut output = Vec::new();
    let mut initialized = false;

    let handled = run_from_args_with_io(
        vec!["adventure-mods".to_string(), "--no-color".to_string()],
        || {
            initialized = true;
            Ok(())
        },
        &mut output,
        false,
    )
    .unwrap();

    assert!(!handled);
    assert!(!initialized);
    assert!(output.is_empty());
}

#[test]
fn run_from_args_does_not_handle_gui_flags_with_no_color_only() {
    let mut output = Vec::new();
    let mut initialized = false;

    let handled = run_from_args_with_io(
        vec![
            "adventure-mods".to_string(),
            "--no-color".to_string(),
            "--display".to_string(),
            ":1".to_string(),
        ],
        || {
            initialized = true;
            Ok(())
        },
        &mut output,
        false,
    )
    .unwrap();

    assert!(!handled);
    assert!(!initialized);
    assert!(output.is_empty());
}

#[test]
fn looks_like_cli_matches_known_subcommands() {
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "detect".to_string()
    ]));
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "list-mods".to_string()
    ]));
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "setup".to_string()
    ]));
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--help".to_string()
    ]));
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--version".to_string()
    ]));
}

#[test]
fn looks_like_cli_ignores_gui_flags() {
    assert!(!super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--gapplication-service".to_string()
    ]));
    assert!(!super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--display".to_string(),
        ":1".to_string()
    ]));
    assert!(!super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--gtk-debug".to_string(),
        "interactive".to_string()
    ]));
    assert!(!super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--gdk-debug=events".to_string()
    ]));
    assert!(!super::looks_like_cli(&["adventure-mods".to_string()]));
}

#[test]
fn looks_like_cli_detects_any_positional_arg() {
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "typo".to_string()
    ]));
}

#[test]
fn looks_like_cli_accepts_global_flags_before_subcommand() {
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--no-color".to_string(),
        "detect".to_string(),
    ]));
}

#[test]
fn looks_like_cli_ignores_no_color_without_subcommand() {
    assert!(!super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--no-color".to_string(),
    ]));
    assert!(!super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--no-color".to_string(),
        "--display".to_string(),
        ":1".to_string(),
    ]));
    assert!(super::looks_like_cli(&[
        "adventure-mods".to_string(),
        "--display".to_string(),
    ]));
}

#[test]
fn terminal_prompt_respects_no_color_setting() {
    let _guard = crate::test_env::lock();
    console::set_colors_enabled_stderr(true);

    let prompt = TerminalPrompt { use_color: false };
    let during = prompt.with_stderr_colors(console::colors_enabled_stderr);

    assert!(!during);
    assert!(console::colors_enabled_stderr());
}

#[test]
fn validate_game_path_rejects_missing_directory() {
    let path = std::path::PathBuf::from("/nonexistent/path/Sonic Adventure DX");
    let result = super::validate_game_path(GameKind::SADX, &path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn validate_game_path_rejects_wrong_game_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let sa2_path = tmp.path().join("Sonic Adventure 2");
    std::fs::create_dir_all(&sa2_path).unwrap();

    let result = super::validate_game_path(GameKind::SADX, &sa2_path);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("does not appear to be")
    );
}

#[test]
fn validate_game_path_accepts_directory_with_game_executable() {
    let tmp = tempfile::tempdir().unwrap();
    let sadx_path = tmp.path().join("Sonic Adventure DX");
    std::fs::create_dir_all(&sadx_path).unwrap();
    std::fs::File::create(sadx_path.join("sonic.exe")).unwrap();

    let result = super::validate_game_path(GameKind::SADX, &sadx_path);
    assert!(result.is_ok());
}

#[test]
fn validate_game_path_rejects_directory_without_executable() {
    let tmp = tempfile::tempdir().unwrap();
    let sadx_path = tmp.path().join("Sonic Adventure DX");
    std::fs::create_dir_all(&sadx_path).unwrap();

    let result = super::validate_game_path(GameKind::SADX, &sadx_path);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("does not appear to be")
    );
}

#[test]
fn cli_output_no_color_writes_plain() {
    let mut buf = Vec::new();
    let mut out = CliOutput::new(&mut buf as &mut dyn Write, false);

    out.heading("Test Heading").unwrap();
    out.success("Test Success").unwrap();
    out.bold_item("name", "desc").unwrap();

    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("Test Heading"));
    assert!(s.contains("Test Success"));
    assert!(s.contains("name: desc"));
    assert!(!s.contains("\x1b["));
}

#[test]
fn cli_output_with_color_emits_ansi_sequences() {
    let _guard = crate::test_env::lock();
    let previous = console::colors_enabled();
    console::set_colors_enabled(true);

    let mut buf = Vec::new();
    let mut out = CliOutput::new(&mut buf as &mut dyn Write, true);

    out.heading("Test Heading").unwrap();
    out.success("Test Success").unwrap();
    out.bold_item("name", "desc").unwrap();

    let s = String::from_utf8(buf).unwrap();
    console::set_colors_enabled(previous);
    assert!(s.contains("\x1b["));
}

#[test]
fn run_from_args_uses_color_for_terminal_output_by_default() {
    let _guard = crate::test_env::lock();
    let had_no_color = std::env::var("NO_COLOR").ok();
    unsafe {
        std::env::remove_var("NO_COLOR");
    }

    let previous = console::colors_enabled();
    console::set_colors_enabled(true);

    let mut output = Vec::new();

    let result = run_from_args_with_io(
        vec![
            "adventure-mods".to_string(),
            "list-mods".to_string(),
            "--game".to_string(),
            "sa2".to_string(),
        ],
        || Ok(()),
        &mut output,
        true,
    );

    console::set_colors_enabled(previous);
    unsafe {
        if let Some(val) = had_no_color {
            std::env::set_var("NO_COLOR", val);
        }
    }

    let handled = result.unwrap();
    assert!(handled);
    assert!(String::from_utf8(output).unwrap().contains("\x1b["));
}

#[test]
fn run_from_args_disables_color_with_no_color_env() {
    let _guard = crate::test_env::lock();
    unsafe {
        std::env::set_var("NO_COLOR", "1");
    }

    let mut output = Vec::new();
    let result = run_from_args_with_io(
        vec![
            "adventure-mods".to_string(),
            "list-mods".to_string(),
            "--game".to_string(),
            "sa2".to_string(),
        ],
        || Ok(()),
        &mut output,
        true,
    );

    unsafe {
        std::env::remove_var("NO_COLOR");
    }

    let handled = result.unwrap();
    assert!(handled);
    assert!(!String::from_utf8(output).unwrap().contains("\x1b["));
}

#[test]
fn cli_output_and_empty_command_paths_are_exercised() {
    let mut output = Vec::new();
    let mut out = CliOutput::new(&mut output as &mut dyn Write, true);
    let _ = out.path("/tmp/game");
    out.dim_writeln("dim").unwrap();
    out.writeln("plain").unwrap();

    run_with_io(
        Cli {
            no_color: true,
            command: None,
        },
        false,
        &mut output,
    )
    .unwrap();
}

#[test]
fn detect_command_reports_empty_detected_and_inaccessible_results() {
    let tmp = tempfile::tempdir().unwrap();
    let library = tmp.path().join("library");
    let game_dir = library.join("steamapps/common").join("Sonic Adventure 2");
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::File::create(game_dir.join("sonic2app.exe")).unwrap();
    let missing_library = tmp.path().join("missing-library");
    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\"\n{{\n  \"0\"\n  {{\n    \"path\"\t\"{}\"\n    \"apps\"\n    {{\n      \"213610\"\t\"0\"\n    }}\n  }}\n  \"1\"\n  {{\n    \"path\"\t\"{}\"\n    \"apps\"\n    {{\n      \"71250\"\t\"0\"\n    }}\n  }}\n}}\n",
            library.display(),
            missing_library.display()
        ),
    )
    .unwrap();

    let mut output = Vec::new();
    run_with_io(
        Cli {
            no_color: true,
            command: Some(Command::Detect(DetectArgs {
                libraryfolders_vdf: Some(vdf_path.clone()),
                steam_libraries: vec![],
            })),
        },
        false,
        &mut output,
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Detected games:"));
    assert!(output.contains("Inaccessible Steam libraries:"));

    let empty_vdf = tmp.path().join("empty.vdf");
    std::fs::write(&empty_vdf, "\"libraryfolders\" {}\n").unwrap();
    let mut empty_output = Vec::new();
    run_with_io(
        Cli {
            no_color: true,
            command: Some(Command::Detect(DetectArgs {
                libraryfolders_vdf: Some(empty_vdf),
                steam_libraries: vec![],
            })),
        },
        false,
        &mut empty_output,
    )
    .unwrap();
    assert!(
        String::from_utf8(empty_output)
            .unwrap()
            .contains("No supported games detected")
    );
}

#[test]
fn game_resolution_handles_detection_errors_and_cardinality() {
    let tmp = tempfile::tempdir().unwrap();
    let library = tmp.path().join("library");
    let sa2_dir = library.join("steamapps/common").join("Sonic Adventure 2");
    std::fs::create_dir_all(&sa2_dir).unwrap();
    std::fs::File::create(sa2_dir.join("sonic2app.exe")).unwrap();
    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\"\n{{\n  \"0\"\n  {{\n    \"path\"\t\"{}\"\n    \"apps\"\n    {{\n      \"213610\"\t\"0\"\n    }}\n  }}\n}}\n",
            library.display()
        ),
    )
    .unwrap();
    let args = DetectArgs {
        libraryfolders_vdf: Some(vdf_path.clone()),
        steam_libraries: vec![],
    };
    let mut setup_args = empty_setup_args();
    setup_args.detect = args;

    assert_eq!(
        super::resolve_game_kind(&setup_args).unwrap(),
        GameKind::SA2
    );
    assert_eq!(
        super::resolve_game_path(&setup_args, GameKind::SA2).unwrap(),
        sa2_dir
    );

    let invalid = SetupArgs {
        detect: DetectArgs {
            libraryfolders_vdf: Some(tmp.path().join("does-not-exist.vdf")),
            steam_libraries: vec![],
        },
        ..empty_setup_args()
    };
    assert!(super::resolve_game_kind(&invalid).is_err());
    assert!(super::resolve_game_path(&invalid, GameKind::SA2).is_err());

    let empty = SetupArgs {
        detect: DetectArgs {
            libraryfolders_vdf: Some({
                let path = tmp.path().join("empty-resolution.vdf");
                std::fs::write(&path, "\"libraryfolders\" {}\n").unwrap();
                path
            }),
            steam_libraries: vec![],
        },
        ..empty_setup_args()
    };
    assert!(super::resolve_game_kind(&empty).is_err());
    assert!(super::resolve_game_path(&empty, GameKind::SA2).is_err());

    let mut output = Vec::new();
    run_with_io(
        Cli {
            no_color: true,
            command: Some(Command::Detect(DetectArgs {
                libraryfolders_vdf: Some(vdf_path),
                steam_libraries: vec![],
            })),
        },
        false,
        &mut output,
    )
    .unwrap();
}

#[test]
fn game_resolution_reports_multiple_installations() {
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("first");
    let second = tmp.path().join("second");
    for path in [&first, &second] {
        std::fs::create_dir_all(path.join("steamapps/common").join("Sonic Adventure 2")).unwrap();
        std::fs::File::create(path.join("steamapps/common/Sonic Adventure 2/sonic2app.exe"))
            .unwrap();
    }
    let vdf_path = tmp.path().join("multiple.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\"\n{{\n  \"0\"\n  {{\n    \"path\"\t\"{}\"\n    \"apps\"\n    {{\n      \"213610\"\t\"0\"\n    }}\n  }}\n  \"1\"\n  {{\n    \"path\"\t\"{}\"\n    \"apps\"\n    {{\n      \"213610\"\t\"0\"\n    }}\n  }}\n}}\n",
            first.display(),
            second.display()
        ),
    )
    .unwrap();
    let args = SetupArgs {
        detect: DetectArgs {
            libraryfolders_vdf: Some(vdf_path),
            steam_libraries: vec![],
        },
        ..empty_setup_args()
    };
    assert!(super::resolve_game_kind(&args).is_err());
    assert!(super::resolve_game_path(&args, GameKind::SA2).is_err());
}

#[test]
fn rich_resolution_without_precomputed_detection_returns_errors() {
    let _guard = crate::test_env::lock();
    let mut args = empty_setup_args();
    args.detect.libraryfolders_vdf = Some(PathBuf::from("/definitely/missing/libraryfolders.vdf"));
    let prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };
    assert!(resolve_game_kind_rich(&args, None, &prompt).is_err());
    assert!(resolve_game_path_rich(&args, GameKind::SA2, None, &prompt).is_err());
}

#[test]
fn language_validation_rejects_unsupported_and_unknown_values() {
    let mut args = empty_setup_args();
    args.subtitle_language = Some("italian".to_string());
    assert!(resolve_setup_languages(&args, GameKind::SADX, None).is_err());

    let mut args = empty_setup_args();
    args.voice_language = Some("klingon".to_string());
    assert!(resolve_setup_languages(&args, GameKind::SA2, None).is_err());
}

#[test]
fn download_progress_and_summary_cover_cli_output_paths() {
    let mut output = Vec::new();
    {
        let mut out = CliOutput::new(&mut output as &mut dyn Write, true);
        super::run_download_step(&mut out, 1, 2, "Download", |progress| {
            if let Some(progress) = progress {
                progress(1_048_576, Some(2_097_152));
                progress(2_097_152, None);
            }
            Ok::<(), anyhow::Error>(())
        })
        .unwrap();

        let selected = vec![&common::recommended_mods_for_game(GameKind::SA2)[0]];
        let summary = super::SetupSummary {
            game_kind: GameKind::SA2,
            game_path: std::path::Path::new("/games/sa2"),
            selected_mods: &selected,
            width: 1280,
            height: 720,
            language_selection: LanguageSelection::defaults_for(GameKind::SA2),
        };
        let cancelled = MockPrompt {
            select_result: 0,
            multi_select_result: vec![],
            confirm_result: false,
        };
        assert!(super::confirm_setup_summary(&mut out, &summary, &cancelled).is_err());
        let accepted = MockPrompt {
            select_result: 0,
            multi_select_result: vec![],
            confirm_result: true,
        };
        super::confirm_setup_summary(&mut out, &summary, &accepted).unwrap();
    }
    assert!(String::from_utf8(output).unwrap().contains("Summary"));
}

// GTK can only be initialized from one thread, so share the gtk test thread.
#[gtk::test]
fn detect_resolution_uses_a_safe_fallback() {
    let (width, height) = super::detect_resolution();
    assert!(width > 0);
    assert!(height > 0);
}

#[test]
fn detect_resolution_prefers_gdk_over_xrandr() {
    // xrandr must not run once GDK knows the resolution.
    let xrandr = ProcessCommand::new("/definitely/missing/xrandr");
    assert_eq!(
        super::detect_resolution_with(|| Some((1280, 720)), xrandr),
        (1280, 720)
    );
}

/// A command printing `stdout` and exiting with `status`, standing in for xrandr.
fn fake_xrandr(stdout: &str, status: i32) -> ProcessCommand {
    let mut command = ProcessCommand::new("sh");
    command
        .arg("-c")
        .arg(format!("printf '{stdout}'; exit {status}"));
    command
}

#[test]
fn detect_resolution_reads_the_primary_monitor_from_xrandr() {
    let (resolution, logs) = capture_logs(|| {
        super::detect_resolution_with(
            || None,
            fake_xrandr(
                "Screen 0: minimum 8 x 8\\nHDMI-1 connected 1920x1080+0+0\\n\
                 DP-1 connected primary 2560x1440+1920+0\\n",
                0,
            ),
        )
    });

    assert_eq!(resolution, (2560, 1440));
    assert!(
        logs.contains("Detected resolution via xrandr: 2560x1440"),
        "logs were: {logs}"
    );
}

#[test]
fn detect_resolution_falls_back_when_xrandr_is_missing() {
    let (resolution, logs) = capture_logs(|| {
        super::detect_resolution_with(|| None, ProcessCommand::new("/definitely/missing/xrandr"))
    });

    assert_eq!(resolution, (1920, 1080));
    assert!(logs.contains("Could not run xrandr"), "logs were: {logs}");
}

#[test]
fn detect_resolution_falls_back_when_xrandr_fails() {
    let (resolution, logs) = capture_logs(|| {
        super::detect_resolution_with(|| None, fake_xrandr("HDMI-1 connected 800x600+0+0\\n", 1))
    });

    // Output from a failed run is not trusted.
    assert_eq!(resolution, (1920, 1080));
    assert!(
        logs.contains("xrandr exited with status exit status: 1"),
        "logs were: {logs}"
    );
}

#[test]
fn detect_resolution_falls_back_when_xrandr_lists_no_monitor() {
    let (resolution, logs) = capture_logs(|| {
        super::detect_resolution_with(
            || None,
            fake_xrandr("Screen 0: no connected monitors\\n", 0),
        )
    });

    assert_eq!(resolution, (1920, 1080));
    assert!(
        logs.contains("Could not detect monitor resolution via xrandr, using fallback 1920x1080"),
        "logs were: {logs}"
    );
}

#[test]
fn gdk_resolution_is_unavailable_when_gtk_cannot_initialize() {
    // gtk::init panics when GTK already runs on another thread.
    let init_state = std::sync::OnceLock::new();
    assert_eq!(
        super::resolution_via_gdk(&init_state, || panic!("GTK runs on another thread")),
        None
    );
    assert_eq!(init_state.get(), Some(&false));

    // The failed attempt is remembered instead of retried.
    assert_eq!(super::resolution_via_gdk(&init_state, gtk::init), None);
}

#[gtk::test]
fn gdk_resolution_reads_the_default_display() {
    let init_state = std::sync::OnceLock::new();
    let resolution = super::resolution_via_gdk(&init_state, gtk::init);

    assert!(resolution.is_some_and(|(width, height)| width > 0 && height > 0));
    assert_eq!(init_state.get(), Some(&true));
}

#[test]
fn clap_parse_error_contains_no_ansi_codes() {
    let _guard = crate::test_env::lock();
    let previous = console::colors_enabled();
    console::set_colors_enabled(true);
    let mut output = Vec::new();
    let result = run_from_args_with_io(
        vec!["adventure-mods".to_string(), "--unknown-flag".to_string()],
        || Ok(()),
        &mut output,
        false,
    );
    console::set_colors_enabled(previous);
    let err_msg = result.unwrap_err().to_string();
    assert!(
        !err_msg.contains('\x1b'),
        "Error message contains ANSI codes: {err_msg:?}"
    );
}

#[test]
fn parse_xrandr_resolution_returns_primary_monitor() {
    let output = "\
Screen 0: minimum 16 x 16, current 4152 x 1920, maximum 32767 x 32767
DP-1 connected primary 3072x1728+1080+0 (normal left inverted right x axis y axis) 597mm x 336mm
   3072x1728    164.80*+
DP-2 connected 1920x1080+0+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080    60.00*+
";
    assert_eq!(parse_xrandr_resolution(output), Some((3072, 1728)));
}

#[test]
fn parse_xrandr_resolution_falls_back_to_first_connected_when_no_primary() {
    let output = "\
Screen 0: minimum 16 x 16, current 1920 x 1080, maximum 32767 x 32767
DP-1 connected 1920x1080+0+0 (normal left inverted right x axis y axis) 527mm x 296mm
   1920x1080    60.00*+
DP-2 connected 2560x1440+1920+0 (normal left inverted right x axis y axis) 597mm x 336mm
   2560x1440    144.00*+
";
    assert_eq!(parse_xrandr_resolution(output), Some((1920, 1080)));
}

#[test]
fn parse_xrandr_resolution_returns_none_on_empty_output() {
    assert_eq!(parse_xrandr_resolution(""), None);
}

#[test]
fn parse_xrandr_resolution_returns_none_when_no_connected_monitors() {
    let output = "\
Screen 0: minimum 16 x 16, current 0 x 0, maximum 32767 x 32767
HDMI-1 disconnected (normal left inverted right x axis y axis)
VGA-1 disconnected (normal left inverted right x axis y axis)
";
    assert_eq!(parse_xrandr_resolution(output), None);
}

#[test]
fn parse_xrandr_resolution_returns_none_on_malformed_geometry() {
    let output = "\
Screen 0: minimum 16 x 16, current 0 x 0, maximum 32767 x 32767
DP-1 connected primary (normal left inverted right x axis y axis) 597mm x 336mm
";
    assert_eq!(parse_xrandr_resolution(output), None);
}

/// Accepts output until `needle` has been written, then fails every write,
/// like a closed pipe partway through a report.
struct FailOnText {
    needle: String,
    written: Vec<u8>,
}

fn fail_on(needle: &str) -> FailOnText {
    FailOnText {
        needle: needle.to_string(),
        written: Vec::new(),
    }
}

impl Write for FailOnText {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.written.extend_from_slice(buf);
        if String::from_utf8_lossy(&self.written).contains(&self.needle) {
            return Err(std::io::Error::other("synthetic write failure"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn assert_write_failure(result: anyhow::Result<()>) {
    let error = result.expect_err("the failed write should end the command");
    assert!(
        error.to_string().contains("synthetic write failure"),
        "error was: {error:#}"
    );
}

/// Answers `select` with `select_result` when set and fails every other
/// prompt, as a terminal closed mid-prompt would.
struct ClosedTerminalPrompt {
    select_result: Option<usize>,
}

impl Prompt for ClosedTerminalPrompt {
    fn select(&self, _prompt: &str, _items: &[String], _default: usize) -> anyhow::Result<usize> {
        self.select_result
            .ok_or_else(|| anyhow::anyhow!("terminal closed during select"))
    }

    fn multi_select(
        &self,
        _prompt: &str,
        _items: &[String],
        _defaults: &[bool],
    ) -> anyhow::Result<Vec<usize>> {
        anyhow::bail!("terminal closed during multi-select")
    }

    fn confirm(&self, _prompt: &str, _default: bool) -> anyhow::Result<bool> {
        anyhow::bail!("terminal closed during confirm")
    }
}

/// A libraryfolders.vdf listing SA2 in an existing library and SADX in a
/// library that does not exist.
fn write_vdf_with_sa2_and_missing_sadx(root: &std::path::Path) -> PathBuf {
    let library = root.join("library");
    let game_dir = library.join("steamapps/common").join("Sonic Adventure 2");
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::File::create(game_dir.join("sonic2app.exe")).unwrap();
    let vdf_path = root.join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" \"apps\" {{ \"213610\" \"0\" }} }} \
             \"1\" {{ \"path\" \"{}\" \"apps\" {{ \"71250\" \"0\" }} }} }}",
            library.display(),
            root.join("missing-library").display()
        ),
    )
    .unwrap();
    vdf_path
}

fn detect_command(vdf_path: PathBuf) -> Cli {
    Cli {
        no_color: true,
        command: Some(Command::Detect(DetectArgs {
            libraryfolders_vdf: Some(vdf_path),
            steam_libraries: vec![],
        })),
    }
}

#[test]
fn crypto_provider_can_only_be_installed_once() {
    // Another test may already have installed it; afterwards it is installed.
    let _ = super::initialize_crypto_provider();

    let error = super::initialize_crypto_provider().unwrap_err();

    assert_eq!(
        error.to_string(),
        "failed to install TLS crypto provider: already installed"
    );
}

#[test]
fn detect_lists_inaccessible_libraries_without_detected_games() {
    let tmp = tempfile::tempdir().unwrap();
    let missing_library = tmp.path().join("missing-library");
    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" \"apps\" {{ \"71250\" \"0\" }} }} }}",
            missing_library.display()
        ),
    )
    .unwrap();

    let mut output = Vec::new();
    run_with_io(detect_command(vdf_path), false, &mut output).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("Detected games:"), "{output}");
    assert!(!output.contains("No supported games detected"), "{output}");
    assert!(
        output.contains(&format!(
            "Inaccessible Steam libraries:\n- Sonic Adventure DX: {}\n",
            crate::path_display::display_path(&missing_library)
        )),
        "{output}"
    );
}

#[test]
fn detect_stops_when_writing_a_game_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf_path = write_vdf_with_sa2_and_missing_sadx(tmp.path());

    let mut games = fail_on("- Sonic Adventure 2:");
    assert_write_failure(run_with_io(
        detect_command(vdf_path.clone()),
        false,
        &mut games,
    ));
    assert!(!String::from_utf8_lossy(&games.written).contains("Inaccessible"));

    let mut libraries = fail_on("- Sonic Adventure DX:");
    assert_write_failure(run_with_io(detect_command(vdf_path), false, &mut libraries));
}

fn list_mods_command(game: &str) -> Cli {
    Cli {
        no_color: true,
        command: Some(Command::ListMods {
            game: game.to_string(),
        }),
    }
}

#[test]
fn list_mods_shows_presets_only_for_games_that_have_them() {
    let mut sadx = Vec::new();
    run_with_io(list_mods_command("sadx"), false, &mut sadx).unwrap();
    let sadx = String::from_utf8(sadx).unwrap();
    let preset = &common::presets_for_game(GameKind::SADX)[0];
    assert!(sadx.contains("Presets:\n"), "{sadx}");
    assert!(
        sadx.contains(&format!("- {}: {}\n", preset.name, preset.description)),
        "{sadx}"
    );

    let mut sa2 = Vec::new();
    run_with_io(list_mods_command("SA2"), false, &mut sa2).unwrap();
    let sa2 = String::from_utf8(sa2).unwrap();
    assert!(common::presets_for_game(GameKind::SA2).is_empty());
    assert!(!sa2.contains("Presets:"), "{sa2}");
    assert!(sa2.contains("Game: Sonic Adventure 2\nMods:\n"), "{sa2}");
}

#[test]
fn list_mods_stops_when_writing_a_mod_fails() {
    let first = &common::recommended_mods_for_game(GameKind::SA2)[0];
    let mut output = fail_on(&format!("[{}]", first.slug));

    assert_write_failure(run_with_io(list_mods_command("sa2"), false, &mut output));
}

#[test]
fn setup_summary_stops_when_writing_fails() {
    let selected = vec![&common::recommended_mods_for_game(GameKind::SADX)[0]];
    let summary = super::SetupSummary {
        game_kind: GameKind::SADX,
        game_path: std::path::Path::new("/games/sadx"),
        selected_mods: &selected,
        width: 1920,
        height: 1080,
        language_selection: LanguageSelection::defaults_for(GameKind::SADX),
    };
    let prompt = MockPrompt {
        select_result: 0,
        multi_select_result: vec![],
        confirm_result: true,
    };

    for line in ["Path: ", "Subtitle language: ", "Voice language: "] {
        let mut writer = fail_on(line);
        let mut out = CliOutput::new(&mut writer, false);
        assert_write_failure(super::confirm_setup_summary(&mut out, &summary, &prompt));
    }
}

#[test]
fn setup_summary_surfaces_confirmation_errors() {
    let selected = vec![&common::recommended_mods_for_game(GameKind::SA2)[0]];
    let summary = super::SetupSummary {
        game_kind: GameKind::SA2,
        game_path: std::path::Path::new("/games/sa2"),
        selected_mods: &selected,
        width: 1280,
        height: 720,
        language_selection: LanguageSelection::defaults_for(GameKind::SA2),
    };
    let mut output = Vec::new();
    let mut out = CliOutput::new(&mut output, false);

    let error = super::confirm_setup_summary(
        &mut out,
        &summary,
        &ClosedTerminalPrompt {
            select_result: None,
        },
    )
    .unwrap_err();

    assert_eq!(error.to_string(), "terminal closed during confirm");
}

#[test]
fn rich_game_path_surfaces_installation_prompt_errors() {
    let detected = DetectionResult {
        games: vec![
            Game {
                kind: GameKind::SA2,
                path: PathBuf::from("/games/sa2-one"),
            },
            Game {
                kind: GameKind::SA2,
                path: PathBuf::from("/games/sa2-two"),
            },
        ],
        inaccessible: vec![],
    };
    let prompt = ClosedTerminalPrompt {
        select_result: None,
    };

    let error =
        resolve_game_path_rich(&empty_setup_args(), GameKind::SA2, Some(&detected), &prompt)
            .unwrap_err();

    assert_eq!(error.to_string(), "terminal closed during select");
}

#[test]
fn rich_mod_selection_surfaces_manual_selection_errors() {
    // SA2 has no presets: 0 installs all recommended mods, 1 picks manually.
    let prompt = ClosedTerminalPrompt {
        select_result: Some(1),
    };

    let Err(error) = resolve_setup_mods_rich(&empty_setup_args(), GameKind::SA2, &prompt) else {
        panic!("a closed terminal should end mod selection");
    };

    assert_eq!(error.to_string(), "terminal closed during multi-select");
}

#[test]
fn unknown_mod_slugs_point_to_the_matching_list_mods_command() {
    for (game_kind, arg) in [(GameKind::SADX, "sadx"), (GameKind::SA2, "sa2")] {
        let Err(error) = resolve_mods_flag(game_kind, "not-a-mod") else {
            panic!("unknown slugs should be rejected");
        };
        assert_eq!(
            error.to_string(),
            format!(
                "Unknown mod slug 'not-a-mod'. Use 'list-mods --game {arg}' to see valid slugs."
            )
        );
    }
}

#[test]
fn game_flag_selects_the_game_without_detection() {
    let mut args = empty_setup_args();
    args.game = Some(" SADX ".to_string());
    // Detection would fail on this file, so it must not run.
    args.detect.libraryfolders_vdf = Some(PathBuf::from("/definitely/missing/libraryfolders.vdf"));
    assert_eq!(super::resolve_game_kind(&args).unwrap(), GameKind::SADX);

    args.game = Some("sa3".to_string());
    assert_eq!(
        super::resolve_game_kind(&args).unwrap_err().to_string(),
        "Unknown game 'sa3'. Use 'sadx' or 'sa2'."
    );
}

#[test]
fn game_path_flag_is_validated_instead_of_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("Sonic Adventure 2");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::File::create(game.join("sonic2app.exe")).unwrap();
    let mut args = empty_setup_args();
    args.game_path = Some(game.clone());
    args.detect.libraryfolders_vdf = Some(PathBuf::from("/definitely/missing/libraryfolders.vdf"));

    assert_eq!(
        super::resolve_game_path(&args, GameKind::SA2).unwrap(),
        game
    );

    let error = super::resolve_game_path(&args, GameKind::SADX).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("does not appear to be a Sonic Adventure DX installation"),
        "{error}"
    );
}

#[test]
fn voice_language_keeps_the_saved_choice_without_flag_or_prompt() {
    with_test_settings(|settings| {
        settings
            .set_string("sa2-voice-language", "english")
            .unwrap();
        let mut args = empty_setup_args();
        args.subtitle_language = Some("german".to_string());

        let selection = resolve_setup_languages(&args, GameKind::SA2, None).unwrap();

        assert_eq!(selection.subtitle, SubtitleLanguage::German);
        assert_eq!(selection.voice, VoiceLanguage::English);
    });
}

fn plain_progress(interactive: bool) -> super::ProgressLines {
    super::ProgressLines::new(interactive, false, console::Style::new().dim())
}

#[test]
fn progress_lines_print_once_per_megabyte() {
    let mut lines = plain_progress(false);

    assert_eq!(
        lines.update("", 512, Some(3_145_728), super::download_progress_text),
        "  0.0 / 3.0 MB (0%)\n"
    );
    assert_eq!(
        lines.update(
            "",
            1_000_000,
            Some(3_145_728),
            super::download_progress_text
        ),
        ""
    );
    assert_eq!(
        lines.update(
            "",
            1_572_864,
            Some(3_145_728),
            super::download_progress_text
        ),
        "  1.5 / 3.0 MB (50%)\n"
    );
    // Other downloads are tracked separately.
    assert_eq!(
        lines.update("Other Mod", 1_572_864, None, super::mod_progress_text),
        "    Other Mod: 1.5 MB\n"
    );
    // Non-interactive output never rewrites a line, so nothing needs ending.
    assert_eq!(lines.end_line(), "");
}

#[test]
fn progress_lines_rewrite_one_line_on_a_terminal() {
    let mut lines = plain_progress(true);
    assert_eq!(lines.end_line(), "");

    assert_eq!(
        lines.update("Mod", 2_097_152, Some(4_194_304), super::mod_progress_text),
        "\r    Mod: 2.0 / 4.0 MB (50%)"
    );
    assert_eq!(lines.end_line(), "\n");
    assert_eq!(lines.end_line(), "");

    // A restarted download prints its first megabyte again.
    assert_eq!(
        lines.update("Mod", 2_097_152, None, super::mod_progress_text),
        ""
    );
    lines.reset("Mod");
    assert_eq!(
        lines.update("Mod", 2_097_152, None, super::mod_progress_text),
        "\r    Mod: 2.0 MB"
    );
}

#[test]
fn progress_lines_dim_the_text_when_color_is_enabled() {
    let dim = console::Style::new().dim().force_styling(true);
    let mut lines = super::ProgressLines::new(false, true, dim.clone());

    let text = lines.update("", 1_048_576, None, super::download_progress_text);

    assert_eq!(text, format!("{}\n", dim.apply_to("  1.0 MB downloaded")));
    assert!(text.contains('\x1b'), "{text:?}");
}
