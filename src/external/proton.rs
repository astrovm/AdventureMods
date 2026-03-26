use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Output;

use anyhow::{Context, Result};

use super::flatpak;
use crate::steam::{library, vdf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixState {
    Ready,
    MissingPrefix,
    MissingMetadata,
    ConfigMismatch {
        prefix_tool: String,
        configured_tool: String,
    },
}

#[derive(Debug, Clone)]
struct PrefixMetadata {
    proton_dir: PathBuf,
    tool_name: String,
}

#[derive(Debug, Clone)]
struct ConfiguredTool {
    name: String,
    proton_dir: Option<PathBuf>,
}

pub fn prefix_state(game_path: &Path, app_id: u32) -> Result<PrefixState> {
    let steamapps = steamapps_dir(game_path)?;
    let compatdata = steamapps.join("compatdata").join(app_id.to_string());
    let prefix = compatdata.join("pfx");

    if !prefix.is_dir() {
        return Ok(PrefixState::MissingPrefix);
    }

    let Some(prefix_metadata) = read_prefix_metadata(&compatdata)? else {
        return Ok(PrefixState::MissingMetadata);
    };

    if let Some(configured_tool) = configured_tool_from_config(game_path, app_id)? {
        let prefix_canonical = prefix_metadata
            .proton_dir
            .canonicalize()
            .unwrap_or_else(|_| prefix_metadata.proton_dir.clone());

        if let Some(configured_path) = configured_tool.proton_dir {
            let configured_canonical = configured_path.canonicalize().unwrap_or(configured_path);

            if configured_canonical != prefix_canonical {
                return Ok(PrefixState::ConfigMismatch {
                    prefix_tool: prefix_metadata.tool_name,
                    configured_tool: configured_tool.name,
                });
            }
        } else if configured_tool.name.trim() != prefix_metadata.tool_name.trim() {
            return Ok(PrefixState::ConfigMismatch {
                prefix_tool: prefix_metadata.tool_name,
                configured_tool: configured_tool.name,
            });
        }
    }

    Ok(PrefixState::Ready)
}

pub fn ensure_prefix_ready(game_path: &Path, app_id: u32) -> Result<()> {
    match prefix_state(game_path, app_id)? {
        PrefixState::Ready => Ok(()),
        PrefixState::MissingPrefix | PrefixState::MissingMetadata => anyhow::bail!(
            "Open the game from Steam once, wait for Proton to finish setting up, then close it and try again."
        ),
        PrefixState::ConfigMismatch {
            prefix_tool,
            configured_tool,
        } => anyhow::bail!(
            "This game's Proton prefix still uses {prefix_tool}, but Steam is configured to use {configured_tool}. Open the game from Steam once so Steam can update the prefix, then try again."
        ),
    }
}

pub fn steam_config_message(game_name: &str, game_path: &Path, app_id: u32) -> String {
    match prefix_state(game_path, app_id) {
        Ok(PrefixState::Ready) => {
            format!("The Proton prefix for {game_name} is ready. You can continue right away.")
        }
        Ok(PrefixState::MissingPrefix) | Ok(PrefixState::MissingMetadata) => format!(
            "Open {game_name} from Steam once, wait for Proton to finish setting it up, then close the game and continue here."
        ),
        Ok(PrefixState::ConfigMismatch {
            prefix_tool,
            configured_tool,
        }) => format!(
            "{game_name} still has a Proton prefix from {prefix_tool}, but Steam is now configured to use {configured_tool}. Open the game from Steam once so Steam can update the prefix, then continue here."
        ),
        Err(err) => {
            tracing::warn!("Failed to inspect Proton prefix state: {err}");
            format!("Open {game_name} from Steam once, then close it and continue here.")
        }
    }
}

/// Locate the Proton installation configured by Steam for a specific game.
pub fn find_proton_for_app(game_path: &Path, app_id: u32) -> Result<PathBuf> {
    ensure_prefix_ready(game_path, app_id)?;

    if let Some(mapped) = find_proton_from_prefix_metadata(game_path, app_id)? {
        tracing::info!(
            "Selected Proton from prefix metadata for app {} at {}",
            app_id,
            mapped.display()
        );
        return Ok(mapped);
    }

    anyhow::bail!(
        "The Proton prefix metadata for this game is missing. Open the game from Steam once, then try again."
    )
}

/// Locate the Proton installation to use for a game.
///
/// Searches Proton directories in the game's library and known Steam client
/// roots for directories matching `Proton *` or `Proton - Experimental`.
/// Picks the highest numeric version, falling back to `Proton - Experimental`
/// if no numbered version has a working Wine binary.
pub fn find_proton(game_path: &Path) -> Result<PathBuf> {
    let mut candidates: Vec<(PathBuf, ProtonVersion)> = Vec::new();

    for common in proton_common_dirs(game_path)? {
        let entries = std::fs::read_dir(&common)
            .with_context(|| format!("Failed to read {}", common.display()))?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if let Some(version) = parse_proton_dir_name(&name_str) {
                let dir = entry.path();
                if has_wine_binary(&dir) && !candidates.iter().any(|(existing, _)| existing == &dir)
                {
                    candidates.push((dir, version));
                }
            }
        }
    }

    if candidates.is_empty() {
        anyhow::bail!(
            "No Proton installation found in {}. Install Proton through Steam first.",
            steamapps_dir(game_path)?.display()
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
    let steam_root = steam_client_root(game_path)?;

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
    let proton_dir = find_proton_for_app(game_path, app_id)?;
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

fn configured_tool_from_config(game_path: &Path, app_id: u32) -> Result<Option<ConfiguredTool>> {
    let steamapps = steamapps_dir(game_path)?;
    let steam_roots = steam_root_candidates(game_path)?;

    for steam_root in &steam_roots {
        if let Some(tool_name) = compat_tool_name_from_config(steam_root, app_id)? {
            let proton_dir = resolve_compat_tool_path(steam_root, &steamapps, &tool_name);
            return Ok(Some(ConfiguredTool {
                name: tool_name,
                proton_dir,
            }));
        }
    }

    Ok(None)
}

fn find_proton_from_prefix_metadata(game_path: &Path, app_id: u32) -> Result<Option<PathBuf>> {
    let steamapps = steamapps_dir(game_path)?;
    let compatdata = steamapps.join("compatdata").join(app_id.to_string());
    Ok(read_prefix_metadata(&compatdata)?.map(|metadata| metadata.proton_dir))
}

fn read_prefix_metadata(compatdata: &Path) -> Result<Option<PrefixMetadata>> {
    let config_info = compatdata.join("config_info");
    if !config_info.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_info)
        .with_context(|| format!("Failed to read {}", config_info.display()))?;
    let tool_name = std::fs::read_to_string(compatdata.join("version"))
        .ok()
        .map(|content| content.trim().to_owned())
        .filter(|name| !name.is_empty())
        .or_else(|| {
            content
                .lines()
                .next()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToOwned::to_owned)
        });

    let proton_dir = content
        .lines()
        .filter_map(proton_dir_from_config_info_line)
        .find(|path| has_wine_binary(path));

    match (proton_dir, tool_name) {
        (Some(proton_dir), Some(tool_name)) => Ok(Some(PrefixMetadata {
            proton_dir,
            tool_name,
        })),
        _ => Ok(None),
    }
}

fn proton_dir_from_config_info_line(line: &str) -> Option<PathBuf> {
    let path = Path::new(line.trim());
    if !path.is_absolute() {
        return None;
    }

    let mut current = path;
    while let Some(parent) = current.parent() {
        if current.file_name().is_some_and(|name| name == "files") {
            return Some(parent.to_path_buf());
        }
        current = parent;
    }

    None
}

fn steam_root_candidates(game_path: &Path) -> Result<Vec<PathBuf>> {
    let steamapps = steamapps_dir(game_path)?;
    let derived_root = steamapps.parent().unwrap_or(&steamapps);
    let mut roots = Vec::new();

    roots.push(derived_root.to_path_buf());
    roots.extend(library::steam_roots());

    let mut unique = Vec::new();
    for root in roots {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
        if !unique.iter().any(|existing| existing == &canonical) {
            unique.push(canonical);
        }
    }

    Ok(unique)
}

fn steam_client_root(game_path: &Path) -> Result<PathBuf> {
    let steamapps = steamapps_dir(game_path)?;

    for root in steam_root_candidates(game_path)? {
        if root.join("config/config.vdf").is_file() {
            return Ok(root);
        }
    }

    Ok(steamapps.parent().unwrap_or(&steamapps).to_path_buf())
}

fn proton_common_dirs(game_path: &Path) -> Result<Vec<PathBuf>> {
    let steamapps = steamapps_dir(game_path)?;
    let mut dirs = Vec::new();

    let current_library_common = steamapps.join("common");
    if current_library_common.is_dir() {
        dirs.push(current_library_common);
    }

    for steam_root in steam_root_candidates(game_path)? {
        let common = steam_root.join("steamapps/common");
        if common.is_dir() && !dirs.iter().any(|existing| existing == &common) {
            dirs.push(common);
        }
    }

    if dirs.is_empty() {
        anyhow::bail!(
            "Steam common directory not found for {}",
            game_path.display()
        );
    }

    Ok(dirs)
}

fn compat_tool_name_from_config(steam_root: &Path, app_id: u32) -> Result<Option<String>> {
    let config_path = steam_root.join("config/config.vdf");
    if !config_path.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let Some(root) = vdf::parse(&content) else {
        anyhow::bail!("Failed to parse {}", config_path.display());
    };

    let Some(mapping) = root
        .get("InstallConfigStore")
        .and_then(|v| v.get("Software"))
        .and_then(|v| v.get("Valve"))
        .and_then(|v| v.get("Steam"))
        .and_then(|v| v.get("CompatToolMapping"))
    else {
        return Ok(None);
    };

    let app_id = app_id.to_string();
    for key in [app_id.as_str(), "0"] {
        if let Some(name) = mapping
            .get(key)
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            return Ok(Some(name.to_owned()));
        }
    }

    Ok(None)
}

fn resolve_compat_tool_path(
    steam_root: &Path,
    steamapps: &Path,
    tool_name: &str,
) -> Option<PathBuf> {
    compat_tool_dir_candidates(steam_root, steamapps, tool_name)
        .into_iter()
        .flat_map(|candidate| {
            [
                steamapps.join("common").join(&candidate),
                steam_root.join("steamapps/common").join(&candidate),
                steam_root.join("compatibilitytools.d").join(&candidate),
            ]
        })
        .find(|path| has_wine_binary(path))
}

fn compat_tool_dir_candidates(steam_root: &Path, steamapps: &Path, tool_name: &str) -> Vec<String> {
    let trimmed = tool_name.trim();
    let mut candidates = vec![trimmed.to_owned()];

    let aliases = match trimmed {
        "proton_experimental" => vec!["Proton - Experimental", "Proton Experimental"],
        "proton_hotfix" => vec!["Proton Hotfix"],
        _ => Vec::new(),
    };

    for alias in aliases {
        if !candidates.iter().any(|candidate| candidate == alias) {
            candidates.push(alias.to_owned());
        }
    }

    if let Some(version) = trimmed
        .strip_prefix("proton_")
        .and_then(|value| value.parse::<u32>().ok())
    {
        for common_dir in [
            steamapps.join("common"),
            steam_root.join("steamapps/common"),
        ] {
            if let Ok(entries) = std::fs::read_dir(common_dir) {
                for entry in entries.flatten() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    let matches_version = matches!(
                        parse_proton_dir_name(&dir_name),
                        Some(ProtonVersion::Numbered(major, _)) if major == version
                    );

                    if matches_version && !candidates.iter().any(|candidate| candidate == &dir_name)
                    {
                        candidates.push(dir_name);
                    }
                }
            }
        }
    }

    candidates
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

    if let Some(major_str) = parts.next() {
        if let Ok(major) = major_str.trim().parse::<u32>() {
            let minor: u32 = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            return Some(ProtonVersion::Numbered(major, minor));
        }
    }

    Some(ProtonVersion::Other(name.to_string()))
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
        assert_eq!(
            parse_proton_dir_name("Proton 9"),
            Some(ProtonVersion::Numbered(9, 0))
        );
        assert_eq!(
            parse_proton_dir_name("Proton 9.0 (Beta)"),
            Some(ProtonVersion::Numbered(9, 0))
        );
        assert_eq!(
            parse_proton_dir_name("Proton 8.0-4"),
            Some(ProtonVersion::Numbered(8, 0))
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
    fn test_find_proton_ignores_higher_version_without_wine_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");

        // Higher version directory exists, but no wine binary.
        std::fs::create_dir_all(common.join("Proton 9.0/files/bin")).unwrap();

        // Experimental has a working wine binary and should be selected.
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

    fn write_prefix_metadata(compatdata: &Path, tool_name: &str, proton_dir: &Path) {
        std::fs::create_dir_all(compatdata).unwrap();
        std::fs::write(compatdata.join("version"), format!("{tool_name}\n")).unwrap();
        std::fs::write(
            compatdata.join("config_info"),
            format!(
                "/tmp/unused\n/tmp/unused\n/tmp/unused\n/tmp/unused\n0\n0\n0\n{}/files/share/default_pfx/\n0\n",
                proton_dir.display()
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_find_proton_for_app_uses_prefix_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton8 = common.join("Proton 8.0/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton8).unwrap();
        std::fs::write(proton8.join("wine64"), "").unwrap();
        write_prefix_metadata(
            &compatdata,
            "Proton 8.0",
            &steam_root.join("steamapps/common/Proton 8.0"),
        );

        let result = find_proton_for_app(&game_path, 71250).unwrap();
        assert_eq!(result, common.join("Proton 8.0"));
    }

    #[test]
    fn test_prefix_state_missing_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");

        std::fs::create_dir_all(&game_path).unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::MissingPrefix);
    }

    #[test]
    fn test_prefix_state_missing_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let prefix = steam_root.join("steamapps/compatdata/71250/pfx");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&prefix).unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::MissingMetadata);
    }

    #[test]
    fn test_prefix_state_detects_config_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton_exp = common.join("Proton - Experimental/files/bin");
        let ge = steam_root.join("compatibilitytools.d/GE-Proton10-33/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton_exp).unwrap();
        std::fs::create_dir_all(&ge).unwrap();
        std::fs::write(proton_exp.join("wine64"), "").unwrap();
        std::fs::write(ge.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "GE-Proton10-33",
            &steam_root.join("compatibilitytools.d/GE-Proton10-33"),
        );

        let config_dir = steam_root.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.vdf"),
            r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "CompatToolMapping"
                {
                    "0"
                    {
                        "name"  "Proton - Experimental"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(
            result,
            PrefixState::ConfigMismatch {
                prefix_tool: "GE-Proton10-33".to_string(),
                configured_tool: "Proton - Experimental".to_string(),
            }
        );
    }

    #[test]
    fn test_prefix_state_accepts_resolved_internal_version_mapping() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure 2");
        let proton9 = common.join("Proton 9.0 (Beta)/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/213610");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton9).unwrap();
        std::fs::write(proton9.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "Proton 9.0 (Beta)",
            &steam_root.join("steamapps/common/Proton 9.0 (Beta)"),
        );

        let config_dir = steam_root.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.vdf"),
            r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "CompatToolMapping"
                {
                    "213610"
                    {
                        "name"  "proton_9"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        let result = prefix_state(&game_path, 213610).unwrap();
        assert_eq!(result, PrefixState::Ready);
    }

    #[test]
    fn test_find_proton_for_app_uses_prefix_config_info_first() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure 2");
        let proton_exp = common.join("Proton - Experimental/files/bin");
        let ge = steam_root.join("compatibilitytools.d/GE-Proton10-33/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/213610");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton_exp).unwrap();
        std::fs::create_dir_all(&ge).unwrap();
        std::fs::write(proton_exp.join("wine64"), "").unwrap();
        std::fs::write(ge.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "Proton - Experimental",
            &steam_root.join("steamapps/common/Proton - Experimental"),
        );

        let config_dir = steam_root.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.vdf"),
            r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "CompatToolMapping"
                {
                    "0"
                    {
                        "name"  "GE-Proton10-33"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        let result = find_proton_for_app(&game_path, 213610).unwrap();
        assert_eq!(
            result,
            steam_root.join("steamapps/common/Proton - Experimental")
        );
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
