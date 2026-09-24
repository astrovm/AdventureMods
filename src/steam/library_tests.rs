use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Build a mock VDF structure for libraryfolders with one library.
fn mock_vdf(lib_path: &str, app_ids: &[&str]) -> vdf::VdfValue {
    let mut apps = HashMap::new();
    for id in app_ids {
        apps.insert(id.to_string(), vdf::VdfValue::String("0".to_string()));
    }

    let mut folder = HashMap::new();
    folder.insert(
        "path".to_string(),
        vdf::VdfValue::String(lib_path.to_string()),
    );
    folder.insert("apps".to_string(), vdf::VdfValue::Map(apps));

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder));

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));

    vdf::VdfValue::Map(root)
}

fn with_environment<T>(name: &str, value: Option<&Path>, test: impl FnOnce() -> T) -> T {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let previous = std::env::var_os(name);

    match value {
        Some(value) => unsafe { std::env::set_var(name, value) },
        None => unsafe { std::env::remove_var(name) },
    }

    let result = test();

    match previous {
        Some(value) => unsafe { std::env::set_var(name, value) },
        None => unsafe { std::env::remove_var(name) },
    }

    result
}

#[test]
fn test_steam_roots_find_native_and_flatpak_steam() {
    let tmp = tempfile::tempdir().unwrap();
    let native = tmp.path().join(".local/share/Steam");
    let flatpak = tmp
        .path()
        .join(".var/app/com.valvesoftware.Steam/.local/share/Steam");
    std::fs::create_dir_all(&native).unwrap();
    std::fs::create_dir_all(&flatpak).unwrap();

    let roots = steam_roots_in(tmp.path());

    assert_eq!(roots, vec![native, flatpak]);
}

#[test]
fn test_steam_roots_skip_missing_installations() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(steam_roots_in(tmp.path()).is_empty());
}

#[test]
fn test_detect_games_reads_synthetic_home_steam_libraries() {
    let home = tempfile::tempdir().unwrap();
    let native = home.path().join(".local/share/Steam");
    let alternate = home
        .path()
        .join(".var/app/com.valvesoftware.Steam/.local/share/Steam");
    std::fs::create_dir_all(home.path().join(".steam/steam/steamapps")).unwrap();
    let stale = home.path().join("stale-library");
    let inaccessible = home.path().join("missing-library");
    let game_dir = make_steam_library(&native, GameKind::SADX);
    std::fs::create_dir_all(
        stale
            .join("steamapps/common")
            .join(GameKind::SADX.install_dir()),
    )
    .unwrap();
    std::fs::create_dir_all(alternate.join("steamapps")).unwrap();
    std::fs::write(alternate.join("steamapps/libraryfolders.vdf"), "invalid {").unwrap();
    std::fs::write(
        native.join("steamapps/libraryfolders.vdf"),
        format!(
            "\"libraryfolders\" {{\n  \"0\" {{ \"path\" \"{}\" \"apps\" {{ \"71250\" \"0\" }} }}\n  \"1\" {{ \"path\" \"{}\" \"apps\" {{ \"71250\" \"0\" }} }}\n  \"2\" {{ \"path\" \"{}\" \"apps\" {{ \"71250\" \"0\" }} }}\n}}",
            native.display(),
            stale.display(),
            inaccessible.display()
        ),
    )
    .unwrap();

    let result = with_environment("HOME", Some(home.path()), || {
        detect_games_with_extra_libraries(&[])
    });
    assert!(result.games.iter().any(|game| game.path == game_dir));
    assert!(
        result
            .inaccessible
            .iter()
            .any(|game| game.library_path == inaccessible)
    );

    let empty_home = tempfile::tempdir().unwrap();
    let empty_result = with_environment("HOME", Some(empty_home.path()), || {
        detect_games_with_extra_libraries(&[])
    });
    assert!(empty_result.games.is_empty());
}

#[test]
fn test_find_sadx_in_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();

    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert_eq!(paths, vec![game_dir]);
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_sa2_in_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("sonic2app.exe"), "").unwrap();

    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["213610"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SA2);
    assert_eq!(paths, vec![game_dir]);
    assert!(inaccessible.is_empty());
}

#[test]
fn test_detect_games_with_extra_library_finds_inaccessible_game() {
    let tmp = tempfile::tempdir().unwrap();
    let extra_lib = tmp.path().join("portable-library");
    let game_dir = extra_lib
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();

    let inaccessible_path = tmp.path().join("missing-library");
    let root = mock_vdf(inaccessible_path.to_str().unwrap(), &["71250"]);
    let result = detect_games_from_parsed_vdfs(&[root], std::slice::from_ref(&extra_lib));

    assert!(result.games.iter().any(|game| game.kind == GameKind::SADX));
    assert!(
        result
            .inaccessible
            .iter()
            .any(|game| game.kind == GameKind::SADX)
    );
}

#[test]
fn test_detect_games_keeps_inaccessible_alongside_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let accessible_lib = tmp.path().join("accessible");
    let game_dir = accessible_lib
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();

    let inaccessible_path = tmp.path().join("inaccessible-library");

    let vdf_accessible = mock_vdf(accessible_lib.to_str().unwrap(), &["71250"]);
    let vdf_inaccessible = mock_vdf(inaccessible_path.to_str().unwrap(), &["71250"]);
    let result = detect_games_from_parsed_vdfs(&[vdf_accessible, vdf_inaccessible], &[]);

    assert!(result.games.iter().any(|g| g.kind == GameKind::SADX));
    assert!(result.inaccessible.iter().any(|g| g.kind == GameKind::SADX));
}

#[test]
fn test_game_not_in_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["400", "500"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SA2);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_missing_libraryfolders_key() {
    let root = vdf::VdfValue::Map(HashMap::new());
    let (paths, inaccessible) = find_all_games_in_libraries(&root, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_game_app_present_but_dir_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_game_missing_apps_key() {
    let mut folder = HashMap::new();
    folder.insert(
        "path".to_string(),
        vdf::VdfValue::String("/some/path".to_string()),
    );

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder));

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));

    let vdf = vdf::VdfValue::Map(root);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_game_missing_path_key() {
    let mut apps = HashMap::new();
    apps.insert("71250".to_string(), vdf::VdfValue::String("0".to_string()));

    let mut folder = HashMap::new();
    folder.insert("apps".to_string(), vdf::VdfValue::Map(apps));

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder));

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));

    let vdf = vdf::VdfValue::Map(root);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_game_libraryfolders_is_string() {
    let mut root = HashMap::new();
    root.insert(
        "libraryfolders".to_string(),
        vdf::VdfValue::String("oops".to_string()),
    );
    let vdf = vdf::VdfValue::Map(root);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_game_skips_non_map_library_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let mut folders = HashMap::new();
    folders.insert(
        "0".to_string(),
        vdf::VdfValue::String("not-a-map".to_string()),
    );
    folders.insert(
        "1".to_string(),
        vdf::VdfValue::Map({
            let mut valid = HashMap::new();
            valid.insert(
                "path".to_string(),
                vdf::VdfValue::String(tmp.path().to_string_lossy().to_string()),
            );
            valid.insert("apps".to_string(), vdf::VdfValue::Map(HashMap::new()));
            valid
        }),
    );

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));
    let vdf = vdf::VdfValue::Map(root);

    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_game_skips_non_map_apps() {
    let tmp = tempfile::tempdir().unwrap();
    let mut folder = HashMap::new();
    folder.insert(
        "path".to_string(),
        vdf::VdfValue::String(tmp.path().to_string_lossy().to_string()),
    );
    folder.insert(
        "apps".to_string(),
        vdf::VdfValue::String("invalid".to_string()),
    );

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder));

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));
    let vdf = vdf::VdfValue::Map(root);

    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_find_both_games_in_same_library() {
    let tmp = tempfile::tempdir().unwrap();
    let sadx_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    let sa2_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&sadx_dir).unwrap();
    std::fs::create_dir_all(&sa2_dir).unwrap();
    std::fs::write(sadx_dir.join("Sonic Adventure DX.exe"), "").unwrap();
    std::fs::write(sa2_dir.join("sonic2app.exe"), "").unwrap();

    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250", "213610"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert_eq!(paths, vec![sadx_dir]);
    assert!(inaccessible.is_empty());
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SA2);
    assert_eq!(paths, vec![sa2_dir]);
    assert!(inaccessible.is_empty());
}

#[test]
fn test_detect_games_from_vdf_both_present() {
    let tmp = tempfile::tempdir().unwrap();
    let lib_path = tmp.path().join("lib");

    let sadx_dir = lib_path
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    let sa2_dir = lib_path
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&sadx_dir).unwrap();
    std::fs::create_dir_all(&sa2_dir).unwrap();
    std::fs::write(sadx_dir.join("Sonic Adventure DX.exe"), "").unwrap();
    std::fs::write(sa2_dir.join("sonic2app.exe"), "").unwrap();

    let vdf_path = tmp.path().join("libraryfolders.vdf");
    let vdf_content = format!(
        r#""libraryfolders"
{{
    "0"
    {{
        "path"		"{}"
        "apps"
        {{
            "71250"		"0"
            "213610"	"0"
        }}
    }}
}}"#,
        lib_path.to_str().unwrap()
    );
    std::fs::write(&vdf_path, &vdf_content).unwrap();

    let result = detect_games_from_vdf_with_extra_libraries(&vdf_path, &[]);
    assert_eq!(result.games.len(), 2);
    assert!(result.games.iter().any(|g| g.kind == GameKind::SADX));
    assert!(result.games.iter().any(|g| g.kind == GameKind::SA2));
    assert!(result.inaccessible.is_empty());
}

#[test]
fn test_detect_games_from_vdf_none_present() {
    let tmp = tempfile::tempdir().unwrap();

    let vdf_path = tmp.path().join("libraryfolders.vdf");
    let vdf_content = format!(
        r#""libraryfolders"
{{
    "0"
    {{
        "path"		"{}"
        "apps"
        {{
            "400"		"0"
            "500"		"0"
        }}
    }}
}}"#,
        tmp.path().to_str().unwrap()
    );
    std::fs::write(&vdf_path, &vdf_content).unwrap();

    let result = detect_games_from_vdf_with_extra_libraries(&vdf_path, &[]);
    assert!(result.games.is_empty());
    assert!(result.inaccessible.is_empty());
}

#[test]
fn test_detect_games_from_vdf_corrupt() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(&vdf_path, "this is not valid VDF content {{{").unwrap();

    let result = detect_games_from_vdf_with_extra_libraries(&vdf_path, &[]);
    assert!(result.games.is_empty());
    assert!(result.inaccessible.is_empty());
}

#[test]
fn test_library_detection_handles_missing_fields_and_stale_entries() {
    let mut folders = HashMap::new();
    folders.insert(
        "scalar".to_string(),
        vdf::VdfValue::String("not-a-folder".to_string()),
    );

    let mut no_apps = HashMap::new();
    no_apps.insert(
        "path".to_string(),
        vdf::VdfValue::String("/tmp/no-apps".to_string()),
    );
    folders.insert("no-apps".to_string(), vdf::VdfValue::Map(no_apps));

    let mut missing_path_apps = HashMap::new();
    missing_path_apps.insert("213610".to_string(), vdf::VdfValue::String("0".to_string()));
    let mut missing_path = HashMap::new();
    missing_path.insert("apps".to_string(), vdf::VdfValue::Map(missing_path_apps));
    folders.insert("missing-path".to_string(), vdf::VdfValue::Map(missing_path));

    let mut empty_path_apps = HashMap::new();
    empty_path_apps.insert("213610".to_string(), vdf::VdfValue::String("0".to_string()));
    let mut empty_path = HashMap::new();
    empty_path.insert("path".to_string(), vdf::VdfValue::String("   ".to_string()));
    empty_path.insert("apps".to_string(), vdf::VdfValue::Map(empty_path_apps));
    folders.insert("empty-path".to_string(), vdf::VdfValue::Map(empty_path));

    let inaccessible_path = "/definitely/missing/steam-library";
    let mut inaccessible_apps = HashMap::new();
    inaccessible_apps.insert("213610".to_string(), vdf::VdfValue::String("0".to_string()));
    let mut inaccessible = HashMap::new();
    inaccessible.insert(
        "path".to_string(),
        vdf::VdfValue::String(inaccessible_path.to_string()),
    );
    inaccessible.insert("apps".to_string(), vdf::VdfValue::Map(inaccessible_apps));
    folders.insert("inaccessible".to_string(), vdf::VdfValue::Map(inaccessible));

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));
    let result = detect_games_from_parsed_vdfs(&[vdf::VdfValue::Map(root)], &[]);
    assert!(result.games.is_empty());
    assert!(
        result
            .inaccessible
            .iter()
            .any(|game| game.library_path == Path::new(inaccessible_path))
    );

    let missing_vdf = PathBuf::from("/definitely/missing/libraryfolders.vdf");
    assert!(
        detect_games_from_vdf_with_extra_libraries(&missing_vdf, &[])
            .games
            .is_empty()
    );
    let _ = detect_games_with_extra_libraries(&[]);
}

#[test]
fn test_library_detection_reports_stale_game_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let stale = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&stale).unwrap();

    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["213610"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SA2);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_multiple_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("sonic2app.exe"), "").unwrap();

    let mut folder0 = HashMap::new();
    folder0.insert(
        "path".to_string(),
        vdf::VdfValue::String("/nonexistent".to_string()),
    );
    let mut apps0 = HashMap::new();
    apps0.insert("400".to_string(), vdf::VdfValue::String("0".to_string()));
    folder0.insert("apps".to_string(), vdf::VdfValue::Map(apps0));

    let mut folder1 = HashMap::new();
    folder1.insert(
        "path".to_string(),
        vdf::VdfValue::String(tmp.path().to_str().unwrap().to_string()),
    );
    let mut apps1 = HashMap::new();
    apps1.insert("213610".to_string(), vdf::VdfValue::String("0".to_string()));
    folder1.insert("apps".to_string(), vdf::VdfValue::Map(apps1));

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder0));
    folders.insert("1".to_string(), vdf::VdfValue::Map(folder1));

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));
    let vdf = vdf::VdfValue::Map(root);

    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SA2);
    assert_eq!(paths, vec![game_dir]);
    assert!(inaccessible.is_empty());
}

#[test]
fn test_inaccessible_library() {
    let mut folder = HashMap::new();
    folder.insert(
        "path".to_string(),
        vdf::VdfValue::String("/mnt/games/SteamLibrary".to_string()),
    );
    let mut apps = HashMap::new();
    apps.insert("71250".to_string(), vdf::VdfValue::String("0".to_string()));
    folder.insert("apps".to_string(), vdf::VdfValue::Map(apps));

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder));

    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));
    let vdf = vdf::VdfValue::Map(root);

    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert_eq!(inaccessible.len(), 1);
    let inc = &inaccessible[0];
    assert_eq!(inc.kind, GameKind::SADX);
    assert_eq!(inc.library_path, PathBuf::from("/mnt/games/SteamLibrary"));
}

#[test]
fn test_duplicate_installations_across_steam_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let lib1 = tmp.path().join("lib1");
    let lib2 = tmp.path().join("lib2");

    for lib in [&lib1, &lib2] {
        let game_dir = lib
            .join("steamapps/common")
            .join(GameKind::SADX.install_dir());
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();
    }

    let root1 = mock_vdf(lib1.to_str().unwrap(), &["71250"]);
    let root2 = mock_vdf(lib2.to_str().unwrap(), &["71250"]);

    let result = detect_games_from_parsed_vdfs(&[root1, root2], &[]);

    // Both distinct installations should be reported
    let sadx_installs: Vec<_> = result
        .games
        .iter()
        .filter(|g| g.kind == GameKind::SADX)
        .collect();
    assert_eq!(
        sadx_installs.len(),
        2,
        "Expected both SADX installations to be reported"
    );
    assert!(result.inaccessible.is_empty());
}

#[test]
fn test_duplicate_installations_same_path_deduped() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    let game_dir = lib
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();

    // Two Steam roots pointing to the same library
    let root1 = mock_vdf(lib.to_str().unwrap(), &["71250"]);
    let root2 = mock_vdf(lib.to_str().unwrap(), &["71250"]);

    let result = detect_games_from_parsed_vdfs(&[root1, root2], &[]);

    // Same physical path should only appear once
    let sadx_installs: Vec<_> = result
        .games
        .iter()
        .filter(|g| g.kind == GameKind::SADX)
        .collect();
    assert_eq!(
        sadx_installs.len(),
        1,
        "Same path from two Steam roots should be deduplicated"
    );
}

#[test]
fn test_inaccessible_deduped_across_roots() {
    let mut folder = HashMap::new();
    folder.insert(
        "path".to_string(),
        vdf::VdfValue::String("/mnt/games/SteamLibrary".to_string()),
    );
    let mut apps = HashMap::new();
    apps.insert("71250".to_string(), vdf::VdfValue::String("0".to_string()));
    folder.insert("apps".to_string(), vdf::VdfValue::Map(apps.clone()));

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder.clone()));

    let mut root_map = HashMap::new();
    root_map.insert(
        "libraryfolders".to_string(),
        vdf::VdfValue::Map(folders.clone()),
    );
    let root1 = vdf::VdfValue::Map(root_map.clone());

    // Second root with same inaccessible library
    let mut root_map2 = HashMap::new();
    root_map2.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));
    let root2 = vdf::VdfValue::Map(root_map2);

    let result = detect_games_from_parsed_vdfs(&[root1, root2], &[]);

    // Same inaccessible library from two roots should only appear once
    assert_eq!(
        result.inaccessible.len(),
        1,
        "Same inaccessible library from two roots should be deduplicated"
    );
}

#[test]
fn test_no_vdf_roots_finds_game_via_extra_library() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    let game_dir = lib
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();

    // No VDF roots at all (e.g. Steam not installed), only an extra library
    let result = detect_games_from_parsed_vdfs(&[], std::slice::from_ref(&lib));

    assert!(result.games.iter().any(|g| g.kind == GameKind::SADX));
    assert!(result.inaccessible.is_empty());
}

#[test]
fn test_extra_libraries_duplicate_paths_deduped() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    let game_dir = lib
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();

    // Same path appears twice in extra_libraries (e.g. user granted access twice)
    let result = detect_games_from_parsed_vdfs(&[], &[lib.clone(), lib.clone()]);

    let sadx_installs: Vec<_> = result
        .games
        .iter()
        .filter(|g| g.kind == GameKind::SADX)
        .collect();
    assert_eq!(
        sadx_installs.len(),
        1,
        "Same extra library path should not produce duplicates"
    );
}

#[test]
fn test_game_in_vdf_and_extra_library_same_path_deduped() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = tmp.path().join("lib");
    let game_dir = lib
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), "").unwrap();

    // Same library appears in both VDF and extra_libraries
    let root = mock_vdf(lib.to_str().unwrap(), &["71250"]);
    let result = detect_games_from_parsed_vdfs(&[root], std::slice::from_ref(&lib));

    let sadx_installs: Vec<_> = result
        .games
        .iter()
        .filter(|g| g.kind == GameKind::SADX)
        .collect();
    assert_eq!(
        sadx_installs.len(),
        1,
        "Game in both VDF and extra_library should not be duplicated"
    );
}

#[test]
fn test_library_folder_exists_but_game_dir_missing() {
    // Library path exists but the game subdirectory does not
    let tmp = tempfile::tempdir().unwrap();
    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
    // steamapps/common/Sonic Adventure DX/ is NOT created

    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_game_dir_exists_but_exe_missing() {
    // Game directory exists but contains no recognized executable
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("unrelated_file.txt"), "").unwrap();

    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_sa2_alt_exe_sonic_exe_not_detected() {
    // SA2 should require sonic2app.exe.
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("sonic.exe"), "").unwrap();

    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["213610"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SA2);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_sadx_alt_exe_sonic_exe_not_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let game_dir = tmp
        .path()
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("sonic.exe"), "").unwrap();

    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn test_skips_whitespace_only_library_path() {
    let mut apps = HashMap::new();
    apps.insert("71250".to_string(), vdf::VdfValue::String("0".to_string()));

    let mut folder = HashMap::new();
    folder.insert("path".to_string(), vdf::VdfValue::String("   ".to_string()));
    folder.insert("apps".to_string(), vdf::VdfValue::Map(apps));

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder));
    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));

    let (paths, inaccessible) =
        find_all_games_in_libraries(&vdf::VdfValue::Map(root), GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[cfg(unix)]
#[test]
fn test_detect_games_dedupes_symlinked_library_paths() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let real_lib = tmp.path().join("real-lib");
    let link_lib = tmp.path().join("link-lib");

    std::fs::create_dir_all(real_lib.join("steamapps/common/Sonic Adventure DX")).unwrap();
    std::fs::write(
        real_lib.join("steamapps/common/Sonic Adventure DX/Sonic Adventure DX.exe"),
        "",
    )
    .unwrap();
    symlink(&real_lib, &link_lib).unwrap();

    let root_a = mock_vdf(real_lib.to_str().unwrap(), &["71250"]);
    let root_b = mock_vdf(link_lib.to_str().unwrap(), &["71250"]);
    let result = detect_games_from_parsed_vdfs(&[root_a, root_b], &[]);

    let sadx: Vec<_> = result
        .games
        .iter()
        .filter(|g| g.kind == GameKind::SADX)
        .collect();
    assert_eq!(sadx.len(), 1);
}

fn make_steam_library(root: &Path, kind: GameKind) -> PathBuf {
    let game_dir = root.join("steamapps/common").join(kind.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    let exe = match kind {
        GameKind::SADX => "Sonic Adventure DX.exe",
        GameKind::SA2 => "sonic2app.exe",
    };
    std::fs::write(game_dir.join(exe), "").unwrap();
    game_dir
}

#[test]
fn resolve_granted_library_accepts_matching_library_root() {
    let tmp = tempfile::tempdir().unwrap();
    let expected = tmp.path().join("SteamLibrary");
    make_steam_library(&expected, GameKind::SADX);

    let resolved = resolve_granted_steam_library(&expected, &expected).unwrap();
    assert_eq!(resolved, expected);
}

#[test]
fn resolve_granted_library_accepts_parent_that_contains_expected_name() {
    let tmp = tempfile::tempdir().unwrap();
    let parent = tmp.path().join("data");
    let nested = parent.join("SteamLibrary");
    let expected = nested.clone();
    make_steam_library(&nested, GameKind::SADX);

    let resolved = resolve_granted_steam_library(&parent, &expected).unwrap();
    assert_eq!(resolved, nested);
}

#[test]
fn resolve_granted_library_accepts_steamapps_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let library = tmp.path().join("SteamLibrary");
    make_steam_library(&library, GameKind::SADX);

    let resolved = resolve_granted_steam_library(&library.join("steamapps"), &library).unwrap();
    assert_eq!(resolved, library);
}

#[test]
fn resolve_granted_library_rejects_different_steam_library() {
    let tmp = tempfile::tempdir().unwrap();
    let expected = tmp.path().join("SteamLibrary");
    let selected = tmp.path().join("OtherSteamLibrary");
    make_steam_library(&expected, GameKind::SADX);
    make_steam_library(&selected, GameKind::SADX);

    assert!(resolve_granted_steam_library(&selected, &expected).is_none());
}

#[test]
fn resolve_granted_library_rejects_unrelated_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let expected = tmp.path().join("SteamLibrary");
    let selected = tmp.path().join("Documents");
    std::fs::create_dir_all(&selected).unwrap();

    assert!(resolve_granted_steam_library(&selected, &expected).is_none());
}

#[cfg(target_os = "linux")]
fn try_set_host_path_xattr(path: &Path, host_path: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn setxattr(
            path: *const core::ffi::c_char,
            name: *const core::ffi::c_char,
            value: *const u8,
            size: usize,
            flags: i32,
        ) -> i32;
    }

    let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
    let c_name = CString::new("user.document-portal.host-path").unwrap();
    let value = host_path.as_os_str().as_bytes();
    let result = unsafe {
        setxattr(
            c_path.as_ptr(),
            c_name.as_ptr(),
            value.as_ptr(),
            value.len(),
            0,
        )
    };
    result == 0
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_granted_library_accepts_matching_portal_path() {
    let tmp = tempfile::tempdir().unwrap();
    let expected = tmp.path().join("host/SteamLibrary");
    let portal = tmp.path().join("doc/d1a2b3c4/SteamLibrary");
    make_steam_library(&portal, GameKind::SADX);

    if !try_set_host_path_xattr(&portal, &expected) {
        eprintln!("skipping xattr-backed portal grant test; filesystem has no user xattrs");
        return;
    }

    let resolved = resolve_granted_steam_library(&portal, &expected).unwrap();
    assert_eq!(resolved, portal);
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_document_portal_path_maps_existing_nested_path() {
    let tmp = tempfile::tempdir().unwrap();
    let host = tmp.path().join("host/SteamLibrary");
    let portal = tmp.path().join("doc/d1a2b3c4/SteamLibrary");
    let nested = portal.join("steamapps/common/Proton 10.0");
    std::fs::create_dir_all(&nested).unwrap();

    if !try_set_host_path_xattr(&portal, &host) {
        eprintln!("skipping xattr-backed portal path test; filesystem has no user xattrs");
        return;
    }

    assert_eq!(resolve_document_portal_path(&portal, &host), Some(portal));
    assert_eq!(
        resolve_document_portal_path(&nested, &host.join("steamapps/common/Proton 10.0")),
        Some(nested)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_granted_library_scans_document_portal_grants() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = tmp.path().join("runtime");
    let doc = runtime.join("doc");
    let expected = tmp.path().join("host/SteamLibrary");
    let portal = doc.join("grant");
    std::fs::create_dir_all(doc.join("by-app")).unwrap();
    make_steam_library(&portal, GameKind::SADX);

    if !try_set_host_path_xattr(&portal, &expected) {
        eprintln!("skipping xattr-backed portal scan test; filesystem has no user xattrs");
        return;
    }

    let selected = tmp.path().join("selected");
    std::fs::create_dir_all(&selected).unwrap();
    let resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
        resolve_granted_steam_library(&selected, &expected)
    });
    assert_eq!(resolved, Some(portal.clone()));

    let nested_expected = tmp.path().join("host/NestedLibrary");
    let nested_grant = doc.join("nested-grant");
    let nested_portal = nested_grant.join("NestedLibrary");
    make_steam_library(&nested_portal, GameKind::SADX);
    if !try_set_host_path_xattr(&nested_portal, &nested_expected) {
        eprintln!("skipping nested xattr-backed portal scan test; filesystem has no user xattrs");
        return;
    }

    let nested_selected = tmp.path().join("nested-selected");
    std::fs::create_dir_all(&nested_selected).unwrap();
    let nested_resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
        resolve_granted_steam_library(&nested_selected, &nested_expected)
    });
    assert_eq!(nested_resolved, Some(nested_portal));
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_document_portal_host_path_maps_nested_path() {
    let tmp = tempfile::tempdir().unwrap();
    let host = tmp.path().join("host/SteamLibrary");
    let portal = tmp.path().join("doc/d1a2b3c4/SteamLibrary");
    let nested = portal.join("steamapps/common/Proton 10.0");
    std::fs::create_dir_all(&nested).unwrap();

    if !try_set_host_path_xattr(&portal, &host) {
        eprintln!("skipping xattr-backed portal host path test; filesystem has no user xattrs");
        return;
    }

    assert_eq!(
        resolve_document_portal_host_path(&nested),
        Some(host.join("steamapps/common/Proton 10.0"))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_document_portal_host_path_preserves_file_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let host_file = tmp.path().join("host/Proton 10.0/proton");
    let portal_file = tmp.path().join("doc/d1a2b3c4/Proton 10.0/proton");
    std::fs::create_dir_all(portal_file.parent().unwrap()).unwrap();
    std::fs::write(&portal_file, b"#!/bin/sh\n").unwrap();

    if !try_set_host_path_xattr(&portal_file, &host_file) {
        eprintln!("skipping xattr-backed portal file test; filesystem has no user xattrs");
        return;
    }

    assert_eq!(
        resolve_document_portal_host_path(&portal_file),
        Some(host_file)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn extra_library_grant_hides_matching_inaccessible_vdf_library() {
    let tmp = tempfile::tempdir().unwrap();
    let host_library = PathBuf::from("/data/SteamLibrary");
    let portal = tmp.path().join("run-user-doc").join("abc123");
    make_steam_library(&portal, GameKind::SADX);

    if !try_set_host_path_xattr(&portal, &host_library) {
        eprintln!("skipping xattr-backed portal grant test; filesystem has no user xattrs");
        return;
    }

    let root = mock_vdf(host_library.to_str().unwrap(), &["71250"]);
    let result = detect_games_from_parsed_vdfs(&[root], std::slice::from_ref(&portal));

    assert!(result.games.iter().any(|game| game.kind == GameKind::SADX));
    assert!(
        result
            .inaccessible
            .iter()
            .all(|game| game.library_path != host_library)
    );
}

#[test]
fn test_empty_vdf_roots_and_empty_extra_libraries() {
    let result = detect_games_from_parsed_vdfs(&[], &[]);
    assert!(result.games.is_empty());
    assert!(result.inaccessible.is_empty());
}

#[test]
fn test_multiple_games_in_multiple_libraries_single_vdf() {
    // SADX in lib1, SA2 in lib2 — both in the same VDF
    let tmp = tempfile::tempdir().unwrap();
    let lib1 = tmp.path().join("lib1");
    let lib2 = tmp.path().join("lib2");

    let sadx_dir = lib1
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    let sa2_dir = lib2
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&sadx_dir).unwrap();
    std::fs::create_dir_all(&sa2_dir).unwrap();
    std::fs::write(sadx_dir.join("Sonic Adventure DX.exe"), "").unwrap();
    std::fs::write(sa2_dir.join("sonic2app.exe"), "").unwrap();

    let mut apps1 = HashMap::new();
    apps1.insert("71250".to_string(), vdf::VdfValue::String("0".to_string()));
    let mut folder1 = HashMap::new();
    folder1.insert(
        "path".to_string(),
        vdf::VdfValue::String(lib1.to_str().unwrap().to_string()),
    );
    folder1.insert("apps".to_string(), vdf::VdfValue::Map(apps1));

    let mut apps2 = HashMap::new();
    apps2.insert("213610".to_string(), vdf::VdfValue::String("0".to_string()));
    let mut folder2 = HashMap::new();
    folder2.insert(
        "path".to_string(),
        vdf::VdfValue::String(lib2.to_str().unwrap().to_string()),
    );
    folder2.insert("apps".to_string(), vdf::VdfValue::Map(apps2));

    let mut folders = HashMap::new();
    folders.insert("0".to_string(), vdf::VdfValue::Map(folder1));
    folders.insert("1".to_string(), vdf::VdfValue::Map(folder2));
    let mut root_map = HashMap::new();
    root_map.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));
    let vdf = vdf::VdfValue::Map(root_map);

    let result = detect_games_from_parsed_vdfs(&[vdf], &[]);
    assert_eq!(result.games.len(), 2);
    assert!(result.games.iter().any(|g| g.kind == GameKind::SADX));
    assert!(result.games.iter().any(|g| g.kind == GameKind::SA2));
    assert!(result.inaccessible.is_empty());
}
