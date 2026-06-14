use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::steam::game::{Game, GameKind};
use crate::steam::vdf;
use anyhow::Context;

// Canonicalize as much of `path` as possible by walking up to the nearest
// existing ancestor, canonicalizing that, and re-attaching the remaining
// non-existent suffix. This handles symlinked parent directories for paths
// that do not exist themselves (e.g. inaccessible Steam library paths).
fn canonicalize_with_suffix(path: &Path) -> PathBuf {
    let mut ancestor = path;
    let mut suffix = PathBuf::new();
    loop {
        if ancestor.exists() {
            let base = ancestor
                .canonicalize()
                .unwrap_or_else(|_| ancestor.to_path_buf());
            return base.join(suffix);
        }
        match ancestor.parent() {
            Some(parent) => {
                if let Some(component) = ancestor.file_name() {
                    let mut new_suffix = PathBuf::from(component);
                    new_suffix.push(&suffix);
                    suffix = new_suffix;
                }
                ancestor = parent;
            }
            None => return path.to_path_buf(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InaccessibleGame {
    pub kind: GameKind,
    pub library_path: PathBuf,
}

pub(crate) fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };

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

        // Deduplicate inaccessible entries. The library path itself does not
        // exist (that is why the game is inaccessible), so canonicalize() will
        // always fail. Instead, canonicalize the nearest existing ancestor and
        // re-attach the remaining suffix so symlinked parent directories are
        // resolved correctly.
        let mut seen_inacc: HashSet<PathBuf> = HashSet::new();
        for inc in kind_inaccessible {
            let canonical = canonicalize_with_suffix(&inc.library_path);
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
#[path = "library_tests.rs"]
mod tests;
