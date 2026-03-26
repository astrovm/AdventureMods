use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Output;

use anyhow::{Context, Result};

use super::flatpak;

/// Locate the Proton installation to use for a game.
///
/// Searches `steamapps/common/` in the same library as the game for directories
/// matching `Proton *` or `Proton - Experimental`. Picks the highest numeric
/// version, falling back to `Proton - Experimental` if no numbered version
/// has a working Wine binary.
pub fn find_proton(game_path: &Path) -> Result<PathBuf> {
    let steamapps = steamapps_dir(game_path)?;
    let common = steamapps.join("common");

    if !common.is_dir() {
        anyhow::bail!("Steam common directory not found at {}", common.display());
    }

    let mut candidates: Vec<(PathBuf, ProtonVersion)> = Vec::new();

    let entries = std::fs::read_dir(&common)
        .with_context(|| format!("Failed to read {}", common.display()))?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if let Some(version) = parse_proton_dir_name(&name_str) {
            let dir = entry.path();
            if has_wine_binary(&dir) {
                candidates.push((dir, version));
            }
        }
    }

    if candidates.is_empty() {
        anyhow::bail!(
            "No Proton installation found in {}. Install Proton through Steam first.",
            common.display()
        );
    }

    // Sort: numbered versions descending, then experimental, then others.
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    let (path, _) = candidates.into_iter().next().unwrap();
    tracing::info!("Selected Proton at {}", path.display());
    Ok(path)
}

/// Build the environment variables needed to run Wine inside a Proton prefix.
pub fn proton_env(game_path: &Path, app_id: u32) -> Result<HashMap<String, String>> {
    let steamapps = steamapps_dir(game_path)?;
    let compat_data = steamapps.join("compatdata").join(app_id.to_string());
    let prefix = compat_data.join("pfx");

    // Steam root is one level above steamapps/
    let steam_root = steamapps.parent().unwrap_or(&steamapps).to_path_buf();

    let mut env = HashMap::new();
    env.insert("WINEPREFIX".into(), prefix.to_string_lossy().into_owned());
    env.insert(
        "STEAM_COMPAT_DATA_PATH".into(),
        compat_data.to_string_lossy().into_owned(),
    );
    env.insert(
        "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
        steam_root.to_string_lossy().into_owned(),
    );
    env.insert("WINEDLLOVERRIDES".into(), "mscoree=n;mshtml=n".into());
    env.insert("SteamAppId".into(), app_id.to_string());

    Ok(env)
}

/// Run an executable inside the game's Proton prefix using Wine.
///
/// `extra_args` are passed to the executable after the exe path.
pub fn run_in_prefix(
    game_path: &Path,
    app_id: u32,
    exe: &Path,
    extra_args: &[&str],
) -> Result<Output> {
    let proton_dir = find_proton(game_path)?;
    let env = proton_env(game_path, app_id)?;

    let wine = wine_binary(&proton_dir);

    let exe_str = exe.to_string_lossy().to_string();
    let mut args: Vec<String> = vec![exe_str.clone()];
    args.extend(extra_args.iter().map(|s| s.to_string()));

    let arg_refs: Vec<&str> = args.iter().map(|s| &**s).collect();
    let wine_str = wine.to_string_lossy().to_string();

    tracing::info!(
        "Running {} in prefix for app {} with Proton at {}",
        exe.display(),
        app_id,
        proton_dir.display()
    );

    flatpak::host_command_with_env_sync(&wine_str, &arg_refs, &env)
}

/// Navigate from a game install path up to the steamapps/ directory.
///
/// Game path is typically `.../steamapps/common/<game>/`.
fn steamapps_dir(game_path: &Path) -> Result<PathBuf> {
    game_path
        .parent() // common/
        .and_then(|p| p.parent()) // steamapps/
        .map(|p| p.to_path_buf())
        .with_context(|| {
            format!(
                "Cannot derive steamapps directory from game path: {}",
                game_path.display()
            )
        })
}

/// Determine the Wine binary path inside a Proton installation.
fn wine_binary(proton_dir: &Path) -> PathBuf {
    let wine64 = proton_dir.join("files/bin/wine64");
    if wine64.is_file() {
        wine64
    } else {
        proton_dir.join("files/bin/wine")
    }
}

/// Check whether a Proton directory contains a usable Wine binary.
fn has_wine_binary(proton_dir: &Path) -> bool {
    proton_dir.join("files/bin/wine64").is_file() || proton_dir.join("files/bin/wine").is_file()
}

/// Version representation for sorting Proton directories.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ProtonVersion {
    /// A numbered version like `Proton 9.0` or `Proton 8.0`.
    Numbered(u32, u32),
    /// `Proton - Experimental` or `Proton Experimental`.
    Experimental,
    /// Anything else that starts with `Proton` (e.g. `Proton Hotfix`).
    Other(String),
}

impl Ord for ProtonVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use ProtonVersion::*;
        match (self, other) {
            (Numbered(a1, a2), Numbered(b1, b2)) => (a1, a2).cmp(&(b1, b2)),
            (Numbered(..), _) => std::cmp::Ordering::Greater,
            (_, Numbered(..)) => std::cmp::Ordering::Less,
            (Experimental, Experimental) => std::cmp::Ordering::Equal,
            (Experimental, _) => std::cmp::Ordering::Greater,
            (_, Experimental) => std::cmp::Ordering::Less,
            (Other(a), Other(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for ProtonVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Parse a directory name like `Proton 9.0` into a `ProtonVersion`.
fn parse_proton_dir_name(name: &str) -> Option<ProtonVersion> {
    if !name.starts_with("Proton") {
        return None;
    }

    let rest = name.strip_prefix("Proton")?.trim();

    if rest.is_empty() {
        return Some(ProtonVersion::Other(name.to_string()));
    }

    if rest.eq_ignore_ascii_case("- Experimental") || rest.eq_ignore_ascii_case("Experimental") {
        return Some(ProtonVersion::Experimental);
    }

    // Try to parse "9.0", "8.0-4", "9.0-1" etc.
    let version_part = rest.split('-').next().unwrap_or(rest);
    let mut parts = version_part.split('.');
    let major: u32 = parts.next()?.trim().parse().ok()?;
    let minor: u32 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    Some(ProtonVersion::Numbered(major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_proton_numbered() {
        assert_eq!(
            parse_proton_dir_name("Proton 9.0"),
            Some(ProtonVersion::Numbered(9, 0))
        );
        assert_eq!(
            parse_proton_dir_name("Proton 8.0"),
            Some(ProtonVersion::Numbered(8, 0))
        );
        assert_eq!(
            parse_proton_dir_name("Proton 7.0"),
            Some(ProtonVersion::Numbered(7, 0))
        );
    }

    #[test]
    fn test_parse_proton_experimental() {
        assert_eq!(
            parse_proton_dir_name("Proton - Experimental"),
            Some(ProtonVersion::Experimental)
        );
        assert_eq!(
            parse_proton_dir_name("Proton Experimental"),
            Some(ProtonVersion::Experimental)
        );
    }

    #[test]
    fn test_parse_proton_other() {
        assert_eq!(
            parse_proton_dir_name("Proton Hotfix"),
            Some(ProtonVersion::Other("Proton Hotfix".to_string()))
        );
    }

    #[test]
    fn test_parse_non_proton() {
        assert_eq!(parse_proton_dir_name("Sonic Adventure DX"), None);
        assert_eq!(parse_proton_dir_name("SteamLinuxRuntime"), None);
    }

    #[test]
    fn test_version_ordering() {
        let v9 = ProtonVersion::Numbered(9, 0);
        let v8 = ProtonVersion::Numbered(8, 0);
        let exp = ProtonVersion::Experimental;
        let other = ProtonVersion::Other("Proton Hotfix".to_string());

        assert!(v9 > v8);
        assert!(v9 > exp);
        assert!(v9 > other);
        assert!(exp > other);
        assert!(v8 > exp);
    }

    #[test]
    fn test_find_proton_selects_highest_version() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");

        // Create two Proton dirs with wine binaries
        let p8 = common.join("Proton 8.0/files/bin");
        let p9 = common.join("Proton 9.0/files/bin");
        std::fs::create_dir_all(&p8).unwrap();
        std::fs::create_dir_all(&p9).unwrap();
        std::fs::write(p8.join("wine64"), "").unwrap();
        std::fs::write(p9.join("wine64"), "").unwrap();

        let game_path = common.join("Sonic Adventure DX");
        std::fs::create_dir_all(&game_path).unwrap();

        let result = find_proton(&game_path).unwrap();
        assert_eq!(result, common.join("Proton 9.0"));
    }

    #[test]
    fn test_find_proton_experimental_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");

        let exp = common.join("Proton - Experimental/files/bin");
        std::fs::create_dir_all(&exp).unwrap();
        std::fs::write(exp.join("wine64"), "").unwrap();

        let game_path = common.join("Sonic Adventure DX");
        std::fs::create_dir_all(&game_path).unwrap();

        let result = find_proton(&game_path).unwrap();
        assert_eq!(result, common.join("Proton - Experimental"));
    }

    #[test]
    fn test_find_proton_no_proton_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        std::fs::create_dir_all(&game_path).unwrap();

        let result = find_proton(&game_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_proton_env_paths() {
        let game_path =
            Path::new("/home/user/.local/share/Steam/steamapps/common/Sonic Adventure DX");
        let env = proton_env(game_path, 71250).unwrap();

        assert_eq!(
            env["WINEPREFIX"],
            "/home/user/.local/share/Steam/steamapps/compatdata/71250/pfx"
        );
        assert_eq!(
            env["STEAM_COMPAT_DATA_PATH"],
            "/home/user/.local/share/Steam/steamapps/compatdata/71250"
        );
        assert_eq!(
            env["STEAM_COMPAT_CLIENT_INSTALL_PATH"],
            "/home/user/.local/share/Steam"
        );
        assert_eq!(env["SteamAppId"], "71250");
    }

    #[test]
    fn test_steamapps_dir_derivation() {
        let game_path = Path::new("/mnt/games/SteamLibrary/steamapps/common/Sonic Adventure 2");
        let result = steamapps_dir(game_path).unwrap();
        assert_eq!(result, PathBuf::from("/mnt/games/SteamLibrary/steamapps"));
    }

    #[test]
    fn test_steamapps_dir_fails_for_root() {
        let game_path = Path::new("/");
        assert!(steamapps_dir(game_path).is_err());
    }
}
