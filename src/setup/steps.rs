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
    /// Automatic check/action — app handles it.
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
    match kind {
        GameKind::SADX => sadx_steps(),
        GameKind::SA2 => sa2_steps(),
    }
}

fn sadx_steps() -> Vec<SetupStep> {
    vec![
        SetupStep {
            id: "check_deps",
            title: "Check Dependencies",
            description: "Checking that protontricks is installed...",
            kind: StepKind::Auto,
        },
        SetupStep {
            id: "steam_config",
            title: "Steam Configuration",
            description: "Make sure you have run Sonic Adventure DX at least once from Steam so that its Proton prefix is created. Also, set the game's compatibility to GE-Proton in Steam > Properties > Compatibility.",
            kind: StepKind::Info,
        },
        SetupStep {
            id: "ge_proton",
            title: "Install GE-Proton",
            description: "Open ProtonUp-Qt to install the latest GE-Proton version if you haven't already.",
            kind: StepKind::ExternalAction {
                button_label: "Launch ProtonUp-Qt",
            },
        },
        SetupStep {
            id: "dotnet",
            title: "Install .NET Runtime",
            description: "Installing .NET Framework 4.8 via protontricks. This may take several minutes...",
            kind: StepKind::Auto,
        },
        SetupStep {
            id: "download_installer",
            title: "Download Mod Installer",
            description: "Downloading the SADX Mod Installer...",
            kind: StepKind::Download,
        },
        SetupStep {
            id: "run_installer",
            title: "Run Mod Installer",
            description: "A Windows installer will open. Follow the prompts to install mods, selecting the Sonic Adventure DX game folder when asked. Click Continue when the installer finishes.",
            kind: StepKind::ExternalAction {
                button_label: "Launch Installer",
            },
        },
        SetupStep {
            id: "complete",
            title: "Setup Complete",
            description: "Sonic Adventure DX mods are installed! Launch the game through Steam — the mod loader should activate automatically.",
            kind: StepKind::Info,
        },
    ]
}

fn sa2_steps() -> Vec<SetupStep> {
    vec![
        SetupStep {
            id: "check_deps",
            title: "Check Dependencies",
            description: "Checking that protontricks is installed...",
            kind: StepKind::Auto,
        },
        SetupStep {
            id: "steam_config",
            title: "Steam Configuration",
            description: "Make sure you have run Sonic Adventure 2 at least once from Steam so that its Proton prefix is created. Also, set the game's compatibility to GE-Proton in Steam > Properties > Compatibility.",
            kind: StepKind::Info,
        },
        SetupStep {
            id: "ge_proton",
            title: "Install GE-Proton",
            description: "Open ProtonUp-Qt to install the latest GE-Proton version if you haven't already.",
            kind: StepKind::ExternalAction {
                button_label: "Launch ProtonUp-Qt",
            },
        },
        SetupStep {
            id: "dotnet",
            title: "Install .NET Runtime",
            description: "Installing .NET Framework 4.8 via protontricks. This may take several minutes...",
            kind: StepKind::Auto,
        },
        SetupStep {
            id: "install_mod_manager",
            title: "Install SA Mod Manager",
            description: "Downloading and installing SA Mod Manager...",
            kind: StepKind::Download,
        },
        SetupStep {
            id: "select_mods",
            title: "Select Mods",
            description: "Choose which recommended mods to install for Sonic Adventure 2:",
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
            description: "Sonic Adventure 2 mods are installed! Launch the game through Steam — the mod manager will appear before the game starts, letting you enable/disable mods.",
            kind: StepKind::Info,
        },
    ]
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
        assert_eq!(steps_for_game(GameKind::SA2).len(), 8);
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
    fn test_step_sequences() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            let steps = steps_for_game(kind);
            let first = steps.first().unwrap();
            let last = steps.last().unwrap();

            assert_eq!(first.id, "check_deps");
            assert!(matches!(first.kind, StepKind::Auto));

            assert_eq!(last.id, "complete");
            assert!(matches!(last.kind, StepKind::Info));
        }
    }
}
