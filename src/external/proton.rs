use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::steam::game::GameKind;
use crate::steam::library::{self, try_canonicalize};
use crate::steam::vdf;
use crate::trf;

/// Highest Proton major version known to run SA Mod Manager reliably.
///
/// Proton/Wine 11+ exits SA Mod Manager during .NET WPF startup
/// (`call_user_apc_dispatcher flags 0x3` then `CorExitProcess(0)`).
/// Official Proton 10 works; Hotfix/Experimental/CachyOS currently ship 11.x.
const MAX_SUPPORTED_PROTON_MAJOR: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixState {
    Ready,
    MissingPrefix,
    MissingMetadata,
    SteamConfigIncomplete,
    ProtonUnavailable {
        tool_name: String,
        proton_dir: PathBuf,
    },
    ConfigMismatch {
        prefix_tool: String,
        configured_tool: String,
    },
    /// Steam is configured to replace the prefix with Proton/Wine 11+.
    UnsupportedConfiguredProton {
        tool_name: String,
        major: u32,
    },
    /// Prefix uses Proton/Wine 11+, which cannot keep SA Mod Manager running.
    UnsupportedProton {
        tool_name: String,
        major: u32,
    },
    /// Prefix tool label could not be mapped to a major version (custom builds, etc.).
    /// Setup may continue, but the UI should warn and recommend Proton 10.0.
    UnknownProton {
        tool_name: String,
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
    proton_dir: PathBuf,
}

#[derive(Debug, Clone)]
enum ConfiguredToolLookup {
    Tool(ConfiguredTool),
    MissingConfig,
    InvalidConfig,
    MissingConfiguredTool,
    ConfiguredToolUnavailable,
}

/// The game's Steam compatdata folder, which holds its Proton prefix in `pfx`.
pub fn compat_data_dir(game_path: &Path, app_id: u32) -> Result<PathBuf> {
    Ok(steamapps_dir(game_path)?
        .join("compatdata")
        .join(app_id.to_string()))
}

pub fn prefix_state(game_path: &Path, app_id: u32) -> Result<PrefixState> {
    let compatdata = compat_data_dir(game_path, app_id)?;
    let prefix = compatdata.join("pfx");

    if !prefix.is_dir() {
        return Ok(PrefixState::MissingPrefix);
    }

    let Some(prefix_metadata) = read_prefix_metadata_for_game(game_path, &compatdata)? else {
        return Ok(PrefixState::MissingMetadata);
    };

    let configured_tool = configured_tool_from_config(game_path, app_id)?;

    if let ConfiguredToolLookup::Tool(configured_tool) = &configured_tool {
        let prefix_canonical = try_canonicalize(&prefix_metadata.proton_dir);
        let configured_canonical = try_canonicalize(&configured_tool.proton_dir);

        if configured_canonical != prefix_canonical {
            if let Some(major) =
                proton_major_from_labels(&configured_tool.name, &configured_tool.proton_dir)
                && major > MAX_SUPPORTED_PROTON_MAJOR
            {
                return Ok(PrefixState::UnsupportedConfiguredProton {
                    tool_name: configured_tool.name.clone(),
                    major,
                });
            }

            return Ok(PrefixState::ConfigMismatch {
                prefix_tool: prefix_metadata.tool_name,
                configured_tool: configured_tool.name.clone(),
            });
        }
    }

    if !has_wine_binary(&prefix_metadata.proton_dir) {
        return Ok(PrefixState::ProtonUnavailable {
            tool_name: prefix_metadata.tool_name,
            proton_dir: prefix_metadata.proton_dir,
        });
    }

    if matches!(
        configured_tool,
        ConfiguredToolLookup::MissingConfig | ConfiguredToolLookup::InvalidConfig
    ) {
        return Ok(PrefixState::SteamConfigIncomplete);
    }

    match proton_major_from_labels(&prefix_metadata.tool_name, &prefix_metadata.proton_dir) {
        Some(major) if major > MAX_SUPPORTED_PROTON_MAJOR => Ok(PrefixState::UnsupportedProton {
            tool_name: prefix_metadata.tool_name,
            major,
        }),
        Some(_) => Ok(PrefixState::Ready),
        None => Ok(PrefixState::UnknownProton {
            tool_name: prefix_metadata.tool_name,
        }),
    }
}

pub fn ensure_prefix_ready(game_path: &Path, app_id: u32) -> Result<()> {
    ensure_state_ready(&game_name(app_id), prefix_state(game_path, app_id))
}

/// The game's name for messages. Other apps show as their ID.
fn game_name(app_id: u32) -> String {
    GameKind::from_app_id(app_id).map_or_else(|| app_id.to_string(), |kind| kind.name().to_owned())
}

/// Like [`ensure_prefix_ready`] for a state that was already inspected. A
/// state that blocks setup fails with the same advice the setup screen shows.
pub fn ensure_state_ready(game_name: &str, state: Result<PrefixState>) -> Result<()> {
    match state? {
        // Unknown labels still allow installs (custom tools / test fixtures), but
        // steam_config_message warns the user to prefer Proton 10.0.
        PrefixState::Ready | PrefixState::UnknownProton { .. } => Ok(()),
        state => bail!(steam_config_message_for_state(game_name, &Ok(state))),
    }
}

pub fn steam_config_message(game_name: &str, game_path: &Path, app_id: u32) -> String {
    steam_config_message_for_state(game_name, &prefix_state(game_path, app_id))
}

/// Like [`steam_config_message`] for a state that was already inspected.
pub fn steam_config_message_for_state(game_name: &str, state: &Result<PrefixState>) -> String {
    match state {
        Ok(PrefixState::Ready) => trf!(
            "{game} is ready for Proton. You can continue.",
            game = game_name
        ),
        Ok(PrefixState::MissingPrefix) => trf!(
            "Steam hasn't set up {game} for Proton yet. In Steam, force Proton 10.0 in Properties → Compatibility, launch the game once and wait for it to start, then close it and try again.",
            game = game_name
        ),
        Ok(PrefixState::MissingMetadata) => trf!(
            "Steam's Proton files for {game} are incomplete. Force Proton 10.0 if needed, launch the game once from Steam so it can repair them, then try again.",
            game = game_name
        ),
        Ok(PrefixState::SteamConfigIncomplete) => trf!(
            "Adventure Mods couldn't read which Proton version Steam uses for {game}. In Steam, force Proton 10.0 in Properties → Compatibility, launch the game once, then try again.",
            game = game_name
        ),
        Ok(PrefixState::ProtonUnavailable { tool_name, .. }) => trf!(
            "{game} is set to use {tool_name}, but it isn't installed. In Steam, install Proton 10.0 and force it for this game, launch the game once, then try again.",
            game = game_name,
            tool_name = tool_name
        ),
        Ok(PrefixState::ConfigMismatch {
            prefix_tool,
            configured_tool,
        }) => trf!(
            "{game} was last run with {prefix_tool}, but Steam now uses {configured_tool}. Launch the game once from Steam so it can update, then try again.",
            game = game_name,
            prefix_tool = prefix_tool,
            configured_tool = configured_tool
        ),
        Ok(PrefixState::UnsupportedConfiguredProton { tool_name, major }) => trf!(
            "Steam is set to run {game} with {tool_name} (version {major}), which can't run SA Mod Manager. In Steam, force Proton 10.0 in Properties → Compatibility, launch the game once, close it, then try again.",
            game = game_name,
            tool_name = tool_name,
            major = major
        ),
        Ok(PrefixState::UnsupportedProton { tool_name, major }) => trf!(
            "{game} was last run with {tool_name} (version {major}). SA Mod Manager doesn't start on Proton 11 or newer, including Hotfix, Experimental and many custom builds. In Steam, force Proton 10.0 in Properties → Compatibility, launch the game once, close it, then try again.",
            game = game_name,
            tool_name = tool_name,
            major = major
        ),
        Ok(PrefixState::UnknownProton { tool_name }) => trf!(
            "{game} is set to use {tool_name}, and Adventure Mods can't tell which Proton version it is. SA Mod Manager needs Proton 10.0 and doesn't start on Proton 11 or newer. If this isn't Proton 10.0, force Proton 10.0 in Properties → Compatibility, launch the game once, close it, then continue.",
            game = game_name,
            tool_name = tool_name
        ),
        Err(err) => {
            tracing::warn!("Failed to inspect Proton prefix state: {err}");
            trf!(
                "In Steam, open {game}'s Properties → Compatibility and force Proton 10.0. Launch the game once, close it, then try again.",
                game = game_name
            )
        }
    }
}

/// Parse a Proton major version from a version label and/or install path.
///
/// Handles labels like `10.1000-105`, `11.0-100`, `CachyOS-11.0-100`,
/// `Proton 8.0`, and directory names like `Proton 10.0`.
fn proton_major_from_labels(tool_name: &str, proton_dir: &Path) -> Option<u32> {
    proton_major_from_text(tool_name)
        .or_else(|| {
            proton_dir
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(proton_major_from_text)
        })
        .or_else(|| {
            std::fs::read_to_string(proton_dir.join("version"))
                .ok()
                .and_then(|version| proton_major_from_text(&version))
        })
}

fn proton_major_from_text(text: &str) -> Option<u32> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            // Prefer "10.0" / "11.0-100" style so we don't match unrelated digits.
            if i < bytes.len() && bytes[i] == b'.' {
                return std::str::from_utf8(&bytes[start..i])
                    .ok()
                    .and_then(|s| s.parse().ok());
            }
        } else {
            i += 1;
        }
    }
    None
}

fn configured_tool_from_config(game_path: &Path, app_id: u32) -> Result<ConfiguredToolLookup> {
    let steamapps = steamapps_dir(game_path)?;
    // Never empty: the library's own root is always a candidate.
    let steam_roots = steam_root_candidates(game_path)?;

    let mut best_failure = ConfiguredToolLookup::MissingConfig;

    for steam_root in &steam_roots {
        match compat_tool_name_from_config(steam_root, app_id)? {
            ConfiguredToolLookup::Tool(ConfiguredTool { name, .. }) => {
                if let Some(proton_dir) = resolve_compat_tool_path(steam_root, &steamapps, &name) {
                    return Ok(ConfiguredToolLookup::Tool(ConfiguredTool {
                        name,
                        proton_dir,
                    }));
                }

                best_failure = ConfiguredToolLookup::ConfiguredToolUnavailable;
            }
            other => {
                if failure_priority(&other) > failure_priority(&best_failure) {
                    best_failure = other;
                }
            }
        }
    }

    Ok(best_failure)
}

fn failure_priority(lookup: &ConfiguredToolLookup) -> u8 {
    match lookup {
        ConfiguredToolLookup::Tool(_) => 4,
        ConfiguredToolLookup::ConfiguredToolUnavailable => 3,
        ConfiguredToolLookup::MissingConfiguredTool => 2,
        ConfiguredToolLookup::InvalidConfig => 1,
        ConfiguredToolLookup::MissingConfig => 0,
    }
}

fn read_prefix_metadata_for_game(
    game_path: &Path,
    compatdata: &Path,
) -> Result<Option<PrefixMetadata>> {
    let Some(mut metadata) = read_prefix_metadata(compatdata)? else {
        return Ok(None);
    };

    if let Some(resolved) = library::resolve_document_portal_path(game_path, &metadata.proton_dir) {
        metadata.proton_dir = resolved;
    }

    Ok(Some(metadata))
}

fn read_prefix_metadata(compatdata: &Path) -> Result<Option<PrefixMetadata>> {
    let config_info = compatdata.join("config_info");
    let version = compatdata.join("version");

    if !config_info.is_file() || !version.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_info)
        .with_context(|| format!("Failed to read {}", config_info.display()))?;
    let tool_name = std::fs::read_to_string(&version)
        .with_context(|| format!("Failed to read {}", version.display()))?
        .trim()
        .to_owned();

    let proton_dirs: Vec<_> = content
        .lines()
        .filter_map(proton_dir_from_config_info_line)
        .collect();
    let proton_dir = proton_dirs
        .iter()
        .find(|path| has_wine_binary(path))
        .cloned()
        .or_else(|| proton_dirs.into_iter().next());

    match proton_dir {
        Some(proton_dir) if !tool_name.is_empty() => Ok(Some(PrefixMetadata {
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
    roots.extend(
        library::steam_roots()
            .into_iter()
            .filter(|root| steam_root_references_library(root, derived_root)),
    );
    roots.extend(
        sibling_steam_root_candidates(derived_root)
            .into_iter()
            .filter(|root| steam_root_references_library(root, derived_root)),
    );

    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let canonical = try_canonicalize(&root);
        if seen.insert(canonical.clone()) {
            unique.push(canonical);
        }
    }

    Ok(unique)
}

fn sibling_steam_root_candidates(library_root: &Path) -> Vec<PathBuf> {
    let Some(parent) = library_root.parent() else {
        return vec![];
    };

    let Ok(entries) = std::fs::read_dir(parent) else {
        return vec![];
    };

    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path != library_root)
        .filter(|path| path.join("config/config.vdf").is_file())
        .collect()
}

fn steam_root_references_library(steam_root: &Path, library_path: &Path) -> bool {
    let mut target_paths = vec![try_canonicalize(library_path)];
    if let Some(host_path) = library::document_portal_host_path(library_path) {
        target_paths.push(try_canonicalize(&host_path));
    }

    if target_paths.contains(&try_canonicalize(steam_root)) {
        return true;
    }

    let libraryfolders = steam_root.join("steamapps/libraryfolders.vdf");
    let Ok(content) = std::fs::read_to_string(&libraryfolders) else {
        return false;
    };
    let Some(root) = vdf::parse(&content) else {
        return false;
    };
    let Some(folders) = root.get("libraryfolders").and_then(|value| value.as_map()) else {
        return false;
    };

    folders.values().any(|folder| {
        folder
            .as_map()
            .and_then(|map| map.get("path"))
            .and_then(|value| value.as_str())
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok().or(Some(path)))
            .is_some_and(|path| target_paths.contains(&path))
    })
}

fn compat_tool_name_from_config(steam_root: &Path, app_id: u32) -> Result<ConfiguredToolLookup> {
    let config_path = steam_root.join("config/config.vdf");
    if !config_path.is_file() {
        return Ok(ConfiguredToolLookup::MissingConfig);
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(err) => {
            tracing::warn!("Failed to read {}: {err}", config_path.display());
            return Ok(ConfiguredToolLookup::InvalidConfig);
        }
    };
    let Some(root) = vdf::parse(&content) else {
        return Ok(ConfiguredToolLookup::InvalidConfig);
    };

    let Some(mapping) = root
        .get("InstallConfigStore")
        .and_then(|v| v.get("Software"))
        .and_then(|v| v.get("Valve"))
        .and_then(|v| v.get("Steam"))
        .and_then(|v| v.get("CompatToolMapping"))
    else {
        return Ok(ConfiguredToolLookup::MissingConfiguredTool);
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
            return Ok(ConfiguredToolLookup::Tool(ConfiguredTool {
                name: name.to_owned(),
                proton_dir: PathBuf::new(),
            }));
        }
    }

    Ok(ConfiguredToolLookup::MissingConfiguredTool)
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
    let mut seen = HashSet::from([trimmed.to_owned()]);

    let aliases = match trimmed {
        "proton_experimental" => vec!["Proton - Experimental", "Proton Experimental"],
        "proton_hotfix" => vec!["Proton Hotfix"],
        _ => Vec::new(),
    };

    // Named tools have distinct aliases and never match `proton_<major>` below.
    candidates.extend(aliases.into_iter().map(str::to_owned));

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

                    if matches_version && seen.insert(dir_name.clone()) {
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

    if let Some(major_str) = parts.next()
        && let Ok(major) = major_str.trim().parse::<u32>()
    {
        let minor: u32 = parts
            .next()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        return Some(ProtonVersion::Numbered(major, minor));
    }

    Some(ProtonVersion::Other(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proton_major_from_text_parses_leading_version() {
        assert_eq!(proton_major_from_text("10.1000-105"), Some(10));
        assert_eq!(proton_major_from_text("11.0-100"), Some(11));
        assert_eq!(proton_major_from_text("CachyOS-11.0-100"), Some(11));
        assert_eq!(proton_major_from_text("Proton 8.0"), Some(8));
        assert_eq!(proton_major_from_text("Proton - Experimental"), None);
        assert_eq!(proton_major_from_text("TestProton"), None);
    }

    #[test]
    fn proton_major_from_labels_uses_directory_name() {
        let dir = PathBuf::from("/steam/steamapps/common/Proton 10.0");
        assert_eq!(
            proton_major_from_labels("Proton - Experimental", &dir),
            Some(10)
        );
    }

    #[test]
    fn proton_major_from_labels_uses_tool_version_file() {
        let tmp = tempfile::tempdir().unwrap();
        let proton_dir = tmp.path().join("Proton-CachyOS Latest");
        std::fs::create_dir_all(&proton_dir).unwrap();
        std::fs::write(
            proton_dir.join("version"),
            "1778931159 cachyos-11.0-20260506-slr\n",
        )
        .unwrap();

        assert_eq!(
            proton_major_from_labels("Proton-CachyOS Latest", &proton_dir),
            Some(11)
        );
    }

    #[test]
    fn prefix_state_rejects_proton_11() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton11 = common.join("Proton 11.0/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton11).unwrap();
        std::fs::write(proton11.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "11.0-100",
            &steam_root.join("steamapps/common/Proton 11.0"),
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
                    "71250"
                    {
                        "name"  "proton_11"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        assert_eq!(
            prefix_state(&game_path, 71250).unwrap(),
            PrefixState::UnsupportedProton {
                tool_name: "11.0-100".to_owned(),
                major: 11,
            }
        );

        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
        );

        let message = steam_config_message("Sonic Adventure DX", &game_path, 71250);
        assert!(message.contains("Proton 10.0"));
        assert!(message.contains("11"));
    }

    #[test]
    fn prefix_state_accepts_proton_10() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton10 = common.join("Proton 10.0/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton10).unwrap();
        std::fs::write(proton10.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "10.1000-105",
            &steam_root.join("steamapps/common/Proton 10.0"),
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
                    "71250"
                    {
                        "name"  "proton_10"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        assert_eq!(prefix_state(&game_path, 71250).unwrap(), PrefixState::Ready);
        assert!(ensure_prefix_ready(&game_path, 71250).is_ok());
        assert!(steam_config_message("Sonic Adventure DX", &game_path, 71250).contains("ready"));
    }

    #[test]
    fn parse_proton_numbered() {
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
    fn parse_proton_experimental() {
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
    fn parse_proton_other() {
        assert_eq!(
            parse_proton_dir_name("Proton Hotfix"),
            Some(ProtonVersion::Other("Proton Hotfix".to_string()))
        );
    }

    #[test]
    fn parse_non_proton() {
        assert_eq!(parse_proton_dir_name("Sonic Adventure DX"), None);
        assert_eq!(parse_proton_dir_name("SteamLinuxRuntime"), None);
    }

    #[test]
    fn version_ordering() {
        let v9 = ProtonVersion::Numbered(9, 0);
        let v8 = ProtonVersion::Numbered(8, 0);
        let exp = ProtonVersion::Experimental;
        let other = ProtonVersion::Other("Proton Hotfix".to_string());

        assert!(v9 > v8);
        assert!(v9 > exp);
        assert!(v9 > other);
        assert!(exp > other);
        assert!(v8 > exp);

        use std::cmp::Ordering;
        assert_eq!(other.cmp(&v8), Ordering::Less);
        assert_eq!(exp.cmp(&v8), Ordering::Less);
        assert_eq!(exp.cmp(&ProtonVersion::Experimental), Ordering::Equal);
        assert_eq!(other.cmp(&exp), Ordering::Less);
        let custom = ProtonVersion::Other("Proton GE".to_string());
        assert_eq!(custom.cmp(&other), Ordering::Less);
        assert_eq!(other.partial_cmp(&custom), Some(Ordering::Greater));
    }

    #[test]
    fn parse_bare_proton_name_is_other() {
        assert_eq!(
            parse_proton_dir_name("Proton"),
            Some(ProtonVersion::Other("Proton".to_string()))
        );
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

    #[cfg(target_os = "linux")]
    #[test]
    fn prefix_state_maps_host_metadata_into_document_portal() {
        let tmp = tempfile::tempdir().unwrap();
        let portal_root = tmp.path().join("doc/abc123/SteamLibrary");
        let portal_common = portal_root.join("steamapps/common");
        let game_path = portal_common.join("Sonic Adventure DX");
        let portal_proton = portal_common.join("Proton 10.0");
        let host_common = tmp.path().join("host/SteamLibrary/steamapps/common");
        let host_proton = host_common.join("Proton 10.0");
        let compatdata = portal_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(portal_proton.join("files/bin")).unwrap();
        std::fs::write(portal_proton.join("files/bin/wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(&compatdata, "10.1000-105", &host_proton);
        std::fs::create_dir_all(portal_root.join("config")).unwrap();
        std::fs::write(
            portal_root.join("config/config.vdf"),
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
                    "71250"
                    {
                        "name"  "proton_10"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();
        std::fs::create_dir_all(&host_common).unwrap();
        set_host_path_xattr(&portal_common, &host_common);

        // Only the portal copy of Proton has Wine, so the prefix is ready only
        // when its host path is mapped back into the portal.
        assert_eq!(prefix_state(&game_path, 71250).unwrap(), PrefixState::Ready);
    }

    /// Tag `path` the way the document portal does. CI and development
    /// checkouts live on filesystems with user xattrs (ext4, btrfs, tmpfs), so
    /// a failure here is a broken test environment, not a skipped test.
    #[cfg(target_os = "linux")]
    fn set_host_path_xattr(path: &Path, host_path: &Path) {
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
        let value = host_path.as_os_str().as_bytes();
        let result = unsafe {
            setxattr(
                c_path.as_ptr(),
                c_name.as_ptr(),
                value.as_ptr(),
                value.len(),
                0,
            )
        };
        (result == 0)
            .then_some(())
            .ok_or_else(std::io::Error::last_os_error)
            .expect("document-portal tests need user xattr support on the temp filesystem");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn steam_root_references_document_portal_library() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path().join("steam-root");
        let host_library = tmp.path().join("host/SteamLibrary");
        let portal_library = tmp.path().join("doc/abc123/SteamLibrary");

        std::fs::create_dir_all(steam_root.join("steamapps")).unwrap();
        std::fs::create_dir_all(portal_library.join("steamapps")).unwrap();
        std::fs::write(
            steam_root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n    }}\n}}\n",
                host_library.display()
            ),
        )
        .unwrap();
        set_host_path_xattr(&portal_library, &host_library);

        assert!(steam_root_references_library(&steam_root, &portal_library));
    }

    #[test]
    fn failure_priority_ordering() {
        let tool = ConfiguredTool {
            name: "Proton 10".to_owned(),
            proton_dir: PathBuf::from("/steam/Proton 10"),
        };
        assert_eq!(failure_priority(&ConfiguredToolLookup::Tool(tool)), 4);
        assert!(
            failure_priority(&ConfiguredToolLookup::ConfiguredToolUnavailable)
                > failure_priority(&ConfiguredToolLookup::MissingConfiguredTool)
        );
        assert!(
            failure_priority(&ConfiguredToolLookup::MissingConfiguredTool)
                > failure_priority(&ConfiguredToolLookup::InvalidConfig)
        );
        assert!(
            failure_priority(&ConfiguredToolLookup::InvalidConfig)
                > failure_priority(&ConfiguredToolLookup::MissingConfig)
        );
    }

    #[test]
    fn compat_tool_name_invalid_vdf_returns_invalid_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.vdf"), "not valid vdf content").unwrap();

        let result = compat_tool_name_from_config(tmp.path(), 71250).unwrap();
        assert!(matches!(result, ConfiguredToolLookup::InvalidConfig));
    }

    #[test]
    fn prefix_state_missing_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");

        std::fs::create_dir_all(&game_path).unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::MissingPrefix);

        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
        );
        assert!(
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
                .starts_with("Steam hasn't set up Sonic Adventure DX for Proton yet.")
        );
        assert!(
            steam_config_message("Sonic Adventure DX", Path::new("/game"), 71250)
                .contains("force Proton 10.0")
        );
    }

    #[test]
    fn prefix_state_missing_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let prefix = steam_root.join("steamapps/compatdata/71250/pfx");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&prefix).unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::MissingMetadata);

        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
        );
        assert!(
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
                .contains("Proton files for Sonic Adventure DX are incomplete")
        );
    }

    #[test]
    fn prefix_metadata_needs_both_files() {
        // Steam writes `version` and `config_info` separately, so a prefix
        // interrupted between the two has only one of them.
        let tmp = tempfile::tempdir().unwrap();
        let proton_dir = tmp.path().join("Proton 10.0");
        for keep in ["version", "config_info"] {
            let compatdata = tmp.path().join(keep);
            write_prefix_metadata(&compatdata, "proton_10", &proton_dir);
            assert!(read_prefix_metadata(&compatdata).unwrap().is_some());

            let other = if keep == "version" {
                "config_info"
            } else {
                "version"
            };
            std::fs::remove_file(compatdata.join(other)).unwrap();
            assert!(read_prefix_metadata(&compatdata).unwrap().is_none());
        }
    }

    #[test]
    fn prefix_state_reports_missing_proton_installation() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let game_path = steam_root.join("steamapps/common/Sonic Adventure DX");
        let compatdata = steam_root.join("steamapps/compatdata/71250");
        let proton_dir = steam_root.join("steamapps/common/Proton Missing");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(&compatdata, "Proton Missing", &proton_dir);

        assert_eq!(
            prefix_state(&game_path, 71250).unwrap(),
            PrefixState::ProtonUnavailable {
                tool_name: "Proton Missing".to_owned(),
                proton_dir: proton_dir.clone(),
            }
        );

        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("Proton Missing"));
        assert!(message.contains("but it isn't installed"));
        assert_eq!(
            format!("{error:#}"),
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
        );
        assert!(
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
                .contains("but it isn't installed")
        );
    }

    #[test]
    fn prefix_state_missing_config() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton_exp = common.join("Proton - Experimental/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton_exp).unwrap();
        std::fs::write(proton_exp.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "Proton - Experimental",
            &steam_root.join("steamapps/common/Proton - Experimental"),
        );

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::SteamConfigIncomplete);

        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
        );
        assert!(
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
                .contains("couldn't read which Proton version")
        );
    }

    #[test]
    fn prefix_state_invalid_config() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton_exp = common.join("Proton - Experimental/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton_exp).unwrap();
        std::fs::write(proton_exp.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "Proton - Experimental",
            &steam_root.join("steamapps/common/Proton - Experimental"),
        );

        let config_dir = steam_root.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.vdf"), "definitely not valid vdf").unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::SteamConfigIncomplete);
    }

    #[test]
    fn prefix_state_missing_configured_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton10 = common.join("Proton 10.0/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton10).unwrap();
        std::fs::write(proton10.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "10.1000-105",
            &steam_root.join("steamapps/common/Proton 10.0"),
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
            }
        }
    }
}"#,
        )
        .unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::Ready);
    }

    #[test]
    fn prefix_state_configured_tool_unavailable() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton10 = common.join("Proton 10.0/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&proton10).unwrap();
        std::fs::write(proton10.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "10.1000-105",
            &steam_root.join("steamapps/common/Proton 10.0"),
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
                        "name"  "Custom-Proton-That-Is-Not-Installed"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::Ready);
    }

    #[test]
    fn prefix_state_warns_on_unknown_proton_label() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let custom = steam_root.join("compatibilitytools.d/TestProton/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(custom.join("wine64"), "").unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "TestProton",
            &steam_root.join("compatibilitytools.d/TestProton"),
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
                        "name"  "TestProton"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        assert_eq!(
            prefix_state(&game_path, 71250).unwrap(),
            PrefixState::UnknownProton {
                tool_name: "TestProton".to_owned(),
            }
        );
        assert!(ensure_prefix_ready(&game_path, 71250).is_ok());

        let message = steam_config_message("Sonic Adventure DX", &game_path, 71250);
        assert!(message.contains("can't tell"));
        assert!(message.contains("Proton 10.0"));
        assert!(message.contains("TestProton"));
    }

    #[test]
    fn prefix_state_detects_config_mismatch() {
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
        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
        );
        assert!(
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
                .contains("was last run with")
        );
    }

    #[test]
    fn errors_name_the_game_or_its_app_id() {
        assert_eq!(game_name(213610), "Sonic Adventure 2");
        assert_eq!(game_name(4000), "4000");
    }

    #[test]
    fn prefix_state_rejects_unsupported_configured_tool_before_prefix_update() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let proton10 = common.join("Proton 10.0");
        let cachyos = steam_root.join("compatibilitytools.d/Proton-CachyOS Latest");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(proton10.join("files/bin")).unwrap();
        std::fs::write(proton10.join("files/bin/wine64"), "").unwrap();
        std::fs::create_dir_all(cachyos.join("files/bin")).unwrap();
        std::fs::write(cachyos.join("files/bin/wine64"), "").unwrap();
        std::fs::write(
            cachyos.join("version"),
            "1778931159 cachyos-11.0-20260506-slr\n",
        )
        .unwrap();
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(&compatdata, "10.1000-105", &proton10);

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
                    "71250"
                    {
                        "name"  "Proton-CachyOS Latest"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        assert_eq!(
            prefix_state(&game_path, 71250).unwrap(),
            PrefixState::UnsupportedConfiguredProton {
                tool_name: "Proton-CachyOS Latest".to_owned(),
                major: 11,
            }
        );

        let message = steam_config_message("Sonic Adventure DX", &game_path, 71250);
        assert!(message.contains("Steam is set to run"));
        assert!(message.contains("(version 11)"));
        assert!(!message.contains("so it can update"));
        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            steam_config_message("Sonic Adventure DX", &game_path, 71250)
        );
    }

    #[test]
    fn prefix_state_detects_mismatch_when_old_proton_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let common = steam_root.join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");
        let configured_proton = common.join("Proton - Experimental/files/bin");
        let compatdata = steam_root.join("steamapps/compatdata/71250");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(&configured_proton).unwrap();
        std::fs::write(configured_proton.join("wine64"), "").unwrap();
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

        assert_eq!(
            prefix_state(&game_path, 71250).unwrap(),
            PrefixState::ConfigMismatch {
                prefix_tool: "GE-Proton10-33".to_owned(),
                configured_tool: "Proton - Experimental".to_owned(),
            }
        );
    }

    #[test]
    fn prefix_state_accepts_resolved_internal_version_mapping() {
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

    const PROTON_10_CONFIG: &str = r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "CompatToolMapping"
                {
                    "71250"
                    {
                        "name"  "proton_10"
                    }
                }
            }
        }
    }
}"#;

    #[test]
    fn read_prefix_metadata_rejects_unreadable_or_incomplete_files() {
        let tmp = tempfile::tempdir().unwrap();
        let compatdata = tmp.path().join("compatdata/71250");
        let proton_dir = tmp.path().join("Proton 10.0");
        let config_info = compatdata.join("config_info");
        let version = compatdata.join("version");
        write_prefix_metadata(&compatdata, "10.1000-105", &proton_dir);
        assert_eq!(
            read_prefix_metadata(&compatdata)
                .unwrap()
                .unwrap()
                .proton_dir,
            proton_dir
        );

        // Not UTF-8, so the files exist but cannot be read as text.
        std::fs::write(&version, [0xff, 0xfe]).unwrap();
        assert_eq!(
            read_prefix_metadata(&compatdata).unwrap_err().to_string(),
            format!("Failed to read {}", version.display())
        );
        std::fs::write(&config_info, [0xff, 0xfe]).unwrap();
        assert_eq!(
            read_prefix_metadata(&compatdata).unwrap_err().to_string(),
            format!("Failed to read {}", config_info.display())
        );

        // A blank tool label or no Proton path is incomplete metadata.
        write_prefix_metadata(&compatdata, "  ", &proton_dir);
        assert!(read_prefix_metadata(&compatdata).unwrap().is_none());
        std::fs::write(&version, "10.1000-105\n").unwrap();
        std::fs::write(&config_info, "relative/files\n0\n").unwrap();
        assert!(read_prefix_metadata(&compatdata).unwrap().is_none());
    }

    #[test]
    fn compat_tool_name_reports_unreadable_config_and_unmapped_apps() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config/config.vdf");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();

        std::fs::write(&config, [0xff, 0xfe, 0x00]).unwrap();
        let (result, logs) =
            crate::test_log::capture_logs(|| compat_tool_name_from_config(tmp.path(), 71250));
        assert!(matches!(
            result.unwrap(),
            ConfiguredToolLookup::InvalidConfig
        ));
        assert!(logs.contains(&format!("Failed to read {}", config.display())));

        // Only another game has a forced tool, and there is no global default.
        std::fs::write(&config, PROTON_10_CONFIG.replace("\"71250\"", "\"213610\"")).unwrap();
        assert!(matches!(
            compat_tool_name_from_config(tmp.path(), 71250).unwrap(),
            ConfiguredToolLookup::MissingConfiguredTool
        ));
    }

    #[test]
    fn configured_tool_keeps_the_most_specific_failure_across_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("library");
        let other_steam = tmp.path().join("steam");
        let game_path = library.join("steamapps/common/Sonic Adventure DX");
        std::fs::create_dir_all(&game_path).unwrap();
        // The library's own root maps only another game...
        std::fs::create_dir_all(library.join("config")).unwrap();
        std::fs::write(
            library.join("config/config.vdf"),
            PROTON_10_CONFIG.replace("\"71250\"", "\"213610\""),
        )
        .unwrap();
        // ...and a sibling Steam install that lists the library has a broken config.
        std::fs::create_dir_all(other_steam.join("config")).unwrap();
        std::fs::create_dir_all(other_steam.join("steamapps")).unwrap();
        std::fs::write(other_steam.join("config/config.vdf"), "not valid vdf").unwrap();
        std::fs::write(
            other_steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n    }}\n}}\n",
                library.display()
            ),
        )
        .unwrap();

        assert_eq!(steam_root_candidates(&game_path).unwrap().len(), 2);
        assert!(matches!(
            configured_tool_from_config(&game_path, 71250).unwrap(),
            ConfiguredToolLookup::MissingConfiguredTool
        ));
    }

    #[test]
    fn steam_root_candidates_dedupe_home_steam_links_to_the_library() {
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("library");
        let game_path = library.join("steamapps/common/Sonic Adventure DX");
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(home.join(".steam")).unwrap();
        std::fs::create_dir_all(home.join(".local/share")).unwrap();
        // Both usual Steam locations point at the library itself.
        std::os::unix::fs::symlink(&library, home.join(".steam/steam")).unwrap();
        std::os::unix::fs::symlink(&library, home.join(".local/share/Steam")).unwrap();

        let roots = {
            let _lock = crate::test_env::lock();
            let previous = std::env::var_os("HOME").expect("tests run with HOME set");
            unsafe { std::env::set_var("HOME", &home) };
            let roots = steam_root_candidates(&game_path);
            unsafe { std::env::set_var("HOME", previous) };
            roots.unwrap()
        };

        assert_eq!(roots, vec![library.canonicalize().unwrap()]);
    }

    #[test]
    fn sibling_steam_roots_need_a_readable_parent() {
        assert!(sibling_steam_root_candidates(Path::new("/")).is_empty());
        let tmp = tempfile::tempdir().unwrap();
        assert!(sibling_steam_root_candidates(&tmp.path().join("missing/library")).is_empty());
    }

    #[test]
    fn steam_root_references_library_checks_libraryfolders() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path().join("steam");
        let library = tmp.path().join("library");
        let libraryfolders = steam_root.join("steamapps/libraryfolders.vdf");
        std::fs::create_dir_all(steam_root.join("steamapps")).unwrap();
        std::fs::create_dir_all(&library).unwrap();

        // A Steam root is its own library.
        assert!(steam_root_references_library(&library, &library));
        // No, unparsable, or unrelated library lists do not reference it.
        assert!(!steam_root_references_library(&steam_root, &library));
        std::fs::write(&libraryfolders, "not valid vdf").unwrap();
        assert!(!steam_root_references_library(&steam_root, &library));
        std::fs::write(&libraryfolders, "\"InstallConfigStore\"\n{\n}\n").unwrap();
        assert!(!steam_root_references_library(&steam_root, &library));
    }

    #[test]
    fn compat_tool_dir_candidates_match_numbered_proton_once() {
        let tmp = tempfile::tempdir().unwrap();
        let steamapps = tmp.path().join("steamapps");
        for dir in ["Proton 10.0", "Proton 9.0", "Proton - Experimental"] {
            std::fs::create_dir_all(steamapps.join("common").join(dir)).unwrap();
        }

        // The library is also the Steam root, so both scans see the same folders.
        assert_eq!(
            compat_tool_dir_candidates(tmp.path(), &steamapps, "proton_10"),
            vec!["proton_10".to_owned(), "Proton 10.0".to_owned()]
        );
        // A Steam root without a common folder adds nothing.
        assert_eq!(
            compat_tool_dir_candidates(&tmp.path().join("no-steam"), &steamapps, "proton_10"),
            vec!["proton_10".to_owned(), "Proton 10.0".to_owned()]
        );
        assert_eq!(
            compat_tool_dir_candidates(tmp.path(), &steamapps, " proton_experimental "),
            vec![
                "proton_experimental".to_owned(),
                "Proton - Experimental".to_owned(),
                "Proton Experimental".to_owned(),
            ]
        );
    }

    #[test]
    fn steamapps_dir_derivation() {
        let game_path = Path::new("/mnt/games/SteamLibrary/steamapps/common/Sonic Adventure 2");
        let result = steamapps_dir(game_path).unwrap();
        assert_eq!(result, PathBuf::from("/mnt/games/SteamLibrary/steamapps"));
    }

    #[test]
    fn steamapps_dir_fails_for_root() {
        let game_path = Path::new("/");
        assert!(steamapps_dir(game_path).is_err());
    }

    proptest::proptest! {
        #[test]
        fn proton_major_is_the_first_number_before_a_dot(
            prefix in "[A-Za-z -]{0,12}",
            major in 0u32..1000,
            rest in "[0-9]{1,4}(-[0-9]{1,4})?",
        ) {
            let text = format!("{prefix}{major}.{rest}");
            proptest::prop_assert_eq!(proton_major_from_text(&text), Some(major));
        }

        #[test]
        fn text_without_a_dotted_number_has_no_proton_major(text in "[A-Za-z0-9 -]{0,24}") {
            proptest::prop_assert_eq!(proton_major_from_text(&text), None);
        }

        #[test]
        fn proton_dir_names_parse_their_version(
            major in 0u32..100,
            minor in 0u32..100,
            build in proptest::option::of(0u32..100),
        ) {
            let build = build.map(|n| format!("-{n}")).unwrap_or_default();
            let name = format!("Proton {major}.{minor}{build}");
            proptest::prop_assert_eq!(
                parse_proton_dir_name(&name),
                Some(ProtonVersion::Numbered(major, minor))
            );
            proptest::prop_assert_eq!(parse_proton_dir_name(&format!("Wine {major}.{minor}")), None);
        }
    }
}
