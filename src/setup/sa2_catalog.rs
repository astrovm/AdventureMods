use crate::tr_noop;

use super::common::{ModEntry, ModLink, ModSource};

pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "Enhanced Chao World",
        slug: "enhanced-chao-world",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 48915,
        },
        description: tr_noop!("A big Chao World overhaul."),
        full_description: Some(tr_noop!(
            "Overhauls Chao World with redesigned gardens, a better camera, a menu to customize your Chao, a market, teleporting between gardens and many bug fixes. Almost every feature can be turned off in the mod's settings."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/enhanced_chao_world/enhanced-chao-world_9.jpg",
        ],
        dir_name: Some("!EnhancedChaoWorld"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/48915",
        }],
    },
    ModEntry {
        name: "SA2 Render Fix",
        slug: "sa2-render-fix",
        source: ModSource::DirectUrl {
            url: "https://github.com/shaddatic/sa2b-render-fix/releases/latest/download/sa2-render-fix.7z",
        },
        description: tr_noop!("Fixes many graphics bugs and brings back Dreamcast visuals."),
        full_description: Some(tr_noop!(
            "Fixes many graphics bugs in the PC version, like see-through objects drawn in the wrong order and missing effects. It also brings back details from the Dreamcast version, such as shine on shiny surfaces and the original textures and lighting."
        )),
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
        name: "Cutscene Revamp Extensions",
        slug: "cutscene-revamp-extensions",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 684473,
        },
        description: tr_noop!("Extra options for Render Fix's cutscenes."),
        full_description: Some(tr_noop!(
            "Adds options to Render Fix's Dreamcast cutscenes: Sonic's shoes from The Trial, Amy's eyeliner, retimed voice lines and The Trial's opening music. They all start off, so turn on the ones you want in the mod's settings."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/cutscene_revamp_extensions/cutscene-revamp-extensions_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/cutscene_revamp_extensions/cutscene-revamp-extensions_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/cutscene_revamp_extensions/cutscene-revamp-extensions_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/cutscene_revamp_extensions/cutscene-revamp-extensions_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/cutscene_revamp_extensions/cutscene-revamp-extensions_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/cutscene_revamp_extensions/cutscene-revamp-extensions_5.jpg",
        ],
        dir_name: Some("Cutscene Revamp Extensions"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/684473",
        }],
    },
    ModEntry {
        name: "HD GUI: SA2 Edition",
        slug: "hd-gui-sa2-edition",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 33171,
        },
        description: tr_noop!("Sharper menus, HUD and icons."),
        full_description: Some(tr_noop!(
            "Replaces blurry HUD, menu, item box and logo graphics with sharper versions that stay close to the originals. It adjusts itself when the Battle DLC is on or off. Chao World, 2-player mode and the credits keep the original graphics."
        )),
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
        description: tr_noop!("Swaps the game's font for the rounder 'Impress' font."),
        full_description: Some(tr_noop!(
            "Replaces the game's Comic Sans-like font with the 'Impress' font from the Japanese version. It works with English, French, German, Spanish and Italian text."
        )),
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
        description: tr_noop!("Separate volume for music, sound effects and voices."),
        full_description: Some(tr_noop!(
            "Lets you set the volume of music, sound effects, voices and cutscenes separately. The default mix matches the Dreamcast version. It also fixes several sound bugs and can play sounds from the side they come from."
        )),
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
        description: tr_noop!("Quieter mech footsteps and a softer targeting sound."),
        full_description: Some(tr_noop!(
            "Makes Tails' and Eggman's mech stages easier on the ears. Footsteps are quieter, the constant targeting whine is nearly gone, and locking on has a crisper sound."
        )),
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
        description: tr_noop!("A helper other mods need. It does nothing on its own."),
        full_description: Some(tr_noop!(
            "Lets other mods use the SDL2 library without clashing with each other. It does nothing by itself, but SA2 Input Controls needs it."
        )),
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
        description: tr_noop!("Smoother analog controls and easier rail grinding."),
        full_description: Some(tr_noop!(
            "Rewrites the game's controls. The analog stick moves smoothly in every direction instead of sticking to 8 directions, and rail grinding is much easier. Most controllers work right away, and every keyboard key can be changed."
        )),
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
        description: tr_noop!("The treasure radar tracks all three pieces at once."),
        full_description: Some(tr_noop!(
            "Upgrades the radar in treasure-hunting stages. It tracks all three pieces at the same time, adds blue and pink indicators and a sound for each piece, and its animation speeds up with the beeps. Each change can be turned off."
        )),
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
        description: tr_noop!("Small tweaks that make Sonic and Shadow feel smoother."),
        full_description: Some(tr_noop!(
            "Sonic keeps his speed in somersaults, you no longer slow down by falling off rails, and Light Attacks bounce you up like Homing Attacks. Homing Attack bounces are never too low, and the Magic Hands prompt shows up by itself. Each tweak can be turned off."
        )),
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
        name: "Higher Draw Distance",
        slug: "higher-draw-distance",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sa2b-limit-break/releases/latest/download/sa2b-limit-break.7z",
        },
        description: tr_noop!("Objects and levels stay visible farther away."),
        full_description: Some(tr_noop!(
            "Draws objects and level parts 5 times farther away than before. If the game slows down or enemies act strangely in the distance, turn on Clip object logic in the mod's settings."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/higher_draw_distance/higher-draw-distance_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/higher_draw_distance/higher-draw-distance_1.jpg",
        ],
        dir_name: Some("sa2b-limit-break"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/48947",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/kellsnc/sa2b-limit-break",
            },
        ],
    },
    ModEntry {
        name: "Level Oddity Patch",
        slug: "level-oddity-patch",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 48894,
        },
        description: tr_noop!("Fixes odd mistakes in the levels."),
        full_description: Some(tr_noop!(
            "Fixes small mistakes in the levels, like missing or broken Gold Beetles and out-of-bounds Rings that make the automatic A-Rank impossible. It also fixes mistakes in the English and Spanish hints of the hunting levels."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sa2/level_oddity_patch/level-oddity-patch_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/level_oddity_patch/level-oddity-patch_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/level_oddity_patch/level-oddity-patch_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/level_oddity_patch/level-oddity-patch_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sa2/level_oddity_patch/level-oddity-patch_4.jpg",
        ],
        dir_name: Some("OddityPatch"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/48894",
        }],
    },
    ModEntry {
        name: "Retranslated Story -COMPLETE-",
        slug: "retranslated-story-complete",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 437858,
        },
        description: tr_noop!("A new English story translation, closer to the Japanese script."),
        full_description: Some(tr_noop!(
            "Replaces the English story text with a translation closer to the original Japanese script, based on Windii's subtitles. It also adds subtitles to videos and voice lines that had none."
        )),
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
        name: "Retranslated Hints",
        slug: "retranslated-hints",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 350607,
        },
        description: tr_noop!("Fixes the hints in treasure-hunting stages."),
        full_description: Some(tr_noop!(
            "Retranslates the treasure-hunting hints from the Japanese text. Wrong areas, missing details and typos are fixed, and the hints use the same area names as Omochao."
        )),
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
    ModEntry {
        name: "Sonic: New Tricks",
        slug: "sonic-new-tricks",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 48941,
        },
        description: tr_noop!("New moves for Sonic and remappable actions."),
        full_description: Some(tr_noop!(
            "Adds the SA1 Spin Dash, a faster and higher Bounce, and more attacks that break boxes. Shadow can use Bounce too. You can put Light Dash, Bounce, Somersault and Spin Dash on B, X or Y, and speed characters can finish most Tails and Eggman stages."
        )),
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
];
