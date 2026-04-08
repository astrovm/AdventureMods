use adventure_mods::steam::game::GameKind;
use adventure_mods::steam::library::detect_games_from_vdf_with_extra_libraries;

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
