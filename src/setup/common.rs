use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download, protontricks};
use crate::steam::game::{Game, GameKind};

use super::{sa2, sadx};

/// GitHub release URL for SA Mod Manager (x64).
const SA_MOD_MANAGER_URL: &str =
    "https://github.com/X-Hax/SA-Mod-Manager/releases/latest/download/release_x64.zip";

/// GitHub release URL for SADX Mod Loader.
const SADX_MOD_LOADER_URL: &str =
    "https://github.com/X-Hax/sadx-mod-loader/releases/latest/download/SADXModLoader.7z";

/// GitHub release URL for SA2 Mod Loader.
const SA2_MOD_LOADER_URL: &str =
    "https://github.com/X-Hax/sa2-mod-loader/releases/latest/download/SA2ModLoader.7z";

/// Source for downloading a mod.
pub enum ModSource {
    GameBanana { file_id: u64 },
    DirectUrl { url: &'static str },
}

/// A recommended mod entry.
pub struct ModEntry {
    pub name: &'static str,
    pub source: ModSource,
    pub description: &'static str,
    pub before_image: Option<&'static str>,
    pub after_image: Option<&'static str>,
    /// Expected directory name inside `mods/`. Used when a flat archive
    /// (no top-level subdirectory) needs to be wrapped in the correct folder.
    pub dir_name: Option<&'static str>,
}

/// Resolve a `ModSource` to a download URL string.
pub fn resolve_download_url(source: &ModSource) -> String {
    match source {
        ModSource::GameBanana { file_id } => format!("https://gamebanana.com/dl/{file_id}"),
        ModSource::DirectUrl { url } => (*url).to_string(),
    }
}

/// Return the recommended mods list for a given game.
pub fn recommended_mods_for_game(kind: GameKind) -> &'static [ModEntry] {
    match kind {
        GameKind::SADX => sadx::RECOMMENDED_MODS,
        GameKind::SA2 => sa2::RECOMMENDED_MODS,
    }
}

/// Check whether a setup step has already been completed for the given game.
///
/// Returns `true` if the step's effects are already present on disk and it
/// can safely be skipped.
pub fn is_step_complete(step_id: &str, game: &Game) -> bool {
    let p = &game.path;
    match step_id {
        // protontricks: checked live in ensure_protontricks, always fast — don't skip
        "check_deps" => false,

        // Info / external-action steps: always show to the user
        "steam_config" => false,

        // Runtimes: check Proton prefix for dotnetdesktop8 marker
        "dotnet" => {
            let prefix = proton_prefix(p, game.kind.app_id());
            prefix
                .join("drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App")
                .is_dir()
        }

        // Steam-to-2004 conversion (SADX only): same markers as convert_steam_to_2004
        "convert_steam" => {
            let system_dir = if p.join("System").is_dir() {
                p.join("System")
            } else {
                p.join("system")
            };
            system_dir.join("CHRMODELS_orig.dll").exists()
                || p.join("SADXModLoader.dll").exists()
                || p.join("mods/.modloader/SADXModLoader.dll").exists()
                || p.join("sonic.exe").exists()
        }

        // SA Mod Manager: installed when the original exe was backed up,
        // the mod loader DLLs are extracted, and the DLL swap has been done.
        "install_mod_manager" => {
            let exe_backed_up = p.join("Launcher.exe.bak").exists()
                || p.join("Sonic Adventure DX.exe.bak").exists();
            let loader_extracted = p.join("mods/.modloader/SADXModLoader.dll").exists()
                || p.join("mods/.modloader/SA2ModLoader.dll").exists();
            let system_dir = if p.join("System").is_dir() {
                p.join("System")
            } else {
                p.join("system")
            };
            let dll_swapped = system_dir.join("CHRMODELS_orig.dll").exists()
                || p.join("resource/gd_PC/DLL/Win32/Data_DLL_orig.dll").exists();
            exe_backed_up && loader_extracted && dll_swapped
        }

        // Mod selection: always show so the user can change their picks
        "select_mods" => false,

        // Mods download: complete when every recommended mod dir exists.
        // We check all mods (not just selected ones) since we don't persist
        // the selection. If any dir is missing the user can re-run.
        "download_mods" => {
            let mods_dir = p.join("mods");
            let mods_list = recommended_mods_for_game(game.kind);
            mods_list.iter().all(|m| {
                let dir = m.dir_name.unwrap_or(m.name);
                mods_dir.join(dir).is_dir()
            })
        }

        // Completion screen: always show
        "complete" => false,

        _ => false,
    }
}

/// Derive the Proton prefix path from a game's install directory and app ID.
///
/// Game path is typically `.../steamapps/common/<game>/`, and the prefix lives
/// at `.../steamapps/compatdata/<appid>/pfx/`.
fn proton_prefix(game_path: &Path, app_id: u32) -> std::path::PathBuf {
    // Go from .../steamapps/common/<game> up to .../steamapps/
    game_path
        .parent() // common/
        .and_then(|p| p.parent()) // steamapps/
        .map(|steamapps| {
            steamapps
                .join("compatdata")
                .join(app_id.to_string())
                .join("pfx")
        })
        .unwrap_or_else(|| game_path.join("pfx"))
}

/// Ensure protontricks is installed, installing it if needed.
pub async fn ensure_protontricks() -> Result<()> {
    if protontricks::is_available().await {
        tracing::info!("protontricks is available");
        return Ok(());
    }

    tracing::info!("Installing protontricks...");
    protontricks::install().await
}

/// Install .NET Desktop Runtime 8.0 and VC++ Redistributable (2015-2022) for the given game's prefix.
pub async fn install_dotnet(app_id: u32) -> Result<()> {
    protontricks::install_dotnet(app_id).await
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
    // Skip if already installed
    let loader_dll = match game_kind {
        GameKind::SADX => "SADXModLoader.dll",
        GameKind::SA2 => "SA2ModLoader.dll",
    };
    if game_path.join("mods/.modloader").join(loader_dll).exists() {
        tracing::info!("SA Mod Manager and loader already present, skipping installation");
        return Ok(());
    }

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("SAModManager.zip");

    download::download_file(SA_MOD_MANAGER_URL, &archive_path, progress)?;

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
        tracing::info!("Mod loader already present, skipping installation");
        return Ok(());
    }

    let url = match game_kind {
        GameKind::SADX => SADX_MOD_LOADER_URL,
        GameKind::SA2 => SA2_MOD_LOADER_URL,
    };

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("ModLoader.7z");

    download::download_file(url, &archive_path, progress)?;

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
/// - SADX: `System/CHRMODELS.dll` → `System/CHRMODELS_orig.dll`
/// - SA2:  `resource/gd_PC/DLL/Win32/Data_DLL.dll` → `…/Data_DLL_orig.dll`
fn install_loader_dll(game_path: &Path, game_kind: GameKind) -> Result<()> {
    let (loader_dll_name, data_dll_path, orig_dll_path) = match game_kind {
        GameKind::SADX => {
            // Handle case-insensitive System directory
            let system_dir = if game_path.join("System").is_dir() {
                game_path.join("System")
            } else {
                game_path.join("system")
            };
            (
                "SADXModLoader.dll",
                system_dir.join("CHRMODELS.dll"),
                system_dir.join("CHRMODELS_orig.dll"),
            )
        }
        GameKind::SA2 => {
            let dll_dir = game_path.join("resource/gd_PC/DLL/Win32");
            (
                "SA2ModLoader.dll",
                dll_dir.join("Data_DLL.dll"),
                dll_dir.join("Data_DLL_orig.dll"),
            )
        }
    };

    let loader_dll = game_path
        .join("mods/.modloader")
        .join(loader_dll_name);

    if !loader_dll.is_file() {
        anyhow::bail!(
            "Mod loader DLL not found at {}",
            loader_dll.display()
        );
    }

    // Already swapped — nothing to do
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
    let url = resolve_download_url(&mod_entry.source);

    let mods_dir = game_path.join("mods");
    std::fs::create_dir_all(&mods_dir)?;

    let temp_dir = tempfile::tempdir()?;

    // Download — the mmdl endpoint redirects, and the filename comes from
    // the Content-Disposition header. We just save to a generic name.
    let archive_path = temp_dir.path().join("mod_download");
    download::download_file(&url, &archive_path, progress)?;

    // Extract to a staging directory first so we can determine the layout.
    let staging_dir = temp_dir.path().join("staging");
    archive::extract(&archive_path, &staging_dir)?;

    if let Some(dir_name) = mod_entry.dir_name {
        // We know the target directory name. Find mod.ini in the staging
        // tree to locate the mod's content root, then move it into place.
        let dest = mods_dir.join(dir_name);
        let content_root = find_mod_root(&staging_dir).unwrap_or(staging_dir.clone());
        move_dir_contents(&content_root, &dest)?;
    } else {
        // No dir_name — extract directly and trust the archive structure.
        move_dir_contents(&staging_dir, &mods_dir)?;
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
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.join("mod.ini").is_file() {
                    return Some(p);
                }
                // Check two levels deep
                if let Ok(inner) = std::fs::read_dir(&p) {
                    for inner_entry in inner.flatten() {
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

/// Recursively move all entries from `src` into `dest`, creating `dest` if needed.
fn move_dir_contents(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if std::fs::rename(entry.path(), &target).is_err() {
            // rename fails across filesystems; fall back to copy + remove
            if entry.path().is_dir() {
                copy_dir_all(&entry.path(), &target)?;
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::copy(&entry.path(), &target)?;
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_all(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(&entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_gamebanana_url() {
        let source = ModSource::GameBanana { file_id: 1388911 };
        assert_eq!(
            resolve_download_url(&source),
            "https://gamebanana.com/dl/1388911"
        );
    }

    #[test]
    fn test_resolve_direct_url() {
        let source = ModSource::DirectUrl {
            url: "https://example.com/mod.7z",
        };
        assert_eq!(resolve_download_url(&source), "https://example.com/mod.7z");
    }

    #[test]
    fn test_sa_mod_manager_url_valid() {
        assert!(SA_MOD_MANAGER_URL.starts_with("https://github.com/"));
        assert!(SA_MOD_MANAGER_URL.contains("/releases/"));
        assert!(SA_MOD_MANAGER_URL.ends_with(".zip"));
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
        // dir_name is None — archive extracts directly into mods/
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
        // No Launcher.exe or Sonic Adventure DX.exe — mod manager stays as-is

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
        // Both exist — Launcher.exe should win (SA2 path)
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
}
