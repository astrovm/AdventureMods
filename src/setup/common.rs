use std::path::Path;

use anyhow::{Context, Result, anyhow};
use gtk::gio;

use crate::blocking;
use crate::external::{archive, download, proton, runtime_installer};
use crate::steam::game::{Game, GameKind};

const GAMEBANANA_API_BASE: &str =
    "https://api.gamebanana.com/Core/Item/Data?fields=Files().aFiles()";

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
pub fn is_step_complete(step_id: &str, game: &Game) -> bool {
    let p = &game.path;
    match step_id {
        // Info / external-action steps: always show to the user
        "steam_config" => proton::prefix_state(p, game.kind.app_id())
            .map(|state| matches!(state, proton::PrefixState::Ready))
            .unwrap_or(false),

        // Runtimes: check Proton prefix for dotnetdesktop8 marker
        "dotnet" => {
            let Ok(prefix) = proton_prefix(p, game.kind.app_id()) else {
                return false;
            };
            matches!(
                proton::prefix_state(p, game.kind.app_id()),
                Ok(proton::PrefixState::Ready)
            ) && prefix
                .join("drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App")
                .is_dir()
        }

        // Steam-to-2004 conversion (SADX only): same markers as convert_steam_to_2004
        "convert_steam" => {
            sadx_data_dir(p)
                .and_then(|dir| find_file_icase(&dir, "CHRMODELS_orig.dll"))
                .is_some()
                || p.join("SADXModLoader.dll").exists()
                || p.join("mods/.modloader/SADXModLoader.dll").exists()
                || p.join("sonic.exe").exists()
        }

        // SA Mod Manager: installed when the original exe was backed up,
        // the mod loader DLLs are extracted, and the DLL swap has been done.
        "install_mod_manager" => is_mod_manager_fully_installed(p, game.kind),

        // Mod selection: always show so the user can change their picks
        "select_mods" => false,

        // Mods download: never skip this step because it also performs mod-specific
        // configuration and generates the SA Mod Manager profile based on the current selection.
        // The `install_mod` function itself handles skipping existing mod directories.
        "download_mods" => false,

        // Completion screen: always show
        "complete" => false,

        _ => false,
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

/// Install .NET Desktop Runtime 8.0 into the game's Proton prefix
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

    // Extract to a temp subdirectory
    let extract_dir = temp_dir.path().join("extracted");
    archive::extract(&archive_path, &extract_dir)?;

    // Copy SAModManager.exe to game directory
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

    // Now install the mod loader itself
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

    // Extract into mods/.modloader (expected by the x64 Mod Manager)
    let loader_dir = game_path.join("mods").join(".modloader");
    std::fs::create_dir_all(&loader_dir).context("Failed to create mods/.modloader directory")?;
    archive::extract(&archive_path, &loader_dir)?;

    tracing::info!("Mod loader installed to {}", loader_dir.display());

    // Perform the DLL replacement so the game loads the mod loader on startup.
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

    // Already swapped, nothing to do
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

    // Back up the original game DLL
    std::fs::rename(&data_dll_path, &orig_dll_path).context(format!(
        "Failed to back up {} to {}",
        data_dll_path.display(),
        orig_dll_path.display()
    ))?;

    // Copy the mod loader DLL in place of the original
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

fn sadx_data_dir(game_path: &Path) -> Option<std::path::PathBuf> {
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

    let dir_name = mod_entry.dir_name.unwrap_or(mod_entry.name);
    if mods_dir.join(dir_name).is_dir() {
        tracing::info!(
            "Mod '{}' already installed, skipping download",
            mod_entry.name
        );
        return Ok(());
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
    // Check root
    if staging.join("mod.ini").is_file() {
        return Some(staging.to_path_buf());
    }
    // Check one level deep
    if let Ok(entries) = std::fs::read_dir(staging) {
        let mut first_level: Vec<_> = entries.flatten().collect();
        first_level.sort_by_key(|e| e.file_name());
        for entry in first_level {
            let p = entry.path();
            if p.is_dir() {
                if p.join("mod.ini").is_file() {
                    return Some(p);
                }
                // Check two levels deep
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
        tracing::info!(
            "Mod directory '{}' already exists, skipping install",
            dest.display()
        );
        return Ok(dest);
    }

    move_dir_contents(&extracted_dir, &dest)?;
    Ok(dest)
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

    let now = glib::DateTime::now_utc().map_err(|err| anyhow!(err.to_string()))?;
    let stamp = now
        .format_iso8601()
        .map_err(|err| anyhow!(err.to_string()))?;

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
            // Overwrite existing files
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
fn find_file_icase(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
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
#[allow(clippy::items_after_test_module)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_gamebanana_item_dl_base_override() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let _ = rustls::crypto::ring::default_provider().install_default();
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());

        // Bind to a random port, then serve one fake GameBanana API response.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let body = r#"[{"999":{"_idRow":999}}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
        });

        let api_base = format!(
            "http://127.0.0.1:{port}/gbapi?fields=Files().aFiles()",
            port = port
        );
        let dl_base = "http://127.0.0.1:9999/custom-dl/";

        unsafe {
            std::env::set_var("ADVENTURE_MODS_GAMEBANANA_API_BASE", &api_base);
            std::env::set_var("ADVENTURE_MODS_GAMEBANANA_DL_BASE", dl_base);
        }

        let source = ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 12345,
        };
        let result = resolve_download_url(&source).unwrap();

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE");
            std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_DL_BASE");
        }

        assert_eq!(result, "http://127.0.0.1:9999/custom-dl/999");
    }

    #[test]
    fn test_resolve_direct_url() {
        let source = ModSource::DirectUrl {
            url: "https://example.com/mod.7z",
        };
        assert_eq!(
            resolve_download_url(&source).unwrap(),
            "https://example.com/mod.7z"
        );
    }

    #[test]
    fn test_resolve_direct_url_rewrites_sadx_base_when_overridden() {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(
                "ADVENTURE_MODS_DCMODS_BASE_URL",
                "http://127.0.0.1:4010/dcmods/",
            );
        }

        let source = ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z",
        };

        assert_eq!(
            resolve_download_url(&source).unwrap(),
            "http://127.0.0.1:4010/dcmods/DreamcastConversion.7z"
        );

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_DCMODS_BASE_URL");
        }
    }

    #[test]
    fn test_sa_mod_manager_url_valid() {
        assert!(SA_MOD_MANAGER_URL.starts_with("https://github.com/"));
        assert!(SA_MOD_MANAGER_URL.contains("/releases/"));
        assert!(SA_MOD_MANAGER_URL.ends_with(".zip"));
    }

    #[test]
    fn test_sa_mod_manager_url_uses_override() {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(
                "ADVENTURE_MODS_URL_SA_MOD_MANAGER",
                "http://127.0.0.1:4010/samodmanager.zip",
            );
        }

        assert_eq!(
            sa_mod_manager_url(),
            "http://127.0.0.1:4010/samodmanager.zip"
        );

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_URL_SA_MOD_MANAGER");
        }
    }

    #[test]
    fn test_mod_loader_url_uses_override() {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var(
                "ADVENTURE_MODS_URL_SA2_MOD_LOADER",
                "http://127.0.0.1:4010/sa2-loader.7z",
            );
        }

        assert_eq!(
            mod_loader_url(GameKind::SA2),
            "http://127.0.0.1:4010/sa2-loader.7z"
        );

        unsafe {
            std::env::remove_var("ADVENTURE_MODS_URL_SA2_MOD_LOADER");
        }
    }

    #[test]
    fn test_install_mod_dir_construction() {
        let game_path = std::path::Path::new("/fake/game/dir");
        let mods_dir = game_path.join("mods");
        assert!(mods_dir.ends_with("mods"));
        assert_eq!(mods_dir, std::path::PathBuf::from("/fake/game/dir/mods"));
    }

    #[test]
    fn test_move_dir_contents_flat_to_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("mod.ini"), b"[mod]").unwrap();
        std::fs::write(src.join("data.bin"), b"data").unwrap();

        move_dir_contents(&src, &dest).unwrap();

        assert!(dest.join("mod.ini").is_file());
        assert!(dest.join("data.bin").is_file());
    }

    #[test]
    fn test_find_mod_root_at_staging_root() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("mod.ini"), b"[mod]").unwrap();

        let root = find_mod_root(&staging).unwrap();
        assert_eq!(root, staging);
    }

    #[test]
    fn test_find_mod_root_one_level_deep() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let sub = staging.join("MyMod");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("mod.ini"), b"[mod]").unwrap();

        let root = find_mod_root(&staging).unwrap();
        assert_eq!(root, sub);
    }

    #[test]
    fn test_find_mod_root_two_levels_deep() {
        // e.g. archive extracts as mods/SteamAchievements/mod.ini
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let nested = staging.join("mods").join("SteamAchievements");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("mod.ini"), b"[mod]").unwrap();

        let root = find_mod_root(&staging).unwrap();
        assert_eq!(root, nested);
    }

    #[test]
    fn test_find_mod_root_none_when_missing() {
        // Archive with no mod.ini at all (e.g. icondata)
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("icon.ico"), b"icon").unwrap();

        assert!(find_mod_root(&staging).is_none());
    }

    #[test]
    fn test_install_mod_flat_archive_with_dir_name() {
        // mod.ini at root, dir_name set → goes to mods/<dir_name>/
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("mod.ini"), b"[mod]").unwrap();
        std::fs::write(staging.join("texture.png"), b"img").unwrap();

        let mods_dir = tmp.path().join("mods");
        std::fs::create_dir_all(&mods_dir).unwrap();

        let dir_name = "TestMod";
        let dest = mods_dir.join(dir_name);
        let content_root = find_mod_root(&staging).unwrap_or(staging.clone());
        move_dir_contents(&content_root, &dest).unwrap();

        assert!(!mods_dir.join("mod.ini").exists());
        assert!(mods_dir.join("TestMod").join("mod.ini").is_file());
        assert!(mods_dir.join("TestMod").join("texture.png").is_file());
    }

    #[test]
    fn test_install_mod_nested_archive_with_dir_name() {
        // Archive has mods/SteamAchievements/mod.ini, dir_name = "SteamAchievements"
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let nested = staging.join("mods").join("SteamAchievements");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("mod.ini"), b"[mod]").unwrap();
        std::fs::write(nested.join("data.dll"), b"dll").unwrap();

        let mods_dir = tmp.path().join("game_mods");
        std::fs::create_dir_all(&mods_dir).unwrap();

        let dir_name = "SteamAchievements";
        let dest = mods_dir.join(dir_name);
        let content_root = find_mod_root(&staging).unwrap_or(staging.clone());
        move_dir_contents(&content_root, &dest).unwrap();

        assert!(mods_dir.join("SteamAchievements").join("mod.ini").is_file());
        assert!(
            mods_dir
                .join("SteamAchievements")
                .join("data.dll")
                .is_file()
        );
        // No stray nested directories
        assert!(!mods_dir.join("mods").exists());
    }

    #[test]
    fn test_install_mod_no_mod_ini_with_dir_name() {
        // Archive has loose files and no mod.ini (e.g. icondata)
        // Falls back to staging root
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("icon.ico"), b"icon").unwrap();
        std::fs::write(staging.join("other.ico"), b"other").unwrap();

        let mods_dir = tmp.path().join("mods");
        std::fs::create_dir_all(&mods_dir).unwrap();

        let dir_name = "icondata";
        let dest = mods_dir.join(dir_name);
        let content_root = find_mod_root(&staging).unwrap_or(staging.clone());
        move_dir_contents(&content_root, &dest).unwrap();

        assert!(mods_dir.join("icondata").join("icon.ico").is_file());
        assert!(mods_dir.join("icondata").join("other.ico").is_file());
    }

    #[test]
    fn test_install_mod_no_dir_name_passthrough() {
        // dir_name is None: archive extracts directly into mods/
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let sub = staging.join("SomeMod");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("mod.ini"), b"[mod]").unwrap();

        let mods_dir = tmp.path().join("mods");
        std::fs::create_dir_all(&mods_dir).unwrap();

        // No dir_name → move directly
        move_dir_contents(&staging, &mods_dir).unwrap();

        assert!(mods_dir.join("SomeMod").join("mod.ini").is_file());
    }

    #[test]
    fn test_install_passthrough_mod_rejects_flat_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let mods_dir = tmp.path().join("mods");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::create_dir_all(&mods_dir).unwrap();
        std::fs::write(staging.join("mod.ini"), b"[mod]").unwrap();

        let err = install_passthrough_mod(&staging, &mods_dir).unwrap_err();
        assert!(err.to_string().contains("single top-level mod directory"));
        assert!(!mods_dir.join("mod.ini").exists());
    }

    #[test]
    fn test_install_passthrough_mod_preserves_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let mods_dir = tmp.path().join("mods");
        let extracted = staging.join("SomeMod");
        let existing = mods_dir.join("SomeMod");
        std::fs::create_dir_all(&extracted).unwrap();
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::write(extracted.join("mod.ini"), b"[new]").unwrap();
        std::fs::write(existing.join("mod.ini"), b"[old]").unwrap();

        install_passthrough_mod(&staging, &mods_dir).unwrap();

        assert_eq!(std::fs::read(existing.join("mod.ini")).unwrap(), b"[old]");
        assert!(extracted.join("mod.ini").is_file());
    }

    #[test]
    fn test_normalize_mod_version_rewrites_stale_packaged_value() {
        let tmp = tempfile::tempdir().unwrap();
        let mod_dir = tmp.path().join("Better Tails AI");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(
            mod_dir.join("mod.ini"),
            b"Name=Better Tails AI\nGitHubRepo=Sora-yx/SADX-Better-Tails-AI\nGitHubAsset=Better.Tails.AI.zip\n",
        )
        .unwrap();
        std::fs::write(mod_dir.join("mod.version"), b"04/26/2021 22:44:24\n").unwrap();

        normalize_mod_version(&mod_dir).unwrap();

        let rewritten = std::fs::read_to_string(mod_dir.join("mod.version")).unwrap();
        assert_ne!(rewritten.trim(), "04/26/2021 22:44:24");
    }

    #[test]
    fn test_normalize_mod_version_creates_file_for_update_tracked_mod() {
        let tmp = tempfile::tempdir().unwrap();
        let mod_dir = tmp.path().join("Fancy Mod");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(
            mod_dir.join("mod.ini"),
            b"Name=Fancy Mod\nGameBananaItemType=Mod\nGameBananaItemId=12345\n",
        )
        .unwrap();

        normalize_mod_version(&mod_dir).unwrap();

        assert!(mod_dir.join("mod.version").is_file());
    }

    #[test]
    fn test_normalize_mod_version_ignores_plain_mods() {
        let tmp = tempfile::tempdir().unwrap();
        let mod_dir = tmp.path().join("Plain Mod");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("mod.ini"), b"Name=Plain Mod\nVersion=1.0\n").unwrap();

        normalize_mod_version(&mod_dir).unwrap();

        assert!(!mod_dir.join("mod.version").exists());
    }

    #[test]
    fn test_find_mod_root_prefers_deterministic_order() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let b_dir = staging.join("b_mod");
        let a_dir = staging.join("a_mod");
        std::fs::create_dir_all(&b_dir).unwrap();
        std::fs::create_dir_all(&a_dir).unwrap();
        std::fs::write(b_dir.join("mod.ini"), b"[mod]").unwrap();
        std::fs::write(a_dir.join("mod.ini"), b"[mod]").unwrap();

        let root = find_mod_root(&staging).unwrap();
        assert_eq!(root, a_dir);
    }

    #[test]
    fn test_move_dir_contents_overwrites_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(src.join("shared.txt"), b"new").unwrap();
        std::fs::write(dest.join("shared.txt"), b"old").unwrap();

        move_dir_contents(&src, &dest).unwrap();
        assert_eq!(std::fs::read(dest.join("shared.txt")).unwrap(), b"new");
        assert!(!src.join("shared.txt").exists());
    }

    /// Helper: simulate the Steam exe replacement logic from `install_mod_manager`.
    /// Creates `SAModManager.exe` in the game dir and runs the replacement logic.
    fn run_exe_replacement(game_path: &std::path::Path) {
        // Create a fake SAModManager.exe (the "dest_exe" that install_mod_manager copies)
        let dest_exe = game_path.join("SAModManager.exe");
        std::fs::write(&dest_exe, b"mod_manager_content").unwrap();

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
                std::fs::rename(&steam_exe, &bak).unwrap();
            }
            std::fs::rename(&dest_exe, &steam_exe).unwrap();
        }
    }

    #[test]
    fn test_exe_replacement_sa2_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        std::fs::write(game_path.join("Launcher.exe"), b"original_launcher").unwrap();

        run_exe_replacement(game_path);

        // Launcher.exe should now contain the mod manager
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe")).unwrap(),
            b"mod_manager_content"
        );
        // Original backed up
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
            b"original_launcher"
        );
        // SAModManager.exe should have been renamed away
        assert!(!game_path.join("SAModManager.exe").exists());
    }

    #[test]
    fn test_exe_replacement_sadx() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        std::fs::write(game_path.join("Sonic Adventure DX.exe"), b"original_sadx").unwrap();

        run_exe_replacement(game_path);

        // "Sonic Adventure DX.exe" should now contain the mod manager
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe")).unwrap(),
            b"mod_manager_content"
        );
        // Original backed up
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe.bak")).unwrap(),
            b"original_sadx"
        );
        assert!(!game_path.join("SAModManager.exe").exists());
    }

    #[test]
    fn test_exe_replacement_sadx_backup_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        // Simulate a prior backup already existing
        std::fs::write(
            game_path.join("Sonic Adventure DX.exe.bak"),
            b"first_backup",
        )
        .unwrap();
        std::fs::write(
            game_path.join("Sonic Adventure DX.exe"),
            b"already_replaced",
        )
        .unwrap();

        run_exe_replacement(game_path);

        // The original backup should be preserved (not overwritten)
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe.bak")).unwrap(),
            b"first_backup"
        );
    }

    #[test]
    fn test_exe_replacement_sa2_backup_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        std::fs::write(game_path.join("Launcher.exe.bak"), b"first_backup").unwrap();
        std::fs::write(game_path.join("Launcher.exe"), b"already_replaced").unwrap();

        run_exe_replacement(game_path);

        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
            b"first_backup"
        );
    }

    #[test]
    fn test_exe_replacement_no_steam_exe() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        // No Launcher.exe or Sonic Adventure DX.exe: mod manager stays as-is

        run_exe_replacement(game_path);

        // SAModManager.exe should remain in place
        assert_eq!(
            std::fs::read(game_path.join("SAModManager.exe")).unwrap(),
            b"mod_manager_content"
        );
        assert!(!game_path.join("Launcher.exe").exists());
        assert!(!game_path.join("Sonic Adventure DX.exe").exists());
    }

    #[test]
    fn test_exe_replacement_launcher_takes_priority_over_sadx() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        // Both exist: Launcher.exe should win (SA2 path)
        std::fs::write(game_path.join("Launcher.exe"), b"launcher").unwrap();
        std::fs::write(game_path.join("Sonic Adventure DX.exe"), b"sadx").unwrap();

        run_exe_replacement(game_path);

        // Launcher.exe replaced
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe")).unwrap(),
            b"mod_manager_content"
        );
        assert_eq!(
            std::fs::read(game_path.join("Launcher.exe.bak")).unwrap(),
            b"launcher"
        );
        // SADX exe untouched
        assert_eq!(
            std::fs::read(game_path.join("Sonic Adventure DX.exe")).unwrap(),
            b"sadx"
        );
    }

    #[test]
    fn test_recommended_mods_for_game_returns_correct_lists() {
        let sadx_mods = recommended_mods_for_game(GameKind::SADX);
        let sa2_mods = recommended_mods_for_game(GameKind::SA2);
        assert!(!sadx_mods.is_empty());
        assert!(!sa2_mods.is_empty());
        assert_ne!(sadx_mods.len(), sa2_mods.len());
    }

    #[test]
    fn test_find_mod_root_three_levels_deep_returns_none() {
        // find_mod_root only searches two levels deep; three levels should return None
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("staging");
        let deep = staging.join("a").join("b").join("DeepMod");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("mod.ini"), b"[mod]").unwrap();

        assert!(find_mod_root(&staging).is_none());
    }

    #[test]
    fn test_move_dir_contents_empty_source() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("empty_src");
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&src).unwrap();

        move_dir_contents(&src, &dest).unwrap();
        assert!(dest.is_dir());
        assert!(std::fs::read_dir(&dest).unwrap().next().is_none());
    }

    #[test]
    fn test_move_dir_contents_nested_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let subdir = src.join("sub");
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(src.join("top.txt"), b"top").unwrap();
        std::fs::write(subdir.join("nested.txt"), b"nested").unwrap();

        move_dir_contents(&src, &dest).unwrap();

        assert!(dest.join("top.txt").is_file());
        assert!(dest.join("sub").join("nested.txt").is_file());
    }

    #[test]
    fn test_proton_prefix_standard_path() {
        let game_path = std::path::Path::new(
            "/home/user/.local/share/Steam/steamapps/common/Sonic Adventure DX",
        );
        let prefix = proton_prefix(game_path, 71250).unwrap();
        assert_eq!(
            prefix,
            std::path::PathBuf::from(
                "/home/user/.local/share/Steam/steamapps/compatdata/71250/pfx"
            )
        );
    }

    #[test]
    fn test_proton_prefix_shallow_path_fails() {
        let game_path = std::path::Path::new("/game");
        assert!(proton_prefix(game_path, 71250).is_err());
    }

    #[test]
    fn test_proton_prefix_sa2_app_id() {
        let game_path = std::path::Path::new("/mnt/steam/steamapps/common/Sonic Adventure 2");
        let prefix = proton_prefix(game_path, 213610).unwrap();
        assert_eq!(
            prefix,
            std::path::PathBuf::from("/mnt/steam/steamapps/compatdata/213610/pfx")
        );
    }

    #[test]
    fn test_find_file_icase_finds_uppercase_variant() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("CHRMODELS.DLL"), b"").unwrap();

        let found = find_file_icase(tmp.path(), "chrmodels.dll");
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("CHRMODELS.DLL"));
    }

    #[test]
    fn test_find_file_icase_missing_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_file_icase(tmp.path(), "nonexistent.dll").is_none());
    }

    #[test]
    fn test_find_file_icase_nonexistent_dir_returns_none() {
        let path = std::path::Path::new("/nonexistent/path/that/does/not/exist");
        assert!(find_file_icase(path, "anything.dll").is_none());
    }

    #[test]
    fn test_install_loader_dll_sadx_uses_lowercase_system_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();
        let uppercase_system = game_path.join("System");
        let lowercase_system = game_path.join("system");
        let modloader_dir = game_path.join("mods/.modloader");

        std::fs::create_dir_all(&uppercase_system).unwrap();
        std::fs::create_dir_all(&lowercase_system).unwrap();
        std::fs::create_dir_all(&modloader_dir).unwrap();
        std::fs::write(uppercase_system.join("sonicDX.ini"), b"ini").unwrap();
        std::fs::write(lowercase_system.join("CHRMODELS.dll"), b"original_dll").unwrap();
        std::fs::write(modloader_dir.join("SADXModLoader.dll"), b"mod_loader").unwrap();

        install_loader_dll(game_path, GameKind::SADX).unwrap();

        assert_eq!(
            std::fs::read(lowercase_system.join("CHRMODELS_orig.dll")).unwrap(),
            b"original_dll"
        );
        assert_eq!(
            std::fs::read(lowercase_system.join("CHRMODELS.dll")).unwrap(),
            b"mod_loader"
        );
    }

    #[test]
    fn test_is_mod_manager_fully_installed_requires_dll_swap() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path();

        std::fs::create_dir_all(game_path.join("mods/.modloader")).unwrap();
        std::fs::write(
            game_path.join("mods/.modloader/SADXModLoader.dll"),
            b"mod_loader",
        )
        .unwrap();
        std::fs::write(game_path.join("Sonic Adventure DX.exe.bak"), b"backup").unwrap();
        std::fs::create_dir_all(game_path.join("system")).unwrap();
        std::fs::write(game_path.join("system/CHRMODELS.dll"), b"original_dll").unwrap();

        assert!(!is_mod_manager_fully_installed(game_path, GameKind::SADX));
    }

    #[test]
    fn test_mod_entry_dir_name_fallback_to_name() {
        // When dir_name is None, the name field is used as the directory name
        let mod_entry = ModEntry {
            name: "MyMod",
            slug: "my-mod",
            source: ModSource::DirectUrl {
                url: "https://example.com/mod.7z",
            },
            description: "A test mod",
            full_description: None,
            pictures: &[],
            dir_name: None,
            links: &[],
        };
        let dir_name = mod_entry.dir_name.unwrap_or(mod_entry.name);
        assert_eq!(dir_name, "MyMod");
    }

    #[test]
    fn test_mod_entry_explicit_dir_name() {
        let mod_entry = ModEntry {
            name: "Display Name",
            slug: "display-name",
            source: ModSource::DirectUrl {
                url: "https://example.com/mod.7z",
            },
            description: "A test mod",
            full_description: None,
            pictures: &[],
            dir_name: Some("FolderName"),
            links: &[],
        };
        let dir_name = mod_entry.dir_name.unwrap_or(mod_entry.name);
        assert_eq!(dir_name, "FolderName");
    }
}

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
