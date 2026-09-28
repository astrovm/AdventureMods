use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    let _guard = crate::test_env::lock();
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
fn steam_roots_find_native_and_flatpak_steam() {
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
fn steam_roots_skip_missing_installations() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(steam_roots_in(tmp.path()).is_empty());
}

#[test]
fn detect_games_reads_synthetic_home_steam_libraries() {
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
fn find_sadx_in_libraries() {
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
fn find_sa2_in_libraries() {
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
fn detect_games_with_extra_library_finds_inaccessible_game() {
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
fn detect_games_keeps_inaccessible_alongside_detected() {
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
fn game_not_in_libraries() {
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
fn missing_libraryfolders_key() {
    let root = vdf::VdfValue::Map(HashMap::new());
    let (paths, inaccessible) = find_all_games_in_libraries(&root, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn find_game_app_present_but_dir_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn find_game_missing_apps_key() {
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
fn find_game_missing_path_key() {
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
fn find_game_libraryfolders_is_string() {
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
fn find_game_skips_non_map_library_entries() {
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
fn find_game_skips_non_map_apps() {
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
fn find_both_games_in_same_library() {
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
fn detect_games_from_vdf_both_present() {
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
fn detect_games_from_vdf_none_present() {
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
fn detect_games_from_vdf_corrupt() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(&vdf_path, "this is not valid VDF content {{{").unwrap();

    let result = detect_games_from_vdf_with_extra_libraries(&vdf_path, &[]);
    assert!(result.games.is_empty());
    assert!(result.inaccessible.is_empty());
}

#[test]
fn library_detection_handles_missing_fields_and_stale_entries() {
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
fn library_detection_reports_stale_game_directory() {
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
fn multiple_libraries() {
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
fn inaccessible_library() {
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
fn duplicate_installations_across_steam_roots() {
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
fn duplicate_installations_same_path_deduped() {
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
fn inaccessible_deduped_across_roots() {
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
fn no_vdf_roots_finds_game_via_extra_library() {
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
fn extra_libraries_duplicate_paths_deduped() {
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
fn game_in_vdf_and_extra_library_same_path_deduped() {
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
fn library_folder_exists_but_game_dir_missing() {
    // Library path exists but the game subdirectory does not
    let tmp = tempfile::tempdir().unwrap();
    let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
    // steamapps/common/Sonic Adventure DX/ is NOT created

    let (paths, inaccessible) = find_all_games_in_libraries(&vdf, GameKind::SADX);
    assert!(paths.is_empty());
    assert!(inaccessible.is_empty());
}

#[test]
fn game_dir_exists_but_exe_missing() {
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
fn sa2_alt_exe_sonic_exe_not_detected() {
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
fn sadx_alt_exe_sonic_exe_not_detected() {
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
fn skips_whitespace_only_library_path() {
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
fn detect_games_dedupes_symlinked_library_paths() {
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

/// Tag `path` the way the document portal does. CI and development checkouts
/// live on filesystems with user xattrs (ext4, btrfs, tmpfs), so a failure
/// here is a broken test environment, not a skipped test.
#[cfg(target_os = "linux")]
fn set_host_path_xattr(path: &Path, host_path: &Path) {
    use std::os::unix::ffi::OsStrExt;

    set_raw_host_path_xattr(path, host_path.as_os_str().as_bytes());
}

#[cfg(target_os = "linux")]
fn set_raw_host_path_xattr(path: &Path, value: &[u8]) {
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
    let result = unsafe {
        setxattr(
            c_path.as_ptr(),
            c_name.as_ptr(),
            value.as_ptr(),
            value.len(),
            0,
        )
    };
    assert_eq!(
        result,
        0,
        "document-portal tests need user xattr support on the temp filesystem: {}",
        std::io::Error::last_os_error()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_granted_library_accepts_matching_portal_path() {
    let tmp = tempfile::tempdir().unwrap();
    let expected = tmp.path().join("host/SteamLibrary");
    let portal = tmp.path().join("doc/d1a2b3c4/SteamLibrary");
    make_steam_library(&portal, GameKind::SADX);

    set_host_path_xattr(&portal, &expected);

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

    set_host_path_xattr(&portal, &host);

    assert_eq!(resolve_document_portal_path(&portal, &host), Some(portal));
    assert_eq!(
        resolve_document_portal_path(&nested, &host.join("steamapps/common/Proton 10.0")),
        Some(nested)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_granted_library_scans_grants_only_for_the_selected_host_library() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = tmp.path().join("runtime");
    let doc = runtime.join("doc");
    let expected = tmp.path().join("host/SteamLibrary");
    let portal = doc.join("grant");
    std::fs::create_dir_all(doc.join("by-app")).unwrap();
    make_steam_library(&portal, GameKind::SADX);

    set_host_path_xattr(&portal, &expected);

    let selected = tmp.path().join("selected");
    std::fs::create_dir_all(&selected).unwrap();
    let resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
        resolve_granted_steam_library(&selected, &expected)
    });
    assert_eq!(
        resolved, None,
        "an unrelated choice must not reuse an old grant"
    );
    let resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
        resolve_granted_steam_library(&expected, &expected)
    });
    assert_eq!(resolved, Some(portal.clone()));
    for selected in [
        expected.parent().unwrap().to_path_buf(),
        expected.join("steamapps"),
    ] {
        let resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
            resolve_granted_steam_library(&selected, &expected)
        });
        assert_eq!(resolved, Some(portal.clone()));
    }
    let wrong_steamapps = tmp.path().join("other/steamapps");
    assert_eq!(
        with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
            resolve_granted_steam_library(&wrong_steamapps, &expected)
        }),
        None
    );

    let nested_expected = tmp.path().join("host/NestedLibrary");
    let nested_grant = doc.join("nested-grant");
    let nested_portal = nested_grant.join("NestedLibrary");
    make_steam_library(&nested_portal, GameKind::SADX);
    set_host_path_xattr(&nested_portal, &nested_expected);

    let nested_selected = tmp.path().join("nested-selected");
    std::fs::create_dir_all(&nested_selected).unwrap();
    let nested_resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
        resolve_granted_steam_library(&nested_selected, &nested_expected)
    });
    assert_eq!(nested_resolved, None);
    let nested_resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
        resolve_granted_steam_library(&nested_expected, &nested_expected)
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

    set_host_path_xattr(&portal, &host);

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

    set_host_path_xattr(&portal_file, &host_file);

    assert_eq!(
        resolve_document_portal_host_path(&portal_file),
        Some(host_file)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn extra_library_grant_hides_matching_inaccessible_vdf_library() {
    let tmp = tempfile::tempdir().unwrap();
    // Unmounted from the sandbox's view; never a real library on the host.
    let host_library = tmp.path().join("unmounted/SteamLibrary");
    let portal = tmp.path().join("run-user-doc").join("abc123");
    make_steam_library(&portal, GameKind::SADX);

    set_host_path_xattr(&portal, &host_library);

    let root = mock_vdf(host_library.to_str().unwrap(), &["71250"]);
    let (result, logs) =
        capture_logs(|| detect_games_from_parsed_vdfs(&[root], std::slice::from_ref(&portal)));

    assert!(result.games.iter().any(|game| game.kind == GameKind::SADX));
    assert!(
        result
            .inaccessible
            .iter()
            .all(|game| game.library_path != host_library)
    );
    // A granted library is neither reported as inaccessible nor found twice.
    assert!(!logs.contains("is inaccessible"), "{logs}");
    assert_eq!(logs.matches("Found ").count(), 1, "{logs}");
}

#[test]
fn empty_vdf_roots_and_empty_extra_libraries() {
    let result = detect_games_from_parsed_vdfs(&[], &[]);
    assert!(result.games.is_empty());
    assert!(result.inaccessible.is_empty());
}

#[test]
fn multiple_games_in_multiple_libraries_single_vdf() {
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

use crate::test_log::capture_logs;

#[test]
fn library_detection_logs_found_stale_and_unmounted_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let found = tmp.path().join("found");
    let stale = tmp.path().join("stale");
    let unmounted = tmp.path().join("unmounted");
    let game_dir = make_steam_library(&found, GameKind::SADX);
    std::fs::create_dir_all(
        stale
            .join("steamapps/common")
            .join(GameKind::SADX.install_dir()),
    )
    .unwrap();

    let mut folders = HashMap::new();
    for (key, path) in [("0", &found), ("1", &stale), ("2", &unmounted)] {
        let mut apps = HashMap::new();
        apps.insert("71250".to_string(), vdf::VdfValue::String("0".to_string()));
        let mut folder = HashMap::new();
        folder.insert(
            "path".to_string(),
            vdf::VdfValue::String(path.to_string_lossy().into_owned()),
        );
        folder.insert("apps".to_string(), vdf::VdfValue::Map(apps));
        folders.insert(key.to_string(), vdf::VdfValue::Map(folder));
    }
    let mut root = HashMap::new();
    root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));

    let (result, logs) =
        capture_logs(|| detect_games_from_parsed_vdfs(&[vdf::VdfValue::Map(root)], &[]));

    let paths: Vec<_> = result.games.iter().map(|game| game.path.clone()).collect();
    assert_eq!(paths, vec![game_dir.clone()]);
    assert_eq!(result.inaccessible.len(), 1);
    assert_eq!(result.inaccessible[0].library_path, unmounted);
    assert!(logs.contains(&format!(
        "Found {} at {}",
        GameKind::SADX.name(),
        game_dir.display()
    )));
    assert!(logs.contains("Likely a stale Steam library entry"));
    assert!(logs.contains(&format!("at {} is inaccessible", unmounted.display())));
}

#[test]
fn home_scan_logs_unreadable_library_file() {
    let home = tempfile::tempdir().unwrap();
    let steamapps = home.path().join(".local/share/Steam/steamapps");
    std::fs::create_dir_all(&steamapps).unwrap();
    let vdf_path = steamapps.join("libraryfolders.vdf");
    // Not UTF-8, so the file exists but cannot be read as text.
    std::fs::write(&vdf_path, [0xff, 0xfe, 0x00, 0x80]).unwrap();

    let (result, logs) = capture_logs(|| {
        with_environment("HOME", Some(home.path()), || {
            detect_games_with_extra_libraries(&[])
        })
    });

    assert!(result.games.is_empty());
    assert!(logs.contains(&format!("Failed to read {}", vdf_path.display())));
    assert!(logs.contains("libraryfolders.vdf"));
}

#[test]
fn try_canonicalize_keeps_unresolvable_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing");

    assert_eq!(try_canonicalize(&missing), missing);
    assert_eq!(
        try_canonicalize(tmp.path()),
        tmp.path().canonicalize().unwrap()
    );
}

#[test]
fn canonicalize_with_suffix_resolves_the_nearest_existing_ancestor() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    let link = tmp.path().join("link");
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    assert_eq!(
        canonicalize_with_suffix(&link.join("missing/SteamLibrary")),
        real.canonicalize().unwrap().join("missing/SteamLibrary")
    );
    // A trailing `..` below a missing directory is kept, not dropped.
    assert_eq!(
        canonicalize_with_suffix(&link.join("missing/..")),
        real.canonicalize().unwrap().join("missing/..")
    );
    // A relative path with no existing ancestor is returned as is.
    let relative = Path::new("adventure-mods-missing/SteamLibrary");
    assert_eq!(canonicalize_with_suffix(relative), relative);
}

#[cfg(target_os = "linux")]
#[test]
fn document_portal_host_path_ignores_trailing_nul_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let with_nul = tmp.path().join("with-nul");
    let only_nul = tmp.path().join("only-nul");
    std::fs::create_dir_all(&with_nul).unwrap();
    std::fs::create_dir_all(&only_nul).unwrap();

    set_raw_host_path_xattr(&with_nul, b"/host/SteamLibrary\0\0");
    set_raw_host_path_xattr(&only_nul, b"\0");

    assert_eq!(
        document_portal_host_path(&with_nul),
        Some(PathBuf::from("/host/SteamLibrary"))
    );
    assert_eq!(document_portal_host_path(&only_nul), None);
    assert_eq!(document_portal_host_path(tmp.path()), None);
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_document_portal_path_rejects_missing_and_unrelated_host_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let host = tmp.path().join("host/SteamLibrary");
    let portal = tmp.path().join("doc/d1a2b3c4/SteamLibrary");
    std::fs::create_dir_all(&portal).unwrap();
    set_host_path_xattr(&portal, &host);

    // Inside the grant, but not present in the sandbox.
    assert_eq!(
        resolve_document_portal_path(&portal, &host.join("steamapps/common/Proton 9.0")),
        None
    );
    // Outside the grant entirely.
    assert_eq!(
        resolve_document_portal_path(&portal, &tmp.path().join("host/OtherLibrary")),
        None
    );
}

#[cfg(target_os = "linux")]
#[test]
fn extra_library_root_for_host_matches_direct_and_nested_grants() {
    let tmp = tempfile::tempdir().unwrap();
    let host_parent = tmp.path().join("host/games");
    let host_library = host_parent.join("SteamLibrary");
    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();

    // The same path needs no portal.
    assert_eq!(
        extra_library_root_for_host(&plain, &plain),
        Some(plain.clone())
    );
    // An unrelated folder without a grant does not match.
    assert_eq!(extra_library_root_for_host(&plain, &host_library), None);

    // A grant of the library's parent maps to the nested library...
    let grant = tmp.path().join("doc/abc123");
    make_steam_library(&grant.join("SteamLibrary"), GameKind::SADX);
    set_host_path_xattr(&grant, &host_parent);
    assert_eq!(
        extra_library_root_for_host(&grant, &host_library),
        Some(grant.join("SteamLibrary"))
    );
    // ...but only when that folder is a Steam library in the sandbox.
    assert_eq!(
        extra_library_root_for_host(&grant, &host_parent.join("NotALibrary")),
        None
    );
}

#[cfg(target_os = "linux")]
#[test]
fn resolve_granted_library_ignores_unrelated_grants_and_nested_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = tmp.path().join("runtime");
    let expected = tmp.path().join("host/SteamLibrary");
    let unrelated = runtime.join("doc/unrelated");
    make_steam_library(&unrelated, GameKind::SA2);
    set_host_path_xattr(&unrelated, &tmp.path().join("host/OtherLibrary"));
    // The portal's per-app view and stray files are never grants, even when
    // they look like the expected library.
    let by_app = runtime.join("doc/by-app");
    make_steam_library(&by_app, GameKind::SADX);
    set_host_path_xattr(&by_app, &expected);
    std::fs::write(runtime.join("doc/stray-file"), b"").unwrap();

    // The expected library is not visible and no grant covers it.
    let resolved = with_environment("XDG_RUNTIME_DIR", Some(&runtime), || {
        resolve_granted_steam_library(&expected, &expected)
    });
    assert_eq!(resolved, None);

    // A selected folder holding a different library with the same name.
    let selected = tmp.path().join("selected");
    make_steam_library(&selected.join("SteamLibrary"), GameKind::SADX);
    assert_eq!(resolve_granted_steam_library(&selected, &expected), None);
}

#[test]
fn resolve_granted_library_rejects_folders_for_a_root_expected_path() {
    let tmp = tempfile::tempdir().unwrap();

    // `/` has no folder name to look for inside the selection.
    assert_eq!(
        resolve_granted_steam_library(tmp.path(), Path::new("/")),
        None
    );
}

#[test]
fn library_folders_paths_dedupes_linked_steam_roots() {
    let home = tempfile::tempdir().unwrap();
    let native = home.path().join(".local/share/Steam");
    std::fs::create_dir_all(native.join("steamapps")).unwrap();
    std::fs::write(
        native.join("steamapps/libraryfolders.vdf"),
        "\"libraryfolders\"\n{\n}\n",
    )
    .unwrap();
    std::fs::create_dir_all(home.path().join(".steam")).unwrap();
    std::os::unix::fs::symlink(&native, home.path().join(".steam/steam")).unwrap();

    let paths = with_environment("HOME", Some(home.path()), library_folders_paths);

    assert_eq!(
        paths,
        vec![
            home.path()
                .join(".steam/steam/steamapps/libraryfolders.vdf")
        ]
    );
}

#[test]
fn strict_detect_reports_unreadable_libraryfolders() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf_path = tmp.path().join("libraryfolders.vdf");

    let error = detect_games_from_vdf_strict(&vdf_path, &[]).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("Failed to read {}", vdf_path.display())
    );

    std::fs::write(&vdf_path, "not valid vdf").unwrap();
    let error = detect_games_from_vdf_strict(&vdf_path, &[]).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("Failed to parse {}", vdf_path.display())
    );
}
