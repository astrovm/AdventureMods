mod support;

use std::collections::HashMap;

use adventure_mods::external::runtime_installer;
use adventure_mods::setup::common::{self, ModEntry, ModSource};
use adventure_mods::setup::config::LanguageSelection;
use adventure_mods::setup::pipeline;
use adventure_mods::steam::game::GameKind;
use adventure_mods::steam::library::detect_games_from_vdf_with_extra_libraries;

use support::http_server::{Response, TestServer};
use support::steam_fixture::create_sa2_fixture;
use support::{EnvGuard, env_lock};

fn leak_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

const RENDER_FIX: ModEntry = ModEntry {
    name: "Render Fix",
    slug: "render-fix",
    source: ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 1,
    },
    description: "test mod",
    full_description: None,
    pictures: &[],
    dir_name: None,
    links: &[],
};

const TEST_FLAT: ModEntry = ModEntry {
    name: "Test Flat",
    slug: "test-flat",
    source: ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 2,
    },
    description: "test mod",
    full_description: None,
    pictures: &[],
    dir_name: Some("Test Flat"),
    links: &[],
};

const BROKEN_MOD: ModEntry = ModEntry {
    name: "Broken Mod",
    slug: "broken-mod",
    source: ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 9999,
    },
    description: "broken mod",
    full_description: None,
    pictures: &[],
    dir_name: Some("Broken Mod"),
    links: &[],
};

#[test]
fn sa2_setup_overlaps_mod_downloads() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    let first_url = leak_str(String::from("/files/render-fix.7z"));
    let second_url = leak_str(String::from("/files/test-flat.7z"));
    let direct_render_fix = ModEntry {
        name: "Render Fix Direct",
        slug: "render-fix-direct",
        source: ModSource::DirectUrl {
            url: leak_str(String::from("https://example.invalid/files/render-fix.7z")),
        },
        description: "test mod",
        full_description: None,
        pictures: &[],
        dir_name: None,
        links: &[],
    };
    let direct_test_flat = ModEntry {
        name: "Test Flat Direct",
        slug: "test-flat-direct",
        source: ModSource::DirectUrl {
            url: leak_str(String::from("https://example.invalid/files/test-flat.7z")),
        },
        description: "test mod",
        full_description: None,
        pictures: &[],
        dir_name: Some("Test Flat"),
        links: &[],
    };
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sa2-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sa2-loader",
            },
        ),
        (
            "/dotnet.exe",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dotnet-installer",
            },
        ),
        (
            first_url,
            Response::SlowOk {
                content_type: "application/octet-stream",
                body: "render-fix",
                delay_ms: 250,
            },
        ),
        (
            second_url,
            Response::SlowOk {
                content_type: "application/octet-stream",
                body: "test-flat",
                delay_ms: 250,
            },
        ),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            server.url("/sa2-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
        (
            "ADVENTURE_MODS_DIRECT_URL_BASE_OVERRIDE",
            server.url("/files/"),
        ),
    ]);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();
    pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&direct_render_fix, &direct_test_flat],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |_| Ok(()),
    )
    .unwrap();

    assert!(server.max_active_requests() >= 2);
}

#[test]
fn sa2_setup_completes_against_fake_steam_install() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sa2-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sa2-loader",
            },
        ),
        (
            "/dotnet.exe",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dotnet-installer",
            },
        ),
        (
            "/files/render-fix.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "render-fix",
            },
        ),
        (
            "/files/test-flat.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "test-flat",
            },
        ),
        ("/dl/1", Response::Redirect("/files/render-fix.7z")),
        ("/dl/2", Response::Redirect("/files/test-flat.7z")),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            server.url("/sa2-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            server.gamebanana_api_base(),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_DL_BASE",
            server.gamebanana_dl_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
    ]);

    let detection = detect_games_from_vdf_with_extra_libraries(&fixture.libraryfolders_vdf, &[]);
    assert_eq!(detection.games.len(), 1);
    assert_eq!(detection.games[0].kind, GameKind::SA2);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();
    pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX, &TEST_FLAT],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |_| Ok(()),
    )
    .unwrap();

    assert!(fixture.game_path.join("Launcher.exe.bak").is_file());
    assert!(
        fixture
            .game_path
            .join("mods/.modloader/SA2ModLoader.dll")
            .is_file()
    );
    assert!(
        fixture
            .game_path
            .join("resource/gd_PC/DLL/Win32/Data_DLL_orig.dll")
            .is_file()
    );
    assert!(fixture.game_path.join("SAManager/Manager.json").is_file());
    assert!(
        fixture
            .game_path
            .join("mods/.modloader/profiles/Default.json")
            .is_file()
    );
    assert!(fixture.game_path.join("Config/UserConfig.cfg").is_file());
    assert!(fixture.game_path.join("mods/Render Fix/mod.ini").is_file());
    assert!(fixture.game_path.join("mods/Test Flat/mod.ini").is_file());
    assert!(fixture.wine_log.is_file());
}

#[test]
fn sa2_setup_reports_progress_for_each_mod_and_config_generation() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sa2-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sa2-loader",
            },
        ),
        (
            "/dotnet.exe",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dotnet-installer",
            },
        ),
        (
            "/files/render-fix.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "render-fix",
            },
        ),
        (
            "/files/test-flat.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "test-flat",
            },
        ),
        ("/dl/1", Response::Redirect("/files/render-fix.7z")),
        ("/dl/2", Response::Redirect("/files/test-flat.7z")),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            server.url("/sa2-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            server.gamebanana_api_base(),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_DL_BASE",
            server.gamebanana_dl_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
    ]);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();

    let mut progress_events = Vec::new();
    pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX, &TEST_FLAT],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |event| {
            match event {
                pipeline::InstallProgress::Started { mod_name } => {
                    progress_events.push(format!("start:{mod_name}"))
                }
                pipeline::InstallProgress::DownloadingMod { .. } => {}
                pipeline::InstallProgress::Finished {
                    mod_name,
                    completed,
                    total,
                } => progress_events.push(format!("finish:{completed}/{total}:{mod_name}")),
                pipeline::InstallProgress::GeneratingConfig => {
                    progress_events.push("config".to_string())
                }
            }
            Ok(())
        },
    )
    .unwrap();

    let start_events: Vec<&str> = progress_events
        .iter()
        .filter_map(|event| event.strip_prefix("start:"))
        .collect();
    let finish_events: Vec<&String> = progress_events
        .iter()
        .filter(|event| event.starts_with("finish:"))
        .collect();

    assert_eq!(start_events.len(), 2);
    assert!(start_events.contains(&"Render Fix"));
    assert!(start_events.contains(&"Test Flat"));
    assert_eq!(finish_events.len(), 2);
    assert!(finish_events[0].starts_with("finish:1/2:"));
    assert!(finish_events[1].starts_with("finish:2/2:"));
    assert_eq!(progress_events.last().map(String::as_str), Some("config"));
}

#[test]
fn sa2_setup_can_rerun_on_existing_installation() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sa2-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sa2-loader",
            },
        ),
        (
            "/dotnet.exe",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dotnet-installer",
            },
        ),
        (
            "/files/render-fix.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "render-fix",
            },
        ),
        (
            "/files/test-flat.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "test-flat",
            },
        ),
        ("/dl/1", Response::Redirect("/files/render-fix.7z")),
        ("/dl/2", Response::Redirect("/files/test-flat.7z")),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            server.url("/sa2-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            server.gamebanana_api_base(),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_DL_BASE",
            server.gamebanana_dl_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
    ]);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();
    pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX, &TEST_FLAT],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |_| Ok(()),
    )
    .unwrap();

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();
    pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX, &TEST_FLAT],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |_| Ok(()),
    )
    .unwrap();

    assert!(fixture.game_path.join("Launcher.exe.bak").is_file());
    assert!(
        fixture
            .game_path
            .join("resource/gd_PC/DLL/Win32/Data_DLL_orig.dll")
            .is_file()
    );
    assert!(fixture.game_path.join("mods/Render Fix/mod.ini").is_file());
    assert!(fixture.game_path.join("mods/Test Flat/mod.ini").is_file());
}

#[test]
fn sa2_setup_generates_config_for_successful_mods_even_when_another_mod_fails() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sa2-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sa2-loader",
            },
        ),
        (
            "/dotnet.exe",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dotnet-installer",
            },
        ),
        (
            "/files/render-fix.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "render-fix",
            },
        ),
        ("/dl/1", Response::Redirect("/files/render-fix.7z")),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            server.url("/sa2-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            server.gamebanana_api_base(),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_DL_BASE",
            server.gamebanana_dl_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
    ]);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();

    let mut saw_config = false;
    let result = pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX, &BROKEN_MOD],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |event| {
            if matches!(event, pipeline::InstallProgress::GeneratingConfig) {
                saw_config = true;
            }
            Ok(())
        },
    );

    assert!(result.is_err());
    assert!(saw_config);
    assert!(fixture.game_path.join("mods/Render Fix/mod.ini").is_file());
    assert!(
        fixture
            .game_path
            .join("mods/.modloader/profiles/Default.json")
            .is_file()
    );
}

#[test]
fn sa2_setup_rejects_duplicate_install_targets_before_running() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sa2-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sa2-loader",
            },
        ),
        (
            "/dotnet.exe",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dotnet-installer",
            },
        ),
        (
            "/files/render-fix.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "render-fix",
            },
        ),
        ("/dl/1", Response::Redirect("/files/render-fix.7z")),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            server.url("/sa2-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            server.gamebanana_api_base(),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_DL_BASE",
            server.gamebanana_dl_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
    ]);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();

    let result = pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX, &RENDER_FIX],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |_| Ok(()),
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Duplicate mod install target")
    );
}

#[test]
fn sa2_setup_stops_when_download_progress_callback_errors() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sa2-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sa2-loader",
            },
        ),
        (
            "/dotnet.exe",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dotnet-installer",
            },
        ),
        (
            "/files/render-fix.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "render-fix",
            },
        ),
        ("/dl/1", Response::Redirect("/files/render-fix.7z")),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            server.url("/sa2-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            server.gamebanana_api_base(),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_DL_BASE",
            server.gamebanana_dl_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
    ]);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SA2.app_id()).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SA2, None).unwrap();

    let result = pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |event| match event {
            pipeline::InstallProgress::DownloadingMod { .. } => Err(anyhow::anyhow!("cancelled")),
            _ => Ok(()),
        },
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cancelled"));
}
