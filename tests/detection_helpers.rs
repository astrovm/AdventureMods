use adventure_mods::steam::game::GameKind;
use adventure_mods::steam::library::{
    detect_games_from_vdf_strict, detect_games_from_vdf_with_extra_libraries,
};

#[test]
fn detects_sa2_from_explicit_libraryfolders_path() {
    let tmp = tempfile::tempdir().unwrap();
    let library_root = tmp.path().join("SteamLibrary");
    let game_dir = library_root
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("sonic2app.exe"), b"").unwrap();

    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n        \"apps\"\n        {{\n            \"213610\"\t\"0\"\n        }}\n    }}\n}}\n",
            library_root.display()
        ),
    )
    .unwrap();

    let result = detect_games_from_vdf_with_extra_libraries(&vdf_path, &[]);

    assert_eq!(result.games.len(), 1);
    assert_eq!(result.games[0].kind, GameKind::SA2);
    assert_eq!(result.games[0].path, game_dir);
}

#[test]
fn strict_detect_handles_utf8_bom_and_paths_with_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    let library_root = tmp.path().join("Steam Library With Spaces");
    let game_dir = library_root
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("Sonic Adventure DX.exe"), b"").unwrap();

    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\u{feff}\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n        \"apps\"\n        {{\n            \"71250\"\t\"0\"\n        }}\n    }}\n}}\n",
            library_root.display()
        ),
    )
    .unwrap();

    let result = detect_games_from_vdf_strict(&vdf_path, &[]).unwrap();

    assert_eq!(result.games.len(), 1);
    assert_eq!(result.games[0].kind, GameKind::SADX);
    assert_eq!(result.games[0].path, game_dir);
}

#[test]
fn strict_detect_merges_extra_libraries() {
    let tmp = tempfile::tempdir().unwrap();
    let vdf_library_root = tmp.path().join("SteamLibraryA");
    let extra_library_root = tmp.path().join("SteamLibraryB");
    let vdf_game_dir = vdf_library_root
        .join("steamapps/common")
        .join(GameKind::SA2.install_dir());
    let extra_game_dir = extra_library_root
        .join("steamapps/common")
        .join(GameKind::SADX.install_dir());
    std::fs::create_dir_all(&vdf_game_dir).unwrap();
    std::fs::create_dir_all(&extra_game_dir).unwrap();
    std::fs::write(vdf_game_dir.join("sonic2app.exe"), b"").unwrap();
    std::fs::write(extra_game_dir.join("Sonic Adventure DX.exe"), b"").unwrap();

    let vdf_path = tmp.path().join("libraryfolders.vdf");
    std::fs::write(
        &vdf_path,
        format!(
            "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n        \"apps\"\n        {{\n            \"213610\"\t\"0\"\n        }}\n    }}\n}}\n",
            vdf_library_root.display()
        ),
    )
    .unwrap();

    let result = detect_games_from_vdf_strict(&vdf_path, &[extra_library_root]).unwrap();

    assert_eq!(result.games.len(), 2);
    assert!(result.games.iter().any(|game| game.kind == GameKind::SA2));
    assert!(result.games.iter().any(|game| game.kind == GameKind::SADX));
}
