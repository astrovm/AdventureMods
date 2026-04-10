use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::steam::game::{Game, GameKind};
use crate::steam::vdf;
use anyhow::Context;

#[derive(Debug, Clone)]
pub struct InaccessibleGame {
    pub kind: GameKind,
    pub library_path: PathBuf,
}

pub(crate) fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };

    // Check all common Steam paths, including both native and Flatpak installs
    let candidates = [
        home.join(".steam/debian-installation"),
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];

    candidates.into_iter().filter(|p| p.is_dir()).collect()
}

fn library_folders_paths() -> Vec<PathBuf> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut result = vec![];

    for root in steam_roots() {
        let path = root.join("steamapps/libraryfolders.vdf");
        if path.is_file() {
            // Deduplicate by canonical path in case multiple Steam roots share a symlink
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if seen.insert(canonical) {
                result.push(path);
            }
        }
    }

    result
}

fn find_all_games_in_libraries(
    libraries: &vdf::VdfValue,
    kind: GameKind,
) -> (Vec<PathBuf>, Vec<InaccessibleGame>) {
    let app_id = kind.app_id().to_string();
    let mut paths = vec![];
    let mut inaccessible = vec![];

    let folders = match libraries.get("libraryfolders").and_then(|v| v.as_map()) {
        Some(f) => f,
        None => return (vec![], vec![]),
    };

    for folder in folders.values() {
        let folder_map = match folder.as_map() {
            Some(m) => m,
            None => continue,
        };

        let apps = match folder_map.get("apps").and_then(|v| v.as_map()) {
            Some(a) => a,
            None => continue,
        };

        if apps.contains_key(&app_id) {
            let lib_path = match folder_map.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.trim().is_empty() => Path::new(p),
                None => continue,
                Some(_) => continue,
            };

            if !lib_path.exists() {
                tracing::warn!(
                    "Steam library for {} at {} is inaccessible (partition may not be mounted)",
                    kind.name(),
                    lib_path.display()
                );
                inaccessible.push(InaccessibleGame {
                    kind,
                    library_path: lib_path.to_path_buf(),
                });
            } else if let Some(game_path) = find_game_in_library_path(lib_path, kind) {
                paths.push(game_path);
            }
        }
    }

    (paths, inaccessible)
}

fn find_game_in_library_path(lib_path: &Path, kind: GameKind) -> Option<PathBuf> {
    let game_path = lib_path.join("steamapps/common").join(kind.install_dir());

    if !game_path.is_dir() {
        return None;
    }

    let executable = match kind {
        GameKind::SADX => "Sonic Adventure DX.exe",
        GameKind::SA2 => "sonic2app.exe",
    };

    let exe_path = game_path.join(executable);
    if exe_path.exists() {
        let real_path = game_path
            .canonicalize()
            .unwrap_or_else(|_| game_path.clone());
        tracing::info!(
            "Found {} at {} (Real path: {})",
            kind.name(),
            game_path.display(),
            real_path.display()
        );
        Some(game_path)
    } else {
        tracing::warn!(
            "Found directory for {} but no executable found at {}. Likely a stale Steam library entry.",
            kind.name(),
            game_path.display()
        );
        None
    }
}

fn detect_games_from_parsed_vdfs(
    roots: &[vdf::VdfValue],
    extra_libraries: &[PathBuf],
) -> DetectionResult {
    let mut result = DetectionResult::default();

    for kind in [GameKind::SADX, GameKind::SA2] {
        let mut seen_canonical: HashSet<PathBuf> = HashSet::new();
        let mut kind_inaccessible: Vec<InaccessibleGame> = vec![];

        for root in roots {
            let (paths, inaccessible) = find_all_games_in_libraries(root, kind);

            for path in paths {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen_canonical.insert(canonical) {
                    result.games.push(Game { kind, path });
                }
            }

            kind_inaccessible.extend(inaccessible);
        }

        for lib_path in extra_libraries {
            if let Some(path) = find_game_in_library_path(lib_path, kind) {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen_canonical.insert(canonical) {
                    result.games.push(Game { kind, path });
                }
            }
        }

        let kind_found = result.games.iter().any(|g| g.kind == kind);
        if !kind_found && kind_inaccessible.is_empty() {
            tracing::info!("{} not found", kind.name());
        }

        // Deduplicate inaccessible entries by canonical library path.
        let mut seen_inacc: HashSet<PathBuf> = HashSet::new();
        for inc in kind_inaccessible {
            let canonical = inc
                .library_path
                .canonicalize()
                .unwrap_or_else(|_| inc.library_path.clone());
            if seen_inacc.insert(canonical) {
                result.inaccessible.push(inc);
            }
        }
    }

    result
}

#[derive(Debug, Clone, Default)]
pub struct DetectionResult {
    pub games: Vec<Game>,
    pub inaccessible: Vec<InaccessibleGame>,
}

pub fn detect_games_from_vdf_with_extra_libraries(
    vdf_path: &Path,
    extra_libraries: &[PathBuf],
) -> DetectionResult {
    let content = match std::fs::read_to_string(vdf_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read {}: {}", vdf_path.display(), e);
            return DetectionResult::default();
        }
    };

    let root = match vdf::parse(&content) {
        Some(r) => r,
        None => {
            tracing::warn!("Failed to parse VDF");
            return DetectionResult::default();
        }
    };

    detect_games_from_parsed_vdfs(&[root], extra_libraries)
}

pub fn detect_games_from_vdf_strict(
    vdf_path: &Path,
    extra_libraries: &[PathBuf],
) -> anyhow::Result<DetectionResult> {
    let content = std::fs::read_to_string(vdf_path)
        .with_context(|| format!("Failed to read {}", vdf_path.display()))?;

    let root = vdf::parse(&content)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", vdf_path.display()))?;

    Ok(detect_games_from_parsed_vdfs(&[root], extra_libraries))
}

pub fn detect_games_with_extra_libraries(extra_libraries: &[PathBuf]) -> DetectionResult {
    let vdf_paths = library_folders_paths();

    if vdf_paths.is_empty() {
        tracing::warn!("Could not find any libraryfolders.vdf");
    }

    let mut roots = vec![];
    for vdf_path in &vdf_paths {
        match std::fs::read_to_string(vdf_path) {
            Ok(content) => match vdf::parse(&content) {
                Some(root) => roots.push(root),
                None => tracing::warn!("Failed to parse VDF at {}", vdf_path.display()),
            },
            Err(e) => tracing::warn!("Failed to read {}: {}", vdf_path.display(), e),
        }
    }

    detect_games_from_parsed_vdfs(&roots, extra_libraries)
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
        assert!(result
            .inaccessible
            .iter()
            .any(|game| game.kind == GameKind::SADX));
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
}
