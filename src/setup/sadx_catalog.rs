use crate::tr_noop;

use super::common::{ModEntry, ModLink, ModPreset, ModSource};

/// Recommended SADX mods.
pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "Dreamcast Conversion",
        slug: "dreamcast-conversion",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z",
        },
        description: tr_noop!("Brings back the Dreamcast version's look."),
        full_description: Some(tr_noop!(
            "Brings back levels, textures, object models, special effects and branding from the Dreamcast version, so the game looks like the original Sonic Adventure. While paused, X+Y hides the pause menu."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_conversion/dreamcast_conversion_before.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_conversion/dreamcast_conversion_after.jpg",
        ],
        dir_name: Some("DreamcastConversion"),
        links: &[ModLink {
            label: "GitLab",
            url: "https://gitlab.com/PiKeyAr/sadx_dreamcast",
        }],
    },
    ModEntry {
        name: "SADX: Fixed Edition",
        slug: "sadx-fixed-edition",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXFE.7z",
        },
        description: tr_noop!("Fixes many bugs in the PC version."),
        full_description: Some(tr_noop!(
            "Fixes misplaced items and objects, broken level geometry and game logic errors in the PC version."
        )),
        pictures: &[],
        dir_name: Some("SADXFE"),
        links: &[
            ModLink {
                label: "GitHub",
                url: "https://github.com/michael-fadely/sadx-fixed-edition",
            },
            ModLink {
                label: "Sonic Retro",
                url: "https://forums.sonicretro.org/index.php?threads/sonic-adventure-dx-fixed-edition.25301/",
            },
        ],
    },
    ModEntry {
        name: "Lantern Engine",
        slug: "lantern-engine",
        source: ModSource::DirectUrl {
            url: "https://github.com/sonicfreak94/sadx-dc-lighting/releases/latest/download/sadx-dc-lighting.7z",
        },
        description: tr_noop!("Dreamcast-style lighting."),
        full_description: Some(tr_noop!(
            "Brings back the palette-based lighting of the Dreamcast version, so characters and levels are lit the way they were on Dreamcast."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/lantern_engine/lantern_before.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/lantern_engine/lantern_after.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/lantern_engine/sadx-dc-lighting_gh_before_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/lantern_engine/sadx-dc-lighting_gh_after_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/lantern_engine/sadx-dc-lighting_gh_before_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/lantern_engine/sadx-dc-lighting_gh_after_1.jpg",
        ],
        dir_name: Some("sadx-dc-lighting"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-dc-lighting",
        }],
    },
    ModEntry {
        name: "Steam Achievements",
        slug: "steam-achievements",
        source: ModSource::DirectUrl {
            url: "https://mm.reimuhakurei.net/sadxmods/SteamAchievements.7z",
        },
        description: tr_noop!("Unlock Steam achievements while using mods."),
        full_description: Some(tr_noop!(
            "Setup turns the Steam version into the 2004 version, which has no Steam achievements. This mod brings back all 15."
        )),
        pictures: &[],
        dir_name: Some("SteamAchievements"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/X-Hax/SADX2004SteamAchievements",
        }],
    },
    ModEntry {
        name: "Smooth Camera",
        slug: "smooth-camera",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/smooth-cam.7z",
        },
        description: tr_noop!("Smoother first-person view."),
        full_description: Some(tr_noop!(
            "Makes looking around in first-person view with the right stick smooth. It also inverts up and down on the right stick."
        )),
        pictures: &[],
        dir_name: Some("smooth-cam"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-smooth-cam",
        }],
    },
    ModEntry {
        name: "Frame Limit",
        slug: "frame-limit",
        source: ModSource::DirectUrl {
            url: "https://github.com/michael-fadely/sadx-frame-Limit/releases/latest/download/sadx-frame-limit.7z",
        },
        description: tr_noop!("A steadier frame rate."),
        full_description: Some(tr_noop!(
            "A more precise frame limiter. It reduces stutter and fixes the game getting stuck at about 63 FPS, using a bit more CPU."
        )),
        pictures: &[],
        dir_name: Some("sadx-frame-limit"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-frame-limit",
        }],
    },
    ModEntry {
        name: "Sound Overhaul",
        slug: "sound-overhaul",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SoundOverhaul.7z",
        },
        description: tr_noop!("Better sound effects from the Dreamcast version."),
        full_description: Some(tr_noop!(
            "Replaces almost all sound effects with higher-quality versions from the Dreamcast version, and fixes several sound bugs."
        )),
        pictures: &[],
        dir_name: Some("SoundOverhaul"),
        links: &[],
    },
    ModEntry {
        name: "SADX Style Water",
        slug: "sadx-style-water",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-style-water.7z",
        },
        description: tr_noop!("Brings back the ocean waves in Emerald Coast."),
        full_description: Some(tr_noop!(
            "Changes some water textures and brings back the ocean wave effect in Emerald Coast, which the GameCube version and later ports removed."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/style_water/sadx_water_before.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/style_water/sadx_water_after.jpg",
        ],
        dir_name: Some("sadx-style-water"),
        links: &[ModLink {
            label: "GitLab",
            url: "https://gitlab.com/PiKeyAr/sadx-style-water",
        }],
    },
    ModEntry {
        name: "Onion Blur",
        slug: "onion-blur",
        source: ModSource::DirectUrl {
            url: "https://github.com/sonicfreak94/sadx-onion-blur/releases/latest/download/sadx-onion-blur.7z",
        },
        description: tr_noop!("Motion blur trails from the original game."),
        full_description: Some(tr_noop!(
            "Brings back the blur trail behind Sonic and Knuckles when they run, and behind Tails' tails, from the original Sonic Adventure."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/onion_blur/onion_blur_before.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/onion_blur/onion_blur_after.jpg",
        ],
        dir_name: Some("sadx-onion-blur"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-onion-blur",
        }],
    },
    ModEntry {
        name: "Dreamcast Characters Pack",
        slug: "dreamcast-characters-pack",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SA1_Chars.7z",
        },
        description: tr_noop!("The original Dreamcast character models."),
        full_description: Some(tr_noop!(
            "Replaces the DX character models with the original Dreamcast ones for Sonic, Tails, Knuckles, Amy, Big and Super Sonic. Tikal gets her original hands, E-102 his original materials, and Eggman is partly restored."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/dreamcast_characters_before.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/dreamcast_characters_after.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/sa1_chars_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/sa1_chars_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/sa1_chars_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/sa1_chars_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/sa1_chars_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/sa1_chars_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_characters/sa1_chars_6.jpg",
        ],
        dir_name: Some("SA1_Chars"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/248063",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/ItsEasyActually/SA1_Chars",
            },
            ModLink {
                label: "Sonic Retro",
                url: "https://forums.sonicretro.org/threads/sadx-dreamcast-characters-pack.37034/",
            },
        ],
    },
    ModEntry {
        name: "DX Characters Refined",
        slug: "dx-characters-refined",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 290728,
        },
        description: tr_noop!("Improved DX character models."),
        full_description: Some(tr_noop!(
            "Improves the DX character models instead of replacing them, with better shapes, textures and effects. It also fixes over 200 broken animations for Sonic, Tails, Knuckles, Amy and Big."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/dx_characters_refined/dxcharactersrefined_9.jpg",
        ],
        dir_name: Some("DX Characters Refined"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/290728",
        }],
    },
    ModEntry {
        name: "Dreamcast DLC",
        slug: "dreamcast-dlc",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DLCs.7z",
        },
        description: tr_noop!("The Dreamcast version's downloadable events."),
        full_description: Some(tr_noop!(
            "Brings back the holiday events and other downloadable content from the Dreamcast version. It can also play the SEGA and Sonic Team voices on the logo screens."
        )),
        pictures: &[],
        dir_name: Some("DLC"),
        links: &[ModLink {
            label: "GitLab",
            url: "https://gitlab.com/PiKeyAr/sadx-dreamcast-dlc",
        }],
    },
    ModEntry {
        name: "Idle Chatter",
        slug: "idle-chatter",
        source: ModSource::DirectUrl {
            url: "https://github.com/michael-fadely/sadx-idle-chatter/releases/latest/download/idle-chatter.7z",
        },
        description: tr_noop!("Press Z to hear your character talk about the stage."),
        full_description: Some(tr_noop!(
            "Press Z to hear what your character has to say about the stage whenever you want, instead of waiting for it."
        )),
        pictures: &[],
        dir_name: Some("idle-chatter"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-idle-chatter",
        }],
    },
    ModEntry {
        name: "Pause Hide",
        slug: "pause-hide",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/pause-hide.7z",
        },
        description: tr_noop!("Press X+Y to hide the pause menu."),
        full_description: Some(tr_noop!(
            "While paused, press X+Y to hide the pause menu and take clean screenshots, like on Dreamcast. Dreamcast Conversion already does this, so you don't need both."
        )),
        pictures: &[],
        dir_name: Some("pause-hide"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-pause-hide",
        }],
    },
    ModEntry {
        name: "Time of Day",
        slug: "time-of-day",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/TrainDaytime.7z",
        },
        description: tr_noop!("Take the train to change the time of day."),
        full_description: Some(tr_noop!(
            "After you finish the story, taking the train between Station Square and the Mystic Ruins changes the time of day."
        )),
        pictures: &[],
        dir_name: Some("TrainDaytime"),
        links: &[ModLink {
            label: "GitLab",
            url: "https://gitlab.com/PiKeyAr/sadx-timeofday-mod",
        }],
    },
    ModEntry {
        name: "Sonic Adventure Retranslated",
        slug: "sonic-adventure-retranslated",
        source: ModSource::DirectUrl {
            url: "https://github.com/SKingBlue/Sonic-Adventure-Retranslated/releases/latest/download/SAR.zip",
        },
        description: tr_noop!("A translation closer to the Japanese script."),
        full_description: Some(tr_noop!(
            "Replaces the English story text with Windii's translation, which follows the original Japanese script closely."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/retranslated/sonicadventureretranslated_9.jpg",
        ],
        dir_name: Some("Sonic Adventure Retranslated"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/49930",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/SKingBlue/Sonic-Adventure-Retranslated",
            },
        ],
    },
    ModEntry {
        name: "HUD Plus",
        slug: "hud-plus",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-hud-plus/releases/latest/download/sadx-hud-plus.7z",
        },
        description: tr_noop!("A score counter and more on the HUD."),
        full_description: Some(tr_noop!(
            "Adds a score counter, shows the Chao animals you've collected in the pause menu, and lets the ring and life counters show numbers past their old limit."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hud_plus/sadx-hud-plus_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hud_plus/sadx-hud-plus_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hud_plus/sadx-hud-plus_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hud_plus/sadx-hud-plus_3.jpg",
        ],
        dir_name: Some("sadx-hud-plus"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/394948",
        }],
    },
    ModEntry {
        name: "HD GUI 2",
        slug: "hd-gui-2",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/HD_DCStyle.7z",
        },
        description: tr_noop!("Sharper menus and HUD."),
        full_description: Some(tr_noop!(
            "High-resolution textures for most menus and on-screen icons, made for 720p and higher."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hd_gui/hd_gui_before.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hd_gui/hd_gui_after.jpg",
        ],
        dir_name: Some("HD_DCStyle"),
        links: &[],
    },
    ModEntry {
        name: "Active Mouths",
        slug: "active-mouths",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 304634,
        },
        description: tr_noop!("Faces move while characters talk."),
        full_description: Some(tr_noop!(
            "Characters move their mouths and faces when they talk or react during gameplay, not just in cutscenes. This includes being underwater, dying and, for Sonic and Tails, clearing a stage."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/active_mouths/activemouths_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/active_mouths/activemouths_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/active_mouths/activemouths_2.jpg",
        ],
        dir_name: Some("Active Mouths"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/304634",
        }],
    },
    ModEntry {
        name: "New Tricks",
        slug: "sonic-new-tricks",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-new-tricks/releases/latest/download/sadx-new-tricks.7z",
        },
        description: tr_noop!("New moves for Tails, Knuckles and Amy."),
        full_description: Some(tr_noop!(
            "Tails can grab and carry friends while flying. Tails and Knuckles can Spin Dash with B. Knuckles can drill down and dig into walls, and Amy gets a hammer propeller and a double jump. Each move can be turned off."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_7.jpg",
        ],
        dir_name: Some("sadx-new-tricks"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/49973",
        }],
    },
    ModEntry {
        name: "Better Tails AI",
        slug: "better-tails-ai",
        source: ModSource::DirectUrl {
            url: "https://github.com/Sora-yx/SADX-Better-Tails-AI/releases/latest/download/Better.Tails.AI.zip",
        },
        description: tr_noop!("Tails follows you in more places and helps out."),
        full_description: Some(tr_noop!(
            "Tails follows Sonic into the hub worlds, Chao Gardens and most boss fights, rides vehicles with him, and pets Chao with you. He flies faster to keep up, and he can fly you around the hub worlds."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/better_tails_ai/bettertailsai_9.jpg",
        ],
        dir_name: Some("Better Tails AI"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/49943",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/Sora-yx/SADX-Better-Tails-AI",
            },
        ],
    },
    ModEntry {
        name: "Super Sonic",
        slug: "super-sonic",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-super-sonic/releases/latest/download/sadx-super-sonic.7z",
        },
        description: tr_noop!("Play as Super Sonic in action stages."),
        full_description: Some(tr_noop!(
            "After you finish the Super Sonic story, jump and press the action button with 50 rings or more to turn into Super Sonic in any stage. Super Sonic can use all of Sonic's moves."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/super_sonic/sadx-super-sonic_9.jpg",
        ],
        dir_name: Some("sadx-super-sonic"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/49986",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/kellsnc/sadx-super-sonic",
            },
        ],
    },
    ModEntry {
        name: "Chao Gameplay",
        slug: "chao-gameplay",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-chao-gameplay/releases/latest/download/sadx-chao-gameplay.7z",
        },
        description: tr_noop!("Take your Chao into stages."),
        full_description: Some(tr_noop!(
            "Hold a Chao as you leave its garden and it follows you through stages and hub worlds, attacking nearby enemies based on its stats. You can drop it anytime."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/chao_gameplay/sadx-chao-gameplay_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/chao_gameplay/sadx-chao-gameplay_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/chao_gameplay/sadx-chao-gameplay_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/chao_gameplay/sadx-chao-gameplay_3.jpg",
        ],
        dir_name: Some("sadx-chao-gameplay"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/49974",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/kellsnc/sadx-chao-gameplay",
            },
        ],
    },
    ModEntry {
        name: "Fixes, Adds, and Beta Restores",
        slug: "fixes-adds-and-beta-restores",
        source: ModSource::DirectUrl {
            url: "https://github.com/supercoolsonic/Fixes_Adds_BetaRestores/releases/latest/download/FABR.7z",
        },
        description: tr_noop!("Optional extras and beta content."),
        full_description: Some(tr_noop!(
            "A set of optional extras: unused voice lines and sound effects, beta objects and effects, and fixes for some cutscenes and objects. Each one can be turned on or off."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/fixes_adds_beta_restores/fixes_adds_betarestores_9.jpg",
        ],
        dir_name: Some("Fixes_Adds_BetaRestores"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/mods/49976",
        }],
    },
    ModEntry {
        name: "Perfect Chaos Music Swap",
        slug: "perfect-chaos-music-swap",
        source: ModSource::GameBananaItem {
            item_type: "Sound",
            item_id: 40537,
        },
        description: tr_noop!("Swaps the two songs of the final boss fight."),
        full_description: Some(tr_noop!(
            "Swaps the two music tracks of the Perfect Chaos fight, so Open Your Heart plays at the end instead of the start."
        )),
        pictures: &[],
        dir_name: Some("Perfect Chaos Music Swap"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/sounds/40537",
        }],
    },
    ModEntry {
        name: "AI HD FMVs",
        slug: "ai-hd-fmvs",
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/sadx-hd-videos/releases/latest/download/AI_HD_FMVs.7z",
        },
        description: tr_noop!("Upscaled 1080p videos."),
        full_description: Some(tr_noop!(
            "Replaces the intro and cutscene videos with versions upscaled to 1080p."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_fmvs/ai_hd_fmvs_8.jpg",
        ],
        dir_name: Some("AI_HD_FMVs"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/49951",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/kawaiikaorichan/sadx-hd-videos",
            },
        ],
    },
    ModEntry {
        name: "AI HD Textures",
        slug: "ai-hd-textures",
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/AI_textures/releases/latest/download/AI_HD_Textures.7z",
        },
        description: tr_noop!("Sharper, AI-upscaled textures."),
        full_description: Some(tr_noop!(
            "Textures for almost the whole game, upscaled with AI for high-resolution screens. It has a version for Dreamcast Conversion and one for the original game."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_6.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_7.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_8.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/ai_hd_textures/ai_hd_textures_9.jpg",
        ],
        dir_name: Some("AI_HD_Textures"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/49978",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/kawaiikaorichan/AI_textures",
            },
        ],
    },
    ModEntry {
        name: "Multiplayer",
        slug: "multiplayer",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-multiplayer/releases/latest/download/sadx-multiplayer.7z",
        },
        description: tr_noop!("Local split-screen for up to 4 players."),
        full_description: Some(tr_noop!(
            "Adds local split-screen for up to 4 players in co-op and battle modes on trial stages, including fishing, treasure hunting and shooting."
        )),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/multiplayer/sadx-multiplayer_0.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/multiplayer/sadx-multiplayer_1.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/multiplayer/sadx-multiplayer_2.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/multiplayer/sadx-multiplayer_3.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/multiplayer/sadx-multiplayer_4.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/multiplayer/sadx-multiplayer_5.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/multiplayer/sadx-multiplayer_6.jpg",
        ],
        dir_name: Some("sadx-multiplayer"),
        links: &[
            ModLink {
                label: "GameBanana",
                url: "https://gamebanana.com/mods/460975",
            },
            ModLink {
                label: "GitHub",
                url: "https://github.com/kellsnc/sadx-multiplayer",
            },
            ModLink {
                label: "SHC",
                url: "https://shc.zone/entries/contest2023/912",
            },
        ],
    },
];

/// SADX Mod Presets.
pub const PRESETS: &[ModPreset] = &[
    ModPreset {
        name: "DX Enhanced",
        description: tr_noop!(
            "Dreamcast levels and lighting with the improved DX characters and DX-style water, plus HD textures, videos and menus, and extra gameplay mods."
        ),
        mod_names: &[
            "Dreamcast Conversion",
            "SADX: Fixed Edition",
            "Lantern Engine",
            "Steam Achievements",
            "Smooth Camera",
            "Frame Limit",
            "Sound Overhaul",
            "SADX Style Water",
            "Onion Blur",
            "DX Characters Refined",
            "Dreamcast DLC",
            "Idle Chatter",
            "Time of Day",
            "Sonic Adventure Retranslated",
            "HUD Plus",
            "HD GUI 2",
            "Active Mouths",
            "New Tricks",
            "Better Tails AI",
            "Super Sonic",
            "Chao Gameplay",
            "Fixes, Adds, and Beta Restores",
            "Perfect Chaos Music Swap",
            "AI HD FMVs",
            "AI HD Textures",
            "Multiplayer",
        ],
    },
    ModPreset {
        name: "Dreamcast Restoration",
        description: tr_noop!(
            "Dreamcast levels and lighting with the original Dreamcast characters, plus HD textures, videos and menus, and extra gameplay mods."
        ),
        mod_names: &[
            "Dreamcast Conversion",
            "SADX: Fixed Edition",
            "Lantern Engine",
            "Steam Achievements",
            "Smooth Camera",
            "Frame Limit",
            "Sound Overhaul",
            "Onion Blur",
            "Dreamcast Characters Pack",
            "Dreamcast DLC",
            "Idle Chatter",
            "Time of Day",
            "Sonic Adventure Retranslated",
            "HUD Plus",
            "HD GUI 2",
            "Active Mouths",
            "New Tricks",
            "Better Tails AI",
            "Super Sonic",
            "Chao Gameplay",
            "Fixes, Adds, and Beta Restores",
            "Perfect Chaos Music Swap",
            "AI HD FMVs",
            "AI HD Textures",
            "Multiplayer",
        ],
    },
];
