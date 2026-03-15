use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download};

use super::common::{ModEntry, ModSource};

/// Direct URL for the SADX Mod Loader archive.
const SADX_MOD_LOADER_URL: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXModLoader.7z";

/// Base URL for mods hosted on dcmods.unreliable.network.
#[cfg(test)]
const DCMODS_BASE: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/";

/// Recommended SADX mods.
pub const RECOMMENDED_MODS: &[ModEntry] = &[
    // --- dcmods DirectUrl mods ---
    ModEntry {
        name: "SADX: Fixed Edition",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXFE.7z",
        },
        description: "Comprehensive bug fix and restoration mod",
    },
    ModEntry {
        name: "Dreamcast Conversion",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z",
        },
        description: "Restores Dreamcast visuals and features",
    },
    ModEntry {
        name: "Dreamcast Characters Pack",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SA1_Chars.7z",
        },
        description: "Original Dreamcast character models",
    },
    ModEntry {
        name: "Lantern Engine",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-dc-lighting.7z",
        },
        description: "Dreamcast-accurate lighting engine",
    },
    ModEntry {
        name: "Sound Overhaul",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SoundOverhaul.7z",
        },
        description: "Restored and improved sound effects",
    },
    ModEntry {
        name: "Dreamcast DLC",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DLCs.7z",
        },
        description: "Restored Dreamcast downloadable content",
    },
    ModEntry {
        name: "ADX Audio",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/ADXAudio.7z",
        },
        description: "High-quality ADX audio replacement",
    },
    ModEntry {
        name: "Super Sonic",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-super-sonic.7z",
        },
        description: "Playable Super Sonic in action stages",
    },
    ModEntry {
        name: "Frame Limit",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-frame-limit.7z",
        },
        description: "Accurate frame rate limiter",
    },
    ModEntry {
        name: "Idle Chatter",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/idle-chatter.7z",
        },
        description: "Restores character idle voice lines",
    },
    ModEntry {
        name: "Pause Hide",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/pause-hide.7z",
        },
        description: "Hide HUD when pausing for screenshots",
    },
    ModEntry {
        name: "Onion Blur",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-onion-blur.7z",
        },
        description: "Dreamcast-style motion blur effect",
    },
    ModEntry {
        name: "Smooth Camera",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/smooth-cam.7z",
        },
        description: "Smoother camera movement",
    },
    ModEntry {
        name: "SADX Style Water",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-style-water.7z",
        },
        description: "Improved water rendering",
    },
    ModEntry {
        name: "Steam Achievements",
        source: ModSource::DirectUrl {
            url: "https://mm.reimuhakurei.net/sadxmods/SteamAchievements.7z",
        },
        description: "Enables Steam achievements with mods",
    },
    // --- GitHub release mods ---
    ModEntry {
        name: "ESRGAN-AI HD Textures",
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/AI_textures/releases/latest/download/AI_HD_Textures.7z",
        },
        description: "AI-upscaled high-definition textures",
    },
    ModEntry {
        name: "AI HD FMVs",
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/sadx-hd-videos/releases/latest/download/AI_HD_FMVs.7z",
        },
        description: "AI-upscaled high-definition FMV cutscenes",
    },
    // --- GitLab mods ---
    ModEntry {
        name: "Time of Day",
        source: ModSource::DirectUrl {
            url: "https://gitlab.com/PiKeyAr/sadx-timeofday-mod/-/archive/1.21/sadx-timeofday-mod-1.21.zip",
        },
        description: "Dynamic time-of-day lighting changes",
    },
    // --- GameBanana mods ---
    ModEntry {
        name: "Sonic Adventure Retranslated",
        source: ModSource::GameBanana { file_id: 384650 },
        description: "Accurate retranslation of the game text",
    },
    ModEntry {
        name: "HUD Plus",
        source: ModSource::GameBanana { file_id: 1309612 },
        description: "Enhanced heads-up display",
    },
    ModEntry {
        name: "New Tricks",
        source: ModSource::GameBanana { file_id: 1102800 },
        description: "Additional moves and abilities",
    },
    ModEntry {
        name: "SADX No Limit",
        source: ModSource::GameBanana { file_id: 1070925 },
        description: "Removes various game limits",
    },
    ModEntry {
        name: "Better Tails AI",
        source: ModSource::GameBanana { file_id: 1148657 },
        description: "Improved Tails companion AI behavior",
    },
    ModEntry {
        name: "Active Mouths",
        source: ModSource::GameBanana { file_id: 622235 },
        description: "Characters move their mouths when speaking",
    },
    ModEntry {
        name: "Autodemo Windy Valley",
        source: ModSource::GameBanana { file_id: 789172 },
        description: "Restored Windy Valley auto-demo stage",
    },
    ModEntry {
        name: "Autodemo Speed Highway",
        source: ModSource::GameBanana { file_id: 412296 },
        description: "Restored Speed Highway auto-demo stage",
    },
    ModEntry {
        name: "Autodemo Red Mountain",
        source: ModSource::GameBanana { file_id: 429274 },
        description: "Restored Red Mountain auto-demo stage",
    },
    ModEntry {
        name: "Hill Top",
        source: ModSource::GameBanana { file_id: 1032244 },
        description: "Restored Hill Top stage from beta builds",
    },
    ModEntry {
        name: "DX Characters Refined",
        source: ModSource::GameBanana { file_id: 1498662 },
        description: "Improved DX character models",
    },
    ModEntry {
        name: "DC Textures for DX Characters",
        source: ModSource::GameBanana { file_id: 1281669 },
        description: "Dreamcast textures applied to DX models",
    },
    ModEntry {
        name: "DX Consistent Models",
        source: ModSource::GameBanana { file_id: 1278768 },
        description: "Consistent character models across scenes",
    },
    ModEntry {
        name: "Character Select Mod",
        source: ModSource::GameBanana { file_id: 520468 },
        description: "Play as any character in any stage",
    },
    ModEntry {
        name: "Multiplayer",
        source: ModSource::GameBanana { file_id: 1046512 },
        description: "Local multiplayer support",
    },
    ModEntry {
        name: "Fixes, Adds and Beta Restores",
        source: ModSource::GameBanana { file_id: 429267 },
        description: "Miscellaneous fixes and beta content",
    },
    ModEntry {
        name: "Chao Gameplay",
        source: ModSource::GameBanana { file_id: 781777 },
        description: "Enhanced Chao Garden gameplay",
    },
];

/// Download and install the SADX Mod Loader into the game directory.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn install_mod_loader(
    game_path: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("SADXModLoader.7z");

    download::download_file(SADX_MOD_LOADER_URL, &archive_path, progress)?;

    // Extract directly into the game directory (the archive contains
    // the mod loader DLL and supporting files for the game root)
    archive::extract(&archive_path, game_path)?;

    tracing::info!("SADX Mod Loader installed to {}", game_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_recommended_mods_count() {
        assert_eq!(RECOMMENDED_MODS.len(), 35);
    }

    #[test]
    fn test_mod_sources_valid() {
        for m in RECOMMENDED_MODS {
            match &m.source {
                ModSource::GameBanana { file_id } => {
                    assert!(*file_id > 0, "Mod '{}' has zero file_id", m.name);
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
        let sources: HashSet<String> = RECOMMENDED_MODS
            .iter()
            .map(|m| super::super::common::resolve_download_url(&m.source))
            .collect();
        assert_eq!(
            sources.len(),
            RECOMMENDED_MODS.len(),
            "Duplicate sources in RECOMMENDED_MODS"
        );
    }

    #[test]
    fn test_mod_names_unique() {
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
            assert!(!m.name.is_empty(), "Mod has empty name");
            assert!(
                !m.description.is_empty(),
                "Mod '{}' has empty description",
                m.name
            );
        }
    }

    #[test]
    fn test_mod_names_safe_for_filesystem() {
        for m in RECOMMENDED_MODS {
            assert!(
                !m.name.contains('/'),
                "Mod name '{}' contains '/'",
                m.name
            );
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

    #[test]
    fn test_sadx_mod_loader_url_valid() {
        assert!(SADX_MOD_LOADER_URL.starts_with("https://"));
        assert!(SADX_MOD_LOADER_URL.ends_with(".7z"));
    }

    #[test]
    fn test_dcmods_base_url_valid() {
        assert!(DCMODS_BASE.starts_with("https://"));
        assert!(DCMODS_BASE.ends_with('/'));
    }

    #[test]
    fn test_dcmods_urls_use_correct_base() {
        for m in RECOMMENDED_MODS {
            if let ModSource::DirectUrl { url } = &m.source {
                if url.contains("dcmods.unreliable.network") {
                    assert!(
                        url.starts_with(DCMODS_BASE),
                        "Mod '{}' dcmods URL doesn't start with DCMODS_BASE: {}",
                        m.name,
                        url
                    );
                }
            }
        }
    }
}
