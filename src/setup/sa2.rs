use super::common::{ModEntry, ModSource};

pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "SA2 Render Fix",
        source: ModSource::GameBanana { file_id: 1626250 },
        description: "Restores Dreamcast-accurate rendering, fixes graphical bugs and adds enhancements.",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "Retranslated Story -COMPLETE-",
        source: ModSource::GameBanana { file_id: 1601215 },
        description: "Accurate retranslation of the story",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "HD GUI: SA2 Edition",
        source: ModSource::GameBanana { file_id: 409120 },
        description: "High-definition GUI textures",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "IMPRESSive",
        source: ModSource::GameBanana { file_id: 1213103 },
        description: "UI display enhancements and formatting improvements",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "Stage Atmosphere Tweaks",
        source: ModSource::GameBanana { file_id: 884395 },
        description: "Improved stage lighting and atmosphere",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "SA2 Volume Controls",
        source: ModSource::GameBanana { file_id: 835829 },
        description: "Adds proper volume control options",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "Mech Sound Improvement",
        source: ModSource::GameBanana { file_id: 893090 },
        description: "Better mech stage sound effects",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "SASDL",
        source: ModSource::GameBanana { file_id: 1503809 },
        description: "SDL2 library interface required by SA2 Input Controls",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "SA2 Input Controls",
        source: ModSource::GameBanana { file_id: 1514050 },
        description: "Adjusts oversensitive controls for better handling",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "Better Radar",
        source: ModSource::GameBanana { file_id: 1580535 },
        description: "Improved treasure hunting radar",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "HedgePanel - Sonic + Shadow Tweaks",
        source: ModSource::GameBanana { file_id: 454296 },
        description: "Gameplay tweaks for Sonic and Shadow",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "Sonic: New Tricks",
        source: ModSource::GameBanana { file_id: 915082 },
        description: "New tricks for speed characters",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
    ModEntry {
        name: "Retranslated Hints",
        source: ModSource::GameBanana { file_id: 1388468 },
        description: "Accurate retranslation of hint messages",
        before_image: None,
        after_image: None,
        dir_name: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_recommended_mods_count() {
        assert_eq!(RECOMMENDED_MODS.len(), 13);
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
                        "Mod '{}' has invalid URL",
                        m.name
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
            assert!(!m.name.contains('/'), "Mod name '{}' contains '/'", m.name);
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
}
