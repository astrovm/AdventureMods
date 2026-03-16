use std::path::{Path, PathBuf};

use crate::steam::game::{Game, GameKind};
use crate::steam::vdf;

fn steam_root() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Check common Steam paths
    let candidates = [
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];

    candidates.into_iter().find(|p| p.is_dir())
}

fn library_folders_path() -> Option<PathBuf> {
    let root = steam_root()?;
    let path = root.join("steamapps/libraryfolders.vdf");
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

fn find_game_in_libraries(
    libraries: &vdf::VdfValue,
    kind: GameKind,
) -> Option<PathBuf> {
    let app_id = kind.app_id().to_string();
    let folders = libraries.get("libraryfolders")?.as_map()?;

    for (_, folder) in folders {
        let folder_map = folder.as_map()?;
        let apps = folder_map.get("apps")?.as_map()?;

        if apps.contains_key(&app_id) {
            let lib_path = folder_map.get("path")?.as_str()?;
            let game_path = Path::new(lib_path)
                .join("steamapps/common")
                .join(kind.install_dir());

            if game_path.is_dir() {
                // Verify it's a real, readable installation and not a ghost entry
                // SADX has 'Sonic Adventure DX.exe' (Steam) or 'sonic.exe' (2004)
                // SA2 has 'Sonic Adventure 2.exe'
                let executable = match kind {
                    GameKind::SADX => "Sonic Adventure DX.exe",
                    GameKind::SA2 => "Sonic Adventure 2.exe",
                };
                
                let exe_path = game_path.join(executable);
                let alt_exe = game_path.join("sonic.exe"); // Fallback for already converted SADX
                
                if exe_path.exists() || alt_exe.exists() {
                    let real_path = game_path.canonicalize().unwrap_or_else(|_| game_path.clone());
                    tracing::info!("Found {} at {} (Real path: {})", kind.name(), game_path.display(), real_path.display());
                    return Some(game_path);
                } else {
                    tracing::warn!("Found directory for {} but no executable found at {}. Likely a stale Steam library entry.", kind.name(), game_path.display());
                }
            }
        }
    }
    None
}

/// Detect games from a specific VDF file path.
pub fn detect_games_from_vdf(vdf_path: &Path) -> Vec<Game> {
    let content = match std::fs::read_to_string(vdf_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read {}: {}", vdf_path.display(), e);
            return Vec::new();
        }
    };

    let root = match vdf::parse(&content) {
        Some(r) => r,
        None => {
            tracing::warn!("Failed to parse VDF");
            return Vec::new();
        }
    };

    let mut games = Vec::new();

    for kind in [GameKind::SADX, GameKind::SA2] {
        if let Some(path) = find_game_in_libraries(&root, kind) {
            games.push(Game { kind, path });
        } else {
            tracing::info!("{} not found", kind.name());
        }
    }

    games
}

pub fn detect_games() -> Vec<Game> {
    let vdf_path = match library_folders_path() {
        Some(p) => p,
        None => {
            tracing::warn!("Could not find libraryfolders.vdf");
            return Vec::new();
        }
    };

    detect_games_from_vdf(&vdf_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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

    #[test]
    fn test_find_sadx_in_libraries() {
        let tmp = tempfile::tempdir().unwrap();
        let game_dir = tmp
            .path()
            .join("steamapps/common")
            .join(GameKind::SADX.install_dir());
        std::fs::create_dir_all(&game_dir).unwrap();

        let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
        let result = find_game_in_libraries(&vdf, GameKind::SADX);
        assert_eq!(result, Some(game_dir));
    }

    #[test]
    fn test_find_sa2_in_libraries() {
        let tmp = tempfile::tempdir().unwrap();
        let game_dir = tmp
            .path()
            .join("steamapps/common")
            .join(GameKind::SA2.install_dir());
        std::fs::create_dir_all(&game_dir).unwrap();

        let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["213610"]);
        let result = find_game_in_libraries(&vdf, GameKind::SA2);
        assert_eq!(result, Some(game_dir));
    }

    #[test]
    fn test_game_not_in_libraries() {
        let tmp = tempfile::tempdir().unwrap();
        let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["400", "500"]);
        assert!(find_game_in_libraries(&vdf, GameKind::SADX).is_none());
        assert!(find_game_in_libraries(&vdf, GameKind::SA2).is_none());
    }

    #[test]
    fn test_missing_libraryfolders_key() {
        let root = vdf::VdfValue::Map(HashMap::new());
        assert!(find_game_in_libraries(&root, GameKind::SADX).is_none());
    }

    #[test]
    fn test_find_game_app_present_but_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        // App ID is in VDF but game directory doesn't exist on disk
        let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250"]);
        assert!(find_game_in_libraries(&vdf, GameKind::SADX).is_none());
    }

    #[test]
    fn test_find_game_missing_apps_key() {
        // Folder has "path" but no "apps" subkey
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
        assert!(find_game_in_libraries(&vdf, GameKind::SADX).is_none());
    }

    #[test]
    fn test_find_game_missing_path_key() {
        // Folder has "apps" with matching ID but no "path" key
        let mut apps = HashMap::new();
        apps.insert("71250".to_string(), vdf::VdfValue::String("0".to_string()));

        let mut folder = HashMap::new();
        folder.insert("apps".to_string(), vdf::VdfValue::Map(apps));

        let mut folders = HashMap::new();
        folders.insert("0".to_string(), vdf::VdfValue::Map(folder));

        let mut root = HashMap::new();
        root.insert("libraryfolders".to_string(), vdf::VdfValue::Map(folders));

        let vdf = vdf::VdfValue::Map(root);
        assert!(find_game_in_libraries(&vdf, GameKind::SADX).is_none());
    }

    #[test]
    fn test_find_game_libraryfolders_is_string() {
        // libraryfolders is a string instead of a map — should return None
        let mut root = HashMap::new();
        root.insert(
            "libraryfolders".to_string(),
            vdf::VdfValue::String("oops".to_string()),
        );
        let vdf = vdf::VdfValue::Map(root);
        assert!(find_game_in_libraries(&vdf, GameKind::SADX).is_none());
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

        let vdf = mock_vdf(tmp.path().to_str().unwrap(), &["71250", "213610"]);
        assert_eq!(find_game_in_libraries(&vdf, GameKind::SADX), Some(sadx_dir));
        assert_eq!(find_game_in_libraries(&vdf, GameKind::SA2), Some(sa2_dir));
    }

    #[test]
    fn test_detect_games_from_vdf_both_present() {
        let tmp = tempfile::tempdir().unwrap();
        let lib_path = tmp.path().join("lib");

        // Create both game directories
        let sadx_dir = lib_path
            .join("steamapps/common")
            .join(GameKind::SADX.install_dir());
        let sa2_dir = lib_path
            .join("steamapps/common")
            .join(GameKind::SA2.install_dir());
        std::fs::create_dir_all(&sadx_dir).unwrap();
        std::fs::create_dir_all(&sa2_dir).unwrap();

        // Write a VDF file referencing both games
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

        let games = detect_games_from_vdf(&vdf_path);
        assert_eq!(games.len(), 2);
        assert!(games.iter().any(|g| g.kind == GameKind::SADX));
        assert!(games.iter().any(|g| g.kind == GameKind::SA2));
    }

    #[test]
    fn test_detect_games_from_vdf_none_present() {
        let tmp = tempfile::tempdir().unwrap();

        // Write a VDF file with no matching app IDs
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

        let games = detect_games_from_vdf(&vdf_path);
        assert!(games.is_empty());
    }

    #[test]
    fn test_detect_games_from_vdf_corrupt() {
        let tmp = tempfile::tempdir().unwrap();
        let vdf_path = tmp.path().join("libraryfolders.vdf");
        std::fs::write(&vdf_path, "this is not valid VDF content {{{").unwrap();

        let games = detect_games_from_vdf(&vdf_path);
        assert!(games.is_empty());
    }

    #[test]
    fn test_multiple_libraries() {
        let tmp = tempfile::tempdir().unwrap();
        let game_dir = tmp
            .path()
            .join("steamapps/common")
            .join(GameKind::SA2.install_dir());
        std::fs::create_dir_all(&game_dir).unwrap();

        // First library has no SA2, second library has it
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

        let result = find_game_in_libraries(&vdf, GameKind::SA2);
        assert_eq!(result, Some(game_dir));
    }
}
