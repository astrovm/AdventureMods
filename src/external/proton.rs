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

/// Where Steam's Linux client looks for custom compatibility tools outside
/// its own folder (distro packages such as `proton-ge-custom`).
const SYSTEM_COMPAT_TOOL_DIRS: [&str; 2] = [
    "/usr/share/steam/compatibilitytools.d",
    "/usr/local/share/steam/compatibilitytools.d",
];

/// Where each Proton build keeps Wine's `ntdll.so`, newest layout first.
const NTDLL_PATHS: [&str; 4] = [
    "files/lib/wine/x86_64-unix/ntdll.so",
    "files/lib64/wine/x86_64-unix/ntdll.so",
    "files/lib/wine/i386-unix/ntdll.so",
    "files/lib64/wine/ntdll.so",
];

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
    /// Neither the build's Wine nor its labels say which version it is.
    /// Setup may continue, but the UI should warn and recommend Proton 10.0.
    UnknownProton {
        tool_name: String,
    },
}

/// Where to look for Steam installs and compatibility tools. The real one
/// comes from [`SteamPaths::from_system`]; tests build their own.
#[derive(Debug, Clone, Default)]
pub(crate) struct SteamPaths {
    /// Steam installs in the user's home.
    roots: Vec<PathBuf>,
    /// `compatibilitytools.d` folders outside any Steam install.
    tool_dirs: Vec<PathBuf>,
}

impl SteamPaths {
    pub(crate) fn from_system() -> Self {
        let mut tool_dirs: Vec<PathBuf> =
            SYSTEM_COMPAT_TOOL_DIRS.iter().map(PathBuf::from).collect();
        // Steam reads extra tool folders from this colon-separated list.
        if let Some(extra) = std::env::var_os("STEAM_EXTRA_COMPAT_TOOLS_PATHS") {
            tool_dirs.extend(std::env::split_paths(&extra).filter(|p| p.is_absolute()));
        }
        Self {
            roots: library::steam_roots(),
            tool_dirs,
        }
    }
}

/// What Steam left in `compatdata/<app>` the last time the game ran.
#[derive(Debug, Clone)]
struct PrefixMetadata {
    /// The Proton build that set the prefix up, from `config_info`.
    proton_dir: PathBuf,
    /// What that build wrote into `version`: `10.1000-105`, `GE-Proton10-34`, …
    prefix_version: String,
}

/// The tool Steam is set to use for the game.
#[derive(Debug, Clone)]
struct ConfiguredTool {
    /// The name stored in `CompatToolMapping`, such as `proton_10`.
    name: String,
    /// Every installed build Steam could mean by that name. Never empty.
    dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
enum ConfiguredToolLookup {
    Tool(ConfiguredTool),
    MissingConfig,
    InvalidConfig,
    MissingConfiguredTool,
    /// Steam names a tool that isn't installed in any place we can see.
    ConfiguredToolUnavailable {
        name: String,
    },
}

/// The game's Steam compatdata folder, which holds its Proton prefix in `pfx`.
pub fn compat_data_dir(game_path: &Path, app_id: u32) -> Result<PathBuf> {
    Ok(steamapps_dir(game_path)?
        .join("compatdata")
        .join(app_id.to_string()))
}

pub fn prefix_state(game_path: &Path, app_id: u32) -> Result<PrefixState> {
    let state = prefix_state_in(game_path, app_id, &SteamPaths::from_system());
    match &state {
        Ok(state) => tracing::info!("Proton check for app {app_id}: {state:?}"),
        Err(err) => tracing::warn!("Proton check for app {app_id} failed: {err:#}"),
    }
    state
}

fn prefix_state_in(game_path: &Path, app_id: u32, paths: &SteamPaths) -> Result<PrefixState> {
    let compatdata = compat_data_dir(game_path, app_id)?;
    let prefix = compatdata.join("pfx");

    if !prefix.is_dir() {
        return Ok(PrefixState::MissingPrefix);
    }

    let Some(prefix_metadata) = read_prefix_metadata_for_game(game_path, &compatdata)? else {
        return Ok(PrefixState::MissingMetadata);
    };
    let prefix_tool = tool_label(&prefix_metadata.proton_dir);

    let configured_tool = configured_tool_from_config(game_path, app_id, paths)?;
    tracing::debug!(
        "App {app_id} prefix was set up by {} ({}); Steam config: {configured_tool:?}",
        prefix_metadata.proton_dir.display(),
        prefix_metadata.prefix_version,
    );

    match &configured_tool {
        ConfiguredToolLookup::Tool(configured) => {
            let prefix_canonical = try_canonicalize(&prefix_metadata.proton_dir);
            let same_build = configured
                .dirs
                .iter()
                .any(|dir| try_canonicalize(dir) == prefix_canonical);

            if !same_build {
                let dir = &configured.dirs[0];
                if let Some(major) = build_major(dir, &[&configured.name])
                    && major > MAX_SUPPORTED_PROTON_MAJOR
                {
                    return Ok(PrefixState::UnsupportedConfiguredProton {
                        tool_name: tool_label(dir),
                        major,
                    });
                }

                return Ok(PrefixState::ConfigMismatch {
                    prefix_tool,
                    configured_tool: tool_label(dir),
                });
            }
        }
        // Steam downloads a missing Valve build on the next launch; its name
        // is all there is to go on.
        ConfiguredToolLookup::ConfiguredToolUnavailable { name } => {
            if let Some(major) = major_from_label(name)
                && major > MAX_SUPPORTED_PROTON_MAJOR
            {
                return Ok(PrefixState::UnsupportedConfiguredProton {
                    tool_name: name.clone(),
                    major,
                });
            }
        }
        _ => {}
    }

    if !has_wine_binary(&prefix_metadata.proton_dir) {
        return Ok(PrefixState::ProtonUnavailable {
            tool_name: prefix_tool,
            proton_dir: prefix_metadata.proton_dir,
        });
    }

    if matches!(
        configured_tool,
        ConfiguredToolLookup::MissingConfig | ConfiguredToolLookup::InvalidConfig
    ) {
        return Ok(PrefixState::SteamConfigIncomplete);
    }

    match build_major(
        &prefix_metadata.proton_dir,
        &[&prefix_metadata.prefix_version],
    ) {
        Some(major) if major > MAX_SUPPORTED_PROTON_MAJOR => Ok(PrefixState::UnsupportedProton {
            tool_name: prefix_tool,
            major,
        }),
        Some(_) => Ok(PrefixState::Ready),
        None => Ok(PrefixState::UnknownProton {
            tool_name: prefix_tool,
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

/// The name people know a build by: its folder, such as `Proton 10.0` or
/// `GE-Proton10-34`.
fn tool_label(proton_dir: &Path) -> String {
    proton_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| proton_dir.display().to_string(), str::to_owned)
}

/// The Wine major version a Proton build runs, which is what decides whether
/// SA Mod Manager starts.
///
/// Every build, whatever its name, has Wine's own version compiled into
/// `ntdll.so`, so that's read first. Names are only a fallback for builds
/// without it, checked in order: `labels`, the folder name, the `version`
/// file.
fn build_major(proton_dir: &Path, labels: &[&str]) -> Option<u32> {
    wine_major(proton_dir)
        .or_else(|| labels.iter().find_map(|label| major_from_label(label)))
        .or_else(|| {
            proton_dir
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(major_from_label)
        })
        .or_else(|| {
            std::fs::read_to_string(proton_dir.join("version"))
                .ok()
                .and_then(|version| major_from_label(&version))
        })
}

fn wine_major(proton_dir: &Path) -> Option<u32> {
    NTDLL_PATHS.iter().find_map(|relative| {
        std::fs::read(proton_dir.join(relative))
            .ok()
            .and_then(|bytes| wine_major_in(&bytes))
    })
}

/// Find Wine's `PACKAGE_VERSION` string, such as `wine-10.0`,
/// `wine-10.0 (Staging)` or `wine-11.0 (CachyOS)`, in a binary and return its
/// major version. It sits alone between NUL-padded data, so a match must not
/// be part of a longer word or path.
fn wine_major_in(bytes: &[u8]) -> Option<u32> {
    const NEEDLE: &[u8] = b"wine-";
    const MAX_LEN: usize = 64;

    let mut from = 0;
    while let Some(offset) = bytes[from..]
        .windows(NEEDLE.len())
        .position(|window| window == NEEDLE)
    {
        let start = from + offset;
        from = start + 1;

        let glued = start
            .checked_sub(1)
            .map(|i| bytes[i])
            .is_some_and(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b));
        if glued {
            continue;
        }

        let rest = &bytes[start + NEEDLE.len()..];
        let Some(end) = rest.iter().take(MAX_LEN).position(|&b| b == 0) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&rest[..end]) else {
            continue;
        };
        if !text.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
            continue;
        }
        let (major, after) = split_number(text);
        if let Some(major) = major
            && after.starts_with('.')
            && after[1..].starts_with(|c: char| c.is_ascii_digit())
        {
            return Some(major);
        }
    }
    None
}

/// The major version in a Proton name or version label.
///
/// Understands dotted versions (`10.1000-105`, `proton-10.0-4b`,
/// `CachyOS-11.0-100`, `Proton 8.0`) and numbered tags (`GE-Proton10-34`,
/// `proton_10`). Names without a number, like `Proton - Experimental`, have
/// none.
fn major_from_label(text: &str) -> Option<u32> {
    dotted_major(text).or_else(|| proton_tag_major(text))
}

fn dotted_major(text: &str) -> Option<u32> {
    let mut rest = text;
    while let Some(start) = rest.find(|c: char| c.is_ascii_digit()) {
        let (major, after) = split_number(&rest[start..]);
        if after.starts_with('.') && after[1..].starts_with(|c: char| c.is_ascii_digit()) {
            return major;
        }
        rest = after;
    }
    None
}

/// `Proton` followed by its version, optionally after `_`, `-` or a space.
fn proton_tag_major(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    lower.match_indices("proton").find_map(|(index, word)| {
        let rest = &lower[index + word.len()..];
        let rest = rest.strip_prefix(['_', '-', ' ']).unwrap_or(rest);
        split_number(rest).0
    })
}

/// Split leading ASCII digits off `text`.
fn split_number(text: &str) -> (Option<u32>, &str) {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    (text[..end].parse().ok(), &text[end..])
}

fn configured_tool_from_config(
    game_path: &Path,
    app_id: u32,
    paths: &SteamPaths,
) -> Result<ConfiguredToolLookup> {
    let steamapps = steamapps_dir(game_path)?;
    // Never empty: the library's own root is always a candidate.
    let steam_roots = steam_root_candidates(game_path, paths)?;

    let mut best_failure = ConfiguredToolLookup::MissingConfig;

    for steam_root in &steam_roots {
        match compat_tool_name_from_config(steam_root, app_id)? {
            Ok(name) => {
                let libraries = steam_libraries(steam_root, &steamapps);
                let mut tool_dirs = vec![steam_root.join("compatibilitytools.d")];
                tool_dirs.extend(paths.tool_dirs.iter().cloned());

                let dirs = find_tool_dirs(&name, &libraries, &tool_dirs);
                if !dirs.is_empty() {
                    return Ok(ConfiguredToolLookup::Tool(ConfiguredTool { name, dirs }));
                }

                best_failure = ConfiguredToolLookup::ConfiguredToolUnavailable { name };
            }
            Err(other) => {
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
        ConfiguredToolLookup::ConfiguredToolUnavailable { .. } => 3,
        ConfiguredToolLookup::MissingConfiguredTool => 2,
        ConfiguredToolLookup::InvalidConfig => 1,
        ConfiguredToolLookup::MissingConfig => 0,
    }
}

/// Every library a Steam install uses, the game's own first. Valve's Proton
/// builds install like games, so they can be in any of them.
fn steam_libraries(steam_root: &Path, game_steamapps: &Path) -> Vec<PathBuf> {
    let mut libraries = vec![
        game_steamapps
            .parent()
            .unwrap_or(game_steamapps)
            .to_path_buf(),
        steam_root.to_path_buf(),
    ];

    if let Ok(content) = std::fs::read_to_string(steam_root.join("steamapps/libraryfolders.vdf"))
        && let Some(root) = vdf::parse(&content)
        && let Some(folders) = root.get("libraryfolders").and_then(|v| v.as_map())
    {
        let mut listed: Vec<_> = folders
            .iter()
            .filter_map(|(key, folder)| {
                let path = folder.get("path")?.as_str()?;
                Some((key.parse::<u32>().unwrap_or(u32::MAX), PathBuf::from(path)))
            })
            .collect();
        listed.sort();
        libraries.extend(listed.into_iter().map(|(_, path)| path));
    }

    dedupe_paths(libraries)
}

/// Every installed build Steam could mean by `name`, best match first.
fn find_tool_dirs(name: &str, libraries: &[PathBuf], tool_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let name = name.trim();
    let mut found = Vec::new();

    for tool_dir in tool_dirs {
        found.extend(custom_tools_named(tool_dir, name));
    }

    let valve_dirs = valve_tool_dir_names(name);
    for library in libraries {
        let common = library.join("steamapps/common");
        found.extend(valve_dirs.iter().map(|dir| common.join(dir)));

        if let Some(major) = name
            .strip_prefix("proton_")
            .and_then(|value| value.parse::<u32>().ok())
            && let Ok(entries) = std::fs::read_dir(&common)
        {
            let mut numbered: Vec<_> = entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|dir| {
                    matches!(
                        parse_proton_dir_name(dir),
                        Some(ProtonVersion::Numbered(found, _)) if found == major
                    )
                })
                .collect();
            numbered.sort();
            found.extend(numbered.into_iter().map(|dir| common.join(dir)));
        }
    }

    dedupe_paths(
        found
            .into_iter()
            .filter(|dir| has_wine_binary(dir))
            .collect(),
    )
}

/// Folder names Valve uses for a tool, besides `proton_<major>` builds.
fn valve_tool_dir_names(name: &str) -> Vec<&str> {
    let mut names = vec![name];
    names.extend_from_slice(match name {
        "proton_experimental" => &["Proton - Experimental", "Proton Experimental"],
        "proton_hotfix" => &["Proton Hotfix"],
        _ => &[],
    });
    names
}

/// Custom tools in a `compatibilitytools.d` folder that register `name`.
///
/// Each tool is a folder with a `compatibilitytool.vdf`, or a manifest file
/// directly in the folder, mapping internal names to an `install_path`. The
/// folder name doesn't have to match, so the manifests are what count; a
/// folder named after the tool is still tried for a broken manifest.
fn custom_tools_named(tool_dir: &Path, name: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(tool_dir) else {
        return Vec::new();
    };

    let mut manifests: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| {
            if path.is_dir() {
                Some(path.join("compatibilitytool.vdf"))
            } else if path.extension().is_some_and(|ext| ext == "vdf") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    manifests.sort();

    let mut found: Vec<PathBuf> = manifests
        .iter()
        .filter_map(|manifest| {
            let content = std::fs::read_to_string(manifest).ok()?;
            let root = vdf::parse(&content)?;
            let tools = root.get("compatibilitytools")?.get("compat_tools")?;
            let install_path = tools.get(name)?.get("install_path")?.as_str()?;
            Some(manifest.parent()?.join(install_path))
        })
        .collect();
    found.push(tool_dir.join(name));
    found
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(try_canonicalize(path)))
        .collect()
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
    let prefix_version = std::fs::read_to_string(&version)
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
        Some(proton_dir) if !prefix_version.is_empty() => Ok(Some(PrefixMetadata {
            proton_dir,
            prefix_version,
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

fn steam_root_candidates(game_path: &Path, paths: &SteamPaths) -> Result<Vec<PathBuf>> {
    let steamapps = steamapps_dir(game_path)?;
    let derived_root = steamapps.parent().unwrap_or(&steamapps);
    let mut roots = Vec::new();

    roots.push(derived_root.to_path_buf());
    roots.extend(
        paths
            .roots
            .iter()
            .filter(|root| steam_root_references_library(root, derived_root))
            .cloned(),
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
            .get("path")
            .and_then(|value| value.as_str())
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok().or(Some(path)))
            .is_some_and(|path| target_paths.contains(&path))
    })
}

/// The tool name `config.vdf` maps the game to, or why there is none.
fn compat_tool_name_from_config(
    steam_root: &Path,
    app_id: u32,
) -> Result<std::result::Result<String, ConfiguredToolLookup>> {
    let config_path = steam_root.join("config/config.vdf");
    if !config_path.is_file() {
        return Ok(Err(ConfiguredToolLookup::MissingConfig));
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(err) => {
            tracing::warn!("Failed to read {}: {err}", config_path.display());
            return Ok(Err(ConfiguredToolLookup::InvalidConfig));
        }
    };
    let Some(root) = vdf::parse(&content) else {
        tracing::warn!("Failed to parse {}", config_path.display());
        return Ok(Err(ConfiguredToolLookup::InvalidConfig));
    };

    let Some(mapping) = root
        .get("InstallConfigStore")
        .and_then(|v| v.get("Software"))
        .and_then(|v| v.get("Valve"))
        .and_then(|v| v.get("Steam"))
        .and_then(|v| v.get("CompatToolMapping"))
    else {
        return Ok(Err(ConfiguredToolLookup::MissingConfiguredTool));
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
            return Ok(Ok(name.to_owned()));
        }
    }

    Ok(Err(ConfiguredToolLookup::MissingConfiguredTool))
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
    fn major_from_label_reads_dotted_versions_and_numbered_tags() {
        assert_eq!(major_from_label("10.1000-105"), Some(10));
        assert_eq!(major_from_label("11.0-100"), Some(11));
        assert_eq!(major_from_label("CachyOS-11.0-100"), Some(11));
        assert_eq!(major_from_label("Proton 8.0"), Some(8));
        assert_eq!(major_from_label("1785139158 proton-10.0-4b"), Some(10));
        assert_eq!(major_from_label("GE-Proton10-34"), Some(10));
        assert_eq!(major_from_label("1787951532 GE-Proton11-6"), Some(11));
        assert_eq!(major_from_label("proton_10"), Some(10));
        assert_eq!(major_from_label("Proton - Experimental"), None);
        assert_eq!(major_from_label("proton_experimental"), None);
        assert_eq!(major_from_label("1791407340 hotfix-20261007-x86_64"), None);
        assert_eq!(major_from_label("TestProton"), None);
    }

    #[test]
    fn wine_major_in_reads_the_compiled_wine_version() {
        assert_eq!(wine_major_in(b"\x80\x80wine-10.0\x00\x00"), Some(10));
        assert_eq!(wine_major_in(b"\x00wine-10.0 (Staging)\x00"), Some(10));
        assert_eq!(wine_major_in(b"wine-11.0 (CachyOS)\x00"), Some(11));
        assert_eq!(wine_major_in(b"\x00wine-9.22\x00"), Some(9));
        // Other Wine names, paths and unterminated text are not the version.
        assert_eq!(
            wine_major_in(b"\x00wine-mono-9.4.0\x00wine-gecko\x00"),
            None
        );
        assert_eq!(wine_major_in(b"/usr/lib/wine-10.0\x00"), None);
        assert_eq!(wine_major_in(b"\x00wine-10.0"), None);
        assert_eq!(wine_major_in(b"\x00wine-10.\x00"), None);
        // The real one is found after a decoy.
        assert_eq!(
            wine_major_in(b"\x00wine-mono\x00\x00wine-10.0\x00"),
            Some(10)
        );
    }

    #[test]
    fn wine_major_in_skips_text_that_is_not_a_version_string() {
        // Not UTF-8, a control character, or no number after the dash.
        assert_eq!(wine_major_in(b"\x00wine-\xff10.0\x00"), None);
        assert_eq!(wine_major_in(b"\x00wine-10.0\n\x00"), None);
        assert_eq!(wine_major_in(b"\x00wine-x.0\x00"), None);
    }

    #[test]
    fn tool_label_is_the_folder_name_or_the_whole_path() {
        assert_eq!(
            tool_label(Path::new("/steam/steamapps/common/Proton 10.0")),
            "Proton 10.0"
        );
        assert_eq!(tool_label(Path::new("/")), "/");
    }

    #[test]
    fn steam_paths_include_system_and_extra_tool_folders() {
        let _lock = crate::test_env::lock();
        let previous = std::env::var_os("STEAM_EXTRA_COMPAT_TOOLS_PATHS");
        crate::test_env::set_var(
            "STEAM_EXTRA_COMPAT_TOOLS_PATHS",
            Some("/opt/tools:relative/tools:/srv/more"),
        );
        let paths = SteamPaths::from_system();
        crate::test_env::set_var("STEAM_EXTRA_COMPAT_TOOLS_PATHS", previous);

        // Relative entries mean nothing to Steam, so they're left out.
        assert_eq!(
            paths.tool_dirs,
            [
                "/usr/share/steam/compatibilitytools.d",
                "/usr/local/share/steam/compatibilitytools.d",
                "/opt/tools",
                "/srv/more",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn prefix_state_reports_a_path_outside_any_library() {
        let (result, logs) = crate::test_log::capture_logs(|| prefix_state(Path::new("/"), 71250));
        assert!(result.is_err());
        assert!(logs.contains("Proton check for app 71250 failed"), "{logs}");
    }

    #[test]
    fn build_major_falls_back_to_the_directory_name() {
        let dir = PathBuf::from("/steam/steamapps/common/Proton 10.0");
        assert_eq!(build_major(&dir, &["Proton - Experimental"]), Some(10));
    }

    #[test]
    fn build_major_falls_back_to_the_tool_version_file() {
        let tmp = tempfile::tempdir().unwrap();
        let proton_dir = tmp.path().join("Proton-CachyOS Latest");
        std::fs::create_dir_all(&proton_dir).unwrap();
        std::fs::write(
            proton_dir.join("version"),
            "1778931159 cachyos-11.0-20260506-slr\n",
        )
        .unwrap();

        assert_eq!(
            build_major(&proton_dir, &["Proton-CachyOS Latest"]),
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
                tool_name: "Proton 11.0".to_owned(),
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
            dirs: vec![PathBuf::from("/steam/Proton 10")],
        };
        assert_eq!(failure_priority(&ConfiguredToolLookup::Tool(tool)), 4);
        assert!(
            failure_priority(&ConfiguredToolLookup::ConfiguredToolUnavailable {
                name: "proton_10".to_owned()
            }) > failure_priority(&ConfiguredToolLookup::MissingConfiguredTool)
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
        assert!(matches!(result, Err(ConfiguredToolLookup::InvalidConfig)));
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
            Err(ConfiguredToolLookup::InvalidConfig)
        ));
        assert!(logs.contains(&format!("Failed to read {}", config.display())));

        // Only another game has a forced tool, and there is no global default.
        std::fs::write(&config, PROTON_10_CONFIG.replace("\"71250\"", "\"213610\"")).unwrap();
        assert!(matches!(
            compat_tool_name_from_config(tmp.path(), 71250).unwrap(),
            Err(ConfiguredToolLookup::MissingConfiguredTool)
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

        let paths = SteamPaths::default();
        assert_eq!(steam_root_candidates(&game_path, &paths).unwrap().len(), 2);
        assert!(matches!(
            configured_tool_from_config(&game_path, 71250, &paths).unwrap(),
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
            let roots = steam_root_candidates(&game_path, &SteamPaths::from_system());
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
    fn find_tool_dirs_matches_numbered_proton_once_and_skips_empty_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let common = tmp.path().join("steamapps/common");
        for dir in ["Proton 10.0", "Proton 9.0", "Proton - Experimental"] {
            std::fs::create_dir_all(common.join(dir).join("files/bin")).unwrap();
            std::fs::write(common.join(dir).join("files/bin/wine"), "").unwrap();
        }
        // A leftover folder without Wine isn't an installed build.
        std::fs::create_dir_all(common.join("Proton 10.0-old")).unwrap();
        let library = tmp.path().to_path_buf();

        // The library is listed twice, as it is when it's also the Steam root.
        let libraries = [library.clone(), library];
        assert_eq!(
            find_tool_dirs("proton_10", &libraries, &[]),
            vec![common.join("Proton 10.0")]
        );
        assert_eq!(
            find_tool_dirs(" proton_experimental ", &libraries, &[]),
            vec![common.join("Proton - Experimental")]
        );
        assert!(find_tool_dirs("proton_11", &libraries, &[]).is_empty());
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
        fn label_major_is_the_first_number_before_a_dot(
            prefix in "[A-Za-z -]{0,12}",
            major in 0u32..1000,
            rest in "[0-9]{1,4}(-[0-9]{1,4})?",
        ) {
            let text = format!("{prefix}{major}.{rest}");
            proptest::prop_assert_eq!(major_from_label(&text), Some(major));
        }

        #[test]
        fn text_without_a_dotted_number_or_proton_tag_has_no_major(
            text in "[A-Za-z0-9 -]{0,24}",
        ) {
            proptest::prop_assume!(!text.to_ascii_lowercase().contains("proton"));
            proptest::prop_assert_eq!(major_from_label(&text), None);
        }

        #[test]
        fn numbered_proton_tags_give_their_major(
            prefix in "[A-Za-z]{0,6}-?",
            separator in "[-_ ]?",
            major in 0u32..1000,
            rest in "(-[0-9]{1,3})?",
        ) {
            let text = format!("{prefix}Proton{separator}{major}{rest}");
            proptest::prop_assert_eq!(major_from_label(&text), Some(major));
        }

        #[test]
        fn wine_version_is_found_anywhere_in_a_binary(
            before in proptest::collection::vec(proptest::num::u8::ANY, 0..256),
            after in proptest::collection::vec(proptest::num::u8::ANY, 0..256),
            major in 1u32..100,
            minor in 0u32..100,
        ) {
            // Without a `w` before it, nothing earlier can pass for the version.
            let mut bytes: Vec<u8> = before.into_iter().filter(|&b| b != b'w').collect();
            bytes.push(0);
            bytes.extend_from_slice(format!("wine-{major}.{minor}").as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(&after);
            proptest::prop_assert_eq!(wine_major_in(&bytes), Some(major));
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

#[cfg(test)]
#[path = "proton_scenarios_tests.rs"]
mod scenarios;
