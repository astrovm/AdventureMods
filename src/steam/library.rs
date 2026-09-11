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

fn is_steam_library_root(path: &Path) -> bool {
    path.join("steamapps").is_dir()
}

fn library_paths_equivalent(left: &Path, right: &Path) -> bool {
    canonicalize_with_suffix(left) == canonicalize_with_suffix(right)
}

/// Host path exported by the xdg-document-portal FUSE mount, if any.
///
/// Flatpak folder grants are visible at `/run/user/$UID/doc/<id>` rather than
/// at the original host path from `libraryfolders.vdf`.
pub(crate) fn document_portal_host_path(path: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::{CString, OsString};
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
        let c_name = CString::new("user.document-portal.host-path").ok()?;
        let mut buf = vec![0u8; 4096];
        let len = unsafe {
            linux_xattr::getxattr(
                c_path.as_ptr(),
                c_name.as_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
            )
        };
        if len <= 0 {
            return None;
        }
        buf.truncate(len as usize);
        while buf.last() == Some(&0) {
            buf.pop();
        }
        if buf.is_empty() {
            return None;
        }
        Some(PathBuf::from(OsString::from_vec(buf)))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

/// Resolve a host path through the document-portal path containing `portal_path`.
///
/// Portal xattrs are available on the mounted path and its ancestors. Walking
/// those ancestors lets callers map sibling paths from metadata written by
/// Steam on the host filesystem into the sandbox-visible document path.
pub(crate) fn resolve_document_portal_path(
    portal_path: &Path,
    host_path: &Path,
) -> Option<PathBuf> {
    let host_path = canonicalize_with_suffix(host_path);
    let mut current = Some(portal_path);

    while let Some(portal_path) = current {
        if let Some(portal_host) = document_portal_host_path(portal_path) {
            let portal_host = canonicalize_with_suffix(&portal_host);
            if let Ok(relative) = host_path.strip_prefix(&portal_host) {
                let resolved = portal_path.join(relative);
                if resolved.exists() {
                    return Some(resolved);
                }
            }
        }

        current = portal_path.parent();
    }

    None
}

#[cfg(target_os = "linux")]
mod linux_xattr {
    unsafe extern "C" {
        pub fn getxattr(
            path: *const core::ffi::c_char,
            name: *const core::ffi::c_char,
            value: *mut u8,
            size: usize,
        ) -> isize;
    }
}

fn extra_library_root_for_host(extra: &Path, host_library: &Path) -> Option<PathBuf> {
    if library_paths_equivalent(extra, host_library) {
        return Some(extra.to_path_buf());
    }

    let portal_host = document_portal_host_path(extra)?;
    let portal_n = canonicalize_with_suffix(&portal_host);
    let host_n = canonicalize_with_suffix(host_library);

    if portal_n == host_n {
        return Some(extra.to_path_buf());
    }

    let rel = host_n.strip_prefix(&portal_n).ok()?;
    let nested = extra.join(rel);
    is_steam_library_root(&nested).then_some(nested)
}

fn document_portal_root() -> Option<PathBuf> {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let doc_root = PathBuf::from(runtime_dir).join("doc");
    doc_root.is_dir().then_some(doc_root)
}

fn find_document_portal_library(expected: &Path) -> Option<PathBuf> {
    let doc_root = document_portal_root()?;
    let expected_n = canonicalize_with_suffix(expected);
    let expected_name = expected.file_name()?;

    for entry in std::fs::read_dir(doc_root).ok()? {
        let path = entry.ok()?.path();
        if !path.is_dir() || path.file_name().is_some_and(|name| name == "by-app") {
            continue;
        }

        if let Some(root) = extra_library_root_for_host(&path, expected)
            && is_steam_library_root(&root)
        {
            return Some(root);
        }

        let nested = path.join(expected_name);
        if is_steam_library_root(&nested)
            && document_portal_host_path(&nested)
                .is_some_and(|host| canonicalize_with_suffix(&host) == expected_n)
        {
            return Some(nested);
        }
    }

    None
}

/// Turn a folder returned by the Flatpak file portal into a usable Steam library root.
///
/// The portal typically yields `/run/user/$UID/doc/<id>` instead of the host
/// path from Steam (`/data/SteamLibrary`). A selected document is usable when
/// it contains `steamapps` and maps back to the expected host library.
pub(crate) fn resolve_granted_steam_library(selected: &Path, expected: &Path) -> Option<PathBuf> {
    if is_steam_library_root(selected) {
        return granted_library_matches_expected(selected, expected)
            .then(|| selected.to_path_buf());
    }

    if let Some(name) = expected.file_name() {
        let nested = selected.join(name);
        if is_steam_library_root(&nested) && granted_library_matches_expected(&nested, expected) {
            return Some(nested);
        }
    }

    if selected.file_name().is_some_and(|name| name == "steamapps")
        && let Some(parent) = selected.parent()
        && is_steam_library_root(parent)
    {
        return granted_library_matches_expected(parent, expected).then(|| parent.to_path_buf());
    }

    find_document_portal_library(expected)
}

fn granted_library_matches_expected(selected: &Path, expected: &Path) -> bool {
    library_paths_equivalent(selected, expected)
        || document_portal_host_path(selected)
            .is_some_and(|host| library_paths_equivalent(&host, expected))
}

#[derive(Debug, Clone)]
pub struct InaccessibleGame {
    pub kind: GameKind,
    pub library_path: PathBuf,
}

fn steam_roots_in(home: &Path) -> Vec<PathBuf> {
    let candidates = [
        home.join(".steam/debian-installation"),
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];

    candidates.into_iter().filter(|p| p.is_dir()).collect()
}

pub(crate) fn steam_roots() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|home| steam_roots_in(&home))
        .unwrap_or_default()
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
        // resolved correctly. Skip libraries already granted through the
        // document portal (same host path, different sandbox path).
        let mut seen_inacc: HashSet<PathBuf> = HashSet::new();
        for inc in kind_inaccessible {
            let covered_by_grant = extra_libraries.iter().any(|extra| {
                extra_library_root_for_host(extra, &inc.library_path)
                    .and_then(|root| find_game_in_library_path(&root, kind))
                    .is_some()
            });
            if covered_by_grant {
                continue;
            }

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
