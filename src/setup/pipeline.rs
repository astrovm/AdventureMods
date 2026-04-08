use std::path::Path;

use anyhow::Result;

use crate::steam::game::GameKind;

use super::common::{self, ModEntry};
use super::config;

pub fn install_selected_mods_and_generate_config(
    game_path: &Path,
    game_kind: GameKind,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
) -> Result<()> {
    for mod_entry in selected_mods {
        common::install_mod(game_path, mod_entry, None)?;
    }

    config::generate_config(game_path, game_kind, selected_mods, width, height)
}
