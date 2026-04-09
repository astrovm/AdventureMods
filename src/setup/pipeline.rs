use std::path::Path;

use anyhow::{Result, anyhow};

use crate::steam::game::GameKind;

use super::common::{self, ModEntry};
use super::config;

pub enum InstallProgress<'a> {
    InstallingMod {
        index: usize,
        total: usize,
        mod_name: &'a str,
    },
    GeneratingConfig,
}

pub fn install_selected_mods_and_generate_config(
    game_path: &Path,
    game_kind: GameKind,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
) -> Result<()> {
    install_selected_mods_and_generate_config_with_progress(
        game_path,
        game_kind,
        selected_mods,
        width,
        height,
        |_| {},
    )
}

pub fn install_selected_mods_and_generate_config_with_progress(
    game_path: &Path,
    game_kind: GameKind,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
    mut progress: impl FnMut(InstallProgress<'_>),
) -> Result<()> {
    for (index, mod_entry) in selected_mods.iter().enumerate() {
        progress(InstallProgress::InstallingMod {
            index: index + 1,
            total: selected_mods.len(),
            mod_name: mod_entry.name,
        });
        common::install_mod(game_path, mod_entry, None)?;
    }

    progress(InstallProgress::GeneratingConfig);
    config::generate_config(game_path, game_kind, selected_mods, width, height)
}

pub fn resolve_selected_mods(
    game_kind: GameKind,
    preset_name: Option<&str>,
    mod_names: &[&str],
) -> Result<Vec<&'static ModEntry>> {
    let mods = common::recommended_mods_for_game(game_kind);

    if !mod_names.is_empty() {
        return mod_names
            .iter()
            .map(|name| {
                mods.iter()
                    .find(|entry| entry.name == *name)
                    .ok_or_else(|| anyhow!("Unknown mod '{}' for {}", name, game_kind.name()))
            })
            .collect();
    }

    if let Some(preset_name) = preset_name {
        let preset = common::presets_for_game(game_kind)
            .iter()
            .find(|preset| preset.name == preset_name)
            .ok_or_else(|| anyhow!("Unknown preset '{}' for {}", preset_name, game_kind.name()))?;

        return preset
            .mod_names
            .iter()
            .map(|name| {
                mods.iter()
                    .find(|entry| entry.name == *name)
                    .ok_or_else(|| {
                        anyhow!(
                            "Preset '{}' references unknown mod '{}' for {}",
                            preset_name,
                            name,
                            game_kind.name()
                        )
                    })
            })
            .collect();
    }

    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::resolve_selected_mods;
    use crate::steam::game::GameKind;

    #[test]
    fn resolves_named_mods_in_requested_order() {
        let selected = resolve_selected_mods(
            GameKind::SA2,
            None,
            &["HD GUI: SA2 Edition", "SA2 Render Fix"],
        )
        .unwrap();

        let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["HD GUI: SA2 Edition", "SA2 Render Fix"]);
    }

    #[test]
    fn resolves_preset_when_no_explicit_mods_are_given() {
        let selected =
            resolve_selected_mods(GameKind::SADX, Some("Dreamcast Restoration"), &[]).unwrap();

        assert!(
            selected
                .iter()
                .any(|entry| entry.name == "Dreamcast Characters Pack")
        );
        assert!(
            !selected
                .iter()
                .any(|entry| entry.name == "DX Characters Refined")
        );
    }

    #[test]
    fn rejects_unknown_mod_names() {
        let error = match resolve_selected_mods(GameKind::SA2, None, &["Not Real"]) {
            Ok(_) => panic!("expected unknown mod to fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("Unknown mod"));
    }
}
