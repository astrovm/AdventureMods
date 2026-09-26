use std::sync::{Mutex, OnceLock};

use super::*;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn test_gamebanana_item_dl_base_override() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());

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
fn test_gamebanana_item_reports_malformed_and_empty_responses() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
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
fn test_resolve_direct_url() {
    let source = ModSource::DirectUrl {
        url: "https://example.com/mod.7z",
    };
    assert_eq!(
        resolve_download_url(&source).unwrap(),
        "https://example.com/mod.7z"
    );
}

#[test]
fn test_resolve_direct_url_rewrites_sadx_base_when_overridden() {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
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
fn test_sa_mod_manager_url_valid() {
    assert!(SA_MOD_MANAGER_URL.starts_with("https://github.com/"));
    assert!(SA_MOD_MANAGER_URL.contains("/releases/"));
    assert!(SA_MOD_MANAGER_URL.ends_with(".zip"));
}

#[test]
fn test_sa_mod_manager_url_uses_override() {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
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
fn test_mod_loader_url_uses_override() {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
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
fn test_install_mod_dir_construction() {
    let game_path = std::path::Path::new("/fake/game/dir");
    let mods_dir = game_path.join("mods");
    assert!(mods_dir.ends_with("mods"));
    assert_eq!(mods_dir, std::path::PathBuf::from("/fake/game/dir/mods"));
}

#[test]
fn test_move_dir_contents_flat_to_subdir() {
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
fn test_move_dir_contents_merges_existing_and_renames_new_dirs() {
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
fn test_staging_tempdir_prefers_the_target_filesystem() {
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

    let fallback = staging_tempdir(&tmp.path().join("missing")).unwrap();
    assert!(fallback.path().is_dir());
    assert_ne!(fallback.path().parent(), Some(tmp.path()));
}

#[test]
fn test_find_mod_root_at_staging_root() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("mod.ini"), b"[mod]").unwrap();

    let root = find_mod_root(&staging).unwrap();
    assert_eq!(root, staging);
}

#[test]
fn test_find_mod_root_one_level_deep() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let sub = staging.join("MyMod");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("mod.ini"), b"[mod]").unwrap();

    let root = find_mod_root(&staging).unwrap();
    assert_eq!(root, sub);
}

#[test]
fn test_find_mod_root_two_levels_deep() {
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
fn test_find_mod_root_none_when_missing() {
    // Archive with no mod.ini at all (e.g. icondata)
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("icon.ico"), b"icon").unwrap();

    assert!(find_mod_root(&staging).is_none());
}

#[test]
fn test_install_mod_flat_archive_with_dir_name() {
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
fn test_install_mod_nested_archive_with_dir_name() {
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
fn test_install_mod_no_mod_ini_with_dir_name() {
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
fn test_install_mod_no_dir_name_passthrough() {
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
fn test_install_passthrough_mod_rejects_flat_archive() {
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
fn test_install_passthrough_mod_preserves_existing_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let mods_dir = tmp.path().join("mods");
    let extracted = staging.join("SomeMod");
    let existing = mods_dir.join("SomeMod");
    std::fs::create_dir_all(&extracted).unwrap();
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(extracted.join("mod.ini"), b"[new]").unwrap();
    std::fs::write(existing.join("mod.ini"), b"[old]").unwrap();

    install_passthrough_mod(&staging, &mods_dir).unwrap();

    assert_eq!(std::fs::read(existing.join("mod.ini")).unwrap(), b"[old]");
    assert!(extracted.join("mod.ini").is_file());
}

#[test]
fn test_install_passthrough_mod_replaces_incomplete_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let mods_dir = tmp.path().join("mods");
    let extracted = staging.join("SomeMod");
    let existing = mods_dir.join("SomeMod");
    std::fs::create_dir_all(&extracted).unwrap();
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(extracted.join("mod.ini"), b"[new]").unwrap();
    std::fs::write(existing.join("old.txt"), b"old").unwrap();

    install_passthrough_mod(&staging, &mods_dir).unwrap();

    assert!(existing.join("mod.ini").is_file());
    assert!(!existing.join("old.txt").exists());
}

#[test]
fn test_normalize_mod_version_rewrites_stale_packaged_value() {
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
fn test_normalize_mod_version_creates_file_for_update_tracked_mod() {
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
fn test_normalize_mod_version_ignores_plain_mods() {
    let tmp = tempfile::tempdir().unwrap();
    let mod_dir = tmp.path().join("Plain Mod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("mod.ini"), b"Name=Plain Mod\nVersion=1.0\n").unwrap();

    normalize_mod_version(&mod_dir).unwrap();

    assert!(!mod_dir.join("mod.version").exists());
}

#[test]
fn test_normalize_mod_version_ignores_directories_without_metadata_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mod_dir = tmp.path().join("Incomplete Mod");
    std::fs::create_dir_all(&mod_dir).unwrap();

    normalize_mod_version(&mod_dir).unwrap();
    assert!(!mod_dir.join("mod.version").exists());
}

#[test]
fn test_update_metadata_ignores_malformed_lines() {
    assert!(!has_update_metadata(
        "not metadata\nGameBananaItemType=Mod\n"
    ));
}

#[test]
fn test_install_mod_normalizes_existing_update_tracked_mod_on_rerun() {
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
fn test_find_mod_root_prefers_deterministic_order() {
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
fn test_move_dir_contents_overwrites_existing_file() {
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
fn test_move_dir_contents_replaces_file_with_directory() {
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

/// Helper: simulate the Steam exe replacement logic from `install_mod_manager`.
/// Creates `SAModManager.exe` in the game dir and runs the replacement logic.
fn run_exe_replacement(game_path: &std::path::Path) {
    // Create a fake SAModManager.exe (the "dest_exe" that install_mod_manager copies)
    let dest_exe = game_path.join("SAModManager.exe");
    std::fs::write(&dest_exe, b"mod_manager_content").unwrap();

    let launcher = game_path.join("Launcher.exe");
    let sadx_exe = game_path.join("Sonic Adventure DX.exe");
    let steam_exe = if launcher.is_file() {
        Some(launcher)
    } else if sadx_exe.is_file() {
        Some(sadx_exe)
    } else {
        None
    };

    if let Some(steam_exe) = steam_exe {
        let bak = steam_exe.with_extension("exe.bak");
        if !bak.exists() {
            std::fs::rename(&steam_exe, &bak).unwrap();
        }
        std::fs::rename(&dest_exe, &steam_exe).unwrap();
    }
}

#[test]
fn test_exe_replacement_sa2_launcher() {
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
fn test_exe_replacement_sadx() {
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
fn test_exe_replacement_sadx_backup_not_overwritten() {
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
fn test_exe_replacement_sa2_backup_not_overwritten() {
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
fn test_exe_replacement_no_steam_exe() {
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
fn test_exe_replacement_launcher_takes_priority_over_sadx() {
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
fn test_recommended_mods_for_game_returns_correct_lists() {
    let sadx_mods = recommended_mods_for_game(GameKind::SADX);
    let sa2_mods = recommended_mods_for_game(GameKind::SA2);
    assert!(!sadx_mods.is_empty());
    assert!(!sa2_mods.is_empty());
    assert_ne!(sadx_mods.len(), sa2_mods.len());
}

#[test]
fn test_find_mod_root_three_levels_deep_returns_none() {
    // find_mod_root only searches two levels deep; three levels should return None
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");
    let deep = staging.join("a").join("b").join("DeepMod");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("mod.ini"), b"[mod]").unwrap();

    assert!(find_mod_root(&staging).is_none());
}

#[test]
fn test_move_dir_contents_empty_source() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("empty_src");
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(&src).unwrap();

    move_dir_contents(&src, &dest).unwrap();
    assert!(dest.is_dir());
    assert!(std::fs::read_dir(&dest).unwrap().next().is_none());
}

#[test]
fn test_move_dir_contents_nested_subdirectory() {
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
fn test_proton_prefix_standard_path() {
    let game_path =
        std::path::Path::new("/home/user/.local/share/Steam/steamapps/common/Sonic Adventure DX");
    let prefix = proton_prefix(game_path, 71250).unwrap();
    assert_eq!(
        prefix,
        std::path::PathBuf::from("/home/user/.local/share/Steam/steamapps/compatdata/71250/pfx")
    );
}

#[test]
fn test_proton_prefix_shallow_path_fails() {
    let game_path = std::path::Path::new("/game");
    assert!(proton_prefix(game_path, 71250).is_err());
}

#[test]
fn test_proton_prefix_sa2_app_id() {
    let game_path = std::path::Path::new("/mnt/steam/steamapps/common/Sonic Adventure 2");
    let prefix = proton_prefix(game_path, 213610).unwrap();
    assert_eq!(
        prefix,
        std::path::PathBuf::from("/mnt/steam/steamapps/compatdata/213610/pfx")
    );
}

#[test]
fn test_find_file_icase_finds_uppercase_variant() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("CHRMODELS.DLL"), b"").unwrap();

    let found = find_file_icase(tmp.path(), "chrmodels.dll");
    assert!(found.is_some());
    assert!(found.unwrap().ends_with("CHRMODELS.DLL"));
}

#[test]
fn test_find_file_icase_missing_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(find_file_icase(tmp.path(), "nonexistent.dll").is_none());
}

#[test]
fn test_find_file_icase_nonexistent_dir_returns_none() {
    let path = std::path::Path::new("/nonexistent/path/that/does/not/exist");
    assert!(find_file_icase(path, "anything.dll").is_none());
}

#[test]
fn test_install_loader_dll_sadx_uses_lowercase_system_data_dir() {
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
fn test_is_mod_manager_fully_installed_requires_dll_swap() {
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
fn test_mod_entry_dir_name_fallback_to_name() {
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
fn test_mod_entry_explicit_dir_name() {
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
fn test_step_completion_detects_conversion_and_manager_markers() {
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
    assert!(is_step_complete(StepId::InstallModManager, &game));

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
    assert!(is_step_complete(StepId::InstallModManager, &sa2));
}

#[test]
fn test_dotnet_step_is_complete_for_a_ready_prefix_with_runtime() {
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
    assert!(is_step_complete(StepId::Dotnet, &game));
}

#[test]
fn test_install_loader_refreshes_existing_and_reports_missing_loader() {
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
fn test_install_manager_and_loader_skip_existing_installations() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("system")).unwrap();
    std::fs::create_dir_all(dir.path().join("mods/.modloader")).unwrap();
    std::fs::write(dir.path().join("Sonic Adventure DX.exe.bak"), b"backup").unwrap();
    std::fs::write(dir.path().join("system/CHRMODELS_orig.dll"), b"orig").unwrap();
    std::fs::write(
        dir.path().join("mods/.modloader/SADXModLoader.dll"),
        b"loader",
    )
    .unwrap();

    install_mod_manager(dir.path(), GameKind::SADX, None).unwrap();
    install_mod_loader(dir.path(), GameKind::SADX, None).unwrap();
}

#[test]
fn test_install_mod_wrapper_and_update_url_metadata() {
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
fn test_move_dir_contents_cross_filesystem_fallback_when_available() {
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
#[test]
fn test_install_manager_and_loader_from_synthetic_downloads() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::os::unix::fs::PermissionsExt;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let body = b"synthetic archive";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
        }
    });

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("system")).unwrap();
    std::fs::write(dir.path().join("system/CHRMODELS.dll"), b"original").unwrap();

    let fake_7zz = dir.path().join("fake-7zz");
    std::fs::write(
        &fake_7zz,
        r##"#!/bin/sh
dest=""
for arg in "$@"; do
    case "$arg" in
        -o*) dest="${arg#-o}" ;;
    esac
done
mkdir -p "$dest"
case "$dest" in
    */extracted) printf manager > "$dest/SAModManager.exe" ;;
    *) printf loader > "$dest/SADXModLoader.dll" ;;
esac
"##,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&fake_7zz).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_7zz, permissions).unwrap();

    unsafe {
        std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
        std::env::set_var(
            "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
            format!("http://127.0.0.1:{port}/manager"),
        );
        std::env::set_var(
            "ADVENTURE_MODS_URL_SADX_MOD_LOADER",
            format!("http://127.0.0.1:{port}/loader"),
        );
    }

    install_mod_manager(dir.path(), GameKind::SADX, Some(Box::new(|_, _| {}))).unwrap();

    unsafe {
        std::env::remove_var("ADVENTURE_MODS_7ZZ");
        std::env::remove_var("ADVENTURE_MODS_URL_SA_MOD_MANAGER");
        std::env::remove_var("ADVENTURE_MODS_URL_SADX_MOD_LOADER");
    }
    server.join().unwrap();

    assert!(dir.path().join("SAModManager.exe").is_file());
    assert!(
        dir.path()
            .join("mods/.modloader/SADXModLoader.dll")
            .is_file()
    );
    assert!(dir.path().join("system/CHRMODELS_orig.dll").is_file());
    assert_eq!(
        std::fs::read(dir.path().join("system/CHRMODELS.dll")).unwrap(),
        b"loader"
    );
}
