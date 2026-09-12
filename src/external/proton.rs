use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Output;

use anyhow::{Context, Result};

use super::flatpak;
use crate::steam::{library, vdf};

fn try_canonicalize(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

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

pub fn prefix_state(game_path: &Path, app_id: u32) -> Result<PrefixState> {
    let steamapps = steamapps_dir(game_path)?;
    let compatdata = steamapps.join("compatdata").join(app_id.to_string());
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
    match prefix_state(game_path, app_id)? {
        // Unknown labels still allow installs (custom tools / test fixtures), but
        // steam_config_message warns the user to prefer Proton 10.0.
        PrefixState::Ready | PrefixState::UnknownProton { .. } => Ok(()),
        PrefixState::MissingPrefix => anyhow::bail!(
            "Steam has not created this game's Proton prefix. Open the game from Steam once, wait for Proton to finish setting up, then close it and try again."
        ),
        PrefixState::MissingMetadata => anyhow::bail!(
            "This game's Proton prefix metadata is incomplete. Open the game from Steam once so Steam can repair it, then close the game and try again."
        ),
        PrefixState::SteamConfigIncomplete => anyhow::bail!(
            "Steam's Proton configuration for this game is missing or unreadable. Force Proton 10.0 under Properties → Compatibility, open the game once, then close it and try again."
        ),
        PrefixState::ProtonUnavailable {
            tool_name,
            proton_dir,
        } => anyhow::bail!(
            "The Proton prefix uses {tool_name}, but its Wine executable is missing from {}. Reinstall that Proton version in Steam, open the game once, then try again.",
            proton_dir.display()
        ),
        PrefixState::ConfigMismatch {
            prefix_tool,
            configured_tool,
        } => anyhow::bail!(
            "This game's Proton prefix still uses {prefix_tool}, but Steam is configured to use {configured_tool}. Open the game from Steam once so Steam can update the prefix, then try again."
        ),
        PrefixState::UnsupportedConfiguredProton { tool_name, major } => anyhow::bail!(
            "Steam is configured to use {tool_name} (Proton/Wine {major}), which cannot run SA Mod Manager. In Steam: Properties → Compatibility → force Proton 10.0, launch the game once, close it, then try again."
        ),
        PrefixState::UnsupportedProton { tool_name, major } => anyhow::bail!(
            "This game's Proton prefix uses {tool_name} (Proton/Wine {major}), which cannot run SA Mod Manager. In Steam: Properties → Compatibility → force Proton 10.0, launch the game once, close it, then try again."
        ),
    }
}

pub fn steam_config_message(game_name: &str, game_path: &Path, app_id: u32) -> String {
    match prefix_state(game_path, app_id) {
        Ok(PrefixState::Ready) => {
            format!("The Proton prefix for {game_name} is ready. You can continue right away.")
        }
        Ok(PrefixState::MissingPrefix) => format!(
            "Steam has not created a Proton prefix for {game_name}. Force Proton 10.0 in Properties → Compatibility, open the game once, wait for setup to finish, then close it and check again here."
        ),
        Ok(PrefixState::MissingMetadata) => format!(
            "The Proton prefix metadata for {game_name} is incomplete. Force Proton 10.0 if needed, open the game from Steam once so Steam can repair it, then check again here."
        ),
        Ok(PrefixState::SteamConfigIncomplete) => format!(
            "Steam's Proton configuration for {game_name} is missing or unreadable. Force Proton 10.0 under Properties → Compatibility, open the game once, then check again here."
        ),
        Ok(PrefixState::ProtonUnavailable { tool_name, .. }) => format!(
            "{game_name} uses {tool_name}, but that Proton installation is unavailable. Install Proton 10.0 in Steam, force it for this game, open the game once, then check again here."
        ),
        Ok(PrefixState::ConfigMismatch {
            prefix_tool,
            configured_tool,
        }) => format!(
            "{game_name} still has a Proton prefix from {prefix_tool}, but Steam is now configured to use {configured_tool}. Open the game from Steam once so Steam can update the prefix, then check again here."
        ),
        Ok(PrefixState::UnsupportedConfiguredProton { tool_name, major }) => format!(
            "Steam is configured to use {tool_name} (Proton/Wine {major}) for {game_name}, which cannot run SA Mod Manager. In Steam: Properties → Compatibility → force Proton 10.0. Launch the game once, close it, then check again here."
        ),
        Ok(PrefixState::UnsupportedProton { tool_name, major }) => format!(
            "{game_name} is using {tool_name} (Proton/Wine {major}). SA Mod Manager does not start on Proton 11 or newer (including Hotfix, Experimental, and many custom builds). In Steam: Properties → Compatibility → force Proton 10.0. Launch the game once, close it, then check again here."
        ),
        Ok(PrefixState::UnknownProton { tool_name }) => format!(
            "{game_name} is using {tool_name}, but Adventure Mods could not tell which Proton/Wine major version that is. SA Mod Manager needs Proton 10.0; Proton 11 and newer will not launch it. If this is not Proton 10.0, force Proton 10.0 under Properties → Compatibility, launch once, close it, then continue here."
        ),
        Err(err) => {
            tracing::warn!("Failed to inspect Proton prefix state: {err}");
            format!(
                "Force Proton 10.0 for {game_name} in Steam, open it once, then close it and check again here."
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

/// Run an executable inside the game's Proton prefix using Proton's Wine runtime.
///
/// `extra_args` are passed to the executable after the exe path.
pub fn run_in_prefix(
    game_path: &Path,
    app_id: u32,
    exe: &Path,
    extra_args: &[&str],
) -> Result<Output> {
    let proton_dir = find_proton_for_app(game_path, app_id)?;
    let host_proton_dir = host_command_path(&proton_dir);
    let mut env = proton_env(game_path, app_id)?;
    map_env_paths_for_host_command(&mut env);
    let exe = host_command_path(exe);
    let (program, command_args) =
        prefix_command(&proton_dir, &host_proton_dir, &exe, extra_args, &mut env);
    let args: Vec<&str> = command_args.iter().map(String::as_str).collect();
    let program_str = program.to_string_lossy().to_string();

    tracing::info!(
        "Running {} in prefix for app {} with Proton at {} using {}",
        exe.display(),
        app_id,
        proton_dir.display(),
        program.display()
    );

    flatpak::host_command_with_env_sync(&program_str, &args, &env)
}

fn prefix_command(
    visible_proton_dir: &Path,
    host_proton_dir: &Path,
    exe: &Path,
    extra_args: &[&str],
    env: &mut HashMap<String, String>,
) -> (PathBuf, Vec<String>) {
    let proton_launcher = visible_proton_dir.join("proton");
    let use_launcher = proton_launcher.is_file();
    let mut args = Vec::with_capacity(extra_args.len() + 2);

    if use_launcher {
        // Proton's launcher initializes the session and repairs tracked prefix
        // files before dispatching through its own compatible Wine loader.
        args.push("runinprefix".to_owned());
    } else {
        configure_proton_runtime_env(env, host_proton_dir);
    }

    args.push(exe.to_string_lossy().into_owned());
    args.extend(extra_args.iter().map(|arg| (*arg).to_owned()));

    let program = if use_launcher {
        host_command_path(&proton_launcher)
    } else {
        host_command_path(&wine_binary(visible_proton_dir))
    };

    (program, args)
}

fn host_command_path(path: &Path) -> PathBuf {
    library::resolve_document_portal_host_path(path).unwrap_or_else(|| path.to_path_buf())
}

fn map_env_paths_for_host_command(env: &mut HashMap<String, String>) {
    for key in [
        "WINEPREFIX",
        "STEAM_COMPAT_DATA_PATH",
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
    ] {
        if let Some(value) = env.get_mut(key) {
            let path = Path::new(value);
            if let Some(host_path) = library::resolve_document_portal_host_path(path) {
                *value = host_path.to_string_lossy().into_owned();
            }
        }
    }
}

fn configured_tool_from_config(game_path: &Path, app_id: u32) -> Result<ConfiguredToolLookup> {
    let steamapps = steamapps_dir(game_path)?;
    let steam_roots = steam_root_candidates(game_path)?;

    if steam_roots.is_empty() {
        return Ok(ConfiguredToolLookup::MissingConfig);
    }

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

fn find_proton_from_prefix_metadata(game_path: &Path, app_id: u32) -> Result<Option<PathBuf>> {
    let steamapps = steamapps_dir(game_path)?;
    let compatdata = steamapps.join("compatdata").join(app_id.to_string());
    Ok(read_prefix_metadata_for_game(game_path, &compatdata)?.map(|metadata| metadata.proton_dir))
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

fn steam_client_root(game_path: &Path) -> Result<PathBuf> {
    for root in steam_root_candidates(game_path)? {
        if root.join("config/config.vdf").is_file() {
            return Ok(root);
        }
    }

    anyhow::bail!(
        "Steam's config.vdf could not be found for {}",
        game_path.display()
    )
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

    for alias in aliases {
        if seen.insert(alias.to_owned()) {
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

/// Determine the Wine binary path inside a Proton installation.
fn wine_binary(proton_dir: &Path) -> PathBuf {
    let wine = proton_dir.join("files/bin/wine");
    if wine.is_file() {
        wine
    } else {
        proton_dir.join("files/bin/wine64")
    }
}

fn configure_proton_runtime_env(env: &mut HashMap<String, String>, proton_dir: &Path) {
    let lib_dir = proton_dir.join("files/lib");
    let runtime_library_path = [
        lib_dir.join("x86_64-linux-gnu"),
        lib_dir.join("i386-linux-gnu"),
    ]
    .into_iter()
    .map(|path| path.to_string_lossy().into_owned())
    .collect::<Vec<_>>()
    .join(":");

    let ld_library_path = env
        .get("LD_LIBRARY_PATH")
        .filter(|existing| !existing.is_empty())
        .map_or(runtime_library_path.clone(), |existing| {
            format!("{runtime_library_path}:{existing}")
        });
    env.insert("LD_LIBRARY_PATH".into(), ld_library_path);

    env.insert(
        "WINEDLLPATH".into(),
        [lib_dir.join("vkd3d"), lib_dir.join("wine")]
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(":"),
    );

    let bin_dir = proton_dir.join("files/bin").to_string_lossy().into_owned();
    let path = env
        .get("PATH")
        .filter(|existing| !existing.is_empty())
        .map_or(bin_dir.clone(), |existing| format!("{bin_dir}:{existing}"));
    env.insert("PATH".into(), path);

    let wineserver = proton_dir.join("files/bin/wineserver");
    if wineserver.is_file() {
        env.insert(
            "WINESERVER".into(),
            wineserver.to_string_lossy().into_owned(),
        );
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
    fn test_proton_major_from_text() {
        assert_eq!(proton_major_from_text("10.1000-105"), Some(10));
        assert_eq!(proton_major_from_text("11.0-100"), Some(11));
        assert_eq!(proton_major_from_text("CachyOS-11.0-100"), Some(11));
        assert_eq!(proton_major_from_text("Proton 8.0"), Some(8));
        assert_eq!(proton_major_from_text("Proton - Experimental"), None);
        assert_eq!(proton_major_from_text("TestProton"), None);
    }

    #[test]
    fn test_proton_major_from_labels_uses_directory_name() {
        let dir = PathBuf::from("/steam/steamapps/common/Proton 10.0");
        assert_eq!(
            proton_major_from_labels("Proton - Experimental", &dir),
            Some(10)
        );
    }

    #[test]
    fn test_proton_major_from_labels_uses_tool_version_file() {
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
    fn test_prefix_state_rejects_proton_11() {
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

        let message = steam_config_message("Sonic Adventure DX", &game_path, 71250);
        assert!(message.contains("Proton 10.0"));
        assert!(message.contains("11"));
    }

    #[test]
    fn test_prefix_state_accepts_proton_10() {
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
    }

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
        std::fs::create_dir_all(compatdata.join("pfx")).unwrap();
        write_prefix_metadata(
            &compatdata,
            "Proton 8.0",
            &steam_root.join("steamapps/common/Proton 8.0"),
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
                        "name"  "Proton 8.0"
                    }
                }
            }
        }
    }
}"#,
        )
        .unwrap();

        let result = find_proton_for_app(&game_path, 71250).unwrap();
        assert_eq!(result, common.join("Proton 8.0"));
    }

    #[cfg(target_os = "linux")]
    fn try_set_host_path_xattr(path: &Path, host_path: &Path) -> bool {
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
        result == 0
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_find_proton_for_app_maps_host_metadata_into_document_portal() {
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

        if !try_set_host_path_xattr(&portal_common, &host_common) {
            eprintln!("skipping xattr-backed Proton portal test; filesystem has no user xattrs");
            return;
        }

        assert_eq!(prefix_state(&game_path, 71250).unwrap(), PrefixState::Ready);
        assert_eq!(
            find_proton_for_app(&game_path, 71250).unwrap(),
            portal_proton
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_host_command_paths_map_document_portal_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let host_root = tmp.path().join("host/SteamLibrary");
        let portal_root = tmp.path().join("doc/abc123/SteamLibrary");
        let portal_prefix = portal_root.join("steamapps/compatdata/71250/pfx");

        std::fs::create_dir_all(&portal_prefix).unwrap();

        if !try_set_host_path_xattr(&portal_root, &host_root) {
            eprintln!(
                "skipping xattr-backed host command path test; filesystem has no user xattrs"
            );
            return;
        }

        assert_eq!(
            host_command_path(&portal_prefix),
            host_root.join("steamapps/compatdata/71250/pfx")
        );

        let mut env = HashMap::from([
            (
                "WINEPREFIX".to_owned(),
                portal_prefix.to_string_lossy().into_owned(),
            ),
            (
                "STEAM_COMPAT_DATA_PATH".to_owned(),
                portal_root
                    .join("steamapps/compatdata/71250")
                    .to_string_lossy()
                    .into_owned(),
            ),
        ]);
        map_env_paths_for_host_command(&mut env);

        assert_eq!(
            env["WINEPREFIX"],
            host_root
                .join("steamapps/compatdata/71250/pfx")
                .to_string_lossy()
        );
        assert_eq!(
            env["STEAM_COMPAT_DATA_PATH"],
            host_root
                .join("steamapps/compatdata/71250")
                .to_string_lossy()
        );
    }

    #[test]
    fn test_steam_client_root_finds_custom_sibling_steam_root_for_extra_library_game() {
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path().join("custom-steam");
        let library_root = tmp.path().join("extra-library");
        let game_path = library_root.join("steamapps/common/Sonic Adventure DX");

        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(steam_root.join("config")).unwrap();
        std::fs::create_dir_all(steam_root.join("steamapps")).unwrap();
        std::fs::write(
            steam_root.join("config/config.vdf"),
            "\"InstallConfigStore\"\n{\n}\n",
        )
        .unwrap();
        std::fs::write(
            steam_root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n    }}\n}}\n",
                library_root.display()
            ),
        )
        .unwrap();

        assert_eq!(steam_client_root(&game_path).unwrap(), steam_root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_steam_root_references_document_portal_library() {
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

        if !try_set_host_path_xattr(&portal_library, &host_library) {
            eprintln!(
                "skipping xattr-backed Steam root portal test; filesystem has no user xattrs"
            );
            return;
        }

        assert!(steam_root_references_library(&steam_root, &portal_library));
    }

    #[test]
    fn test_failure_priority_ordering() {
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
    fn test_compat_tool_name_invalid_vdf_returns_invalid_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.vdf"), "not valid vdf content").unwrap();

        let result = compat_tool_name_from_config(tmp.path(), 71250).unwrap();
        assert!(matches!(result, ConfiguredToolLookup::InvalidConfig));
    }

    #[test]
    fn test_prefix_state_missing_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");
        let game_path = common.join("Sonic Adventure DX");

        std::fs::create_dir_all(&game_path).unwrap();

        let result = prefix_state(&game_path, 71250).unwrap();
        assert_eq!(result, PrefixState::MissingPrefix);

        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert!(format!("{error:#}").contains("has not created this game's Proton prefix"));
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

        let error = ensure_prefix_ready(&game_path, 71250).unwrap_err();
        assert!(format!("{error:#}").contains("prefix metadata is incomplete"));
    }

    #[test]
    fn test_prefix_state_reports_missing_proton_installation() {
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
        assert!(message.contains(&proton_dir.display().to_string()));
        assert!(message.contains("Wine executable is missing"));
    }

    #[test]
    fn test_prefix_state_missing_config() {
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
        assert!(format!("{error:#}").contains("configuration for this game is missing"));
    }

    #[test]
    fn test_prefix_state_invalid_config() {
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
    fn test_prefix_state_missing_configured_tool() {
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
    fn test_prefix_state_configured_tool_unavailable() {
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
    fn test_prefix_state_warns_on_unknown_proton_label() {
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
        assert!(message.contains("could not tell"));
        assert!(message.contains("Proton 10.0"));
        assert!(message.contains("TestProton"));
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
    fn test_prefix_state_rejects_unsupported_configured_tool_before_prefix_update() {
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
        assert!(message.contains("Steam is configured to use"));
        assert!(message.contains("Proton/Wine 11"));
        assert!(!message.contains("update the prefix"));
    }

    #[test]
    fn test_prefix_state_detects_mismatch_when_old_proton_is_missing() {
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
                        "name"  "proton_experimental"
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
        let tmp = tempfile::tempdir().unwrap();
        let steam_root = tmp.path();
        let game_path = steam_root.join("steamapps/common/Sonic Adventure DX");
        std::fs::create_dir_all(&game_path).unwrap();
        std::fs::create_dir_all(steam_root.join("config")).unwrap();
        std::fs::write(
            steam_root.join("config/config.vdf"),
            "\"InstallConfigStore\"\n{\n}\n",
        )
        .unwrap();

        let env = proton_env(&game_path, 71250).unwrap();

        assert_eq!(
            env["WINEPREFIX"],
            steam_root
                .join("steamapps/compatdata/71250/pfx")
                .to_string_lossy()
        );
        assert_eq!(
            env["STEAM_COMPAT_DATA_PATH"],
            steam_root
                .join("steamapps/compatdata/71250")
                .to_string_lossy()
        );
        assert_eq!(
            env["STEAM_COMPAT_CLIENT_INSTALL_PATH"],
            steam_root.to_string_lossy()
        );
        assert_eq!(env["SteamAppId"], "71250");
    }

    #[test]
    fn test_configure_proton_runtime_env_sets_loader_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let proton_dir = tmp.path().join("Proton 10.0");
        std::fs::create_dir_all(proton_dir.join("files/bin")).unwrap();
        std::fs::write(proton_dir.join("files/bin/wineserver"), "").unwrap();
        let mut env = HashMap::from([
            ("LD_LIBRARY_PATH".to_owned(), "/host/lib".to_owned()),
            ("PATH".to_owned(), "/usr/bin".to_owned()),
        ]);

        configure_proton_runtime_env(&mut env, &proton_dir);

        let proton_dir = proton_dir.to_string_lossy();

        assert_eq!(
            env["LD_LIBRARY_PATH"],
            format!(
                "{proton_dir}/files/lib/x86_64-linux-gnu:{proton_dir}/files/lib/i386-linux-gnu:/host/lib"
            )
        );
        assert_eq!(
            env["WINEDLLPATH"],
            format!("{proton_dir}/files/lib/vkd3d:{proton_dir}/files/lib/wine")
        );
        assert_eq!(env["PATH"], format!("{proton_dir}/files/bin:/usr/bin"));
        assert_eq!(
            env["WINESERVER"],
            format!("{proton_dir}/files/bin/wineserver")
        );
    }

    #[test]
    fn test_wine_binary_prefers_compatible_loader() {
        let tmp = tempfile::tempdir().unwrap();
        let wine_dir = tmp.path().join("files/bin");
        std::fs::create_dir_all(&wine_dir).unwrap();
        std::fs::write(wine_dir.join("wine"), "").unwrap();
        std::fs::write(wine_dir.join("wine64"), "").unwrap();

        assert_eq!(wine_binary(tmp.path()), wine_dir.join("wine"));
    }

    #[test]
    fn test_prefix_command_prefers_proton_launcher() {
        let tmp = tempfile::tempdir().unwrap();
        let proton_dir = tmp.path().join("Proton 10.0");
        std::fs::create_dir_all(proton_dir.join("files/bin")).unwrap();
        std::fs::write(proton_dir.join("files/bin/wine"), "").unwrap();
        std::fs::write(proton_dir.join("proton"), "").unwrap();
        let mut env = HashMap::new();

        let (program, args) = prefix_command(
            &proton_dir,
            &proton_dir,
            Path::new("/tmp/windowsdesktop-runtime.exe"),
            &["/install", "/quiet"],
            &mut env,
        );

        assert_eq!(program, proton_dir.join("proton"));
        assert_eq!(
            args,
            vec![
                "runinprefix".to_owned(),
                "/tmp/windowsdesktop-runtime.exe".to_owned(),
                "/install".to_owned(),
                "/quiet".to_owned(),
            ]
        );
        assert!(env.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_prefix_command_detects_launcher_from_portal_path() {
        let tmp = tempfile::tempdir().unwrap();
        let portal_dir = tmp.path().join("doc/abc123/Proton 10.0");
        let host_dir = tmp.path().join("host/Proton 10.0");
        std::fs::create_dir_all(&portal_dir).unwrap();
        std::fs::write(portal_dir.join("proton"), "").unwrap();

        if !try_set_host_path_xattr(&portal_dir, &host_dir) {
            eprintln!("skipping xattr-backed Proton launcher test; filesystem has no user xattrs");
            return;
        }

        let mut env = HashMap::new();
        let (program, args) = prefix_command(
            &portal_dir,
            &host_dir,
            Path::new("/tmp/windowsdesktop-runtime.exe"),
            &["/install"],
            &mut env,
        );

        assert_eq!(program, host_dir.join("proton"));
        assert_eq!(
            args,
            vec![
                "runinprefix".to_owned(),
                "/tmp/windowsdesktop-runtime.exe".to_owned(),
                "/install".to_owned(),
            ]
        );
        assert!(env.is_empty());
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
