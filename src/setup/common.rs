use std::path::Path;

use anyhow::{Context, Result, anyhow};
use gtk::gio;

use crate::blocking;
use crate::external::{archive, download, proton, runtime_installer};
use crate::steam::game::{Game, GameKind};

const GAMEBANANA_API_BASE: &str =
    "https://api.gamebanana.com/Core/Item/Data?fields=Files().aFiles()";

use super::steps::StepId;
use super::{config, sa2, sadx, types};
pub use types::{ModEntry, ModLink, ModPreset, ModSource};

/// GitHub release URL for SA Mod Manager (x64).
const SA_MOD_MANAGER_URL: &str =
    "https://github.com/X-Hax/SA-Mod-Manager/releases/latest/download/release_x64.zip";

const SADX_DCMODS_BASE_URL: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/";

/// GitHub release URL for SADX Mod Loader.
const SADX_MOD_LOADER_URL: &str =
    "https://github.com/X-Hax/sadx-mod-loader/releases/latest/download/SADXModLoader.7z";

/// GitHub release URL for SA2 Mod Loader.
const SA2_MOD_LOADER_URL: &str =
    "https://github.com/X-Hax/sa2-mod-loader/releases/latest/download/SA2ModLoader.7z";

/// Resolve a `ModSource` to a download URL string.
pub fn resolve_download_url(source: &ModSource) -> Result<String> {
    match source {
        ModSource::GameBananaItem { item_type, item_id } => {
            resolve_gamebanana_item_url(item_type, *item_id)
        }
        ModSource::DirectUrl { url } => Ok(rewrite_direct_url(url)),
    }
}

fn rewrite_direct_url(url: &str) -> String {
    if let Ok(base_override) = std::env::var("ADVENTURE_MODS_DCMODS_BASE_URL")
        && let Some(suffix) = url.strip_prefix(SADX_DCMODS_BASE_URL)
    {
        return format!("{base_override}{suffix}");
    }

    if let Ok(base_override) = std::env::var("ADVENTURE_MODS_DIRECT_URL_BASE_OVERRIDE")
        && let Some(filename) = url.rsplit('/').next()
        && !filename.is_empty()
    {
        return format!("{base_override}{filename}");
    }

    url.to_string()
}

pub(crate) fn env_or_default(var: &str, default: &'static str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_string())
}

/// Query the GameBanana Core API for the latest file of an item and return its download URL.
fn resolve_gamebanana_item_url(item_type: &str, item_id: u32) -> Result<String> {
    let api_base = std::env::var("ADVENTURE_MODS_GAMEBANANA_API_BASE")
        .unwrap_or_else(|_| GAMEBANANA_API_BASE.to_string());
    let url = format!("{api_base}&itemtype={item_type}&itemid={item_id}");
    let dl_base = std::env::var("ADVENTURE_MODS_GAMEBANANA_DL_BASE")
        .unwrap_or_else(|_| "https://gamebanana.com/dl/".to_string());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    rt.block_on(async {
        let client = reqwest::Client::new();
        let body = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GameBanana API request failed for {item_type}/{item_id}"))?
            .error_for_status()
            .with_context(|| format!("GameBanana API error for {item_type}/{item_id}"))?
            .text()
            .await
            .context("Failed to read GameBanana API response")?;

        let parsed: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&body)
            .with_context(|| {
            format!("Failed to parse GameBanana API response for {item_type}/{item_id}: {body}")
        })?;

        let files = parsed
            .into_iter()
            .next()
            .with_context(|| format!("Empty GameBanana API response for {item_type}/{item_id}"))?;

        let latest_id = files
            .values()
            .filter_map(|v| v.get("_idRow").and_then(|id| id.as_u64()))
            .max()
            .with_context(|| {
                format!("No files found in GameBanana API response for {item_type}/{item_id}")
            })?;

        Ok(format!("{dl_base}{latest_id}"))
    })
}

fn sa_mod_manager_url() -> String {
    env_or_default("ADVENTURE_MODS_URL_SA_MOD_MANAGER", SA_MOD_MANAGER_URL)
}

fn mod_loader_url(game_kind: GameKind) -> String {
    match game_kind {
        GameKind::SADX => env_or_default("ADVENTURE_MODS_URL_SADX_MOD_LOADER", SADX_MOD_LOADER_URL),
        GameKind::SA2 => env_or_default("ADVENTURE_MODS_URL_SA2_MOD_LOADER", SA2_MOD_LOADER_URL),
    }
}

/// Return the recommended mods list for a given game.
pub fn recommended_mods_for_game(kind: GameKind) -> &'static [ModEntry] {
    match kind {
        GameKind::SADX => sadx::RECOMMENDED_MODS,
        GameKind::SA2 => sa2::RECOMMENDED_MODS,
    }
}

/// Return the presets for a given game.
pub fn presets_for_game(kind: GameKind) -> &'static [ModPreset] {
    match kind {
        GameKind::SADX => sadx::PRESETS,
        GameKind::SA2 => &[], // No presets for SA2 yet
    }
}

/// Check whether a setup step has already been completed for the given game.
///
/// Returns `true` if the step's effects are already present on disk and it
/// can safely be skipped.
pub fn is_step_complete(step_id: StepId, game: &Game) -> bool {
    let p = &game.path;
    match step_id {
        StepId::SteamConfig => proton::prefix_state(p, game.kind.app_id())
            .map(|state| matches!(state, proton::PrefixState::Ready))
            .unwrap_or(false),

        StepId::Dotnet => {
            let Ok(prefix) = proton_prefix(p, game.kind.app_id()) else {
                return false;
            };
            matches!(
                proton::prefix_state(p, game.kind.app_id()),
                Ok(proton::PrefixState::Ready)
            ) && runtime_installer::is_dotnet_installed(&prefix)
        }

        StepId::ConvertSteam => {
            sadx_data_dir(p)
                .and_then(|dir| find_file_icase(&dir, "CHRMODELS_orig.dll"))
                .is_some()
                || p.join("SADXModLoader.dll").exists()
                || p.join("mods/.modloader/SADXModLoader.dll").exists()
                || p.join("sonic.exe").exists()
        }

        StepId::InstallModManager => is_mod_manager_fully_installed(p, game.kind),

        StepId::SelectMods | StepId::LanguageOptions | StepId::DownloadMods | StepId::Complete => {
            false
        }
    }
}

pub fn steam_config_message(game: &Game) -> String {
    proton::steam_config_message(game.kind.name(), &game.path, game.kind.app_id())
}

fn is_mod_manager_fully_installed(game_path: &Path, game_kind: GameKind) -> bool {
    let exe_backed_up = game_path.join("Launcher.exe.bak").exists()
        || game_path.join("Sonic Adventure DX.exe.bak").exists();
    let loader_extracted = match game_kind {
        GameKind::SADX => game_path.join("mods/.modloader/SADXModLoader.dll").exists(),
        GameKind::SA2 => game_path.join("mods/.modloader/SA2ModLoader.dll").exists(),
    };
    let dll_swapped = match game_kind {
        GameKind::SADX => sadx_data_dir(game_path)
            .and_then(|dir| find_file_icase(&dir, "CHRMODELS_orig.dll"))
            .is_some(),
        GameKind::SA2 => find_file_icase(
            &game_path.join("resource/gd_PC/DLL/Win32"),
            "Data_DLL_orig.dll",
        )
        .is_some(),
    };

    exe_backed_up && loader_extracted && dll_swapped
}

/// Derive the Proton prefix path from a game's install directory and app ID.
///
/// Game path is typically `.../steamapps/common/<game>/`, and the prefix lives
/// at `.../steamapps/compatdata/<appid>/pfx/`.
fn proton_prefix(game_path: &Path, app_id: u32) -> Result<std::path::PathBuf> {
    game_path
        .parent() // common/
        .and_then(|p| p.parent()) // steamapps/
        .map(|steamapps| {
            steamapps
                .join("compatdata")
                .join(app_id.to_string())
                .join("pfx")
        })
        .context(format!(
            "Cannot derive Proton prefix from game path: {}",
            game_path.display()
        ))
}

/// Install .NET Desktop Runtime 10.0 into the game's Proton prefix
/// using the game's own Proton/Wine.
pub async fn install_runtimes(game_path: std::path::PathBuf, app_id: u32) -> Result<()> {
    blocking::flatten_spawn_result(
        gio::spawn_blocking(move || runtime_installer::install_runtimes(&game_path, app_id)).await,
    )
}

/// Download and install SA Mod Manager and the mod loader into the game directory.
///
/// Downloads the manager from GitHub, extracts, and replaces Launcher.exe with
/// SAModManager.exe (backing up the original). Then downloads and extracts
/// the mod loader DLLs.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_mod_manager(
    game_path: &Path,
    game_kind: GameKind,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    if is_mod_manager_fully_installed(game_path, game_kind) {
        tracing::info!("SA Mod Manager and loader already present, skipping installation");
        return Ok(());
    }

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("SAModManager.zip");

    let manager_url = sa_mod_manager_url();
    download::download_file(&manager_url, &archive_path, progress)?;

    let extract_dir = temp_dir.path().join("extracted");
    archive::extract(&archive_path, &extract_dir)?;

    let manager_exe = extract_dir.join("SAModManager.exe");
    if !manager_exe.is_file() {
        anyhow::bail!("SAModManager.exe not found in release archive");
    }

    let dest_exe = game_path.join("SAModManager.exe");
    std::fs::copy(&manager_exe, &dest_exe)
        .context("Failed to copy SAModManager.exe to game directory")?;

    // Replace the game's Steam launch executable with the mod manager so
    // Steam launches the mod manager, which then launches the real game exe.
    // SA2 uses Launcher.exe; SADX uses "Sonic Adventure DX.exe".
    let launcher = game_path.join("Launcher.exe");
    let sadx_exe = game_path.join("Sonic Adventure DX.exe");
    let steam_exe = if launcher.is_file() {
        Some(launcher)
    } else if sadx_exe.is_file() {
        Some(sadx_exe)
    } else {
        None
    };

    if let Some(steam_exe) = steam_exe {
        let bak = steam_exe.with_extension("exe.bak");
        if !bak.exists() {
            std::fs::rename(&steam_exe, &bak)
                .context(format!("Failed to backup {}", steam_exe.display()))?;
        }
        std::fs::rename(&dest_exe, &steam_exe).context(format!(
            "Failed to install mod manager as {}",
            steam_exe.display()
        ))?;
    }

    install_mod_loader(game_path, game_kind, None)?;

    tracing::info!(
        "SA Mod Manager and loader installed to {}",
        game_path.display()
    );
    Ok(())
}

/// Download and install the mod loader into the game directory.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_mod_loader(
    game_path: &Path,
    game_kind: GameKind,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let loader_dll = match game_kind {
        GameKind::SADX => "SADXModLoader.dll",
        GameKind::SA2 => "SA2ModLoader.dll",
    };

    if game_path.join("mods/.modloader").join(loader_dll).exists() {
        tracing::info!("Mod loader already present, refreshing DLL replacement");
        install_loader_dll(game_path, game_kind)?;
        return Ok(());
    }

    let url = mod_loader_url(game_kind);

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("ModLoader.7z");

    download::download_file(&url, &archive_path, progress)?;

    // The x64 manager discovers loaders only in this directory.
    let loader_dir = game_path.join("mods").join(".modloader");
    std::fs::create_dir_all(&loader_dir).context("Failed to create mods/.modloader directory")?;
    archive::extract(&archive_path, &loader_dir)?;

    tracing::info!("Mod loader installed to {}", loader_dir.display());

    install_loader_dll(game_path, game_kind)?;

    Ok(())
}

/// Replace the game's data DLL with the mod loader DLL.
///
/// This is how the mod loader hooks into the game: the original DLL that the
/// game executable loads at startup is backed up (with an `_orig` suffix) and
/// the mod loader DLL is copied in its place.
///
/// - SADX: `system/CHRMODELS.dll` → `system/CHRMODELS_orig.dll`
/// - SA2:  `resource/gd_PC/DLL/Win32/Data_DLL.dll` → `…/Data_DLL_orig.dll`
fn install_loader_dll(game_path: &Path, game_kind: GameKind) -> Result<()> {
    let (loader_dll_name, data_dll_path, orig_dll_path) = match game_kind {
        GameKind::SADX => {
            let sys = sadx_data_dir(game_path).unwrap_or_else(|| config::system_dir(game_path));
            // The DLL may have different casing on Linux (e.g. CHRMODELS.DLL vs
            // CHRMODELS.dll) depending on how Steam extracted or hpatchz produced it.
            let chrmodels =
                find_file_icase(&sys, "CHRMODELS.dll").unwrap_or_else(|| sys.join("CHRMODELS.dll"));
            let chrmodels_orig = find_file_icase(&sys, "CHRMODELS_orig.dll")
                .unwrap_or_else(|| sys.join("CHRMODELS_orig.dll"));
            ("SADXModLoader.dll", chrmodels, chrmodels_orig)
        }
        GameKind::SA2 => {
            let dll_dir = game_path.join("resource/gd_PC/DLL/Win32");
            let data_dll = find_file_icase(&dll_dir, "Data_DLL.dll")
                .unwrap_or_else(|| dll_dir.join("Data_DLL.dll"));
            let data_dll_orig = find_file_icase(&dll_dir, "Data_DLL_orig.dll")
                .unwrap_or_else(|| dll_dir.join("Data_DLL_orig.dll"));
            ("SA2ModLoader.dll", data_dll, data_dll_orig)
        }
    };

    let loader_dll = game_path.join("mods/.modloader").join(loader_dll_name);

    if !loader_dll.is_file() {
        anyhow::bail!("Mod loader DLL not found at {}", loader_dll.display());
    }

    if orig_dll_path.is_file() {
        tracing::info!("Original DLL already backed up, refreshing mod loader DLL");
        std::fs::copy(&loader_dll, &data_dll_path).context(format!(
            "Failed to copy mod loader DLL to {}",
            data_dll_path.display()
        ))?;
        return Ok(());
    }

    if !data_dll_path.is_file() {
        tracing::warn!(
            "Game data DLL not found at {}, skipping DLL replacement",
            data_dll_path.display()
        );
        return Ok(());
    }

    std::fs::rename(&data_dll_path, &orig_dll_path).context(format!(
        "Failed to back up {} to {}",
        data_dll_path.display(),
        orig_dll_path.display()
    ))?;

    std::fs::copy(&loader_dll, &data_dll_path).context(format!(
        "Failed to copy mod loader DLL to {}",
        data_dll_path.display()
    ))?;

    tracing::info!(
        "DLL replacement complete: {} → {}",
        loader_dll_name,
        data_dll_path.display()
    );
    Ok(())
}

pub(super) fn sadx_data_dir(game_path: &Path) -> Option<std::path::PathBuf> {
    for dir in [game_path.join("system"), game_path.join("System")] {
        if dir.is_dir()
            && (find_file_icase(&dir, "CHRMODELS.dll").is_some()
                || find_file_icase(&dir, "CHRMODELS_orig.dll").is_some())
        {
            return Some(dir);
        }
    }

    [game_path.join("system"), game_path.join("System")]
        .into_iter()
        .find(|dir| dir.is_dir())
}

/// Download and install a single mod into the game's mods directory.
///
/// When `dir_name` is set on the mod entry, the archive is searched for its
/// `mod.ini` and the containing directory is placed into `mods/<dir_name>/`.
/// This handles archives that are flat, already have a subdirectory, or have
/// extra nesting (e.g. `mods/<name>/`).
///
/// When `dir_name` is `None` (e.g. GameBanana mods), the archive is extracted
/// directly into `mods/` and is expected to contain its own subdirectory.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_mod(
    game_path: &Path,
    mod_entry: &ModEntry,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let progress_opt = progress;
    let mut cb = |downloaded: u64, total_bytes: Option<u64>| {
        if let Some(ref progress) = progress_opt {
            progress(downloaded, total_bytes);
        }
        Ok(())
    };
    install_mod_with_progress(game_path, mod_entry, Some(&mut cb))
}

/// Like `install_mod` but accepts any `FnMut` without `Send` or `'static` bounds.
/// Use from pipeline callbacks that capture non-Send state.
pub fn install_mod_with_progress(
    game_path: &Path,
    mod_entry: &ModEntry,
    progress: Option<&mut dyn FnMut(u64, Option<u64>) -> Result<()>>,
) -> Result<()> {
    let mods_dir = game_path.join("mods");
    std::fs::create_dir_all(&mods_dir)?;

    if let Some(dir_name) = mod_entry.dir_name {
        let dest = mods_dir.join(dir_name);
        if dest.is_dir() {
            if mod_install_is_complete(&dest) {
                normalize_mod_version(&dest)?;
                tracing::info!(
                    "Mod '{}' already installed, skipping download",
                    mod_entry.name
                );
                return Ok(());
            }

            tracing::warn!(
                "Mod '{}' exists but is incomplete, reinstalling",
                mod_entry.name
            );
            std::fs::remove_dir_all(&dest).with_context(|| {
                format!("Failed to remove incomplete mod at {}", dest.display())
            })?;
        }
    }

    let url = resolve_download_url(&mod_entry.source)?;

    let temp_dir = tempfile::tempdir()?;

    // Download: the mmdl endpoint redirects, and the filename comes from
    // the Content-Disposition header. We just save to a generic name.
    let archive_path = temp_dir.path().join("mod_download");
    download::download_file_with(&url, &archive_path, progress)?;

    // Extract to a staging directory first so we can determine the layout.
    let staging_dir = temp_dir.path().join("staging");
    archive::extract(&archive_path, &staging_dir)?;

    if let Some(dir_name) = mod_entry.dir_name {
        // We know the target directory name. Find mod.ini in the staging
        // tree to locate the mod's content root, then move it into place.
        let dest = mods_dir.join(dir_name);
        let content_root = find_mod_root(&staging_dir).unwrap_or(staging_dir.clone());
        move_dir_contents(&content_root, &dest)?;
        normalize_mod_version(&dest)?;
    } else {
        let installed_dir = install_passthrough_mod(&staging_dir, &mods_dir)?;
        normalize_mod_version(&installed_dir)?;
    }

    tracing::info!("Installed mod: {}", mod_entry.name);
    Ok(())
}

/// Find the directory containing `mod.ini` within a staging tree.
///
/// Searches the staging root and up to two levels deep.  Returns `None`
/// if no `mod.ini` is found (the caller should fall back to the root).
fn find_mod_root(staging: &Path) -> Option<std::path::PathBuf> {
    if staging.join("mod.ini").is_file() {
        return Some(staging.to_path_buf());
    }
    if let Ok(entries) = std::fs::read_dir(staging) {
        let mut first_level: Vec<_> = entries.flatten().collect();
        first_level.sort_by_key(|e| e.file_name());
        for entry in first_level {
            let p = entry.path();
            if p.is_dir() {
                if p.join("mod.ini").is_file() {
                    return Some(p);
                }
                if let Ok(inner) = std::fs::read_dir(&p) {
                    let mut second_level: Vec<_> = inner.flatten().collect();
                    second_level.sort_by_key(|e| e.file_name());
                    for inner_entry in second_level {
                        let ip = inner_entry.path();
                        if ip.is_dir() && ip.join("mod.ini").is_file() {
                            return Some(ip);
                        }
                    }
                }
            }
        }
    }
    None
}

fn install_passthrough_mod(staging: &Path, mods_dir: &Path) -> Result<std::path::PathBuf> {
    let mut entries = std::fs::read_dir(staging)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    if entries.len() != 1 || !entries[0].path().is_dir() {
        anyhow::bail!(
            "Expected archive to contain a single top-level mod directory, found {} entries",
            entries.len()
        );
    }

    let extracted_dir = entries.remove(0).path();
    let dest = mods_dir.join(
        extracted_dir
            .file_name()
            .context("Extracted mod directory is missing a file name")?,
    );

    if dest.is_dir() {
        if mod_install_is_complete(&dest) {
            tracing::info!(
                "Mod directory '{}' already exists, skipping install",
                dest.display()
            );
            return Ok(dest);
        }

        tracing::warn!(
            "Mod directory '{}' exists but is incomplete, reinstalling",
            dest.display()
        );
        std::fs::remove_dir_all(&dest)
            .with_context(|| format!("Failed to remove incomplete mod at {}", dest.display()))?;
    }

    move_dir_contents(&extracted_dir, &dest)?;
    Ok(dest)
}

fn mod_install_is_complete(mod_dir: &Path) -> bool {
    find_file_icase(mod_dir, "mod.ini").is_some()
}

fn normalize_mod_version(mod_dir: &Path) -> Result<()> {
    let Some(mod_ini_path) = find_file_icase(mod_dir, "mod.ini") else {
        return Ok(());
    };

    let mod_ini = std::fs::read_to_string(&mod_ini_path)
        .with_context(|| format!("Failed to read {}", mod_ini_path.display()))?;

    if !has_update_metadata(&mod_ini) {
        return Ok(());
    }

    let now = glib::DateTime::now_utc().map_err(|err| anyhow!("{err}"))?;
    let stamp = now.format_iso8601().map_err(|err| anyhow!("{err}"))?;

    std::fs::write(mod_dir.join("mod.version"), format!("{stamp}\n"))?;
    Ok(())
}

fn has_update_metadata(mod_ini: &str) -> bool {
    let mut has_gamebanana_type = false;
    let mut has_gamebanana_id = false;

    for line in mod_ini.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim();

        if key.eq_ignore_ascii_case("GitHubRepo") && !value.is_empty() {
            return true;
        }
        if key.eq_ignore_ascii_case("UpdateUrl") && !value.is_empty() {
            return true;
        }
        if key.eq_ignore_ascii_case("GameBananaItemType") && !value.is_empty() {
            has_gamebanana_type = true;
        }
        if key.eq_ignore_ascii_case("GameBananaItemId") && !value.is_empty() {
            has_gamebanana_id = true;
        }
    }

    has_gamebanana_type && has_gamebanana_id
}

/// Recursively move all entries from `src` into `dest`, creating `dest` if needed.
pub(super) fn move_dir_contents(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let target = dest.join(name);

        if path.is_dir() {
            if target.exists() && !target.is_dir() {
                std::fs::remove_file(&target)?;
            }
            if !target.exists() {
                std::fs::create_dir_all(&target)?;
            }
            move_dir_contents(&path, &target)?;
        } else {
            if target.exists() && target.is_dir() {
                std::fs::remove_dir_all(&target)?;
            }
            std::fs::rename(&path, &target).or_else(|_| {
                // Fallback to copy+remove if rename fails (e.g. across filesystems)
                std::fs::copy(&path, &target)?;
                std::fs::remove_file(&path)?;
                Ok::<(), std::io::Error>(())
            })?;
        }
    }
    Ok(())
}

/// Find a file in a directory by case-insensitive name match.
///
/// Returns `Some(path)` with the actual on-disk casing, or `None` if no match.
pub(super) fn find_file_icase(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    let name_lower = name.to_lowercase();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().to_lowercase() == name_lower {
                return Some(entry.path());
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;

/// Generate the 6 standard data-integrity tests for a `RECOMMENDED_MODS` constant.
///
/// Usage: `crate::recommended_mods_tests!(EXPECTED_COUNT);`
/// The invoking module must have `RECOMMENDED_MODS` and `ModSource` in scope.
#[cfg(test)]
#[macro_export]
macro_rules! recommended_mods_tests {
    ($count:expr) => {
        #[test]
        fn test_recommended_mods_count() {
            assert_eq!(RECOMMENDED_MODS.len(), $count);
        }

        #[test]
        fn test_mod_sources_valid() {
            for m in RECOMMENDED_MODS {
                match &m.source {
                    ModSource::GameBananaItem { item_type, item_id } => {
                        assert!(
                            !item_type.is_empty(),
                            "Mod '{}' has empty item_type",
                            m.name
                        );
                        assert!(*item_id > 0, "Mod '{}' has zero item_id", m.name);
                    }
                    ModSource::DirectUrl { url } => {
                        assert!(
                            url.starts_with("https://"),
                            "Mod '{}' has invalid URL: {}",
                            m.name,
                            url
                        );
                    }
                }
            }
        }

        #[test]
        fn test_mod_sources_unique() {
            use std::collections::HashSet;
            let sources: HashSet<String> = RECOMMENDED_MODS
                .iter()
                .map(|m| match &m.source {
                    ModSource::GameBananaItem { item_type, item_id } => {
                        format!("gamebanana:{item_type}/{item_id}")
                    }
                    ModSource::DirectUrl { url } => url.to_string(),
                })
                .collect();
            assert_eq!(
                sources.len(),
                RECOMMENDED_MODS.len(),
                "Duplicate sources in RECOMMENDED_MODS"
            );
        }

        #[test]
        fn test_mod_names_unique() {
            use std::collections::HashSet;
            let names: HashSet<&str> = RECOMMENDED_MODS.iter().map(|m| m.name).collect();
            assert_eq!(
                names.len(),
                RECOMMENDED_MODS.len(),
                "Duplicate mod names in RECOMMENDED_MODS"
            );
        }

        #[test]
        fn test_mod_entries_have_names_and_descriptions() {
            for m in RECOMMENDED_MODS {
                assert!(!m.slug.is_empty(), "Mod '{}' has empty slug", m.name);
                assert!(!m.name.is_empty(), "Mod has empty name");
                assert!(
                    !m.description.is_empty(),
                    "Mod '{}' has empty description",
                    m.name
                );
            }
        }

        #[test]
        fn test_mod_entries_define_install_directories() {
            for m in RECOMMENDED_MODS {
                assert!(
                    m.dir_name.is_some(),
                    "Mod '{}' is missing dir_name, which makes install targets ambiguous",
                    m.name
                );
            }
        }

        #[test]
        fn test_mod_slugs_unique() {
            use std::collections::HashSet;
            let slugs: HashSet<&str> = RECOMMENDED_MODS.iter().map(|m| m.slug).collect();
            assert_eq!(
                slugs.len(),
                RECOMMENDED_MODS.len(),
                "Duplicate mod slugs in RECOMMENDED_MODS"
            );
        }

        #[test]
        fn test_mod_names_safe_for_filesystem() {
            for m in RECOMMENDED_MODS {
                assert!(!m.name.contains('/'), "Mod name '{}' contains '/'", m.name);
                assert!(
                    !m.name.contains('\\'),
                    "Mod name '{}' contains '\\'",
                    m.name
                );
                assert!(
                    !m.name.contains('\0'),
                    "Mod name '{}' contains null byte",
                    m.name
                );
            }
        }
    };
}
