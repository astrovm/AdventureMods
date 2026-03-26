use crate::steam::game::GameKind;

#[derive(Debug, Clone)]
pub struct SetupStep {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub kind: StepKind,
}

#[derive(Debug, Clone)]
pub enum StepKind {
    /// Automatic check/action, app handles it.
    Auto,
    /// Shows info and waits for user to click Continue.
    Info,
    /// User launches an external tool, then clicks Continue.
    ExternalAction { button_label: &'static str },
    /// Download + install with progress tracking.
    Download,
    /// Mod selection with checkboxes.
    ModSelection,
}

pub fn steps_for_game(kind: GameKind) -> Vec<SetupStep> {
    let mut steps = vec![
        SetupStep {
            id: "steam_config",
            title: "Steam Configuration",
            description: match kind {
                GameKind::SADX => {
                    "Make sure you have run Sonic Adventure DX at least once from Steam so that its Proton prefix is created."
                }
                GameKind::SA2 => {
                    "Make sure you have run Sonic Adventure 2 at least once from Steam so that its Proton prefix is created."
                }
            },
            kind: StepKind::Info,
        },
        SetupStep {
            id: "dotnet",
            title: "Install .NET Runtime",
            description: "Installing .NET Desktop Runtime 8.0. This may take several minutes...",
            kind: StepKind::Auto,
        },
    ];

    // SADX-only: Steam-to-2004 conversion
    if kind == GameKind::SADX {
        steps.push(SetupStep {
            id: "convert_steam",
            title: "Convert Steam to 2004",
            description: "Downloading conversion tools and patching the Steam version to the 2004 version required by the mod loader...",
            kind: StepKind::Download,
        });
    }

    steps.extend([
        SetupStep {
            id: "install_mod_manager",
            title: "Install Mod Manager & Loader",
            description: "Downloading and installing SA Mod Manager and the mod loader...",
            kind: StepKind::Download,
        },
        SetupStep {
            id: "select_mods",
            title: "Select Mods",
            description: match kind {
                GameKind::SADX => "Choose which recommended mods to install for Sonic Adventure DX:",
                GameKind::SA2 => "Choose which recommended mods to install for Sonic Adventure 2:",
            },
            kind: StepKind::ModSelection,
        },
        SetupStep {
            id: "download_mods",
            title: "Download Mods",
            description: "Downloading and installing selected mods...",
            kind: StepKind::Download,
        },
        SetupStep {
            id: "complete",
            title: "Setup Complete",
            description: if kind == GameKind::SADX {
                "Sonic Adventure DX mods are installed! Launch the game through Steam. The mod manager will appear before the game starts, letting you enable or disable mods."
            } else {
                "Sonic Adventure 2 mods are installed! Launch the game through Steam. The mod manager will appear before the game starts, letting you enable or disable mods."
            },
            kind: StepKind::Info,
        },
    ]);

    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_sadx_step_count() {
        assert_eq!(steps_for_game(GameKind::SADX).len(), 7);
    }

    #[test]
    fn test_sa2_step_count() {
        assert_eq!(steps_for_game(GameKind::SA2).len(), 6);
    }

    #[test]
    fn test_sadx_step_ids_unique() {
        let steps = steps_for_game(GameKind::SADX);
        let ids: HashSet<&str> = steps.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), steps.len(), "Duplicate step IDs in SADX");
    }

    #[test]
    fn test_sa2_step_ids_unique() {
        let steps = steps_for_game(GameKind::SA2);
        let ids: HashSet<&str> = steps.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), steps.len(), "Duplicate step IDs in SA2");
    }

    #[test]
    fn test_all_steps_have_nonempty_text() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            for step in steps_for_game(kind) {
                assert!(!step.id.is_empty(), "Step has empty id");
                assert!(!step.title.is_empty(), "Step '{}' has empty title", step.id);
                assert!(
                    !step.description.is_empty(),
                    "Step '{}' has empty description",
                    step.id
                );
            }
        }
    }

    #[test]
    fn test_external_action_steps_have_labels() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            for step in steps_for_game(kind) {
                if let StepKind::ExternalAction { button_label } = step.kind {
                    assert!(
                        !button_label.is_empty(),
                        "Step '{}' has empty button label",
                        step.id
                    );
                }
            }
        }
    }

    #[test]
    fn test_sadx_has_download_step() {
        let steps = steps_for_game(GameKind::SADX);
        assert!(
            steps.iter().any(|s| matches!(s.kind, StepKind::Download)),
            "SADX should have at least one Download step"
        );
    }

    #[test]
    fn test_sa2_has_mod_selection_step() {
        let steps = steps_for_game(GameKind::SA2);
        assert!(
            steps
                .iter()
                .any(|s| matches!(s.kind, StepKind::ModSelection)),
            "SA2 should have a ModSelection step"
        );
    }

    #[test]
    fn test_sa2_mod_selection_before_download_mods() {
        let steps = steps_for_game(GameKind::SA2);
        let select_pos = steps.iter().position(|s| s.id == "select_mods");
        let download_pos = steps.iter().position(|s| s.id == "download_mods");
        assert!(
            select_pos.is_some() && download_pos.is_some(),
            "SA2 must have select_mods and download_mods steps"
        );
        assert!(
            select_pos.unwrap() < download_pos.unwrap(),
            "select_mods must come before download_mods"
        );
    }

    #[test]
    fn test_dotnet_step_exists_for_both_games() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            assert!(
                steps.iter().any(|s| s.id == "dotnet"),
                "{:?} should have a dotnet step",
                kind
            );
        }
    }

    #[test]
    fn test_step_sequences() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            let first = steps.first().unwrap();
            let last = steps.last().unwrap();

            assert_eq!(first.id, "steam_config");
            assert!(matches!(first.kind, StepKind::Info));

            assert_eq!(last.id, "complete");
            assert!(matches!(last.kind, StepKind::Info));
        }
    }

    #[test]
    fn test_sadx_has_convert_steam_step() {
        let steps = steps_for_game(GameKind::SADX);
        assert!(steps.iter().any(|s| s.id == "convert_steam"));
    }

    #[test]
    fn test_sa2_has_no_convert_steam_step() {
        let steps = steps_for_game(GameKind::SA2);
        assert!(!steps.iter().any(|s| s.id == "convert_steam"));
    }

    #[test]
    fn test_descriptions_contain_game_name() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            let steam_step = steps.iter().find(|s| s.id == "steam_config").unwrap();
            assert!(steam_step.description.contains(kind.name()));

            let complete_step = steps.iter().find(|s| s.id == "complete").unwrap();
            assert!(complete_step.description.contains(kind.name()));
        }
    }
}
