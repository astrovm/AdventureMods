//! Undo what setup changed in a game folder so Steam launches the unmodded game.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::steam::game::GameKind;

use super::common::{find_file_icase, sadx_data_dir};

/// Steam launch executables that setup backs up as `<name>.bak` before putting
/// SA Mod Manager in their place.
const LAUNCH_EXECUTABLES: [&str; 2] = ["Launcher.exe", "Sonic Adventure DX.exe"];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RestoreReport {
    /// Human-readable list of what was put back or removed.
    pub changes: Vec<String>,
    /// SADX was converted to the 2004 layout, which only Steam can undo by
    /// verifying the game files.
    pub needs_steam_verify: bool,
}

/// Whether setup has modified this game folder.
pub fn is_modded(game_path: &Path, game_kind: GameKind) -> bool {
    LAUNCH_EXECUTABLES
        .iter()
        .any(|exe| backup_path(&game_path.join(exe)).exists())
        || data_dll_paths(game_path, game_kind).is_some_and(|(_, orig)| orig.is_file())
        || game_path.join("mods/.modloader").is_dir()
        || is_converted_to_2004(game_path, game_kind)
}

/// Put back the original launcher and data DLL and remove the mod loader.
///
/// Installed mods in `mods/` are kept so a later setup can reuse them; they do
/// nothing without the loader.
pub fn restore_original_game(game_path: &Path, game_kind: GameKind) -> Result<RestoreReport> {
    let mut report = RestoreReport::default();

    for exe in LAUNCH_EXECUTABLES {
        let exe_path = game_path.join(exe);
        let backup = backup_path(&exe_path);
        if backup.is_file() {
            std::fs::rename(&backup, &exe_path)
                .with_context(|| format!("Failed to restore {}", exe_path.display()))?;
            report.changes.push(format!("Restored {exe}"));
        }
    }

    let manager_copy = game_path.join("SAModManager.exe");
    if manager_copy.is_file() {
        std::fs::remove_file(&manager_copy)
            .with_context(|| format!("Failed to remove {}", manager_copy.display()))?;
        report.changes.push("Removed SAModManager.exe".to_owned());
    }

    if let Some((data_dll, orig_dll)) = data_dll_paths(game_path, game_kind)
        && orig_dll.is_file()
    {
        std::fs::rename(&orig_dll, &data_dll)
            .with_context(|| format!("Failed to restore {}", data_dll.display()))?;
        report.changes.push(format!(
            "Restored {}",
            data_dll
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ));
    }

    let loader_dir = game_path.join("mods/.modloader");
    if loader_dir.is_dir() {
        std::fs::remove_dir_all(&loader_dir)
            .with_context(|| format!("Failed to remove {}", loader_dir.display()))?;
        report.changes.push("Removed the mod loader".to_owned());
    }

    report.needs_steam_verify = is_converted_to_2004(game_path, game_kind);
    tracing::info!(
        "Restored {} at {}: {:?}",
        game_kind.name(),
        game_path.display(),
        report
    );
    Ok(report)
}

/// `steam://` link that asks Steam to verify (and repair) the game's files.
pub fn steam_verify_uri(game_kind: GameKind) -> String {
    format!("steam://validate/{}", game_kind.app_id())
}

fn backup_path(exe: &Path) -> PathBuf {
    exe.with_extension("exe.bak")
}

fn is_converted_to_2004(game_path: &Path, game_kind: GameKind) -> bool {
    game_kind == GameKind::SADX && game_path.join("sonic.exe").is_file()
}

/// The data DLL the mod loader replaces, and where its original is kept.
fn data_dll_paths(game_path: &Path, game_kind: GameKind) -> Option<(PathBuf, PathBuf)> {
    let (dir, name, orig_name) = match game_kind {
        GameKind::SADX => (
            sadx_data_dir(game_path)?,
            "CHRMODELS.dll",
            "CHRMODELS_orig.dll",
        ),
        GameKind::SA2 => (
            game_path.join("resource/gd_PC/DLL/Win32"),
            "Data_DLL.dll",
            "Data_DLL_orig.dll",
        ),
    };
    let orig = find_file_icase(&dir, orig_name)?;
    let dll = find_file_icase(&dir, name).unwrap_or_else(|| dir.join(name));
    Some((dll, orig))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn restores_sa2_launcher_and_data_dll() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        let dll_dir = game.join("resource/gd_PC/DLL/Win32");
        write(&game.join("Launcher.exe"), "manager");
        write(&game.join("Launcher.exe.bak"), "launcher");
        write(&dll_dir.join("Data_DLL.dll"), "loader");
        write(&dll_dir.join("Data_DLL_orig.dll"), "data");
        write(&game.join("mods/.modloader/SA2ModLoader.dll"), "loader");
        write(&game.join("mods/SomeMod/mod.ini"), "[mod]");

        assert!(is_modded(game, GameKind::SA2));
        let report = restore_original_game(game, GameKind::SA2).unwrap();

        assert_eq!(
            std::fs::read_to_string(game.join("Launcher.exe")).unwrap(),
            "launcher"
        );
        assert_eq!(
            std::fs::read_to_string(dll_dir.join("Data_DLL.dll")).unwrap(),
            "data"
        );
        assert!(!dll_dir.join("Data_DLL_orig.dll").exists());
        assert!(!game.join("mods/.modloader").exists());
        assert!(game.join("mods/SomeMod/mod.ini").exists());
        assert!(!report.needs_steam_verify);
        assert_eq!(report.changes.len(), 3);
        assert!(!is_modded(game, GameKind::SA2));
    }

    #[test]
    fn restores_sadx_and_asks_for_steam_verify_after_conversion() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        write(&game.join("Sonic Adventure DX.exe"), "manager");
        write(&game.join("Sonic Adventure DX.exe.bak"), "launcher");
        write(&game.join("SAModManager.exe"), "manager");
        write(&game.join("system/CHRMODELS.dll"), "loader");
        write(&game.join("system/CHRMODELS_orig.dll"), "chr");
        write(&game.join("sonic.exe"), "2004");

        assert!(is_modded(game, GameKind::SADX));
        let report = restore_original_game(game, GameKind::SADX).unwrap();

        assert_eq!(
            std::fs::read_to_string(game.join("Sonic Adventure DX.exe")).unwrap(),
            "launcher"
        );
        assert!(!game.join("SAModManager.exe").exists());
        assert_eq!(
            std::fs::read_to_string(game.join("system/CHRMODELS.dll")).unwrap(),
            "chr"
        );
        assert!(report.needs_steam_verify);
        assert_eq!(steam_verify_uri(GameKind::SADX), "steam://validate/71250");
    }

    #[test]
    fn unmodded_game_restores_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("Launcher.exe"), "launcher");

        assert!(!is_modded(tmp.path(), GameKind::SA2));
        let report = restore_original_game(tmp.path(), GameKind::SA2).unwrap();
        assert_eq!(report, RestoreReport::default());
    }

    #[test]
    fn is_modded_detects_each_setup_marker_on_its_own() {
        let loader = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(loader.path().join("mods/.modloader")).unwrap();
        assert!(is_modded(loader.path(), GameKind::SA2));

        let data_dll = tempfile::tempdir().unwrap();
        write(
            &data_dll
                .path()
                .join("resource/gd_PC/DLL/Win32/Data_DLL_orig.dll"),
            "data",
        );
        assert!(is_modded(data_dll.path(), GameKind::SA2));

        let converted = tempfile::tempdir().unwrap();
        write(&converted.path().join("sonic.exe"), "2004");
        assert!(is_modded(converted.path(), GameKind::SADX));
        assert!(!is_modded(converted.path(), GameKind::SA2));
    }

    #[test]
    fn restores_data_dll_when_the_loader_dll_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let dll_dir = tmp.path().join("resource/gd_PC/DLL/Win32");
        write(&dll_dir.join("data_dll_orig.DLL"), "data");

        let report = restore_original_game(tmp.path(), GameKind::SA2).unwrap();

        assert_eq!(
            std::fs::read_to_string(dll_dir.join("Data_DLL.dll")).unwrap(),
            "data"
        );
        assert_eq!(report.changes, ["Restored Data_DLL.dll"]);
    }

    #[test]
    fn restore_fails_when_the_launcher_cannot_be_put_back() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("Launcher.exe.bak"), "launcher");
        write(&tmp.path().join("Launcher.exe/blocker"), "not a file");

        let err = restore_original_game(tmp.path(), GameKind::SA2).unwrap_err();

        assert!(err.to_string().starts_with("Failed to restore"), "{err}");
    }

    #[test]
    fn restore_fails_when_the_data_dll_cannot_be_put_back() {
        let tmp = tempfile::tempdir().unwrap();
        let dll_dir = tmp.path().join("resource/gd_PC/DLL/Win32");
        write(&dll_dir.join("Data_DLL_orig.dll"), "data");
        write(&dll_dir.join("Data_DLL.dll/blocker"), "not a file");

        let err = restore_original_game(tmp.path(), GameKind::SA2).unwrap_err();

        assert!(err.to_string().contains("Data_DLL.dll"), "{err}");
    }
}
