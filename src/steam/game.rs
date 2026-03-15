use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Game {
    pub kind: GameKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameKind {
    SADX,
    SA2,
}

impl GameKind {
    pub fn app_id(self) -> u32 {
        match self {
            GameKind::SADX => 71250,
            GameKind::SA2 => 213610,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            GameKind::SADX => "Sonic Adventure DX",
            GameKind::SA2 => "Sonic Adventure 2",
        }
    }

    pub fn install_dir(self) -> &'static str {
        match self {
            GameKind::SADX => "Sonic Adventure DX",
            GameKind::SA2 => "Sonic Adventure 2",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sadx_app_id() {
        assert_eq!(GameKind::SADX.app_id(), 71250);
    }

    #[test]
    fn test_sa2_app_id() {
        assert_eq!(GameKind::SA2.app_id(), 213610);
    }

    #[test]
    fn test_game_names() {
        assert_eq!(GameKind::SADX.name(), "Sonic Adventure DX");
        assert_eq!(GameKind::SA2.name(), "Sonic Adventure 2");
    }

    #[test]
    fn test_game_install_dirs() {
        assert_eq!(GameKind::SADX.install_dir(), "Sonic Adventure DX");
        assert_eq!(GameKind::SA2.install_dir(), "Sonic Adventure 2");
    }
}
