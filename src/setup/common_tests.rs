use super::*;

#[test]
fn gamebanana_item_dl_base_override() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    // Bind to a random port, then serve one fake GameBanana API response.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let body = r#"[{"999":{"_idRow":999}}]"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(response.as_bytes());
    });

    let api_base = format!(
        "http://127.0.0.1:{port}/gbapi?fields=Files().aFiles()",
        port = port
    );
    let dl_base = "http://127.0.0.1:9999/custom-dl/";

    unsafe {
        std::env::set_var("ADVENTURE_MODS_GAMEBANANA_API_BASE", &api_base);
        std::env::set_var("ADVENTURE_MODS_GAMEBANANA_DL_BASE", dl_base);
    }

    let source = ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 12345,
    };
    let result = resolve_download_url(&source).unwrap();

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_DL_BASE");
    }

    assert_eq!(result, "http://127.0.0.1:9999/custom-dl/999");
}

#[test]
fn gamebanana_item_reports_malformed_and_empty_responses() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let bodies = ["not-json".to_string(), "[]".to_string(), "[{}]".to_string()];
    let server = std::thread::spawn(move || {
        for body in bodies {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });

    let api_base = format!("http://127.0.0.1:{port}/gbapi?fields=Files().aFiles()");
    unsafe {
        std::env::set_var("ADVENTURE_MODS_GAMEBANANA_API_BASE", &api_base);
    }

    let malformed = resolve_download_url(&ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 1,
    })
    .unwrap_err();
    assert!(
        malformed
            .to_string()
            .contains("Failed to parse GameBanana API response")
    );

    let empty = resolve_download_url(&ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 2,
    })
    .unwrap_err();
    assert!(empty.to_string().contains("Empty GameBanana API response"));

    let no_files = resolve_download_url(&ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 3,
    })
    .unwrap_err();
    assert!(
        no_files
            .to_string()
            .contains("No files found in GameBanana API response")
    );

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
    }
    server.join().unwrap();
}

#[test]
fn resolve_direct_url() {
    let source = ModSource::DirectUrl {
        url: "https://example.com/mod.7z",
    };
    assert_eq!(
        resolve_download_url(&source).unwrap(),
        "https://example.com/mod.7z"
    );
}

#[test]
fn resolve_direct_url_rewrites_sadx_base_when_overridden() {
    let _guard = crate::test_env::lock();
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_DCMODS_BASE_URL",
            "http://127.0.0.1:4010/dcmods/",
        );
    }

    let source = ModSource::DirectUrl {
        url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z",
    };

    assert_eq!(
        resolve_download_url(&source).unwrap(),
        "http://127.0.0.1:4010/dcmods/DreamcastConversion.7z"
    );

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_DCMODS_BASE_URL");
    }
}

#[test]
fn sa_mod_manager_url_valid() {
    assert!(SA_MOD_MANAGER_URL.starts_with("https://github.com/"));
    assert!(SA_MOD_MANAGER_URL.contains("/releases/"));
    assert!(SA_MOD_MANAGER_URL.ends_with(".zip"));
}

#[test]
fn sa_mod_manager_url_uses_override() {
    let _guard = crate::test_env::lock();
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            "http://127.0.0.1:4010/samodmanager.zip",
        );
    }

    assert_eq!(
        sa_mod_manager_url(),
        "http://127.0.0.1:4010/samodmanager.zip"
    );

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_URL_SA_MOD_MANAGER");
    }
}

#[test]
fn mod_loader_url_uses_override() {
    let _guard = crate::test_env::lock();
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
            "http://127.0.0.1:4010/sa2-loader.7z",
        );
    }

    assert_eq!(
        mod_loader_url(GameKind::SA2),
        "http://127.0.0.1:4010/sa2-loader.7z"
    );

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_URL_SA2_MOD_LOADER");
    }
}

#[test]
fn install_mod_dir_construction() {
    let game_path = std::path::Path::new("/fake/game/dir");
    let mods_dir = game_path.join("mods");
    assert!(mods_dir.ends_with("mods"));
    assert_eq!(mods_dir, std::path::PathBuf::from("/fake/game/dir/mods"));
}

#[test]
fn move_dir_contents_flat_to_subdir() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("mod.ini"), b"[mod]").unwrap();
    std::fs::write(src.join("data.bin"), b"data").unwrap();

    move_dir_contents(&src, &dest).unwrap();

    assert!(dest.join("mod.ini").is_file());
    assert!(dest.join("data.bin").is_file());
}

#[test]
fn move_dir_contents_merges_existing_and_renames_new_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(src.join("existing")).unwrap();
    std::fs::create_dir_all(src.join("new/nested")).unwrap();
    std::fs::write(src.join("existing/updated.txt"), "new").unwrap();
    std::fs::write(src.join("new/nested/file.txt"), "moved").unwrap();
    std::fs::create_dir_all(dest.join("existing")).unwrap();
    std::fs::write(dest.join("existing/kept.txt"), "kept").unwrap();
    std::fs::write(dest.join("existing/updated.txt"), "old").unwrap();

    move_dir_contents(&src, &dest).unwrap();

    assert_eq!(
        std::fs::read_to_string(dest.join("existing/kept.txt")).unwrap(),
        "kept"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("existing/updated.txt")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("new/nested/file.txt")).unwrap(),
        "moved"
    );
    assert!(!src.join("new").exists());
}

#[test]
fn staging_tempdir_prefers_the_target_filesystem() {
    let tmp = tempfile::tempdir().unwrap();

    let staged = staging_tempdir(tmp.path()).unwrap();
    assert_eq!(staged.path().parent(), Some(tmp.path()));
    assert!(
        staged
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".adventure-mods-")
    );

    let capture = crate::test_log::LogCapture::start();
    let missing = tmp.path().join("missing");
    let fallback = staging_tempdir(&missing).unwrap();
    assert!(fallback.path().is_dir());
    assert_ne!(fallback.path().parent(), Some(tmp.path()));
    assert!(
        capture
            .contents()
            .contains(&format!("Could not stage in {}", missing.display())),
        "{}",
        capture.contents()
    );
}

#[test]
fn find_mod_root_at_staging_root() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("mod.ini"), b"[mod]").unwrap();

    let root = find_mod_root(&staging).unwrap();
    assert_eq!(root, staging);
}

#[test]
fn find_mod_root_one_level_deep() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let sub = staging.join("MyMod");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("mod.ini"), b"[mod]").unwrap();

    let root = find_mod_root(&staging).unwrap();
    assert_eq!(root, sub);
}

#[test]
fn find_mod_root_two_levels_deep() {
    // e.g. archive extracts as mods/SteamAchievements/mod.ini
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let nested = staging.join("mods").join("SteamAchievements");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("mod.ini"), b"[mod]").unwrap();

    let root = find_mod_root(&staging).unwrap();
    assert_eq!(root, nested);
}

#[test]
fn find_mod_root_none_when_missing() {
    // Archive with no mod.ini at all (e.g. icondata)
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("icon.ico"), b"icon").unwrap();

    assert!(find_mod_root(&staging).is_none());
}

#[test]
fn install_mod_flat_archive_with_dir_name() {
    // mod.ini at root, dir_name set → goes to mods/<dir_name>/
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("mod.ini"), b"[mod]").unwrap();
    std::fs::write(staging.join("texture.png"), b"img").unwrap();

    let mods_dir = tmp.path().join("mods");
    std::fs::create_dir_all(&mods_dir).unwrap();

    let dir_name = "TestMod";
    let dest = mods_dir.join(dir_name);
    let content_root = find_mod_root(&staging).unwrap_or(staging.clone());
    move_dir_contents(&content_root, &dest).unwrap();

    assert!(!mods_dir.join("mod.ini").exists());
    assert!(mods_dir.join("TestMod").join("mod.ini").is_file());
    assert!(mods_dir.join("TestMod").join("texture.png").is_file());
}

#[test]
fn install_mod_nested_archive_with_dir_name() {
    // Archive has mods/SteamAchievements/mod.ini, dir_name = "SteamAchievements"
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let nested = staging.join("mods").join("SteamAchievements");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("mod.ini"), b"[mod]").unwrap();
    std::fs::write(nested.join("data.dll"), b"dll").unwrap();

    let mods_dir = tmp.path().join("game_mods");
    std::fs::create_dir_all(&mods_dir).unwrap();

    let dir_name = "SteamAchievements";
    let dest = mods_dir.join(dir_name);
    let content_root = find_mod_root(&staging).unwrap_or(staging.clone());
    move_dir_contents(&content_root, &dest).unwrap();

    assert!(mods_dir.join("SteamAchievements").join("mod.ini").is_file());
    assert!(
        mods_dir
            .join("SteamAchievements")
            .join("data.dll")
            .is_file()
    );
    // No stray nested directories
    assert!(!mods_dir.join("mods").exists());
}

#[test]
fn install_mod_no_mod_ini_with_dir_name() {
    // Archive has loose files and no mod.ini (e.g. icondata)
    // Falls back to staging root
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("icon.ico"), b"icon").unwrap();
    std::fs::write(staging.join("other.ico"), b"other").unwrap();

    let mods_dir = tmp.path().join("mods");
    std::fs::create_dir_all(&mods_dir).unwrap();

    let dir_name = "icondata";
    let dest = mods_dir.join(dir_name);
    let content_root = find_mod_root(&staging).unwrap_or(staging.clone());
    move_dir_contents(&content_root, &dest).unwrap();

    assert!(mods_dir.join("icondata").join("icon.ico").is_file());
    assert!(mods_dir.join("icondata").join("other.ico").is_file());
}

#[test]
fn install_mod_no_dir_name_passthrough() {
    // dir_name is None: archive extracts directly into mods/
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let sub = staging.join("SomeMod");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("mod.ini"), b"[mod]").unwrap();

    let mods_dir = tmp.path().join("mods");
    std::fs::create_dir_all(&mods_dir).unwrap();

    // No dir_name → move directly
    move_dir_contents(&staging, &mods_dir).unwrap();

    assert!(mods_dir.join("SomeMod").join("mod.ini").is_file());
}

#[test]
fn install_passthrough_mod_rejects_flat_archive() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let mods_dir = tmp.path().join("mods");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::create_dir_all(&mods_dir).unwrap();
    std::fs::write(staging.join("mod.ini"), b"[mod]").unwrap();

    let err = install_passthrough_mod(&staging, &mods_dir).unwrap_err();
    assert!(err.to_string().contains("single top-level mod directory"));
    assert!(!mods_dir.join("mod.ini").exists());
}

#[test]
fn install_passthrough_mod_preserves_existing_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let mods_dir = tmp.path().join("mods");
    let extracted = staging.join("SomeMod");
    let existing = mods_dir.join("SomeMod");
    std::fs::create_dir_all(&extracted).unwrap();
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(extracted.join("mod.ini"), b"[new]").unwrap();
    std::fs::write(existing.join("mod.ini"), b"[old]").unwrap();

    let capture = crate::test_log::LogCapture::start();
    install_passthrough_mod(&staging, &mods_dir).unwrap();

    assert!(capture.contents().contains(&format!(
        "Mod directory '{}' already exists, skipping install",
        existing.display()
    )));
    assert_eq!(std::fs::read(existing.join("mod.ini")).unwrap(), b"[old]");
    assert!(extracted.join("mod.ini").is_file());
}

#[test]
fn install_passthrough_mod_replaces_incomplete_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let mods_dir = tmp.path().join("mods");
    let extracted = staging.join("SomeMod");
    let existing = mods_dir.join("SomeMod");
    std::fs::create_dir_all(&extracted).unwrap();
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(extracted.join("mod.ini"), b"[new]").unwrap();
    std::fs::write(existing.join("old.txt"), b"old").unwrap();

    let capture = crate::test_log::LogCapture::start();
    install_passthrough_mod(&staging, &mods_dir).unwrap();

    assert!(capture.contents().contains(&format!(
        "Mod directory '{}' exists but is incomplete, reinstalling",
        existing.display()
    )));

    assert!(existing.join("mod.ini").is_file());
    assert!(!existing.join("old.txt").exists());
}

#[test]
fn normalize_mod_version_rewrites_stale_packaged_value() {
    let tmp = tempfile::tempdir().unwrap();
    let mod_dir = tmp.path().join("Better Tails AI");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
            mod_dir.join("mod.ini"),
            b"Name=Better Tails AI\nGitHubRepo=Sora-yx/SADX-Better-Tails-AI\nGitHubAsset=Better.Tails.AI.zip\n",
        )
        .unwrap();
    std::fs::write(mod_dir.join("mod.version"), b"04/26/2021 22:44:24\n").unwrap();

    normalize_mod_version(&mod_dir).unwrap();

    let rewritten = std::fs::read_to_string(mod_dir.join("mod.version")).unwrap();
    assert_ne!(rewritten.trim(), "04/26/2021 22:44:24");
}

#[test]
fn normalize_mod_version_creates_file_for_update_tracked_mod() {
    let tmp = tempfile::tempdir().unwrap();
    let mod_dir = tmp.path().join("Fancy Mod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
        mod_dir.join("mod.ini"),
        b"Name=Fancy Mod\nGameBananaItemType=Mod\nGameBananaItemId=12345\n",
    )
    .unwrap();

    normalize_mod_version(&mod_dir).unwrap();

    assert!(mod_dir.join("mod.version").is_file());
}

#[test]
fn normalize_mod_version_ignores_plain_mods() {
    let tmp = tempfile::tempdir().unwrap();
    let mod_dir = tmp.path().join("Plain Mod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("mod.ini"), b"Name=Plain Mod\nVersion=1.0\n").unwrap();

    normalize_mod_version(&mod_dir).unwrap();

    assert!(!mod_dir.join("mod.version").exists());
}

#[test]
fn normalize_mod_version_ignores_directories_without_metadata_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mod_dir = tmp.path().join("Incomplete Mod");
    std::fs::create_dir_all(&mod_dir).unwrap();

    normalize_mod_version(&mod_dir).unwrap();
    assert!(!mod_dir.join("mod.version").exists());
}

#[test]
fn update_metadata_ignores_malformed_lines() {
    assert!(!has_update_metadata(
        "not metadata\nGameBananaItemType=Mod\n"
    ));
}

#[test]
fn install_mod_normalizes_existing_update_tracked_mod_on_rerun() {
    let tmp = tempfile::tempdir().unwrap();
    let game_path = tmp.path();
    let mod_dir = game_path.join("mods/BetterTailsAI");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
            mod_dir.join("mod.ini"),
            b"Name=Better Tails AI\nGitHubRepo=Sora-yx/SADX-Better-Tails-AI\nGitHubAsset=Better.Tails.AI.zip\n",
        )
        .unwrap();
    std::fs::write(mod_dir.join("mod.version"), b"04/26/2021 22:44:24\n").unwrap();

    let mod_entry = ModEntry {
        name: "Better Tails AI",
        slug: "better-tails-ai",
        source: ModSource::DirectUrl {
            url: "https://example.com/better-tails-ai.zip",
        },
        description: "A test mod",
        full_description: None,
        pictures: &[],
        dir_name: Some("BetterTailsAI"),
        links: &[],
    };

    install_mod_with_progress(game_path, &mod_entry, None).unwrap();

    let rewritten = std::fs::read_to_string(mod_dir.join("mod.version")).unwrap();
    assert_ne!(rewritten.trim(), "04/26/2021 22:44:24");
}

#[test]
fn find_mod_root_prefers_deterministic_order() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let b_dir = staging.join("b_mod");
    let a_dir = staging.join("a_mod");
    std::fs::create_dir_all(&b_dir).unwrap();
    std::fs::create_dir_all(&a_dir).unwrap();
    std::fs::write(b_dir.join("mod.ini"), b"[mod]").unwrap();
    std::fs::write(a_dir.join("mod.ini"), b"[mod]").unwrap();

    let root = find_mod_root(&staging).unwrap();
    assert_eq!(root, a_dir);
}

#[test]
fn move_dir_contents_overwrites_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(src.join("shared.txt"), b"new").unwrap();
    std::fs::write(dest.join("shared.txt"), b"old").unwrap();

    move_dir_contents(&src, &dest).unwrap();
    assert_eq!(std::fs::read(dest.join("shared.txt")).unwrap(), b"new");
    assert!(!src.join("shared.txt").exists());
}

#[test]
fn move_dir_contents_replaces_file_with_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(src.join("nested")).unwrap();
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(dest.join("nested"), b"old file").unwrap();
    std::fs::write(src.join("nested/new.txt"), b"new file").unwrap();

    move_dir_contents(&src, &dest).unwrap();

    assert_eq!(
        std::fs::read(dest.join("nested/new.txt")).unwrap(),
        b"new file"
    );
}

/// Helper: run the Steam exe replacement from `install_mod_manager` with a
/// freshly copied `SAModManager.exe` in the game dir.
fn run_exe_replacement(game_path: &std::path::Path) {
    let dest_exe = game_path.join("SAModManager.exe");
    std::fs::write(&dest_exe, b"mod_manager_content").unwrap();

    install_as_steam_launcher(game_path, &dest_exe).unwrap();
}

#[test]
fn exe_replacement_reports_a_manager_that_cannot_be_moved_into_place() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    std::fs::write(game_path.join("Launcher.exe"), b"original_launcher").unwrap();

    let err =
        install_as_steam_launcher(game_path, &game_path.join("SAModManager.exe")).unwrap_err();

    assert_eq!(
        err.to_string(),
        format!(
            "Failed to install mod manager as {}",
            game_path.join("Launcher.exe").display()
        )
    );
    // The original stays backed up for a later attempt.
    assert_eq!(
        std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
        b"original_launcher"
    );
}

#[test]
fn exe_replacement_sa2_launcher() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    std::fs::write(game_path.join("Launcher.exe"), b"original_launcher").unwrap();

    run_exe_replacement(game_path);

    // Launcher.exe should now contain the mod manager
    assert_eq!(
        std::fs::read(game_path.join("Launcher.exe")).unwrap(),
        b"mod_manager_content"
    );
    // Original backed up
    assert_eq!(
        std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
        b"original_launcher"
    );
    // SAModManager.exe should have been renamed away
    assert!(!game_path.join("SAModManager.exe").exists());
}

#[test]
fn exe_replacement_sadx() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    std::fs::write(game_path.join("Sonic Adventure DX.exe"), b"original_sadx").unwrap();

    run_exe_replacement(game_path);

    // "Sonic Adventure DX.exe" should now contain the mod manager
    assert_eq!(
        std::fs::read(game_path.join("Sonic Adventure DX.exe")).unwrap(),
        b"mod_manager_content"
    );
    // Original backed up
    assert_eq!(
        std::fs::read(game_path.join("Sonic Adventure DX.exe.bak")).unwrap(),
        b"original_sadx"
    );
    assert!(!game_path.join("SAModManager.exe").exists());
}

#[test]
fn exe_replacement_sadx_backup_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    // Simulate a prior backup already existing
    std::fs::write(
        game_path.join("Sonic Adventure DX.exe.bak"),
        b"first_backup",
    )
    .unwrap();
    std::fs::write(
        game_path.join("Sonic Adventure DX.exe"),
        b"already_replaced",
    )
    .unwrap();

    run_exe_replacement(game_path);

    // The original backup should be preserved (not overwritten)
    assert_eq!(
        std::fs::read(game_path.join("Sonic Adventure DX.exe.bak")).unwrap(),
        b"first_backup"
    );
}

#[test]
fn exe_replacement_sa2_backup_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    std::fs::write(game_path.join("Launcher.exe.bak"), b"first_backup").unwrap();
    std::fs::write(game_path.join("Launcher.exe"), b"already_replaced").unwrap();

    run_exe_replacement(game_path);

    assert_eq!(
        std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
        b"first_backup"
    );
}

#[test]
fn exe_replacement_no_steam_exe() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    // No Launcher.exe or Sonic Adventure DX.exe: mod manager stays as-is

    run_exe_replacement(game_path);

    // SAModManager.exe should remain in place
    assert_eq!(
        std::fs::read(game_path.join("SAModManager.exe")).unwrap(),
        b"mod_manager_content"
    );
    assert!(!game_path.join("Launcher.exe").exists());
    assert!(!game_path.join("Sonic Adventure DX.exe").exists());
}

#[test]
fn exe_replacement_launcher_takes_priority_over_sadx() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    // Both exist: Launcher.exe should win (SA2 path)
    std::fs::write(game_path.join("Launcher.exe"), b"launcher").unwrap();
    std::fs::write(game_path.join("Sonic Adventure DX.exe"), b"sadx").unwrap();

    run_exe_replacement(game_path);

    // Launcher.exe replaced
    assert_eq!(
        std::fs::read(game_path.join("Launcher.exe")).unwrap(),
        b"mod_manager_content"
    );
    assert_eq!(
        std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
        b"launcher"
    );
    // SADX exe untouched
    assert_eq!(
        std::fs::read(game_path.join("Sonic Adventure DX.exe")).unwrap(),
        b"sadx"
    );
}

#[test]
fn recommended_mods_for_game_returns_correct_lists() {
    let sadx_mods = recommended_mods_for_game(GameKind::SADX);
    let sa2_mods = recommended_mods_for_game(GameKind::SA2);
    assert!(!sadx_mods.is_empty());
    assert!(!sa2_mods.is_empty());
    assert_ne!(sadx_mods.len(), sa2_mods.len());
}

#[test]
fn find_mod_root_three_levels_deep_returns_none() {
    // find_mod_root only searches two levels deep; three levels should return None
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let deep = staging.join("a").join("b").join("DeepMod");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("mod.ini"), b"[mod]").unwrap();

    assert!(find_mod_root(&staging).is_none());
}

#[test]
fn move_dir_contents_empty_source() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("empty_src");
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(&src).unwrap();

    move_dir_contents(&src, &dest).unwrap();
    assert!(dest.is_dir());
    assert!(std::fs::read_dir(&dest).unwrap().next().is_none());
}

#[test]
fn move_dir_contents_nested_subdirectory() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let subdir = src.join("sub");
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(&subdir).unwrap();
    std::fs::write(src.join("top.txt"), b"top").unwrap();
    std::fs::write(subdir.join("nested.txt"), b"nested").unwrap();

    move_dir_contents(&src, &dest).unwrap();

    assert!(dest.join("top.txt").is_file());
    assert!(dest.join("sub").join("nested.txt").is_file());
}

#[test]
fn find_file_icase_finds_uppercase_variant() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("CHRMODELS.DLL"), b"").unwrap();

    let found = find_file_icase(tmp.path(), "chrmodels.dll");
    assert!(found.is_some());
    assert!(found.unwrap().ends_with("CHRMODELS.DLL"));
}

#[test]
fn find_file_icase_missing_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(find_file_icase(tmp.path(), "nonexistent.dll").is_none());
}

#[test]
fn find_file_icase_nonexistent_dir_returns_none() {
    let path = std::path::Path::new("/nonexistent/path/that/does/not/exist");
    assert!(find_file_icase(path, "anything.dll").is_none());
}

#[test]
fn install_loader_dll_sadx_uses_lowercase_system_data_dir() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();
    let uppercase_system = game_path.join("System");
    let lowercase_system = game_path.join("system");
    let modloader_dir = game_path.join("mods/.modloader");

    std::fs::create_dir_all(&uppercase_system).unwrap();
    std::fs::create_dir_all(&lowercase_system).unwrap();
    std::fs::create_dir_all(&modloader_dir).unwrap();
    std::fs::write(uppercase_system.join("sonicDX.ini"), b"ini").unwrap();
    std::fs::write(lowercase_system.join("CHRMODELS.dll"), b"original_dll").unwrap();
    std::fs::write(modloader_dir.join("SADXModLoader.dll"), b"mod_loader").unwrap();

    install_loader_dll(game_path, GameKind::SADX).unwrap();

    assert_eq!(
        std::fs::read(lowercase_system.join("CHRMODELS_orig.dll")).unwrap(),
        b"original_dll"
    );
    assert_eq!(
        std::fs::read(lowercase_system.join("CHRMODELS.dll")).unwrap(),
        b"mod_loader"
    );
}

#[test]
fn is_mod_manager_fully_installed_requires_dll_swap() {
    let dir = tempfile::tempdir().unwrap();
    let game_path = dir.path();

    std::fs::create_dir_all(game_path.join("mods/.modloader")).unwrap();
    std::fs::write(
        game_path.join("mods/.modloader/SADXModLoader.dll"),
        b"mod_loader",
    )
    .unwrap();
    std::fs::write(game_path.join("Sonic Adventure DX.exe.bak"), b"backup").unwrap();
    std::fs::create_dir_all(game_path.join("system")).unwrap();
    std::fs::write(game_path.join("system/CHRMODELS.dll"), b"original_dll").unwrap();

    assert!(!is_mod_manager_fully_installed(game_path, GameKind::SADX));
}

#[test]
fn mod_entry_dir_name_fallback_to_name() {
    // When dir_name is None, the name field is used as the directory name
    let mod_entry = ModEntry {
        name: "MyMod",
        slug: "my-mod",
        source: ModSource::DirectUrl {
            url: "https://example.com/mod.7z",
        },
        description: "A test mod",
        full_description: None,
        pictures: &[],
        dir_name: None,
        links: &[],
    };
    let dir_name = mod_entry.dir_name.unwrap_or(mod_entry.name);
    assert_eq!(dir_name, "MyMod");
}

#[test]
fn mod_entry_explicit_dir_name() {
    let mod_entry = ModEntry {
        name: "Display Name",
        slug: "display-name",
        source: ModSource::DirectUrl {
            url: "https://example.com/mod.7z",
        },
        description: "A test mod",
        full_description: None,
        pictures: &[],
        dir_name: Some("FolderName"),
        links: &[],
    };
    let dir_name = mod_entry.dir_name.unwrap_or(mod_entry.name);
    assert_eq!(dir_name, "FolderName");
}

#[test]
fn step_completion_detects_conversion_and_manager_markers() {
    let dir = tempfile::tempdir().unwrap();
    let game = Game {
        kind: GameKind::SADX,
        path: dir.path().to_path_buf(),
    };

    assert!(!is_step_complete(
        StepId::Dotnet,
        &Game {
            path: "/game".into(),
            ..game.clone()
        }
    ));
    assert!(!is_step_complete(StepId::SelectMods, &game));

    std::fs::create_dir_all(dir.path().join("system")).unwrap();
    std::fs::write(dir.path().join("system/CHRMODELS_orig.dll"), b"orig").unwrap();
    assert!(is_step_complete(StepId::ConvertSteam, &game));

    std::fs::remove_file(dir.path().join("system/CHRMODELS_orig.dll")).unwrap();
    std::fs::write(dir.path().join("SADXModLoader.dll"), b"loader").unwrap();
    assert!(is_step_complete(StepId::ConvertSteam, &game));
    // A restore waiting for a Steam repair converts again despite leftovers.
    std::fs::write(dir.path().join(".adventure-mods-steam-repair"), b"").unwrap();
    assert!(!is_step_complete(StepId::ConvertSteam, &game));
    std::fs::remove_file(dir.path().join(".adventure-mods-steam-repair")).unwrap();

    std::fs::remove_file(dir.path().join("SADXModLoader.dll")).unwrap();
    std::fs::create_dir_all(dir.path().join("mods/.modloader")).unwrap();
    std::fs::write(
        dir.path().join("mods/.modloader/SADXModLoader.dll"),
        b"loader",
    )
    .unwrap();
    assert!(is_step_complete(StepId::ConvertSteam, &game));

    std::fs::remove_file(dir.path().join("mods/.modloader/SADXModLoader.dll")).unwrap();
    std::fs::write(dir.path().join("sonic.exe"), b"game").unwrap();
    assert!(is_step_complete(StepId::ConvertSteam, &game));

    std::fs::write(dir.path().join("Sonic Adventure DX.exe.bak"), b"backup").unwrap();
    std::fs::write(
        dir.path().join("mods/.modloader/SADXModLoader.dll"),
        b"loader",
    )
    .unwrap();
    std::fs::write(dir.path().join("system/CHRMODELS_orig.dll"), b"orig").unwrap();
    assert!(is_mod_manager_fully_installed(dir.path(), GameKind::SADX));
    // Always runs, so the manager and loader are checked for updates.
    assert!(!is_step_complete(StepId::InstallModManager, &game));

    let sa2_dir = tempfile::tempdir().unwrap();
    let sa2 = Game {
        kind: GameKind::SA2,
        path: sa2_dir.path().to_path_buf(),
    };
    std::fs::write(sa2_dir.path().join("Launcher.exe.bak"), b"backup").unwrap();
    std::fs::create_dir_all(sa2_dir.path().join("mods/.modloader")).unwrap();
    std::fs::write(
        sa2_dir.path().join("mods/.modloader/SA2ModLoader.dll"),
        b"loader",
    )
    .unwrap();
    std::fs::create_dir_all(sa2_dir.path().join("resource/gd_PC/DLL/Win32")).unwrap();
    std::fs::write(
        sa2_dir
            .path()
            .join("resource/gd_PC/DLL/Win32/Data_DLL_orig.dll"),
        b"orig",
    )
    .unwrap();
    assert!(is_mod_manager_fully_installed(
        sa2_dir.path(),
        GameKind::SA2
    ));
    assert!(!is_step_complete(StepId::InstallModManager, &sa2));
}

#[test]
fn dotnet_step_runs_even_when_the_runtimes_are_installed() {
    let tmp = tempfile::tempdir().unwrap();
    let steam_root = tmp.path();
    let game_path = steam_root.join("steamapps/common/Sonic Adventure DX");
    let proton_dir = steam_root.join("steamapps/common/Proton 10.0");
    let compatdata = steam_root.join("steamapps/compatdata/71250");
    std::fs::create_dir_all(&game_path).unwrap();
    std::fs::create_dir_all(proton_dir.join("files/bin")).unwrap();
    std::fs::write(proton_dir.join("files/bin/wine64"), b"").unwrap();
    std::fs::create_dir_all(
        compatdata
            .join("pfx/drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App/10.0.0"),
    )
    .unwrap();
    std::fs::create_dir_all(
        compatdata
            .join("pfx/drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App/8.0.0"),
    )
    .unwrap();
    std::fs::write(compatdata.join("version"), "10.1000-105\n").unwrap();
    std::fs::write(
        compatdata.join("config_info"),
        format!("{}\n", proton_dir.join("files").display()),
    )
    .unwrap();
    std::fs::create_dir_all(steam_root.join("config")).unwrap();
    std::fs::write(
        steam_root.join("config/config.vdf"),
        r#""InstallConfigStore"
{
    "Software" { "Valve" { "Steam" { "CompatToolMapping" {
        "71250" { "name" "proton_10" }
    } } } }
}"#,
    )
    .unwrap();

    let game = Game {
        kind: GameKind::SADX,
        path: game_path,
    };
    // Even with both runtimes present the step runs, to install newer patches.
    assert!(!is_step_complete(StepId::Dotnet, &game));
}

#[test]
fn install_loader_refreshes_existing_and_reports_missing_loader() {
    let sadx_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(sadx_dir.path().join("system")).unwrap();
    std::fs::create_dir_all(sadx_dir.path().join("mods/.modloader")).unwrap();
    std::fs::write(sadx_dir.path().join("system/CHRMODELS_orig.dll"), b"orig").unwrap();
    std::fs::write(sadx_dir.path().join("system/CHRMODELS.dll"), b"old").unwrap();
    std::fs::write(
        sadx_dir.path().join("mods/.modloader/SADXModLoader.dll"),
        b"new",
    )
    .unwrap();
    install_loader_dll(sadx_dir.path(), GameKind::SADX).unwrap();
    assert_eq!(
        std::fs::read(sadx_dir.path().join("system/CHRMODELS.dll")).unwrap(),
        b"new"
    );

    let sa2_dir = tempfile::tempdir().unwrap();
    let dll_dir = sa2_dir.path().join("resource/gd_PC/DLL/Win32");
    std::fs::create_dir_all(&dll_dir).unwrap();
    std::fs::create_dir_all(sa2_dir.path().join("mods/.modloader")).unwrap();
    std::fs::write(dll_dir.join("Data_DLL.dll"), b"data").unwrap();
    std::fs::write(
        sa2_dir.path().join("mods/.modloader/SA2ModLoader.dll"),
        b"loader",
    )
    .unwrap();
    install_loader_dll(sa2_dir.path(), GameKind::SA2).unwrap();
    assert!(dll_dir.join("Data_DLL_orig.dll").is_file());

    let missing = tempfile::tempdir().unwrap();
    let error = install_loader_dll(missing.path(), GameKind::SADX).unwrap_err();
    assert!(error.to_string().contains("Mod loader DLL not found"));

    let no_data = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(no_data.path().join("mods/.modloader")).unwrap();
    std::fs::write(
        no_data.path().join("mods/.modloader/SADXModLoader.dll"),
        b"loader",
    )
    .unwrap();
    install_loader_dll(no_data.path(), GameKind::SADX).unwrap();
}

#[test]
fn install_mod_wrapper_and_update_url_metadata() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let dir = tempfile::tempdir().unwrap();
    let mod_dir = dir.path().join("mods/TestMod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
        mod_dir.join("mod.ini"),
        b"Name=Test\nUpdateUrl=https://example.test\n",
    )
    .unwrap();

    let mod_entry = ModEntry {
        name: "Test Mod",
        slug: "test-mod",
        source: ModSource::DirectUrl {
            url: "https://example.test/test.zip",
        },
        description: "test",
        full_description: None,
        pictures: &[],
        dir_name: Some("TestMod"),
        links: &[],
    };
    let updates = std::sync::Arc::new(AtomicUsize::new(0));
    let updates_clone = updates.clone();
    install_mod(
        dir.path(),
        &mod_entry,
        Some(Box::new(move |_, _| {
            updates_clone.fetch_add(1, Ordering::Relaxed);
        })),
    )
    .unwrap();
    assert_eq!(updates.load(Ordering::Relaxed), 0);
    assert!(mod_dir.join("mod.version").is_file());
}

#[test]
fn move_dir_contents_cross_filesystem_fallback_when_available() {
    let Ok(dest_root) = tempfile::tempdir_in("/dev/shm") else {
        return;
    };
    let source_root = tempfile::tempdir().unwrap();
    let source = source_root.path().join("src");
    let dest = dest_root.path().join("dest");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("cross-device.txt"), b"copied").unwrap();

    move_dir_contents(&source, &dest).unwrap();

    assert_eq!(
        std::fs::read(dest.join("cross-device.txt")).unwrap(),
        b"copied"
    );
}
fn update_test_mod(url: &'static str) -> ModEntry {
    ModEntry {
        name: "Update Mod",
        slug: "update-mod",
        source: ModSource::DirectUrl { url },
        description: "test",
        full_description: None,
        pictures: &[],
        dir_name: Some("UpdateMod"),
        links: &[],
    }
}

/// A fake 7zz that "extracts" whatever text the archive holds into mod.ini.
fn install_echo_7zz(dir: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let fake_7zz = dir.join("fake-7zz");
    std::fs::write(
        &fake_7zz,
        r##"#!/bin/sh
dest=""
archive=""
for arg in "$@"; do
    case "$arg" in
        -o*) dest="${arg#-o}" ;;
        x|-y) ;;
        *) archive="$arg" ;;
    esac
done
if grep -q broken "$archive"; then exit 2; fi
mkdir -p "$dest/UpdateMod"
cp "$archive" "$dest/UpdateMod/mod.ini"
printf 'defaults' > "$dest/UpdateMod/config.ini"
"##,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&fake_7zz).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_7zz, permissions).unwrap();
    fake_7zz
}

#[test]
fn manager_and_loader_update_only_when_their_release_changes() {
    use crate::external::test_http::{Reply, serve};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    // Each release has its own ETag; HEAD can be made to fail like a flaky server.
    let release = std::sync::Arc::new(AtomicUsize::new(1));
    let head_fails = std::sync::Arc::new(AtomicBool::new(false));
    let (base, log) = serve({
        let release = release.clone();
        let head_fails = head_fails.clone();
        move |request| {
            if request.method == "HEAD" && head_fails.load(Ordering::SeqCst) {
                return Reply::ok("").status("500 Internal Server Error");
            }
            let version = release.load(Ordering::SeqCst);
            Reply::ok(format!("release {version}")).header("ETag", format!("\"v{version}\""))
        }
    });

    let dir = tempfile::tempdir().unwrap();
    let game = dir.path();
    std::fs::create_dir_all(game.join("system")).unwrap();
    std::fs::write(game.join("system/CHRMODELS.dll"), b"original").unwrap();
    std::fs::write(game.join("Sonic Adventure DX.exe"), b"steam launcher").unwrap();

    // Extracts the downloaded release text into the manager or loader file.
    let fake_7zz = dir.path().join("fake-7zz");
    std::fs::write(
        &fake_7zz,
        r##"#!/bin/sh
dest=""
archive=""
for arg in "$@"; do
    case "$arg" in
        -o*) dest="${arg#-o}" ;;
        x|-y) ;;
        *) archive="$arg" ;;
    esac
done
mkdir -p "$dest"
case "$dest" in
    */extracted) cp "$archive" "$dest/SAModManager.exe" ;;
    *) cp "$archive" "$dest/SADXModLoader.dll" ;;
esac
"##,
    )
    .unwrap();
    std::fs::set_permissions(&fake_7zz, std::fs::Permissions::from_mode(0o755)).unwrap();
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var(
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            format!("{base}/manager"),
        );
        std::env::set_var(
            "ADVENTURE_MODS_URL_SADX_MOD_LOADER",
            format!("{base}/loader"),
        );
    }
    let downloads = || {
        log.lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with("GET"))
            .count()
    };
    let read = |path: &str| std::fs::read_to_string(game.join(path)).unwrap();

    // First install puts the manager in place of the launcher and adds the loader.
    install_mod_manager(game, GameKind::SADX, Some(Box::new(|_, _| {}))).unwrap();
    assert_eq!(downloads(), 2);
    assert_eq!(read("Sonic Adventure DX.exe"), "release 1");
    assert_eq!(read("Sonic Adventure DX.exe.bak"), "steam launcher");
    assert_eq!(read("mods/.modloader/SADXModLoader.dll"), "release 1");
    assert_eq!(read("system/CHRMODELS.dll"), "release 1");

    // Nothing changed upstream: nothing is downloaded again.
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 2);

    // A new release is installed; the launcher backup stays the original.
    release.store(2, Ordering::SeqCst);
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 4);
    assert_eq!(read("Sonic Adventure DX.exe"), "release 2");
    assert_eq!(read("Sonic Adventure DX.exe.bak"), "steam launcher");
    assert_eq!(read("mods/.modloader/SADXModLoader.dll"), "release 2");
    assert_eq!(read("system/CHRMODELS.dll"), "release 2");

    // When the server cannot say, the installed copies are kept.
    release.store(3, Ordering::SeqCst);
    head_fails.store(true, Ordering::SeqCst);
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 4);

    // Installs from before versions were recorded are kept while offline...
    std::fs::remove_file(game.join(".adventure-mods-manager-source")).unwrap();
    std::fs::remove_file(game.join("mods/.modloader/.adventure-mods-source")).unwrap();
    head_fails.store(true, Ordering::SeqCst);
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 4);
    assert_eq!(read("Sonic Adventure DX.exe"), "release 2");
    head_fails.store(false, Ordering::SeqCst);

    // ...and updated once the source can be reached.
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 6);
    assert_eq!(read("Sonic Adventure DX.exe"), "release 3");
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 6);

    // A version tag that could not be read at install is checked again later.
    for record in [
        ".adventure-mods-manager-source",
        "mods/.modloader/.adventure-mods-source",
    ] {
        let url = if record.contains("manager") {
            "manager"
        } else {
            "loader"
        };
        std::fs::write(game.join(record), format!("url={base}/{url}\nvalidator=\n")).unwrap();
    }
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 8);
    install_mod_manager(game, GameKind::SADX, None).unwrap();
    assert_eq!(downloads(), 8);

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_URL_SA_MOD_MANAGER");
        std::env::remove_var("ADVENTURE_MODS_URL_SADX_MOD_LOADER");
    }
}

#[test]
fn install_mod_updates_changed_files_and_keeps_user_config() {
    use crate::external::test_http::{Reply, serve};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    // The served file changes version once `version` is bumped.
    let version = Arc::new(AtomicUsize::new(1));
    let served = version.clone();
    let (base, log) = serve(move |_| {
        let v = served.load(Ordering::SeqCst);
        Reply::ok(format!("Name=Update Mod v{v}")).header("ETag", format!("\"v{v}\""))
    });
    let url: &'static str = Box::leak(format!("{base}/update.7z").into_boxed_str());

    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    let fake_7zz = install_echo_7zz(tmp.path());
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }
    let mod_entry = update_test_mod(url);
    let mod_dir = game.join("mods/UpdateMod");

    install_mod_with_progress(&game, &mod_entry, None).unwrap();
    assert_eq!(
        std::fs::read_to_string(mod_dir.join("mod.ini")).unwrap(),
        "Name=Update Mod v1"
    );
    let record = ModSourceRecord::read(&mod_dir).unwrap();
    assert_eq!(record.url, url);
    assert_eq!(record.validator.as_deref(), Some("\"v1\""));

    // Same version: nothing is downloaded again.
    std::fs::write(mod_dir.join("config.ini"), "user settings").unwrap();
    std::fs::write(mod_dir.join("old-only.dll"), "stale").unwrap();
    let requests_before = log.lock().unwrap().len();
    install_mod_with_progress(&game, &mod_entry, None).unwrap();
    let new_requests: Vec<_> = log.lock().unwrap()[requests_before..].to_vec();
    assert_eq!(new_requests, vec!["HEAD /update.7z".to_string()]);

    // New version: replaced, stale files gone, user config kept.
    version.store(2, Ordering::SeqCst);
    install_mod_with_progress(&game, &mod_entry, None).unwrap();
    assert_eq!(
        std::fs::read_to_string(mod_dir.join("mod.ini")).unwrap(),
        "Name=Update Mod v2"
    );
    assert_eq!(
        std::fs::read_to_string(mod_dir.join("config.ini")).unwrap(),
        "user settings"
    );
    assert!(!mod_dir.join("old-only.dll").exists());
    assert_eq!(
        ModSourceRecord::read(&mod_dir)
            .unwrap()
            .validator
            .as_deref(),
        Some("\"v2\"")
    );
    // The archive is removed from the cache once installed.
    assert_eq!(
        std::fs::read_dir(tmp.path().join("cache/downloads"))
            .unwrap()
            .count(),
        0
    );

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
}

#[test]
fn prefetched_archives_install_without_downloading_again() {
    use crate::external::test_http::{Reply, serve};
    use std::sync::atomic::AtomicBool;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    let (base, log) = serve(|_| Reply::ok("Name=Prefetched"));
    let url: &'static str = Box::leak(format!("{base}/prefetch.7z").into_boxed_str());

    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    let fake_7zz = install_echo_7zz(tmp.path());
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }
    let mod_entry = update_test_mod(url);
    let gets = || {
        log.lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with("GET"))
            .count()
    };

    // A cancelled prefetch fetches nothing.
    crate::setup::pipeline::prefetch_mod_archives(&game, &[&mod_entry], &AtomicBool::new(true));
    assert_eq!(gets(), 0);

    // Prefetching downloads the archive but does not install it.
    let running = AtomicBool::new(false);
    crate::setup::pipeline::prefetch_mod_archives(&game, &[&mod_entry], &running);
    assert!(cached_archive_path(url).is_file());
    assert!(!game.join("mods/UpdateMod").exists());
    assert_eq!(gets(), 1);

    // The install extracts the prefetched archive without a second download.
    install_mod_with_progress(&game, &mod_entry, None).unwrap();
    assert_eq!(gets(), 1);
    assert_eq!(
        std::fs::read_to_string(game.join("mods/UpdateMod/mod.ini")).unwrap(),
        "Name=Prefetched"
    );
    assert!(!cached_archive_path(url).exists());

    // Installed and current mods are not fetched again.
    crate::setup::pipeline::prefetch_mod_archives(&game, &[&mod_entry], &running);
    assert_eq!(gets(), 1);
    assert!(!cached_archive_path(url).exists());

    // A mod that cannot be fetched only logs; the install reports it later.
    let broken = update_test_mod("http://127.0.0.1:9/unreachable.7z");
    let mut broken = broken;
    broken.dir_name = Some("BrokenMod");
    crate::setup::pipeline::prefetch_mod_archives(&game, &[&broken], &running);

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
}

#[test]
fn install_uses_the_prefetched_archive_without_asking_gamebanana_again() {
    use crate::external::test_http::{Reply, serve};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    // The API answers once, then fails, as if rate limited.
    let api_calls = std::sync::Arc::new(AtomicUsize::new(0));
    let calls = api_calls.clone();
    let (base, log) = serve(move |request| {
        if request.path.starts_with("/gbapi") {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Reply::ok(r#"[{"777":{"_idRow":777}}]"#)
            } else {
                Reply::ok("rate limited").status("429 Too Many Requests")
            }
        } else {
            Reply::ok("Name=FromGameBanana")
        }
    });

    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    let fake_7zz = install_echo_7zz(tmp.path());
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
        std::env::set_var(
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            format!("{base}/gbapi?fields=Files().aFiles()"),
        );
        std::env::set_var("ADVENTURE_MODS_GAMEBANANA_DL_BASE", format!("{base}/dl/"));
    }
    let mod_entry = ModEntry {
        name: "Banana Mod",
        slug: "banana-mod",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 4242,
        },
        description: "test",
        full_description: None,
        pictures: &[],
        dir_name: Some("BananaMod"),
        links: &[],
    };

    crate::setup::pipeline::prefetch_mod_archives(&game, &[&mod_entry], &AtomicBool::new(false));
    assert_eq!(api_calls.load(Ordering::SeqCst), 1);

    install_mod_with_progress(&game, &mod_entry, None).unwrap();
    assert_eq!(api_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read_to_string(game.join("mods/BananaMod/mod.ini")).unwrap(),
        "Name=FromGameBanana"
    );
    let downloads = log
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.starts_with("GET") && request.contains("/dl/"))
        .count();
    assert_eq!(downloads, 1);

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_DL_BASE");
    }
}

#[test]
fn install_mod_reuses_cached_archive_and_drops_broken_ones() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    let (base, log) = serve(|_| Reply::ok("broken"));
    let url: &'static str = Box::leak(format!("{base}/cached.7z").into_boxed_str());

    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    let fake_7zz = install_echo_7zz(tmp.path());
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }
    let mod_entry = update_test_mod(url);

    // A cached archive from an earlier attempt is used without downloading.
    let cached = cached_archive_path(url);
    std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
    std::fs::write(&cached, "Name=Cached").unwrap();
    let mut reported = Vec::new();
    let mut progress = |downloaded: u64, total: Option<u64>| {
        reported.push((downloaded, total));
        Ok(())
    };
    install_mod_with_progress(&game, &mod_entry, Some(&mut progress)).unwrap();
    assert_eq!(reported, vec![(11, Some(11))]);
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .all(|request| request.starts_with("HEAD"))
    );
    assert!(!cached.exists());

    // An archive that fails to extract is not kept for the next attempt.
    std::fs::remove_dir_all(game.join("mods/UpdateMod")).unwrap();
    assert!(install_mod_with_progress(&game, &mod_entry, None).is_err());
    assert!(!cached.exists());

    // Archives older than a day are downloaded again.
    std::fs::write(&cached, "Name=Old").unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 24 * 60 * 60);
    std::fs::File::options()
        .write(true)
        .open(&cached)
        .unwrap()
        .set_modified(old)
        .unwrap();
    discard_stale_archive(&cached);
    assert!(!cached.exists());

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
}

#[test]
fn installed_mod_is_current_by_url_and_validator() {
    let tmp = tempfile::tempdir().unwrap();
    let mod_dir = tmp.path();

    // Legacy installs are adopted without a network check for GameBanana URLs.
    assert!(installed_mod_is_current(mod_dir, "https://gb.test/dl/1", false).unwrap());
    assert_eq!(
        ModSourceRecord::read(mod_dir).unwrap(),
        ModSourceRecord {
            url: "https://gb.test/dl/1".to_owned(),
            validator: None,
        }
    );
    // A new GameBanana upload changes the URL.
    assert!(!installed_mod_is_current(mod_dir, "https://gb.test/dl/2", false).unwrap());
    assert!(installed_mod_is_current(mod_dir, "https://gb.test/dl/1", false).unwrap());
    // No recorded validator means nothing to compare.
    assert!(installed_mod_is_current(mod_dir, "https://gb.test/dl/1", true).unwrap());

    std::fs::write(mod_dir.join(MOD_SOURCE_FILE), "garbage\n").unwrap();
    assert!(ModSourceRecord::read(mod_dir).is_none());
}

#[test]
fn mod_download_size_and_installed_state() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    let (base, _) = serve(|request| {
        if request.path.starts_with("/gbapi") {
            Reply::ok(r#"[{"5":{"_idRow":5,"_nFilesize":100},"7":{"_idRow":7,"_nFilesize":2048}}]"#)
        } else {
            Reply::ok(vec![0u8; 4096])
        }
    });
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            format!("{base}/gbapi?fields=Files().aFiles()"),
        );
    }

    let gamebanana = ModEntry {
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 1,
        },
        ..update_test_mod("unused")
    };
    assert_eq!(mod_download_size(&gamebanana).unwrap(), Some(2048));
    let direct = update_test_mod(Box::leak(format!("{base}/file.7z").into_boxed_str()));
    assert_eq!(mod_download_size(&direct).unwrap(), Some(4096));

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
    }

    let tmp = tempfile::tempdir().unwrap();
    assert!(!is_mod_installed(tmp.path(), &direct));
    std::fs::create_dir_all(tmp.path().join("mods/UpdateMod")).unwrap();
    std::fs::write(tmp.path().join("mods/UpdateMod/mod.ini"), "[mod]").unwrap();
    assert!(is_mod_installed(tmp.path(), &direct));
}

#[test]
fn steam_config_status_reports_missing_prefix() {
    let tmp = tempfile::tempdir().unwrap();
    let game_path = tmp.path().join("steamapps/common/Sonic Adventure 2");
    std::fs::create_dir_all(&game_path).unwrap();
    let game = Game {
        kind: GameKind::SA2,
        path: game_path,
    };

    let status = steam_config_status(&game);
    assert!(!status.ready);
    assert_eq!(status.message, steam_config_message(&game));
    assert_eq!(status.ready, can_continue_from_steam_config(&game));
}

#[test]
fn install_mod_keeps_installed_copy_when_update_checks_fail() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path();
    let mod_dir = game.join("mods/UpdateMod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("mod.ini"), "Name=Installed").unwrap();

    // GameBanana unreachable: keep what is installed.
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            "http://127.0.0.1:9/gbapi?fields=Files().aFiles()",
        );
    }
    let gamebanana = ModEntry {
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 1,
        },
        ..update_test_mod("unused")
    };
    install_mod_with_progress(game, &gamebanana, None).unwrap();
    unsafe {
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
    }

    // The version check fails: keep what is installed.
    let (base, _) = serve(|_| Reply::ok(Vec::new()).status("500 Internal Server Error"));
    let url: &'static str = Box::leak(format!("{base}/mod.7z").into_boxed_str());
    ModSourceRecord {
        url: url.to_owned(),
        validator: Some("\"v1\"".to_owned()),
    }
    .write(&mod_dir)
    .unwrap();
    install_mod_with_progress(game, &update_test_mod(url), None).unwrap();

    assert_eq!(
        std::fs::read_to_string(mod_dir.join("mod.ini")).unwrap(),
        "Name=Installed"
    );
}

#[test]
fn install_mod_replaces_incomplete_install() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    let (base, _) = serve(|_| Reply::ok("Name=Fresh"));
    let url: &'static str = Box::leak(format!("{base}/fresh.7z").into_boxed_str());
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    let mod_dir = game.join("mods/UpdateMod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("leftover.bin"), "partial").unwrap();
    let fake_7zz = install_echo_7zz(tmp.path());
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }

    install_mod_with_progress(&game, &update_test_mod(url), None).unwrap();

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
    assert_eq!(
        std::fs::read_to_string(mod_dir.join("mod.ini")).unwrap(),
        "Name=Fresh"
    );
    assert!(!mod_dir.join("leftover.bin").exists());
}

#[test]
fn install_mod_forwards_download_progress_to_callback() {
    use crate::external::test_http::{Reply, serve};
    use std::sync::{Arc, Mutex};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();

    let body = "Name=Progress Mod";
    let (base, _log) = serve(move |_| Reply::ok(body));
    let url: &'static str = Box::leak(format!("{base}/progress.7z").into_boxed_str());

    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    let fake_7zz = install_echo_7zz(tmp.path());
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }

    let updates = Arc::new(Mutex::new(Vec::new()));
    let recorded = updates.clone();
    let result = install_mod(
        &game,
        &update_test_mod(url),
        Some(Box::new(move |downloaded, total| {
            recorded.lock().unwrap().push((downloaded, total));
        })),
    );

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
    result.unwrap();

    let updates = updates.lock().unwrap();
    let total = body.len() as u64;
    assert!(!updates.is_empty());
    assert_eq!(updates.last(), Some(&(total, Some(total))));
    assert!(updates.iter().all(|&(downloaded, _)| downloaded <= total));
    assert_eq!(
        std::fs::read_to_string(game.join("mods/UpdateMod/mod.ini")).unwrap(),
        body
    );
}

#[test]
fn direct_url_base_override_keeps_only_the_file_name() {
    let _guard = crate::test_env::lock();
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_DIRECT_URL_BASE_OVERRIDE",
            "http://127.0.0.1:4010/files/",
        );
    }

    let file = resolve_download_url(&ModSource::DirectUrl {
        url: "https://github.com/owner/mod/releases/latest/download/mod.7z",
    });
    // A URL without a file name has nothing to rewrite.
    let folder = resolve_download_url(&ModSource::DirectUrl {
        url: "https://example.com/mods/",
    });

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_DIRECT_URL_BASE_OVERRIDE");
    }
    assert_eq!(file.unwrap(), "http://127.0.0.1:4010/files/mod.7z");
    assert_eq!(folder.unwrap(), "https://example.com/mods/");
}

#[test]
fn gamebanana_item_urls_default_to_gamebanana() {
    let _guard = crate::test_env::lock();

    let (api_url, dl_base) = gamebanana_item_urls("Mod", 5);

    assert_eq!(
        api_url,
        "https://api.gamebanana.com/Core/Item/Data?fields=Files().aFiles()&itemtype=Mod&itemid=5"
    );
    assert_eq!(dl_base, "https://gamebanana.com/dl/");
}

#[test]
fn gamebanana_item_reports_api_errors() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();
    let (base, _) = serve(|_| Reply::ok("down").status("503 Service Unavailable"));
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            format!("{base}/gbapi?fields=Files().aFiles()"),
        );
    }

    let result = resolve_download_url(&ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 4,
    });

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
    }
    assert_eq!(
        result.unwrap_err().to_string(),
        "GameBanana API error for Mod/4"
    );
}

#[test]
fn iso8601_timestamps_are_utc_with_microseconds() {
    use std::time::{Duration, UNIX_EPOCH};

    assert_eq!(iso8601_utc(UNIX_EPOCH), "1970-01-01T00:00:00.000000Z");
    assert_eq!(
        iso8601_utc(UNIX_EPOCH + Duration::from_micros(1_791_576_715_123_456)),
        "2026-10-09T20:11:55.123456Z"
    );
    // 2000-02-29 is a leap day; 2100-03-01 follows a skipped one.
    assert_eq!(
        iso8601_utc(UNIX_EPOCH + Duration::from_secs(951_782_400)),
        "2000-02-29T00:00:00.000000Z"
    );
    assert_eq!(
        iso8601_utc(UNIX_EPOCH + Duration::from_secs(4_107_542_400)),
        "2100-03-01T00:00:00.000000Z"
    );
    assert_eq!(
        iso8601_utc(UNIX_EPOCH - Duration::from_secs(1)),
        "1970-01-01T00:00:00.000000Z"
    );
}

/// A fake 7zz that extracts nothing.
fn install_empty_7zz(dir: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let fake_7zz = dir.join("empty-7zz");
    std::fs::write(
        &fake_7zz,
        "#!/bin/sh\nfor arg in \"$@\"; do case \"$arg\" in -o*) mkdir -p \"${arg#-o}\" ;; esac; done\n",
    )
    .unwrap();
    std::fs::set_permissions(&fake_7zz, std::fs::Permissions::from_mode(0o755)).unwrap();
    fake_7zz
}

#[test]
fn manager_release_without_the_manager_is_rejected() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();
    let (base, _) = serve(|_| Reply::ok("not a manager release"));
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("Launcher.exe"), "steam launcher").unwrap();
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", install_empty_7zz(tmp.path()));
    }

    let result = install_manager_release(&game, &format!("{base}/release.zip"), None);

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
    }
    assert_eq!(
        result.unwrap_err().to_string(),
        "SAModManager.exe not found in release archive"
    );
    assert_eq!(
        std::fs::read_to_string(game.join("Launcher.exe")).unwrap(),
        "steam launcher"
    );
    assert!(!game.join("Launcher.exe.bak").exists());
}

/// An SA2 game folder with the mod loader extracted; returns its DLL folder.
fn sa2_game_with_loader(game: &std::path::Path) -> std::path::PathBuf {
    let dll_dir = game.join("resource/gd_PC/DLL/Win32");
    std::fs::create_dir_all(&dll_dir).unwrap();
    std::fs::create_dir_all(game.join("mods/.modloader")).unwrap();
    std::fs::write(game.join("mods/.modloader/SA2ModLoader.dll"), b"loader").unwrap();
    dll_dir
}

#[test]
fn install_loader_dll_logs_the_swap_and_skips_a_missing_data_dll() {
    let capture = crate::test_log::LogCapture::start();

    let missing = tempfile::tempdir().unwrap();
    let missing_dir = sa2_game_with_loader(missing.path());
    install_loader_dll(missing.path(), GameKind::SA2).unwrap();
    assert!(capture.contents().contains(&format!(
        "Game data DLL not found at {}, skipping DLL replacement",
        missing_dir.join("Data_DLL.dll").display()
    )));
    assert!(!missing_dir.join("Data_DLL.dll").exists());

    let present = tempfile::tempdir().unwrap();
    let dll_dir = sa2_game_with_loader(present.path());
    std::fs::write(dll_dir.join("data_dll.DLL"), b"data").unwrap();
    install_loader_dll(present.path(), GameKind::SA2).unwrap();
    assert!(capture.contents().contains(&format!(
        "DLL replacement complete: SA2ModLoader.dll → {}",
        dll_dir.join("data_dll.DLL").display()
    )));
    assert_eq!(
        std::fs::read(dll_dir.join("data_dll.DLL")).unwrap(),
        b"loader"
    );
    assert_eq!(
        std::fs::read(dll_dir.join("Data_DLL_orig.dll")).unwrap(),
        b"data"
    );
}

#[test]
fn install_loader_dll_reports_copy_and_backup_failures() {
    // The backup exists, but a folder is in the data DLL's place.
    let refresh = tempfile::tempdir().unwrap();
    let dll_dir = sa2_game_with_loader(refresh.path());
    std::fs::write(dll_dir.join("Data_DLL_orig.dll"), b"data").unwrap();
    std::fs::create_dir_all(dll_dir.join("Data_DLL.dll")).unwrap();
    let err = install_loader_dll(refresh.path(), GameKind::SA2).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "Failed to copy mod loader DLL to {}",
            dll_dir.join("Data_DLL.dll").display()
        )
    );

    // A folder is where the original would be backed up.
    let backup = tempfile::tempdir().unwrap();
    let dll_dir = sa2_game_with_loader(backup.path());
    std::fs::write(dll_dir.join("Data_DLL.dll"), b"data").unwrap();
    std::fs::create_dir_all(dll_dir.join("Data_DLL_orig.dll/blocker")).unwrap();
    let err = install_loader_dll(backup.path(), GameKind::SA2).unwrap_err();
    assert!(err.to_string().starts_with("Failed to back up"), "{err}");
    assert_eq!(
        std::fs::read(dll_dir.join("Data_DLL.dll")).unwrap(),
        b"data"
    );
}

#[test]
fn install_mod_without_a_progress_callback_downloads_and_installs() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();
    let (base, _) = serve(|_| Reply::ok("Name=Quiet Mod"));
    let url: &'static str = Box::leak(format!("{base}/quiet.7z").into_boxed_str());
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", install_echo_7zz(tmp.path()));
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }

    let result = install_mod(&game, &update_test_mod(url), None);

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
    result.unwrap();
    assert_eq!(
        std::fs::read_to_string(game.join("mods/UpdateMod/mod.ini")).unwrap(),
        "Name=Quiet Mod"
    );
}

#[test]
fn install_mod_fails_when_a_new_mod_cannot_be_looked_up() {
    let _guard = crate::test_env::lock();
    let tmp = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var(
            "ADVENTURE_MODS_GAMEBANANA_API_BASE",
            "http://127.0.0.1:9/gbapi?fields=Files().aFiles()",
        );
    }
    let gamebanana = ModEntry {
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 1,
        },
        ..update_test_mod("unused")
    };

    let result = install_mod_with_progress(tmp.path(), &gamebanana, None);

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
    }
    assert_eq!(
        result.unwrap_err().to_string(),
        "GameBanana API request failed for Mod/1"
    );
    assert!(!tmp.path().join("mods/UpdateMod").exists());
}

#[test]
fn install_mod_passes_through_archives_with_their_own_folder() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();
    let (base, _) = serve(|_| Reply::ok("Name=Own Folder\nUpdateUrl=https://example.test\n"));
    let url: &'static str = Box::leak(format!("{base}/own-folder.7z").into_boxed_str());
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", install_echo_7zz(tmp.path()));
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }
    let own_folder = ModEntry {
        dir_name: None,
        ..update_test_mod(url)
    };

    let result = install_mod_with_progress(&game, &own_folder, None);

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
    result.unwrap();
    // The archive's own folder name is kept, and update tracking is set up.
    let installed = game.join("mods/UpdateMod");
    assert!(
        std::fs::read_to_string(installed.join("mod.ini"))
            .unwrap()
            .starts_with("Name=Own Folder")
    );
    assert!(installed.join("mod.version").is_file());
}

#[test]
fn prefetched_archives_survive_a_crashed_installer_thread() {
    use crate::external::test_http::{Reply, serve};
    use std::sync::atomic::AtomicBool;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = crate::test_env::lock();
    let (base, log) = serve(|_| Reply::ok("Name=After Crash"));
    let url: &'static str = Box::leak(format!("{base}/after-crash.7z").into_boxed_str());
    let tmp = tempfile::tempdir().unwrap();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(&game).unwrap();
    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", install_echo_7zz(tmp.path()));
        std::env::set_var("ADVENTURE_MODS_CACHE_DIR", tmp.path().join("cache"));
    }

    // A thread panics while holding the record of prefetched archives.
    let crashed = std::thread::spawn(|| {
        let _held = prefetched_archives().lock().unwrap();
        panic!("installer thread crashed");
    })
    .join();
    assert!(crashed.is_err());
    assert!(prefetched_archives().is_poisoned());

    let mod_entry = update_test_mod(url);
    crate::setup::pipeline::prefetch_mod_archives(&game, &[&mod_entry], &AtomicBool::new(false));
    let installed = install_mod_with_progress(&game, &mod_entry, None);
    prefetched_archives().clear_poison();

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
    }
    installed.unwrap();
    let gets = log
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.starts_with("GET"))
        .count();
    assert_eq!(gets, 1);
    assert_eq!(
        std::fs::read_to_string(game.join("mods/UpdateMod/mod.ini")).unwrap(),
        "Name=After Crash"
    );
}

#[test]
fn sources_without_a_version_tag_count_as_current() {
    use crate::external::test_http::{Reply, serve};

    let _ = rustls::crypto::ring::default_provider().install_default();
    // No ETag or Last-Modified header.
    let (base, _) = serve(|_| Reply::ok("untagged"));
    let url = format!("{base}/untagged.7z");
    let tmp = tempfile::tempdir().unwrap();
    ModSourceRecord {
        url: url.clone(),
        validator: Some("\"v1\"".to_owned()),
    }
    .write(tmp.path())
    .unwrap();

    assert!(installed_mod_is_current(tmp.path(), &url, true).unwrap());
    assert!(component_is_current(
        &tmp.path().join(MOD_SOURCE_FILE),
        &url
    ));
}

#[test]
fn source_records_log_unreadable_versions_and_write_failures() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let capture = crate::test_log::LogCapture::start();
    let tmp = tempfile::tempdir().unwrap();
    let unreachable = "http://127.0.0.1:9/manager.zip";

    // The version cannot be read: the source is recorded without one.
    let record_path = tmp.path().join(MANAGER_SOURCE_FILE);
    record_component_source(&record_path, unreachable);
    assert!(
        capture
            .contents()
            .contains(&format!("Could not read the version of {unreachable}")),
        "{}",
        capture.contents()
    );
    assert_eq!(
        ModSourceRecord::read_file(&record_path).unwrap(),
        ModSourceRecord {
            url: unreachable.to_owned(),
            validator: None,
        }
    );

    // The record cannot be written: setup goes on and logs it.
    let missing_dir = tmp.path().join("missing");
    record_component_source(&missing_dir.join(MANAGER_SOURCE_FILE), unreachable);
    assert!(capture.contents().contains(&format!(
        "Failed to record {}",
        missing_dir.join(MANAGER_SOURCE_FILE).display()
    )));
    record_mod_source(&missing_dir, "https://gb.test/dl/1", false);
    assert!(capture.contents().contains(&format!(
        "Failed to record the source of {}",
        missing_dir.display()
    )));
    assert!(!missing_dir.exists());
}

#[test]
fn replace_mod_dir_keeps_user_config_and_reports_unremovable_installs() {
    let tmp = tempfile::tempdir().unwrap();
    let new_files = tmp.path().join("new");
    let dest = tmp.path().join("mods/SomeMod");
    std::fs::create_dir_all(&new_files).unwrap();
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(new_files.join("mod.ini"), "Name=New").unwrap();
    std::fs::write(dest.join("mod.ini"), "Name=Old").unwrap();
    std::fs::write(dest.join("Config.ini"), "user settings").unwrap();

    // The new version ships no config.ini: the user's is put back as is.
    replace_mod_dir(&new_files, &dest).unwrap();
    assert_eq!(
        std::fs::read_to_string(dest.join("mod.ini")).unwrap(),
        "Name=New"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("Config.ini")).unwrap(),
        "user settings"
    );

    // A file where the mod folder should be cannot be removed as a folder.
    let blocked = tmp.path().join("mods/Blocked");
    std::fs::write(&blocked, "not a folder").unwrap();
    let err = replace_mod_dir(&dest, &blocked).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("Failed to remove old mod files at {}", blocked.display())
    );
}

#[test]
fn find_mod_root_searches_each_folder_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    // The first folder holds only folders without a mod.ini.
    std::fs::create_dir_all(staging.join("a/docs")).unwrap();
    std::fs::create_dir_all(staging.join("a/extras")).unwrap();
    std::fs::create_dir_all(staging.join("b/Mod")).unwrap();
    std::fs::write(staging.join("b/Mod/mod.ini"), "[mod]").unwrap();

    assert_eq!(find_mod_root(&staging), Some(staging.join("b/Mod")));
    assert_eq!(find_mod_root(&tmp.path().join("missing")), None);
}

#[test]
fn install_passthrough_mod_counts_every_top_level_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(staging.join("ModA")).unwrap();
    std::fs::create_dir_all(staging.join("ModB")).unwrap();

    let err = install_passthrough_mod(&staging, &tmp.path().join("mods")).unwrap_err();

    assert_eq!(
        err.to_string(),
        "Expected archive to contain a single top-level mod directory, found 2 entries"
    );
}

#[test]
fn install_passthrough_mod_reports_an_incomplete_mod_it_cannot_remove() {
    // procfs entries cannot be removed, even by root.
    let proc_self = std::path::Path::new("/proc/self");
    if !proc_self.join("fd").is_dir() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(staging.join("fd")).unwrap();
    std::fs::write(staging.join("fd/mod.ini"), "[mod]").unwrap();

    let err = install_passthrough_mod(&staging, proc_self).unwrap_err();

    assert_eq!(
        err.to_string(),
        "Failed to remove incomplete mod at /proc/self/fd"
    );
    assert!(staging.join("fd/mod.ini").is_file());
}

#[test]
fn normalize_mod_version_reports_an_unreadable_mod_ini() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("mod.ini")).unwrap();

    let err = normalize_mod_version(tmp.path()).unwrap_err();

    assert_eq!(
        err.to_string(),
        format!("Failed to read {}", tmp.path().join("mod.ini").display())
    );
    assert!(!tmp.path().join("mod.version").exists());
}

#[test]
fn move_dir_contents_copies_folders_across_filesystems_when_available() {
    let Ok(dest_root) = tempfile::tempdir_in("/dev/shm") else {
        return;
    };
    let source_root = tempfile::tempdir().unwrap();
    let source = source_root.path().join("src");
    std::fs::create_dir_all(source.join("textures/hd")).unwrap();
    std::fs::write(source.join("textures/hd/sky.dds"), b"sky").unwrap();

    move_dir_contents(&source, &dest_root.path().join("dest")).unwrap();

    assert_eq!(
        std::fs::read(dest_root.path().join("dest/textures/hd/sky.dds")).unwrap(),
        b"sky"
    );
    assert!(!source.join("textures/hd/sky.dds").exists());

    // A link to nothing can be neither renamed nor copied across.
    let broken = source_root.path().join("broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::os::unix::fs::symlink("missing-target", broken.join("link")).unwrap();
    assert!(move_dir_contents(&broken, &dest_root.path().join("broken")).is_err());
}
