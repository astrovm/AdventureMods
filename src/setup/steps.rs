use crate::steam::game::GameKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepId {
    SteamConfig,
    Dotnet,
    ConvertSteam,
    InstallModManager,
    SelectMods,
    LanguageOptions,
    DownloadMods,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupAction {
    InstallDotnet,
    ConvertSteam,
    InstallModManager,
    InstallMods,
}

impl SetupAction {
    pub fn cli_title(self) -> &'static str {
        match self {
            Self::InstallDotnet => "Install .NET Runtime",
            Self::ConvertSteam => "Convert Steam to 2004",
            Self::InstallModManager => "Install Mod Manager & Loader",
            Self::InstallMods => "Install Mods & Generate Config",
        }
    }
}

impl StepId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SteamConfig => "steam_config",
            Self::Dotnet => "dotnet",
            Self::ConvertSteam => "convert_steam",
            Self::InstallModManager => "install_mod_manager",
            Self::SelectMods => "select_mods",
            Self::LanguageOptions => "language_options",
            Self::DownloadMods => "download_mods",
            Self::Complete => "complete",
        }
    }

    pub fn action(self) -> Option<SetupAction> {
        match self {
            Self::Dotnet => Some(SetupAction::InstallDotnet),
            Self::ConvertSteam => Some(SetupAction::ConvertSteam),
            Self::InstallModManager => Some(SetupAction::InstallModManager),
            Self::DownloadMods => Some(SetupAction::InstallMods),
            Self::SteamConfig | Self::SelectMods | Self::LanguageOptions | Self::Complete => None,
        }
    }
}

impl std::fmt::Display for StepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct SetupStep {
    pub id: StepId,
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
    /// Download + install with progress tracking.
    Download,
    /// Mod selection with checkboxes.
    ModSelection,
}

impl StepKind {
    /// Steps that do work rather than ask the user something.
    pub fn is_work(&self) -> bool {
        matches!(self, Self::Auto | Self::Download)
    }
}

/// The setup flow: every choice first, then all the work in one go.
///
/// The GUI shows the consecutive Auto/Download steps as a single install
/// screen, so their titles and descriptions are task names and statuses.
pub fn steps_for_game(kind: GameKind) -> Vec<SetupStep> {
    let mut steps = vec![
        SetupStep {
            id: StepId::SteamConfig,
            title: "Set Up Proton",
            description: match kind {
                GameKind::SADX => {
                    "In Steam, open Sonic Adventure DX's Properties → Compatibility and force Proton 10.0. Launch the game once, close it, then check again."
                }
                GameKind::SA2 => {
                    "In Steam, open Sonic Adventure 2's Properties → Compatibility and force Proton 10.0. Launch the game once, close it, then check again."
                }
            },
            kind: StepKind::Info,
        },
        SetupStep {
            id: StepId::SelectMods,
            title: "Choose Mods",
            description: "Pick a preset or choose mods one by one.",
            kind: StepKind::ModSelection,
        },
        SetupStep {
            id: StepId::LanguageOptions,
            title: "Languages",
            description: "Choose the languages for subtitles and voices.",
            kind: StepKind::Info,
        },
        SetupStep {
            id: StepId::Dotnet,
            title: "Install .NET Runtime",
            description: "Version 8, needed by the mod manager",
            kind: StepKind::Auto,
        },
    ];

    if kind == GameKind::SADX {
        steps.push(SetupStep {
            id: StepId::ConvertSteam,
            title: "Convert to the 2004 Version",
            description: "Needed by the mod loader",
            kind: StepKind::Download,
        });
    }

    steps.extend([
        SetupStep {
            id: StepId::InstallModManager,
            title: "Install Mod Manager",
            description: "SA Mod Manager and the mod loader",
            kind: StepKind::Download,
        },
        SetupStep {
            id: StepId::DownloadMods,
            title: "Install Mods",
            description: "Download the selected mods and set them up",
            kind: StepKind::Download,
        },
        SetupStep {
            id: StepId::Complete,
            title: "Ready to Play",
            description: match kind {
                GameKind::SADX => {
                    "Start Sonic Adventure DX from Steam. The mod manager opens first, so you can turn mods on or off before playing."
                }
                GameKind::SA2 => {
                    "Start Sonic Adventure 2 from Steam. The mod manager opens first, so you can turn mods on or off before playing."
                }
            },
            kind: StepKind::Info,
        },
    ]);

    steps
}

pub fn actions_for_game(kind: GameKind) -> Vec<SetupAction> {
    steps_for_game(kind)
        .into_iter()
        .filter_map(|step| step.id.action())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn sadx_step_count() {
        assert_eq!(steps_for_game(GameKind::SADX).len(), 8);
    }

    #[test]
    fn sa2_step_count() {
        assert_eq!(steps_for_game(GameKind::SA2).len(), 7);
    }

    #[test]
    fn sadx_step_ids_unique() {
        let steps = steps_for_game(GameKind::SADX);
        let ids: HashSet<StepId> = steps.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), steps.len(), "Duplicate step IDs in SADX");
    }

    #[test]
    fn sa2_step_ids_unique() {
        let steps = steps_for_game(GameKind::SA2);
        let ids: HashSet<StepId> = steps.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), steps.len(), "Duplicate step IDs in SA2");
    }

    #[test]
    fn all_steps_have_nonempty_text() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            for step in steps_for_game(kind) {
                assert!(!step.id.as_str().is_empty(), "Step has empty id");
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
    fn sadx_has_download_step() {
        let steps = steps_for_game(GameKind::SADX);
        assert!(
            steps.iter().any(|s| matches!(s.kind, StepKind::Download)),
            "SADX should have at least one Download step"
        );
    }

    #[test]
    fn sa2_has_mod_selection_step() {
        let steps = steps_for_game(GameKind::SA2);
        assert!(
            steps
                .iter()
                .any(|s| matches!(s.kind, StepKind::ModSelection)),
            "SA2 should have a ModSelection step"
        );
    }

    #[test]
    fn sa2_mod_selection_before_download_mods() {
        let steps = steps_for_game(GameKind::SA2);
        let select_pos = steps.iter().position(|s| s.id == StepId::SelectMods);
        let download_pos = steps.iter().position(|s| s.id == StepId::DownloadMods);
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
    fn dotnet_step_exists_for_both_games() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            assert!(
                steps.iter().any(|s| s.id == StepId::Dotnet),
                "{:?} should have a dotnet step",
                kind
            );
        }
    }

    #[test]
    fn step_sequences() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            let first = steps.first().unwrap();
            let last = steps.last().unwrap();

            assert_eq!(first.id, StepId::SteamConfig);
            assert!(matches!(first.kind, StepKind::Info));

            assert_eq!(last.id, StepId::Complete);
            assert!(matches!(last.kind, StepKind::Info));
        }
    }

    #[test]
    fn every_choice_comes_before_the_work() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            let first_work = steps.iter().position(|s| s.kind.is_work()).unwrap();
            let last_work = steps.iter().rposition(|s| s.kind.is_work()).unwrap();

            assert!(
                steps[first_work..=last_work]
                    .iter()
                    .all(|s| s.kind.is_work())
            );
            assert!(
                steps[..first_work]
                    .iter()
                    .any(|s| s.id == StepId::LanguageOptions)
            );
            assert_eq!(steps[last_work + 1].id, StepId::Complete);
        }
    }

    #[test]
    fn sadx_has_convert_steam_step() {
        let steps = steps_for_game(GameKind::SADX);
        assert!(steps.iter().any(|s| s.id == StepId::ConvertSteam));
    }

    #[test]
    fn sa2_has_no_convert_steam_step() {
        let steps = steps_for_game(GameKind::SA2);
        assert!(!steps.iter().any(|s| s.id == StepId::ConvertSteam));
    }

    #[test]
    fn descriptions_contain_game_name() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            let steam_step = steps.iter().find(|s| s.id == StepId::SteamConfig).unwrap();
            assert!(steam_step.description.contains(kind.name()));

            let complete_step = steps.iter().find(|s| s.id == StepId::Complete).unwrap();
            assert!(complete_step.description.contains(kind.name()));
        }
    }

    #[test]
    fn language_options_step_comes_before_download_mods() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            let language_pos = steps.iter().position(|s| s.id == StepId::LanguageOptions);
            let download_pos = steps.iter().position(|s| s.id == StepId::DownloadMods);
            assert!(
                language_pos.is_some(),
                "language_options step missing for {kind:?}"
            );
            assert!(
                download_pos.is_some(),
                "download_mods step missing for {kind:?}"
            );
            assert!(language_pos.unwrap() < download_pos.unwrap());
        }
    }

    #[test]
    fn setup_actions_follow_step_order() {
        assert_eq!(
            actions_for_game(GameKind::SADX),
            vec![
                SetupAction::InstallDotnet,
                SetupAction::ConvertSteam,
                SetupAction::InstallModManager,
                SetupAction::InstallMods,
            ]
        );
        assert_eq!(
            actions_for_game(GameKind::SA2),
            vec![
                SetupAction::InstallDotnet,
                SetupAction::InstallModManager,
                SetupAction::InstallMods,
            ]
        );
    }

    #[test]
    fn step_ids_display_their_stable_identifiers() {
        assert_eq!(StepId::Complete.to_string(), "complete");
    }
}
