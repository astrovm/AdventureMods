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
