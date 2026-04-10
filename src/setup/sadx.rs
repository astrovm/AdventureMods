use std::path::Path;

use anyhow::{Context, Result};

use crate::external::{archive, download};

use super::common::{ModEntry, ModLink, ModSource};
use super::config;

/// Direct URL for the Steam-to-2004 conversion tools archive.
const STEAM_TOOLS_URL: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/steam_tools.7z";

fn steam_tools_url() -> String {
    super::common::env_or_default("ADVENTURE_MODS_URL_SADX_STEAM_TOOLS", STEAM_TOOLS_URL)
}

fn hpatchz_program() -> std::path::PathBuf {
    std::env::var_os("ADVENTURE_MODS_HPATCHZ")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("hpatchz"))
}

/// Base URL for mods hosted on dcmods.unreliable.network.
#[cfg(test)]
const DCMODS_BASE: &str =
    "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/";

/// Recommended SADX mods.
pub const RECOMMENDED_MODS: &[ModEntry] = &[
    ModEntry {
        name: "Dreamcast Conversion",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z",
        },
        description: "Restores Dreamcast visuals and features",
        full_description: Some(
            "A massive restoration project that reverts the graphical and environmental changes made in the DX port. It replaces SADX level models and textures with original Dreamcast assets, restores vertex colors, and brings back the original title screens and UI elements for the definitive 1998 experience.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXFE.7z",
        },
        description: "Comprehensive bug fix mod for the PC port",
        full_description: Some(
            "The foundational bug-fix mod for the PC version of SADX. It addresses hundreds of technical oversights, including broken collision, incorrect object placement, scripting errors, and transparency issues that were not present in the original Dreamcast version.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://github.com/sonicfreak94/sadx-dc-lighting/releases/latest/download/sadx-dc-lighting.7z",
        },
        description: "Dreamcast-accurate lighting engine",
        full_description: Some(
            "Restores the original palette-based lighting system from the Dreamcast version of Sonic Adventure. It replaces the flat lighting of the DX ports with dynamic lighting that makes characters and environments react to light sources with vibrant color depth.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://mm.reimuhakurei.net/sadxmods/SteamAchievements.7z",
        },
        description: "Enables Steam achievements with mods",
        full_description: Some(
            "A specialized mod that bridges the gap between modded/downgraded versions of the game and the Steam API. It allows players to earn all 15 official Steam achievements while playing with the SADX Mod Loader and other enhancements.",
        ),
        pictures: &[],
        dir_name: Some("SteamAchievements"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/X-Hax/SADX2004SteamAchievements",
        }],
    },
    ModEntry {
        name: "Smooth Camera",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/smooth-cam.7z",
        },
        description: "Smooth first-person camera (analog instead of 8-directional)",
        full_description: Some(
            "Improves the game's camera behavior by smoothing out transitions and movements. It reduces the jittery or 'snapping' feel of the original SADX camera, making the gameplay experience feel more modern and less nauseating.",
        ),
        pictures: &[],
        dir_name: Some("smooth-cam"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-smooth-cam",
        }],
    },
    ModEntry {
        name: "Frame Limit",
        source: ModSource::DirectUrl {
            url: "https://github.com/michael-fadely/sadx-frame-Limit/releases/latest/download/sadx-frame-limit.7z",
        },
        description: "Accurate frame rate limiter",
        full_description: Some(
            "A critical utility that locks the game's framerate to 60 FPS. Since SADX physics are tied to the framerate, this mod prevents the game from running at double speed on high-refresh-rate monitors, ensuring stable and intended gameplay speed.",
        ),
        pictures: &[],
        dir_name: Some("sadx-frame-limit"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-frame-limit",
        }],
    },
    ModEntry {
        name: "Sound Overhaul",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SoundOverhaul.7z",
        },
        description: "Restored and improved sound effects",
        full_description: Some(
            "Restores the high-quality soundscape of the original 1998 release. It replaces compressed PC audio with high-fidelity samples from the Dreamcast version, fixes sound trigger bugs, and improves 3D positional audio handling.",
        ),
        pictures: &[],
        dir_name: Some("SoundOverhaul"),
        links: &[],
    },
    ModEntry {
        name: "ADX Audio",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/ADXAudio.7z",
        },
        description: "High-quality ADX audio replacement",
        full_description: Some(
            "Restores the use of high-quality .adx audio files for music and voices. Unlike the standard .wma files used in the PC port, .adx files allow for perfect, seamless loops and higher overall audio fidelity without gaps in the soundtrack.",
        ),
        pictures: &[],
        dir_name: Some("ADXAudio"),
        links: &[],
    },
    ModEntry {
        name: "SADX Style Water",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-style-water.7z",
        },
        description: "Restores the ocean wave effect in Emerald Coast",
        full_description: Some(
            "Restores the specific 'shiny' and opaque water textures used in the original 2003 GameCube/PC DX release. This mod is for players who prefer the DX-era water aesthetics over the transparent Dreamcast-style water.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://github.com/sonicfreak94/sadx-onion-blur/releases/latest/download/sadx-onion-blur.7z",
        },
        description: "Dreamcast-style motion blur effect",
        full_description: Some(
            "Restores the iconic 'onion skinning' motion blur effect seen when characters move at high speeds in the original Dreamcast version. This visual trail was removed in the DX ports and is a staple of the classic Sonic Adventure look.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SA1_Chars.7z",
        },
        description: "Original Dreamcast character models",
        full_description: Some(
            "Replaces the high-poly, glossy character models of the DX version with their original lower-poly designs from the 1998 Dreamcast version. It includes original models for the entire main cast, Eggman, Tikal, and Metal Sonic.",
        ),
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
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 290728,
        },
        description: "High-fidelity refinements for the default DX character models.",
        full_description: Some(
            "DX Characters Refined improves the default character models introduced in the DX port rather than replacing them. It features updated topology, UV mapping, and textures for the main cast, along with a massive animation update that fixes over 200 bugged animations inherited from the Dreamcast models.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DLCs.7z",
        },
        description: "Restored Dreamcast downloadable content",
        full_description: Some(
            "Restores the original Dreamcast-exclusive seasonal and promotional online events. This includes holiday events like Christmas and Halloween, as well as scavenger hunts and challenges that were originally only available via the Dreamcast's online features.",
        ),
        pictures: &[],
        dir_name: Some("DLC"),
        links: &[ModLink {
            label: "GitLab",
            url: "https://gitlab.com/PiKeyAr/sadx-dreamcast-dlc",
        }],
    },
    ModEntry {
        name: "Idle Chatter",
        source: ModSource::DirectUrl {
            url: "https://github.com/michael-fadely/sadx-idle-chatter/releases/latest/download/idle-chatter.7z",
        },
        description: "Press a button to hear character commentary about the current stage",
        full_description: Some(
            "Allows players to manually trigger character idle dialogue by pressing a dedicated button. This allows players to hear the characters' thoughts on the story, their location, or current mission without waiting for the idle timer to run out.",
        ),
        pictures: &[],
        dir_name: Some("idle-chatter"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-idle-chatter",
        }],
    },
    ModEntry {
        name: "Pause Hide",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/pause-hide.7z",
        },
        description: "Press X+Y to hide the pause menu for screenshots",
        full_description: Some(
            "A simple but effective tool for virtual photographers. By pressing a specific button combination while the game is paused, players can hide the entire pause menu and HUD to capture clean, unobstructed screenshots of the game world.",
        ),
        pictures: &[],
        dir_name: Some("pause-hide"),
        links: &[ModLink {
            label: "GitHub",
            url: "https://github.com/michael-fadely/sadx-pause-hide",
        }],
    },
    ModEntry {
        name: "Time of Day",
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/TrainDaytime.7z",
        },
        description: "Change the time of day by taking the train after beating the story",
        full_description: Some(
            "Restores the dynamic time-of-day system for Hub Worlds. As the player travels between Station Square and the Mystic Ruins, the clock advances, changing the lighting and atmosphere between Day, Evening, and Night to reflect the passage of time.",
        ),
        pictures: &[],
        dir_name: Some("TrainDaytime"),
        links: &[ModLink {
            label: "GitLab",
            url: "https://gitlab.com/PiKeyAr/sadx-timeofday-mod",
        }],
    },
    ModEntry {
        name: "Sonic Adventure Retranslated",
        source: ModSource::DirectUrl {
            url: "https://github.com/SKingBlue/Sonic-Adventure-Retranslated/releases/latest/download/SAR.zip",
        },
        description: "Faithful translation of the Japanese script for SADX.",
        full_description: Some(
            "This mod replaces the English script with Windii's faithful translation of the original Japanese dialogue. It corrects numerous localization errors and 'Americanizations,' providing a more accurate narrative experience. It is recommended for use with Japanese voices.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-hud-plus/releases/latest/download/sadx-hud-plus.7z",
        },
        description: "Expanded limits and contextual improvements for the gameplay HUD.",
        full_description: Some(
            "HUD Plus enhances the UI by increasing the ring and life counter limits and displaying collected Chao animals in the pause menu. It also includes contextual changes like hiding the ring HUD in specific stages where it's not needed and adding a score counter to the main screen.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/HD_DCStyle.7z",
        },
        description: "High resolution GUI textures for menus, HUD and icons",
        full_description: Some(
            "A complete high-resolution texture overhaul for the game's user interface. It replaces every menu texture, HUD element, and font with crisp, HD versions that are faithful to the original UI designs while looking great on modern displays.",
        ),
        pictures: &[
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hd_gui/hd_gui_before.jpg",
            "/io/github/astrovm/AdventureMods/resources/images/sadx/hd_gui/hd_gui_after.jpg",
        ],
        dir_name: Some("HD_DCStyle"),
        links: &[],
    },
    ModEntry {
        name: "Active Mouths",
        source: ModSource::GameBananaItem {
            item_type: "Mod",
            item_id: 304634,
        },
        description: "Enables character face and mouth animations during gameplay.",
        full_description: Some(
            "Active Mouths enables mouth and facial animations for characters when they speak idle lines or react to the environment in-game, features previously restricted to cutscenes. It includes synced mouth movements for voice clips and environmental reactions like drowning or clearing a stage.",
        ),
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
        name: "Sonic: New Tricks",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-new-tricks/releases/latest/download/sadx-new-tricks.7z",
        },
        description: "Modernizes Sonic and Shadow's movesets with new and restored abilities.",
        full_description: Some(
            "Sonic: New Tricks allows players to remap Sonic's actions across multiple buttons (separating Jump, Bounce, and Light Dash). It restores the powerful SA1 Spin Dash and jump ball form, enhances the Bounce Bracelet, and allows Shadow and Metal Sonic to use abilities they previously lacked, such as the Bounce attack.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://github.com/Sora-yx/SADX-Better-Tails-AI/releases/latest/download/Better.Tails.AI.zip",
        },
        description: "Major improvements to Tails' behavior as a follower.",
        full_description: Some(
            "Better Tails AI makes Tails much more useful and interactive. He can now follow you into Hub Worlds and Boss Fights, pet Chao alongside the player, and sit in vehicles. It also adds a fast travel system in Hub Worlds and improves his flight speed to better keep up with Sonic.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-super-sonic/releases/latest/download/sadx-super-sonic.7z",
        },
        description: "Playable Super Sonic in action stages",
        full_description: Some(
            "A gameplay overhaul that enables Super Sonic for use in regular Action Stages after completing the story. It includes improved mechanics, fixed animations, and the ability to transform at will during normal levels.",
        ),
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
        name: "Multiplayer",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-multiplayer/releases/latest/download/sadx-multiplayer.7z",
        },
        description: "Adds local 4-player split-screen support to SADX.",
        full_description: Some(
            "The SADX Multiplayer mod adds local split-screen support for up to 4 players. It overhauls systems like fishing, hunting, and shooting to work in a multiplayer environment and supports both Co-op and Battle modes across various trial stages.",
        ),
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
    ModEntry {
        name: "Chao Gameplay",
        source: ModSource::DirectUrl {
            url: "https://github.com/kellsnc/sadx-chao-gameplay/releases/latest/download/sadx-chao-gameplay.7z",
        },
        description: "Allows taking your Chao out of the gardens and into action stages.",
        full_description: Some(
            "Also known as Chao Partner, this mod allows you to take your Chao with you into levels and hub worlds. Your Chao will follow you, attack nearby enemies based on its stats, and can be petted or dropped at will. It includes a water fix to allow Chao to 'swim' in standard level water.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://github.com/supercoolsonic/Fixes_Adds_BetaRestores/releases/latest/download/FABR.7z",
        },
        description: "Restores cut content and fixes bugs in the PC version.",
        full_description: Some(
            "This mod restores various beta elements like unused voice clips and animations while fixing hundreds of small bugs in the PC port. It adds 'Extra' mode layouts with restored level objects and improves visual elements like the Nightopian Egg and environmental sound effects.",
        ),
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
        source: ModSource::GameBananaItem {
            item_type: "Sound",
            item_id: 40537,
        },
        description: "Swaps the music tracks for the Perfect Chaos boss phases.",
        full_description: Some(
            "This mod swaps the music for Phase 1 and Phase 2 of the final boss fight. It is commonly used to ensure 'Open Your Heart' plays during the main gameplay segment of the fight, restoring the intended musical progression from the original Dreamcast release.",
        ),
        pictures: &[],
        dir_name: Some("Perfect Chaos Music Swap"),
        links: &[ModLink {
            label: "GameBanana",
            url: "https://gamebanana.com/sounds/40537",
        }],
    },
    ModEntry {
        name: "AI HD FMVs",
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/sadx-hd-videos/releases/latest/download/AI_HD_FMVs.7z",
        },
        description: "HD upscaled video cutscenes",
        full_description: Some(
            "Upscales the game's original pre-rendered cinematic cutscenes to 1080p using AI neural networks. This mod removes compression artifacts and blurriness, making the transition between gameplay and cutscenes feel much more seamless on HD monitors.",
        ),
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
        source: ModSource::DirectUrl {
            url: "https://github.com/kawaiikaorichan/AI_textures/releases/latest/download/AI_HD_Textures.7z",
        },
        description: "AI upscaled textures for both vanilla and Dreamcast Conversion assets",
        full_description: Some(
            "Uses AI technology like ESRGAN to upscale the game's textures to high definition. It sharpens the environment and character textures significantly while strictly preserving the original art style and color palette of the game.",
        ),
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
];

/// SADX Mod Presets.
pub const PRESETS: &[super::common::ModPreset] = &[
    super::common::ModPreset {
        name: "DX Enhanced",
        description: "Combines authentic Dreamcast levels and lighting with high-fidelity 'Refined' DX characters and DX-style water for a balanced modern-classic mix.",
        mod_names: &[
            "Dreamcast Conversion",
            "SADX: Fixed Edition",
            "Lantern Engine",
            "Steam Achievements",
            "Smooth Camera",
            "Frame Limit",
            "Sound Overhaul",
            "ADX Audio",
            "SADX Style Water",
            "Onion Blur",
            "DX Characters Refined",
            "Dreamcast DLC",
            "Idle Chatter",
            "Pause Hide",
            "Time of Day",
            "Sonic Adventure Retranslated",
            "HUD Plus",
            "HD GUI 2",
            "Active Mouths",
            "Sonic: New Tricks",
            "Better Tails AI",
            "Super Sonic",
            "Multiplayer",
            "Chao Gameplay",
            "Fixes, Adds, and Beta Restores",
            "Perfect Chaos Music Swap",
            "AI HD FMVs",
            "AI HD Textures",
        ],
    },
    super::common::ModPreset {
        name: "Dreamcast Restoration",
        description: "Provides a total visual restoration of the 1998 Dreamcast experience, including classic low-poly character models and original environmental effects.",
        mod_names: &[
            "Dreamcast Conversion",
            "SADX: Fixed Edition",
            "Lantern Engine",
            "Steam Achievements",
            "Smooth Camera",
            "Frame Limit",
            "Sound Overhaul",
            "ADX Audio",
            "Onion Blur",
            "Dreamcast Characters Pack",
            "Dreamcast DLC",
            "Idle Chatter",
            "Pause Hide",
            "Time of Day",
            "Sonic Adventure Retranslated",
            "HUD Plus",
            "HD GUI 2",
            "Active Mouths",
            "Sonic: New Tricks",
            "Better Tails AI",
            "Super Sonic",
            "Multiplayer",
            "Chao Gameplay",
            "Fixes, Adds, and Beta Restores",
            "Perfect Chaos Music Swap",
            "AI HD FMVs",
            "AI HD Textures",
        ],
    },
];

/// Convert the Steam version of SADX to the 2004 version using HDiffPatch.
///
/// The Steam version of sonic.exe is binary-incompatible with the mod loader.
/// This downloads `steam_tools.7z` (containing `patch_steam_inst.dat`) and
/// applies a directory diff patch that converts ~124 game files to the 2004
/// version that the mod loader expects.
///
/// Must be called from a blocking thread (e.g. `gio::spawn_blocking`).
pub fn convert_steam_to_2004(
    game_path: &Path,
    progress: Option<download::ProgressFn>,
) -> Result<()> {
    let system_dir = config::system_dir(game_path);

    // Skip if already converted. Check multiple markers since previous setups
    // (including the official Windows installer) leave different traces.
    let chrmodels_orig = system_dir.join("CHRMODELS_orig.dll");
    if chrmodels_orig.exists() {
        tracing::info!("Game appears already converted (CHRMODELS_orig.dll exists), skipping");
        return Ok(());
    }

    if game_path.join("mods/.modloader/SADXModLoader.dll").exists() {
        tracing::info!(
            "Game appears already converted (SADXModLoader.dll exists in .modloader), skipping"
        );
        return Ok(());
    }

    if game_path.join("sonic.exe").exists() {
        tracing::info!("Game appears already converted (sonic.exe exists), skipping");
        return Ok(());
    }

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let archive_path = temp_dir.path().join("steam_tools.7z");

    let steam_tools_url = steam_tools_url();
    download::download_file(&steam_tools_url, &archive_path, progress)?;

    let extract_dir = temp_dir.path().join("steam_tools");
    archive::extract(&archive_path, &extract_dir)?;

    let patch_file = extract_dir.join("patch_steam_inst.dat");
    if !patch_file.is_file() {
        anyhow::bail!("patch_steam_inst.dat not found in steam_tools.7z");
    }

    // Apply the directory diff patch using hpatchz (bundled in the Flatpak)
    // We use a separate output directory to avoid hpatchz failing due to
    // in-place modification conflicts or permission issues with its own temp dir.
    let out_dir = temp_dir.path().join("patched_game");
    std::fs::create_dir_all(&out_dir)?;

    let game_str = game_path
        .to_string_lossy()
        .trim_end_matches('/')
        .to_string();
    let patch_str = patch_file.to_string_lossy().to_string();
    let out_str = out_dir.to_string_lossy().trim_end_matches('/').to_string();

    // The hpatchz patch was built on a case-insensitive Windows filesystem.
    // On Linux (case-sensitive), directory names must match exactly.
    // Steam on Linux may extract directories with different casing than what
    // the patch expects, so we normalize them before patching.
    normalize_case_for_patch(game_path)?;

    tracing::info!("Applying Steam-to-2004 patch to {}", game_str);

    let hpatchz = hpatchz_program();
    let output = std::process::Command::new(&hpatchz)
        .arg("-f")
        .arg(&game_str)
        .arg(&patch_str)
        .arg(&out_str)
        .output()
        .with_context(|| {
            format!(
                "Failed to run {}. Is HDiffPatch installed?",
                hpatchz.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        tracing::error!("hpatchz failed. Stderr:\n{}", stderr);

        // If it failed due to "open oldFile ERROR", it's likely a source file mismatch.
        if stderr.contains("open oldFile ERROR!") || stderr.contains("check oldPathType") {
            tracing::error!(
                "Source file mismatch detected. This usually happens if the game is already modded or corrupted."
            );
            anyhow::bail!(
                "Steam-to-2004 conversion failed. Your game installation might be modified or corrupted. Please verify game integrity in Steam and try again.\n\nDetails:\n{stderr}"
            );
        }

        anyhow::bail!("Steam-to-2004 conversion failed:\n{stdout}\n{stderr}");
    }

    // Success! Now we move the patched files back to the game directory.
    // hpatchz in directory mode produces a new directory with the patched files.
    // We want to merge/overwrite these into the game directory.
    tracing::info!("Patch applied successfully to temp dir, moving files back...");

    // Move files from out_dir to game_path
    super::common::move_dir_contents(&out_dir, game_path)?;

    tracing::info!("Steam-to-2004 conversion complete");
    Ok(())
}

/// Rename directories to match the casing the hpatchz patch expects.
///
/// The hpatchz directory patch was created on a case-insensitive Windows filesystem.
/// On case-sensitive Linux filesystems, hpatchz will fail to find "old" files to patch
/// if their casing doesn't exactly match the manifest (which is often lowercase).
/// Steam on Linux may extract directories with different casing (e.g. VOICE_JP instead of voice_jp).
fn normalize_case_for_patch(game_path: &Path) -> Result<()> {
    // List of known directory casing mismatches between Steam and the patch manifest.
    let renames = [
        ("SoundData/VOICE_JP", "SoundData/voice_jp"),
        ("SoundData/VOICE_US", "SoundData/voice_us"),
        ("SoundData/SE", "SoundData/se"),
        ("SoundData/voice_jp/WMA", "SoundData/voice_jp/wma"),
        ("SoundData/voice_us/WMA", "SoundData/voice_us/wma"),
    ];

    for (old, new) in renames {
        let old_path = game_path.join(old);
        let new_path = game_path.join(new);

        if old_path.is_dir() && !new_path.exists() {
            std::fs::rename(&old_path, &new_path).with_context(|| {
                format!(
                    "Failed to rename {} → {}",
                    old_path.display(),
                    new_path.display()
                )
            })?;
            tracing::info!("Renamed {} → {} for patch compatibility", old, new);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::recommended_mods_tests!(29);

    #[test]
    fn test_dcmods_base_url_valid() {
        assert!(DCMODS_BASE.starts_with("https://"));
        assert!(DCMODS_BASE.ends_with('/'));
    }

    #[test]
    fn test_dcmods_urls_use_correct_base() {
        for m in RECOMMENDED_MODS {
            if let ModSource::DirectUrl { url } = &m.source
                && url.contains("dcmods.unreliable.network")
            {
                assert!(
                    url.starts_with(DCMODS_BASE),
                    "Mod '{}' dcmods URL doesn't start with DCMODS_BASE: {}",
                    m.name,
                    url
                );
            }
        }
    }

    #[test]
    fn test_sonic_new_tricks_uses_sadx_image_set() {
        let new_tricks = RECOMMENDED_MODS
            .iter()
            .find(|m| m.name == "Sonic: New Tricks")
            .expect("Sonic: New Tricks entry missing");

        assert_eq!(new_tricks.pictures.len(), 8);
        for picture in new_tricks.pictures {
            assert!(
                picture.starts_with(
                    "/io/github/astrovm/AdventureMods/resources/images/sadx/new_tricks/sadx-new-tricks_"
                ),
                "Unexpected SADX New Tricks picture path: {}",
                picture
            );
        }
    }

    // --- move_dir_contents() tests ---

    #[test]
    fn test_move_dir_contents_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        std::fs::write(src.join("a.txt"), "hello").unwrap();
        std::fs::write(src.join("b.txt"), "world").unwrap();

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "hello");
        assert_eq!(std::fs::read_to_string(dst.join("b.txt")).unwrap(), "world");
    }

    #[test]
    fn test_move_dir_contents_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::create_dir_all(dst.join("sub")).unwrap();

        std::fs::write(src.join("sub/new.txt"), "new").unwrap();
        std::fs::write(dst.join("sub/existing.txt"), "existing").unwrap();

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

        // Both files should exist in destination
        assert_eq!(
            std::fs::read_to_string(dst.join("sub/new.txt")).unwrap(),
            "new"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("sub/existing.txt")).unwrap(),
            "existing"
        );
    }

    #[test]
    fn test_move_dir_contents_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        std::fs::write(src.join("file.txt"), "new content").unwrap();
        std::fs::write(dst.join("file.txt"), "old content").unwrap();

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst.join("file.txt")).unwrap(),
            "new content"
        );
    }

    #[test]
    fn test_move_dir_contents_file_replaces_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        // dst has a directory named "item"
        std::fs::create_dir_all(dst.join("item")).unwrap();
        std::fs::write(dst.join("item/inner.txt"), "inner").unwrap();

        // src has a file named "item"
        std::fs::write(src.join("item"), "I am a file").unwrap();

        crate::setup::common::move_dir_contents(&src, &dst).unwrap();

        // "item" should now be a file, not a directory
        assert!(dst.join("item").is_file());
        assert_eq!(
            std::fs::read_to_string(dst.join("item")).unwrap(),
            "I am a file"
        );
    }

    // --- convert_steam_to_2004() skip detection tests ---

    #[test]
    fn test_convert_skips_if_chrmodels_orig_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("system")).unwrap();
        std::fs::write(tmp.path().join("system/CHRMODELS_orig.dll"), "dummy").unwrap();

        // Should return Ok without needing hpatchz or downloads
        convert_steam_to_2004(tmp.path(), None).unwrap();
    }

    #[test]
    fn test_convert_skips_if_sadxmodloader_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let loader_dir = tmp.path().join("mods/.modloader");
        std::fs::create_dir_all(&loader_dir).unwrap();
        std::fs::write(loader_dir.join("SADXModLoader.dll"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }

    #[test]
    fn test_convert_skips_if_sonic_exe_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("sonic.exe"), "dummy").unwrap();

        convert_steam_to_2004(tmp.path(), None).unwrap();
    }
}
