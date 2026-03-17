# Sonic Adventure mod reference report

Generated from the two Rust source lists used by the project (`sadx.rs` and `sa2.rs`).

## What this report does

- Enumerates every mod present in the two uploaded source files.
- Resolves every **GameBanana file id** in those lists to the actual public GameBanana page URL.
- Adds any other public pages I could confidently confirm in this pass: GitHub, GitLab, ModDB, forum thread, SHC entry, etc.
- Keeps the **installer/source URL from code** intact so you can diff this report against the implementation.

## Limits / confidence rules

- I only listed public links I could verify with reasonable confidence.
- If a mod only exposed a direct archive URL in the code and I could not confidently find a public landing page/repo, I marked it that way instead of guessing.
- Some mods exist both as a direct archive in the installer and as a public GameBanana/GitHub page. In those cases both are listed.
- `Perfect Chaos Music Swap` resolves to a **GameBanana Sound** page, not a GameBanana Mod page.

## SADX

## SADX mod list

### 1. Dreamcast Conversion
- **Directory name / key:** `DreamcastConversion`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DreamcastConversion.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/GerbilSoft/sadx_dreamcast>
  - GitLab: <https://gitlab.com/PiKeyAr/sadx_dreamcast>
  - ModDB: <https://www.moddb.com/mods/sadx-dreamcast-conversion>
- **Notes:** Core visual restoration project. Public project pages are available in addition to the direct archive URL.

### 2. SADX: Fixed Edition
- **Directory name / key:** `SADXFE`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SADXFE.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/michael-fadely/sadx-fixed-edition>
- **Notes:** No GameBanana page confirmed in this pass; GitHub repo confirmed.

### 3. Lantern Engine
- **Directory name / key:** `sadx-dc-lighting`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-dc-lighting.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/michael-fadely/sadx-dc-lighting>
- **Notes:** Dreamcast-style lighting restoration.

### 4. Steam Achievements
- **Directory name / key:** `SteamAchievements`
- **Installer source in code:** Direct download: https://mm.reimuhakurei.net/sadxmods/SteamAchievements.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/X-Hax/SADX2004SteamAchievements>
- **Notes:** Steam achievements bridge for modded/downgraded SADX.

### 5. Smooth Camera
- **Directory name / key:** `smooth-cam`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/smooth-cam.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/michael-fadely/sadx-smooth-cam>
- **Notes:** GitHub release path was confirmed; repo root listed here.

### 6. Frame Limit
- **Directory name / key:** `sadx-frame-limit`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-frame-limit.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/michael-fadely/sadx-frame-limit>
- **Notes:** _none_

### 7. Sound Overhaul
- **Directory name / key:** `SoundOverhaul`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SoundOverhaul.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitLab: <https://gitlab.com/PiKeyAr/sadx-sound-overhaul>
  - ModDB legacy release/news: <https://www.moddb.com/mods/sadx-dreamcast-conversion/news/sound-overhaul-2-a-dreamcast-sound-mod>
- **Notes:** Current public code/project page is best represented by the GitLab repo; ModDB has older release/news coverage under the Dreamcast Conversion project.

### 8. ADX Audio
- **Directory name / key:** `ADXAudio`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/ADXAudio.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** No confident public landing page/repo located in this pass beyond the installer/archive URL.

### 9. SADX Style Water
- **Directory name / key:** `sadx-style-water`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-style-water.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitLab: <https://gitlab.com/PiKeyAr/sadx-style-water>
- **Notes:** Former Dreamcast Conversion sub-component spun out as its own mod.

### 10. Onion Blur
- **Directory name / key:** `sadx-onion-blur`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-onion-blur.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/michael-fadely/sadx-onion-blur>
- **Notes:** _none_

### 11. Dreamcast Characters Pack
- **Directory name / key:** `SA1_Chars`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/SA1_Chars.7z
- **Resolved primary public page:** <https://gamebanana.com/mods/248063>
- **Other public links:**
  - GitHub: <https://github.com/ItsEasyActually/SA1_Chars>
  - Sonic Retro forum thread: <https://forums.sonicretro.org/threads/sadx-dreamcast-characters-pack.37034/>
- **Notes:** This one has a direct archive in the installer, but it also has public community/source pages.

### 12. DX Characters Refined
- **Directory name / key:** `DX Characters Refined`
- **Installer source in code:** GameBanana file id: 1498662
- **Resolved primary public page:** <https://gamebanana.com/mods/290728>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 13. Dreamcast DLC
- **Directory name / key:** `DLC`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/DLCs.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitLab: <https://gitlab.com/PiKeyAr/sadx-dreamcast-dlc>
  - ModDB news: <https://www.moddb.com/news/dreamcast-exclusive-dlcs-now-playable-in-sadx-pc>
- **Notes:** Current public project page is on GitLab.

### 14. Idle Chatter
- **Directory name / key:** `idle-chatter`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/idle-chatter.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/michael-fadely/sadx-idle-chatter>
- **Notes:** _none_

### 15. Pause Hide
- **Directory name / key:** `pause-hide`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/pause-hide.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/michael-fadely/sadx-pause-hide>
- **Notes:** _none_

### 16. Time of Day
- **Directory name / key:** `TrainDaytime`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/TrainDaytime.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitLab: <https://gitlab.com/PiKeyAr/sadx-timeofday-mod>
- **Notes:** GitLab project page confirmed.

### 17. Sonic Adventure Retranslated
- **Directory name / key:** `Sonic Adventure Retranslated`
- **Installer source in code:** GameBanana file id: 384650
- **Resolved primary public page:** <https://gamebanana.com/mods/49930>
- **Other public links:**
  - GitHub: <https://github.com/SKingBlue/Sonic-Adventure-Retranslated>
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 18. HUD Plus
- **Directory name / key:** `sadx-hud-plus`
- **Installer source in code:** GameBanana file id: 1309612
- **Resolved primary public page:** <https://gamebanana.com/mods/394948>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 19. HD GUI 2
- **Directory name / key:** `HD_DCStyle`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/HD_DCStyle.7z
- **Resolved primary public page:** _none confidently confirmed / not needed_
- **Other public links:**
  - GitHub: <https://github.com/X-Hax/sadx-hd-gui>
- **Notes:** _none_

### 20. Active Mouths
- **Directory name / key:** `Active Mouths`
- **Installer source in code:** GameBanana file id: 622235
- **Resolved primary public page:** <https://gamebanana.com/mods/304634>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 21. Sonic: New Tricks
- **Directory name / key:** `sadx-new-tricks`
- **Installer source in code:** GameBanana file id: 1102800
- **Resolved primary public page:** <https://gamebanana.com/mods/49973>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 22. Better Tails AI
- **Directory name / key:** `Better Tails AI`
- **Installer source in code:** GameBanana file id: 1148657
- **Resolved primary public page:** <https://gamebanana.com/mods/49943>
- **Other public links:**
  - GitHub: <https://github.com/Sora-yx/SADX-Better-Tails-AI>
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 23. Super Sonic
- **Directory name / key:** `sadx-super-sonic`
- **Installer source in code:** Direct download: https://dcmods.unreliable.network/owncloud/data/PiKeyAr/files/Setup/data/sadx-super-sonic.7z
- **Resolved primary public page:** <https://gamebanana.com/mods/49986>
- **Other public links:**
  - GitHub: <https://github.com/kellsnc/sadx-super-sonic>
- **Notes:** Installer uses a direct archive, but public GameBanana/GitHub pages also exist.

### 24. Multiplayer
- **Directory name / key:** `sadx-multiplayer`
- **Installer source in code:** GameBanana file id: 1046512
- **Resolved primary public page:** <https://gamebanana.com/mods/460975>
- **Other public links:**
  - GitHub: <https://github.com/kellsnc/sadx-multiplayer>
  - Sonic Hacking Contest entry: <https://shc.zone/entries/contest2023/912>
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 25. Chao Gameplay
- **Directory name / key:** `sadx-chao-gameplay`
- **Installer source in code:** GameBanana file id: 781777
- **Resolved primary public page:** <https://gamebanana.com/mods/49974>
- **Other public links:**
  - GitHub: <https://github.com/kellsnc/sadx-chao-gameplay>
- **Notes:** Also known in some places as Chao Partner / SADX Chao Gameplay.

### 26. Fixes, Adds, and Beta Restores
- **Directory name / key:** `Fixes_Adds_BetaRestores`
- **Installer source in code:** GameBanana file id: 429267
- **Resolved primary public page:** <https://gamebanana.com/mods/49976>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 27. Perfect Chaos Music Swap
- **Directory name / key:** `Perfect Chaos Music Swap`
- **Installer source in code:** GameBanana file id: 1217474
- **Resolved primary public page:** <https://gamebanana.com/sounds/40537>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Important: this resolves to a GameBanana Sound entry, not a Mod entry.

### 28. AI HD FMVs
- **Directory name / key:** `AI_HD_FMVs`
- **Installer source in code:** Direct download: https://github.com/kawaiikaorichan/sadx-hd-videos/releases/latest/download/AI_HD_FMVs.7z
- **Resolved primary public page:** <https://gamebanana.com/mods/49951>
- **Other public links:**
  - GitHub: <https://github.com/kawaiikaorichan/sadx-hd-videos>
- **Notes:** Installer points at GitHub release asset directly; public GameBanana page also exists.

### 29. AI HD Textures
- **Directory name / key:** `AI_HD_Textures`
- **Installer source in code:** Direct download: https://github.com/kawaiikaorichan/AI_textures/releases/latest/download/AI_HD_Textures.7z
- **Resolved primary public page:** <https://gamebanana.com/mods/49978>
- **Other public links:**
  - GitHub: <https://github.com/kawaiikaorichan/AI_textures>
- **Notes:** Installer points at GitHub release asset directly; public GameBanana page also exists.


## SA2

## SA2 mod list

### 1. SA2 Render Fix
- **Directory name / key:** `sa2-render-fix`
- **Installer source in code:** GameBanana file id: 1626250
- **Resolved primary public page:** <https://gamebanana.com/mods/452445>
- **Other public links:**
  - GitHub: <https://github.com/shaddatic/sa2b-render-fix>
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 2. Retranslated Story -COMPLETE-
- **Directory name / key:** `Retranslated Story -COMPLETE-`
- **Installer source in code:** GameBanana file id: 1601215
- **Resolved primary public page:** <https://gamebanana.com/mods/437858>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 3. HD GUI: SA2 Edition
- **Directory name / key:** `HD GUI for SA2`
- **Installer source in code:** GameBanana file id: 409120
- **Resolved primary public page:** <https://gamebanana.com/mods/33171>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 4. IMPRESSive
- **Directory name / key:** `IMPRESSive`
- **Installer source in code:** GameBanana file id: 1213103
- **Resolved primary public page:** <https://gamebanana.com/mods/469542>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 5. Stage Atmosphere Tweaks
- **Directory name / key:** `StageAtmosphereTweaks`
- **Installer source in code:** GameBanana file id: 884395
- **Resolved primary public page:** <https://gamebanana.com/mods/407838>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 6. SA2 Volume Controls
- **Directory name / key:** `SA2VolumeControls`
- **Installer source in code:** GameBanana file id: 835829
- **Resolved primary public page:** <https://gamebanana.com/mods/381193>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 7. Mech Sound Improvement
- **Directory name / key:** `Mech Sound Improvement`
- **Installer source in code:** GameBanana file id: 893090
- **Resolved primary public page:** <https://gamebanana.com/mods/412706>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 8. SASDL
- **Directory name / key:** `SASDL`
- **Installer source in code:** GameBanana file id: 1503809
- **Resolved primary public page:** <https://gamebanana.com/mods/615843>
- **Other public links:**
  - GitHub: <https://github.com/Shaddatic/sa2b-sdl-loader>
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 9. SA2 Input Controls
- **Directory name / key:** `sa2-input-controls`
- **Installer source in code:** GameBanana file id: 1514050
- **Resolved primary public page:** <https://gamebanana.com/mods/515637>
- **Other public links:**
  - GitHub: <https://github.com/shaddatic/sa2b-input-controls>
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 10. Better Radar
- **Directory name / key:** `SA2BetterRadar`
- **Installer source in code:** GameBanana file id: 1580535
- **Resolved primary public page:** <https://gamebanana.com/mods/313161>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 11. HedgePanel - Sonic + Shadow Tweaks
- **Directory name / key:** `HedgePanel`
- **Installer source in code:** GameBanana file id: 454296
- **Resolved primary public page:** <https://gamebanana.com/mods/48950>
- **Other public links:**
  - GitHub (related/light-dash remap helper commonly paired with HedgePanel): <https://github.com/michael-fadely/sa2-action-remap>
- **Notes:** Resolved from file id to actual GameBanana mod page. The GitHub link above is a related companion/remap project that repeatedly shows up alongside HedgePanel in community guides; it is not the HedgePanel page itself.

### 12. Sonic: New Tricks
- **Directory name / key:** `Sonic New Tricks`
- **Installer source in code:** GameBanana file id: 915082
- **Resolved primary public page:** <https://gamebanana.com/mods/48941>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.

### 13. Retranslated Hints
- **Directory name / key:** `Retranslated Hints`
- **Installer source in code:** GameBanana file id: 1388468
- **Resolved primary public page:** <https://gamebanana.com/mods/350607>
- **Other public links:** _none confidently confirmed in this pass_
- **Notes:** Resolved from file id to actual GameBanana mod page.


## Appendix A: SADX GameBanana file-id resolution

| Mod | file_id used in code | Resolved public page |
|---|---:|---|
| DX Characters Refined | 1498662 | <https://gamebanana.com/mods/290728> |
| Sonic Adventure Retranslated | 384650 | <https://gamebanana.com/mods/49930> |
| HUD Plus | 1309612 | <https://gamebanana.com/mods/394948> |
| Active Mouths | 622235 | <https://gamebanana.com/mods/304634> |
| Sonic: New Tricks | 1102800 | <https://gamebanana.com/mods/49973> |
| Better Tails AI | 1148657 | <https://gamebanana.com/mods/49943> |
| Multiplayer | 1046512 | <https://gamebanana.com/mods/460975> |
| Chao Gameplay | 781777 | <https://gamebanana.com/mods/49974> |
| Fixes, Adds, and Beta Restores | 429267 | <https://gamebanana.com/mods/49976> |
| Perfect Chaos Music Swap | 1217474 | <https://gamebanana.com/sounds/40537> |

## Appendix B: SA2 GameBanana file-id resolution

| Mod | file_id used in code | Resolved public page |
|---|---:|---|
| SA2 Render Fix | 1626250 | <https://gamebanana.com/mods/452445> |
| Retranslated Story -COMPLETE- | 1601215 | <https://gamebanana.com/mods/437858> |
| HD GUI: SA2 Edition | 409120 | <https://gamebanana.com/mods/33171> |
| IMPRESSive | 1213103 | <https://gamebanana.com/mods/469542> |
| Stage Atmosphere Tweaks | 884395 | <https://gamebanana.com/mods/407838> |
| SA2 Volume Controls | 835829 | <https://gamebanana.com/mods/381193> |
| Mech Sound Improvement | 893090 | <https://gamebanana.com/mods/412706> |
| SASDL | 1503809 | <https://gamebanana.com/mods/615843> |
| SA2 Input Controls | 1514050 | <https://gamebanana.com/mods/515637> |
| Better Radar | 1580535 | <https://gamebanana.com/mods/313161> |
| HedgePanel - Sonic + Shadow Tweaks | 454296 | <https://gamebanana.com/mods/48950> |
| Sonic: New Tricks | 915082 | <https://gamebanana.com/mods/48941> |
| Retranslated Hints | 1388468 | <https://gamebanana.com/mods/350607> |

## Appendix C: source files used

- Uploaded source file: `sadx.rs`
- Uploaded source file: `sa2.rs`
- Extra reference supplied by user: <https://gitlab.com/PiKeyAr/sadx-mod-installer/-/wikis/Mods>
