use super::common::{ModEntry, ModLink, ModSource};

pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "SA2 Render Fix",
        slug: "sa2-render-fix",
        source: ModSource::DirectUrl {
            url: "https://github.com/shaddatic/sa2b-render-fix/releases/latest/download/sa2-render-fix.7z",
        },
        description: "Comprehensive graphics restoration and enhancement for SA2 PC.",
        full_description: Some(
            "SA2 Render Fix is an essential mod for Sonic Adventure 2 on PC that repairs numerous graphical bugs and oversights. It fixes transparency sorting issues, back-face culling, and broken material properties while restoring features like Cart billboards and Dreamcast-style specular highlights. It also merges several specialized fixes like the Eggman Lighting Fix to achieve a visual style closer to the original 2001 Dreamcast release.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/render_fix/sa2-render-fix_9.jpg",
        ],
        dir_name: Some("sa2-render-fix"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/452445",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/shaddatic/sa2b-render-fix",
            },
        ],
    },
    ModEntry {
        name: "Retranslated Story -COMPLETE-",
        slug: "retranslated-story-complete",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 437858,
        },
        description: "A more faithful English translation of the Japanese script.",
        full_description: Some(
            "This mod replaces the original English localization with a new script that is more faithful to the original Japanese dialogue. Based on Windii's translations, it corrects creative liberties and errors found in the official localization. It is intended to be played with Japanese voices for the most authentic experience and includes compatibility with various restoration mods.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated/retranslatedstory-complete-_9.jpg",
        ],
        dir_name: Some("Retranslated Story -COMPLETE-"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/437858",
        }],
    },
    ModEntry {
        name: "HD GUI: SA2 Edition",
        slug: "hd-gui-sa2-edition",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 33171,
        },
        description: "High-definition replacements for the game's 2D UI elements.",
        full_description: Some(
            "HD GUI: SA2 Edition replaces the game's low-resolution HUD, menu icons, and item boxes with high-definition versions that match the Dreamcast original's aesthetic. It covers gameplay HUDs, menu screens (Title, Stage Select, etc.), and includes a DLL for automatic configuration based on your active mods, such as NoBattle or Battle DLC.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_9.jpg",
        ],
        dir_name: Some("HD GUI for SA2"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/33171",
        }],
    },
    ModEntry {
        name: "IMPRESSive",
        slug: "impressive",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 469542,
        },
        description: "Restores the original 'Impress' font from the Dreamcast version.",
        full_description: Some(
            "IMPRESSive replaces the Comic Sans-like font in the PC version of SA2 with the 'Impress' font used in the original Japanese Dreamcast release. It includes custom character widths for natural spacing and supports all European languages. It is designed for compatibility with the SA2 Render Fix font API.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/impressive/impressive_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/impressive/impressive_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/impressive/impressive_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/impressive/impressive_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/impressive/impressive_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/impressive/impressive_5.jpg",
        ],
        dir_name: Some("IMPRESSive"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/469542",
        }],
    },
    ModEntry {
        name: "SA2 Volume Controls",
        slug: "sa2-volume-controls",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 381193,
        },
        description: "Adds independent volume sliders for Music, SFX, and Voices.",
        full_description: Some(
            "This mod addresses SA2's notorious sound mixing issues by adding separate volume controls for music, sound effects, and character voices. It also includes 3D audio fixes for positional sound and master volume settings, allowing players to create a balanced mix that prevents music from drowning out dialogue.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/volume_controls/sa2volumecontrols_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/volume_controls/sa2volumecontrols_1.jpg",
        ],
        dir_name: Some("SA2VolumeControls"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/381193",
        }],
    },
    ModEntry {
        name: "Mech Sound Improvement",
        slug: "mech-sound-improvement",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 412706,
        },
        description: "Overhauls mech footstep and targeting sounds for a less grating experience.",
        full_description: Some(
            "Mech Sound Improvement makes the mech-based levels for Tails and Eggman much more pleasant by lowering the volume of intrusive footsteps and replacing the high-pitched targeting whine with a softer sound. It also introduces snappier lock-on sound effects and adjusted pitch for various mech noises.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/mech_sound/mechsoundimprovement_0.jpg",
        ],
        dir_name: Some("Mech Sound Improvement"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/412706",
        }],
    },
    ModEntry {
        name: "SASDL",
        slug: "sasdl",
        source: ModSource::DirectUrl {
            url: "https://github.com/shaddatic/sa2b-sdl-loader/releases/latest/download/sasdl.7z",
        },
        description: "Dependency mod providing a common interface for SDL2 library.",
        full_description: Some(
            "SASDL (Simple Adventure SDL) is a background mod that allows other mods to use the SDL2 library for features like modern controller support without conflicting with each other. It is a mandatory dependency for mods like SA2 Input Controls.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/sasdl/sasdl_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/sasdl/sasdl_1.jpg",
        ],
        dir_name: Some("SASDL"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/615843",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/Shaddatic/sa2b-sdl-loader",
            },
        ],
    },
    ModEntry {
        name: "SA2 Input Controls",
        slug: "sa2-input-controls",
        source: ModSource::DirectUrl {
            url: "https://github.com/shaddatic/sa2b-input-controls/releases/latest/download/sa2-input-controls.7z",
        },
        description: "Complete overhaul of the input system for modern controllers and sensitivity.",
        full_description: Some(
            "SA2 Input Controls fixes long-standing issues with the game's sensitivity, particularly the rail grinding sensitivity. It implements proper circular deadzones, adds native support for PlayStation and Switch controllers via SDL2, and allows for full keyboard remapping. It also restores full 360-degree analog precision that was hindered by the original axial deadzones.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/input_controls/sa2-input-controls_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/input_controls/sa2-input-controls_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/input_controls/sa2-input-controls_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/input_controls/sa2-input-controls_3.jpg",
        ],
        dir_name: Some("sa2-input-controls"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/515637",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/shaddatic/sa2b-input-controls",
            },
        ],
    },
    ModEntry {
        name: "Better Radar",
        slug: "better-radar",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/SA2BetterRadar/releases/latest/download/SA2BetterRadar.7z",
        },
        description: "Restores SA1-style simultaneous tracking for treasure hunting.",
        full_description: Some(
            "Better Radar improves the treasure-hunting mechanics by allowing the radar to track all three items simultaneously, rather than forcing a specific order. It adds new color indicators (blue and pink) for distance tracking and increases the tempo of the radar sound/animation as you get closer to a shard.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/better_radar/sa2betterradar_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/better_radar/sa2betterradar_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/better_radar/sa2betterradar_2.jpg",
        ],
        dir_name: Some("SA2BetterRadar"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/313161",
        }],
    },
    ModEntry {
        name: "HedgePanel - Sonic + Shadow Tweaks",
        slug: "hedgepanel-sonic-shadow-tweaks",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 48950,
        },
        description: "Fluidity and physics refinements for Sonic and Shadow.",
        full_description: Some(
            "HedgePanel refines the speed characters' mechanics by allowing them to maintain momentum during somersaults and adding an upward bounce after Light Attacks to prevent accidental falls. It also fixes the 'low bounce' physics bug and adds automatic prompts for Magic Hands when in range of an enemy.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hedgepanel/hedgepanel_0.jpg",
        ],
        dir_name: Some("HedgePanel"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/48950",
        }],
    },
    ModEntry {
        name: "Sonic: New Tricks",
        slug: "sonic-new-tricks",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 48941,
        },
        description: "Modernizes Sonic and Shadow's movesets with new and restored abilities.",
        full_description: Some(
            "Sonic: New Tricks allows players to remap Sonic's actions across multiple buttons (separating Jump, Bounce, and Light Dash). It restores the powerful SA1 Spin Dash and jump ball form, enhances the Bounce Bracelet, and allows Shadow and Metal Sonic to use abilities they previously lacked, such as the Bounce attack.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/sonicnewtricks_9.jpg",
        ],
        dir_name: Some("Sonic New Tricks"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/48941",
        }],
    },
    ModEntry {
        name: "Retranslated Hints",
        slug: "retranslated-hints",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 350607,
        },
        description: "Corrects localization errors and ambiguities in the hint system.",
        full_description: Some(
            "Retranslated Hints replaces the original English stage clues with accurate translations of the Japanese text. It famously fixes the reversed 'siht ekil' hints in Mad Space and improves Omochao's dialogue across almost every stage, making the treasure-hunting segments significantly less frustrating.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated_hints/retranslated_hints_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated_hints/retranslated_hints_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/retranslated_hints/retranslated_hints_2.jpg",
        ],
        dir_name: Some("Retranslated Hints"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/350607",
        }],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    crate::recommended_mods_tests!(12);

    #[test]
    fn test_sonic_new_tricks_uses_sa2_image_folder() {
        let new_tricks = RECOMMENDED_MODS
            .iter()
            .find(|m| m.name == "Sonic: New Tricks")
            .expect("Sonic: New Tricks entry missing");

        assert!(!new_tricks.pictures.is_empty());
        for picture in new_tricks.pictures {
            assert!(
                picture.starts_with(
                    "/io/github/astrovm/AdventureMods/resources/images/sa2/new_tricks/"
                ),
                "Unexpected SA2 New Tricks picture path: {}",
                picture
            );
        }
    }

    #[test]
    fn test_stage_atmosphere_tweaks_not_in_recommended_mods() {
        assert!(
            RECOMMENDED_MODS
                .iter()
                .all(|m| m.name != "Stage Atmosphere Tweaks")
        );
    }

    #[test]
    fn test_render_fix_uses_github_direct_url() {
        let mod_entry = RECOMMENDED_MODS
            .iter()
            .find(|m| m.name == "SA2 Render Fix")
            .expect("SA2 Render Fix entry missing");

        match mod_entry.source {
            ModSource::DirectUrl { url } => assert!(
                url.contains("github.com/shaddatic/sa2b-render-fix"),
                "SA2 Render Fix should use GitHub releases URL, got: {url}"
            ),
            ModSource::GameBananaItem { .. } => {
                panic!("SA2 Render Fix should use DirectUrl, not GameBananaItem")
            }
        }
    }
}
