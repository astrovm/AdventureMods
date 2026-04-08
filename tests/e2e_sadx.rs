mod support;

use std::collections::HashMap;

use adventure_mods::external::runtime_installer;
use adventure_mods::setup::common::{self, ModEntry, ModSource};
use adventure_mods::setup::pipeline;
use adventure_mods::setup::sadx;
use adventure_mods::steam::game::GameKind;
use adventure_mods::steam::library::detect_games_from_vdf_with_extra_libraries;

use support::http_server::{Response, TestServer};
use support::steam_fixture::create_sadx_fixture;
use support::{env_lock, EnvGuard};

const DREAMCAST_TEST: ModEntry = ModEntry {
    name: "Dreamcast Test",
    source: ModSource::GameBanana { file_id: 1 },
    description: "test mod",
    full_description: None,
    pictures: &[],
    dir_name: None,
    links: &[],
};

const TEST_FLAT: ModEntry = ModEntry {
    name: "Test Flat",
    source: ModSource::GameBanana { file_id: 2 },
    description: "test mod",
    full_description: None,
    pictures: &[],
    dir_name: Some("Test Flat"),
    links: &[],
};

#[test]
fn sadx_setup_completes_against_fake_steam_install() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sadx_fixture();
    let server = TestServer::start(HashMap::from([
        (
            "/samodmanager.zip",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "samodmanager",
            },
        ),
        (
            "/sadx-loader.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "sadx-loader",
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
            "/steam_tools.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "steam-tools",
            },
        ),
        (
            "/files/dreamcast-test.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "dreamcast-test",
            },
        ),
        (
            "/files/test-flat.7z",
            Response::Ok {
                content_type: "application/octet-stream",
                body: "test-flat",
            },
        ),
        ("/dl/1", Response::Redirect("/files/dreamcast-test.7z")),
        ("/dl/2", Response::Redirect("/files/test-flat.7z")),
    ]));

    let _env = EnvGuard::set(&[
        (
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            server.url("/samodmanager.zip"),
        ),
        (
            "ADVENTURE_MODS_URL_SADX_MOD_LOADER",
            server.url("/sadx-loader.7z"),
        ),
        (
            "ADVENTURE_MODS_URL_DOTNET_DESKTOP_8",
            server.url("/dotnet.exe"),
        ),
        (
            "ADVENTURE_MODS_URL_SADX_STEAM_TOOLS",
            server.url("/steam_tools.7z"),
        ),
        (
            "ADVENTURE_MODS_GAMEBANANA_BASE_URL",
            server.gamebanana_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
        (
            "ADVENTURE_MODS_HPATCHZ",
            fixture.fake_hpatchz.display().to_string(),
        ),
    ]);

    let detection = detect_games_from_vdf_with_extra_libraries(&fixture.libraryfolders_vdf, &[]);
    assert_eq!(detection.games.len(), 1);
    assert_eq!(detection.games[0].kind, GameKind::SADX);

    runtime_installer::install_runtimes(&fixture.game_path, GameKind::SADX.app_id()).unwrap();
    sadx::convert_steam_to_2004(&fixture.game_path, None).unwrap();
    common::install_mod_manager(&fixture.game_path, GameKind::SADX, None).unwrap();
    pipeline::install_selected_mods_and_generate_config(
        &fixture.game_path,
        GameKind::SADX,
        &[&DREAMCAST_TEST, &TEST_FLAT],
        1920,
        1080,
    )
    .unwrap();

    assert!(fixture
        .prefix_path
        .join("drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App")
        .is_dir());
    assert!(fixture.game_path.join("sonic.exe").is_file());
    assert!(fixture
        .game_path
        .join("Sonic Adventure DX.exe.bak")
        .is_file());
    assert!(fixture
        .game_path
        .join("mods/.modloader/SADXModLoader.dll")
        .is_file());
    assert!(fixture
        .game_path
        .join("system/CHRMODELS_orig.dll")
        .is_file());
    assert!(fixture.game_path.join("SAManager/Manager.json").is_file());
    assert!(fixture
        .game_path
        .join("mods/.modloader/profiles/Default.json")
        .is_file());
    assert!(fixture.game_path.join("system/sonicDX.ini").is_file());
    assert!(fixture
        .game_path
        .join("mods/Dreamcast Test/mod.ini")
        .is_file());
    assert!(fixture.game_path.join("mods/Test Flat/mod.ini").is_file());
    assert!(fixture.game_path.join("SoundData/voice_jp/wma").is_dir());
    assert!(fixture.wine_log.is_file());
}
