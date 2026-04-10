mod support;

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

use adventure_mods::cli::{run_with_io, Cli};
use adventure_mods::setup::common::ModSource;
use adventure_mods::setup::{sa2, sadx};
use clap::Parser;

use support::http_server::{Response, TestServer};
use support::scripts;
use support::steam_fixture::{create_sa2_fixture, create_sadx_fixture};
use support::{env_lock, EnvGuard};

fn leak_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn add_fake_mod_archive(
    extract_root: &std::path::Path,
    key: &str,
    dir_name: Option<&str>,
    fallback_name: &str,
) {
    let root = extract_root.join(key);
    let top_level = root.join(dir_name.unwrap_or(fallback_name));
    std::fs::create_dir_all(&top_level).unwrap();
    std::fs::write(top_level.join("mod.ini"), format!("Name={fallback_name}")).unwrap();
    std::fs::write(top_level.join("asset.txt"), key).unwrap();
}

fn seed_sa2_all_mod_routes(
    routes: &mut HashMap<&'static str, Response>,
    extract_root: &std::path::Path,
) {
    for (index, mod_entry) in sa2::RECOMMENDED_MODS.iter().enumerate() {
        let key = format!("sa2-all-{index}");
        add_fake_mod_archive(extract_root, &key, mod_entry.dir_name, mod_entry.name);

        let file_path = leak_str(format!("/files/{key}.7z"));
        let body = leak_str(key.clone());
        routes.insert(
            file_path,
            Response::Ok {
                content_type: "application/octet-stream",
                body,
            },
        );

        let ModSource::GameBanana { file_id } = mod_entry.source else {
            panic!(
                "SA2 recommended mod '{}' is not GameBanana-backed",
                mod_entry.name
            );
        };
        let dl_path = leak_str(format!("/dl/{file_id}"));
        routes.insert(dl_path, Response::Redirect(file_path));
    }
}

fn seed_sadx_preset_routes(
    routes: &mut HashMap<&'static str, Response>,
    extract_root: &std::path::Path,
    preset_name: &str,
) {
    let preset = sadx::PRESETS
        .iter()
        .find(|preset| preset.name == preset_name)
        .unwrap();

    for mod_name in preset.mod_names {
        let mod_entry = sadx::RECOMMENDED_MODS
            .iter()
            .find(|entry| entry.name == *mod_name)
            .unwrap();
        let key = format!("sadx-{}", mod_name.to_lowercase().replace([' ', ':'], "-"));
        add_fake_mod_archive(extract_root, &key, mod_entry.dir_name, mod_entry.name);

        match mod_entry.source {
            ModSource::GameBanana { file_id } => {
                let file_path = leak_str(format!("/files/{key}.7z"));
                let body = leak_str(key.clone());
                routes.insert(
                    file_path,
                    Response::Ok {
                        content_type: "application/octet-stream",
                        body,
                    },
                );
                let dl_path = leak_str(format!("/dl/{file_id}"));
                routes.insert(dl_path, Response::Redirect(file_path));
            }
            ModSource::DirectUrl { url } => {
                let filename = url.rsplit('/').next().unwrap();
                let dcmods_path = leak_str(format!("/dcmods/{filename}"));
                let body = leak_str(key.clone());
                routes.insert(
                    dcmods_path,
                    Response::Ok {
                        content_type: "application/octet-stream",
                        body,
                    },
                );
            }
        }
    }
}

#[test]
fn detect_rejects_malformed_explicit_vdf() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(&vdf_path, "not valid vdf").unwrap();

    let cli = Cli::parse_from([
        "adventure-mods",
        "detect",
        "--libraryfolders-vdf",
        vdf_path.to_str().unwrap(),
    ]);
    let mut output = Vec::new();

    let result = run_with_io(cli, false, &mut output);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to parse"));
}

#[test]
fn detect_reports_inaccessible_libraries_from_explicit_vdf() {
    let fixture = create_sadx_fixture();
    let tmp = tempfile::tempdir().unwrap();
    let missing_library = tmp.path().join("MissingSteamLibrary");
    let vdf_path = tmp.path().join("libraryfolders.vdf");

    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n        \"apps\"\n        {{\n            \"71250\"\t\"0\"\n        }}\n    }}\n    \"1\"\n    {{\n        \"path\"\t\"{}\"\n        \"apps\"\n        {{\n            \"71250\"\t\"0\"\n        }}\n    }}\n}}\n",
            fixture
                .libraryfolders_vdf
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .display(),
            missing_library.display(),
        ),
    )
    .unwrap();

    let cli = Cli::parse_from([
        "adventure-mods",
        "detect",
        "--libraryfolders-vdf",
        vdf_path.to_str().unwrap(),
    ]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut output).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Detected games:"));
    assert!(output.contains("Inaccessible Steam libraries:"));
    assert!(output.contains(missing_library.to_str().unwrap()));
}

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

    run_with_io(cli, false, &mut output).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Sonic Adventure DX"));
    assert!(output.contains(fixture.game_path.to_str().unwrap()));
}

#[test]
fn list_mods_reports_presets_and_mods() {
    let cli = Cli::parse_from(["adventure-mods", "list-mods", "--game", "sadx"]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut output).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("DX Enhanced"));
    assert!(output.contains("Dreamcast Restoration"));
    assert!(output.contains("Dreamcast Conversion"));
    assert!(output.contains("dreamcast-conversion"));
}

#[test]
fn setup_rejects_unknown_preset() {
    let fixture = create_sadx_fixture();
    let cli = Cli::parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sadx",
        "--game-path",
        fixture.game_path.to_str().unwrap(),
        "--preset",
        "Nope",
    ]);
    let mut output = Vec::new();

    let result = run_with_io(cli, false, &mut output);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown preset"));
}

#[test]
fn setup_accepts_human_readable_mod_names_with_whitespace() {
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
        " SA2 Render Fix , hd gui: sa2 edition ",
    ]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut output).unwrap();

    assert!(fixture
        .game_path
        .join("mods/sa2-render-fix/mod.ini")
        .is_file());
    assert!(fixture
        .game_path
        .join("mods/HD GUI for SA2/mod.ini")
        .is_file());
}

#[test]
fn setup_installs_all_recommended_sa2_mods_from_cli_flag() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();

    let mut routes = HashMap::from([
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
    ]);
    seed_sa2_all_mod_routes(&mut routes, &fixture.extract_root);
    let server = TestServer::start(routes);

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
        "--all-mods",
    ]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut output).unwrap();

    for mod_entry in sa2::RECOMMENDED_MODS {
        let dir = mod_entry.dir_name.unwrap_or(mod_entry.name);
        assert!(fixture
            .game_path
            .join("mods")
            .join(dir)
            .join("mod.ini")
            .is_file());
    }
}

#[test]
fn setup_installs_sadx_preset_from_cli_flag() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sadx_fixture();

    let mut routes = HashMap::from([
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
    ]);
    seed_sadx_preset_routes(&mut routes, &fixture.extract_root, "DX Enhanced");
    let server = TestServer::start(routes);

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
        ("ADVENTURE_MODS_DCMODS_BASE_URL", server.url("/dcmods/")),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
        (
            "ADVENTURE_MODS_HPATCHZ",
            fixture.fake_hpatchz.display().to_string(),
        ),
    ]);

    let cli = Cli::parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sadx",
        "--game-path",
        fixture.game_path.to_str().unwrap(),
        "--preset",
        "DX Enhanced",
    ]);
    let mut output = Vec::new();

    run_with_io(cli, false, &mut output).unwrap();

    let preset = sadx::PRESETS
        .iter()
        .find(|preset| preset.name == "DX Enhanced")
        .unwrap();
    for mod_name in preset.mod_names {
        let mod_entry = sadx::RECOMMENDED_MODS
            .iter()
            .find(|entry| entry.name == *mod_name)
            .unwrap();
        let dir = mod_entry.dir_name.unwrap_or(mod_entry.name);
        assert!(fixture
            .game_path
            .join("mods")
            .join(dir)
            .join("mod.ini")
            .is_file());
    }
}

#[test]
fn setup_surfaces_mod_download_failures() {
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
        "sa2-render-fix",
    ]);
    let mut output = Vec::new();

    let result = run_with_io(cli, false, &mut output);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTP error 404"));
}

#[test]
fn setup_surfaces_archive_extraction_failures() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();
    scripts::write_script(&fixture.fake_7zz, "#!/bin/sh\nexit 1\n");

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
        ("/dl/1656654", Response::Redirect("/files/render-fix.7z")),
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
        "sa2-render-fix",
    ]);
    let mut output = Vec::new();

    let result = run_with_io(cli, false, &mut output);

    assert!(result.is_err());
    let message = result.unwrap_err().to_string();
    assert!(message.contains("Failed to extract") || message.contains("extract"));
}

#[test]
fn interactive_setup_can_be_cancelled_via_tty() {
    let script = match Command::new("script").arg("--version").output() {
        Ok(_) => "script",
        Err(_) => return,
    };

    let fixture = create_sa2_fixture();
    let binary = env!("CARGO_BIN_EXE_adventure-mods");
    let command = format!(
        "\"{}\" setup --game sa2 --game-path \"{}\"",
        binary,
        fixture.game_path.display()
    );

    let mut child = Command::new(script)
        .args(["-qfec", &command, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.take().unwrap().write_all(b"\nn\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("Choose setup mode") || stderr.contains("Choose setup mode"));
    assert!(stdout.contains("Setup cancelled") || stderr.contains("Setup cancelled"));
}

#[test]
fn interactive_sa2_setup_completes_via_tty() {
    let script = match Command::new("script").arg("--version").output() {
        Ok(_) => "script",
        Err(_) => return,
    };

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sa2_fixture();

    let mut routes = HashMap::from([
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
    ]);
    seed_sa2_all_mod_routes(&mut routes, &fixture.extract_root);
    let server = TestServer::start(routes);

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

    let binary = env!("CARGO_BIN_EXE_adventure-mods");
    let command = format!(
        "\"{}\" setup --game sa2 --game-path \"{}\"",
        binary,
        fixture.game_path.display()
    );

    let mut child = Command::new(script)
        .args(["-qfec", &command, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.take().unwrap().write_all(b"\ny\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(fixture
        .game_path
        .join("mods/sa2-render-fix/mod.ini")
        .is_file());
    assert!(fixture
        .game_path
        .join("mods/HD GUI for SA2/mod.ini")
        .is_file());
}

#[test]
fn interactive_sadx_preset_setup_completes_via_tty() {
    let script = match Command::new("script").arg("--version").output() {
        Ok(_) => "script",
        Err(_) => return,
    };

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _env_lock = env_lock();
    let fixture = create_sadx_fixture();

    let mut routes = HashMap::from([
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
    ]);
    seed_sadx_preset_routes(&mut routes, &fixture.extract_root, "DX Enhanced");
    let server = TestServer::start(routes);

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
        ("ADVENTURE_MODS_DCMODS_BASE_URL", server.url("/dcmods/")),
        ("ADVENTURE_MODS_7ZZ", fixture.fake_7zz.display().to_string()),
        (
            "ADVENTURE_MODS_HPATCHZ",
            fixture.fake_hpatchz.display().to_string(),
        ),
    ]);

    let binary = env!("CARGO_BIN_EXE_adventure-mods");
    let command = format!(
        "\"{}\" setup --game sadx --game-path \"{}\"",
        binary,
        fixture.game_path.display()
    );

    let mut child = Command::new(script)
        .args(["-qfec", &command, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.take().unwrap().write_all(b"\ny\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(fixture
        .game_path
        .join("mods/DreamcastConversion/mod.ini")
        .is_file());
    assert!(fixture.game_path.join("mods/SADXFE/mod.ini").is_file());
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

    run_with_io(cli, false, &mut output).unwrap();

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

#[test]
fn setup_bails_without_mod_selection_in_noninteractive_mode() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let fixture = create_sa2_fixture();
    let cli = Cli::parse_from([
        "adventure-mods",
        "setup",
        "--game",
        "sa2",
        "--game-path",
        fixture.game_path.to_str().unwrap(),
    ]);
    let mut output = Vec::new();

    let result = run_with_io(cli, false, &mut output);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("--all-mods"));
    assert!(error.to_string().contains("--preset"));
    assert!(error.to_string().contains("--mods"));
}

#[test]
fn setup_bails_without_game_in_noninteractive_mode() {
    let fixture = create_sa2_fixture();
    let cli = Cli::parse_from([
        "adventure-mods",
        "setup",
        "--all-mods",
        "--game-path",
        fixture.game_path.to_str().unwrap(),
    ]);
    let mut output = Vec::new();

    let result = run_with_io(cli, false, &mut output);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("--game"));
}
