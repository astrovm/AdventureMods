mod support;

use std::collections::HashMap;

use adventure_mods::cli::{run_with_io, Cli};
use clap::Parser;

use support::http_server::{Response, TestServer};
use support::steam_fixture::{create_sa2_fixture, create_sadx_fixture};
use support::{env_lock, EnvGuard};

#[test]
fn detect_reports_games_from_explicit_vdf() {
    let fixture = create_sadx_fixture();
    let cli = Cli::parse_from([
        "adventure-mods",
        "detect",
        "--libraryfolders-vdf",
        fixture.libraryfolders_vdf.to_str().unwrap(),
    ]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut std::io::empty(), &mut output).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Sonic Adventure DX"));
    assert!(output.contains(fixture.game_path.to_str().unwrap()));
}

#[test]
fn list_mods_reports_presets_and_mods() {
    let cli = Cli::parse_from(["adventure-mods", "list-mods", "--game", "sadx"]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut std::io::empty(), &mut output).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("DX Enhanced"));
    assert!(output.contains("Dreamcast Restoration"));
    assert!(output.contains("Dreamcast Conversion"));
    assert!(output.contains("dreamcast-conversion"));
}

#[test]
fn setup_installs_selected_mods_from_cli_flags() {
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
        ("/dl/1656654", Response::Redirect("/files/render-fix.7z")),
        ("/dl/409120", Response::Redirect("/files/test-flat.7z")),
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
            "ADVENTURE_MODS_GAMEBANANA_BASE_URL",
            server.gamebanana_base(),
        ),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
    ]);

    let cli = Cli::parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sa2",
        "--game-path",
        fixture.game_path.to_str().unwrap(),
        "--mods",
        "sa2-render-fix,hd-gui-sa2-edition",
        "--width",
        "1920",
        "--height",
        "1080",
    ]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut std::io::empty(), &mut output).unwrap();

    let output = String::from_utf8(output).unwrap();

    assert!(fixture.game_path.join("Launcher.exe.bak").is_file());
    assert!(fixture
        .game_path
        .join("mods/.modloader/SA2ModLoader.dll")
        .is_file());
    assert!(fixture
        .game_path
        .join("mods/sa2-render-fix/mod.ini")
        .is_file());
    assert!(fixture
        .game_path
        .join("mods/HD GUI for SA2/mod.ini")
        .is_file());
    assert!(output.contains("Step 1/3: Install .NET Runtime\nDone\n"));
    assert!(output.contains("Step 2/3: Install Mod Manager & Loader\nDone\n"));
    assert!(output.contains("Step 3/3: Install Mods & Generate Config"));
    assert!(output.contains("Installing mod 1/2: SA2 Render Fix"));
    assert!(output.contains("Generating mod config"));
}
