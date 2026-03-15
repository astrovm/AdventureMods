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
                return Some(game_path);
            }
        }
    }
    None
}

pub fn detect_games() -> Vec<Game> {
    let vdf_path = match library_folders_path() {
        Some(p) => p,
        None => {
            tracing::warn!("Could not find libraryfolders.vdf");
            return Vec::new();
        }
    };

    let content = match std::fs::read_to_string(&vdf_path) {
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
            tracing::info!("Found {} at {}", kind.name(), path.display());
            games.push(Game { kind, path });
        } else {
            tracing::info!("{} not found", kind.name());
        }
    }

    games
}
