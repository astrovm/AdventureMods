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

const RENDER_FIX: ModEntry = ModEntry {
    name: "Render Fix",
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
                pipeline::InstallProgress::InstallingMod {
                    index,
                    total,
                    mod_name,
                } => progress_events.push(format!("{index}/{total}:{mod_name}")),
                pipeline::InstallProgress::DownloadingMod { .. } => {}
                pipeline::InstallProgress::GeneratingConfig => {
                    progress_events.push("config".to_string())
                }
            }
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(
        progress_events,
        vec!["1/2:Render Fix", "2/2:Test Flat", "config"]
    );
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
fn sa2_setup_does_not_emit_config_progress_after_mod_failure() {
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

    let mut progress_events = Vec::new();
    let result = pipeline::install_selected_mods_and_generate_config_with_progress(
        &fixture.game_path,
        GameKind::SA2,
        &[&RENDER_FIX, &BROKEN_MOD],
        1920,
        1080,
        LanguageSelection::defaults_for(GameKind::SA2),
        |event| {
            match event {
                pipeline::InstallProgress::InstallingMod {
                    index,
                    total,
                    mod_name,
                } => progress_events.push(format!("{index}/{total}:{mod_name}")),
                pipeline::InstallProgress::DownloadingMod { .. } => {}
                pipeline::InstallProgress::GeneratingConfig => {
                    progress_events.push("config".to_string())
                }
            }
            Ok(())
        },
    );

    assert!(result.is_err());
    assert_eq!(progress_events, vec!["1/2:Render Fix", "2/2:Broken Mod"]);
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
