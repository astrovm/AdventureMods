//! Real Proton builds, laid out the way Steam leaves them, so the Proton check
//! is tested against what users actually have rather than made-up names.
//!
//! Every label below was copied from a real install (the `version` file, the
//! `CURRENT_PREFIX_VERSION` the build writes into the prefix, the name Steam
//! stores in `config.vdf`, and the Wine version compiled into `ntdll.so`).
//! To record a new build, run on a machine that has it:
//!
//! ```sh
//! d=".../Proton 10.0"   # or compatibilitytools.d/<tool>
//! cat "$d/version"; grep -m1 CURRENT_PREFIX_VERSION= "$d/proton"
//! grep -ao 'wine-[0-9]*\.[0-9]*[^[:cntrl:]]*' "$d/files/lib/wine/x86_64-unix/ntdll.so"
//! ```

use std::path::{Path, PathBuf};

use super::*;

/// Where Steam keeps a compatibility tool.
#[derive(Clone, Copy)]
enum Place {
    /// Valve's builds, installed like games in `steamapps/common`.
    Common,
    /// Custom builds (ProtonUp-Qt, ProtonPlus, …) in `compatibilitytools.d`,
    /// registered with a `compatibilitytool.vdf`.
    CompatToolsD,
}

#[derive(Clone, Copy)]
struct Tool {
    place: Place,
    /// Folder name on disk.
    dir: &'static str,
    /// The name Steam stores in `CompatToolMapping`.
    internal_name: &'static str,
    /// Contents of the tool's `version` file.
    version_file: &'static str,
    /// What the build writes into `compatdata/<app>/version`.
    prefix_version: &'static str,
    /// The version string compiled into `files/lib/wine/x86_64-unix/ntdll.so`.
    wine: &'static str,
}

const PROTON_10: Tool = Tool {
    place: Place::Common,
    dir: "Proton 10.0",
    internal_name: "proton_10",
    version_file: "1785139158 proton-10.0-4b",
    prefix_version: "10.1000-105",
    wine: "wine-10.0",
};

const PROTON_11: Tool = Tool {
    place: Place::Common,
    dir: "Proton 11.0",
    internal_name: "proton_11",
    version_file: "1791559474 proton-11.0-2e-x86_64",
    prefix_version: "11.0-100",
    wine: "wine-11.0",
};

const PROTON_EXPERIMENTAL: Tool = Tool {
    place: Place::Common,
    dir: "Proton - Experimental",
    internal_name: "proton_experimental",
    version_file: "1791989105 experimental-11.0-20261014-x86_64",
    prefix_version: "11.0-100",
    wine: "wine-11.0",
};

const PROTON_HOTFIX: Tool = Tool {
    place: Place::Common,
    dir: "Proton Hotfix",
    internal_name: "proton_hotfix",
    version_file: "1791407340 hotfix-20261007-x86_64",
    prefix_version: "11.0-100",
    wine: "wine-11.0",
};

/// GE-Proton as ProtonUp-Qt installs it: folder and name are the release tag.
const GE_PROTON_10: Tool = Tool {
    place: Place::CompatToolsD,
    dir: "GE-Proton10-34",
    internal_name: "GE-Proton10-34",
    version_file: "1774238111 GE-Proton10-34",
    prefix_version: "GE-Proton10-34",
    wine: "wine-10.0 (Staging)",
};

const GE_PROTON_11: Tool = Tool {
    place: Place::CompatToolsD,
    dir: "GE-Proton11-6",
    internal_name: "GE-Proton11-6",
    version_file: "1787951532 GE-Proton11-6",
    prefix_version: "GE-Proton11-6",
    wine: "wine-11.0 (Staging)",
};

/// A self-updating GE-Proton whose name never says which version it holds.
const GE_PROTON_LATEST_11: Tool = Tool {
    place: Place::CompatToolsD,
    dir: "Proton-GE Latest",
    internal_name: "Proton-GE Latest",
    version_file: "1787951532 GE-Proton11-6",
    prefix_version: "GE-Proton11-6",
    wine: "wine-11.0 (Staging)",
};

const CACHYOS_11: Tool = Tool {
    place: Place::CompatToolsD,
    dir: "Proton-CachyOS Latest",
    internal_name: "Proton-CachyOS Latest",
    version_file: "1784681208 cachyos-11.0-20260703-slr",
    prefix_version: "CachyOS-11.0-100",
    wine: "wine-11.0 (CachyOS)",
};

const SA2: u32 = 213610;

/// A home folder with a native Steam install, as on SteamOS.
struct Steam {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    libraries: Vec<PathBuf>,
    /// A distro's `/usr/share/steam/compatibilitytools.d`.
    system_tools: PathBuf,
}

impl Steam {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("home/deck").join(".local/share/Steam");
        std::fs::create_dir_all(root.join("steamapps/common")).unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        let system_tools = tmp.path().join("usr/share/steam/compatibilitytools.d");
        let steam = Self {
            _tmp: tmp,
            root: root.clone(),
            libraries: vec![root],
            system_tools,
        };
        steam.write_libraryfolders();
        steam.write_config("");
        steam
    }

    /// Add a library on another drive, like a microSD card.
    fn add_library(&mut self, name: &str) -> PathBuf {
        let library = self._tmp.path().join("run/media/deck").join(name);
        std::fs::create_dir_all(library.join("steamapps/common")).unwrap();
        self.libraries.push(library.clone());
        self.write_libraryfolders();
        library
    }

    fn write_libraryfolders(&self) {
        let mut vdf = String::from("\"libraryfolders\"\n{\n");
        for (index, library) in self.libraries.iter().enumerate() {
            vdf.push_str(&format!(
                "\t\"{index}\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n",
                library.display()
            ));
        }
        vdf.push_str("}\n");
        std::fs::write(self.root.join("steamapps/libraryfolders.vdf"), vdf).unwrap();
    }

    fn write_config(&self, mapping: &str) {
        std::fs::write(
            self.root.join("config/config.vdf"),
            format!(
                "\"InstallConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"Valve\"\n\t\t{{\n\t\t\t\"Steam\"\n\t\t\t{{\n\t\t\t\t\"CompatToolMapping\"\n\t\t\t\t{{\n{mapping}\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
            ),
        )
        .unwrap();
    }

    /// Install a tool. Valve's go into `library`, custom ones into the Steam
    /// root's `compatibilitytools.d`.
    fn install(&self, library: &Path, tool: Tool) -> PathBuf {
        self.install_as(library, tool, tool.dir)
    }

    fn install_as(&self, library: &Path, tool: Tool, dir_name: &str) -> PathBuf {
        let dir = match tool.place {
            Place::Common => library.join("steamapps/common").join(dir_name),
            Place::CompatToolsD => self.root.join("compatibilitytools.d").join(dir_name),
        };
        self.install_at(&dir, tool);
        dir
    }

    /// Install a custom tool the way a distro package does.
    fn install_system(&self, tool: Tool) -> PathBuf {
        let dir = self.system_tools.join(tool.dir);
        self.install_at(&dir, tool);
        dir
    }

    fn install_at(&self, dir: &Path, tool: Tool) {
        std::fs::create_dir_all(dir.join("files/bin")).unwrap();
        std::fs::write(dir.join("files/bin/wine"), b"").unwrap();
        std::fs::write(dir.join("version"), format!("{}\n", tool.version_file)).unwrap();
        let ntdll = dir.join("files/lib/wine/x86_64-unix/ntdll.so");
        std::fs::create_dir_all(ntdll.parent().unwrap()).unwrap();
        // Surrounded by unrelated bytes, the way it sits in the real file.
        let mut bytes = b"\x7fELF\x02\x01\x01\x00load_steam_overlay\x00\x80\x80".to_vec();
        bytes.extend_from_slice(tool.wine.as_bytes());
        bytes.extend_from_slice(b"\x00\x00\x00\x01\x1b\x03;@");
        std::fs::write(ntdll, bytes).unwrap();
        if let Place::CompatToolsD = tool.place {
            std::fs::write(
                dir.join("compatibilitytool.vdf"),
                format!(
                    "\"compatibilitytools\"\n{{\n  \"compat_tools\"\n  {{\n    \"{0}\" // Internal name of this tool\n    {{\n      \"install_path\" \".\"\n      \"display_name\" \"{0}\"\n      \"from_oslist\"  \"windows\"\n      \"to_oslist\"    \"linux\"\n    }}\n  }}\n}}\n",
                    tool.internal_name
                ),
            )
            .unwrap();
        }
    }

    fn install_game(&self, library: &Path) -> PathBuf {
        let game = library.join("steamapps/common/Sonic Adventure 2");
        std::fs::create_dir_all(&game).unwrap();
        game
    }

    /// Force `tool` for SA2 in Properties → Compatibility.
    fn force(&self, tool: Tool) {
        self.write_config(&Self::mapping(tool));
    }

    fn mapping(tool: Tool) -> String {
        format!(
            "\t\t\t\t\t\"{SA2}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"name\"\t\t\"{}\"\n\t\t\t\t\t\t\"config\"\t\t\"\"\n\t\t\t\t\t\t\"priority\"\t\t\"250\"\n\t\t\t\t\t}}\n",
            tool.internal_name
        )
    }

    /// Launch SA2 from Steam with the tool at `tool_dir`: write the prefix
    /// files exactly as Proton's `setup_prefix` does.
    fn launch(&self, game: &Path, tool: Tool, tool_dir: &Path) {
        let compatdata = game
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("compatdata")
            .join(SA2.to_string());
        std::fs::create_dir_all(compatdata.join("pfx/drive_c")).unwrap();
        std::fs::write(
            compatdata.join("version"),
            format!("{}\n", tool.prefix_version),
        )
        .unwrap();
        let tool_dir = tool_dir.display();
        let root = self.root.display();
        std::fs::write(
            compatdata.join("config_info"),
            format!(
                "{}\n{tool_dir}/files/share/fonts/\n{tool_dir}/files/lib/\n{root}/\n1700000000\n1700000000\n1700000000\n{tool_dir}/files/share/default_pfx/\n1700000000\nFalse\nTrue\nmfplat.dll\nFalse\nFalse",
                tool.prefix_version
            ),
        )
        .unwrap();
    }

    fn state(&self, game: &Path) -> PrefixState {
        let paths = SteamPaths {
            roots: vec![self.root.clone()],
            tool_dirs: vec![self.system_tools.clone()],
        };
        prefix_state_in(game, SA2, &paths).unwrap()
    }
}

/// Force a tool, launch the game once with it, then check.
fn forced_and_launched(tool: Tool) -> PrefixState {
    let steam = Steam::new();
    let game = steam.install_game(&steam.root);
    let dir = steam.install(&steam.root, tool);
    steam.force(tool);
    steam.launch(&game, tool, &dir);
    steam.state(&game)
}

#[test]
fn proton_10_is_ready() {
    assert_eq!(forced_and_launched(PROTON_10), PrefixState::Ready);
}

#[test]
fn ge_proton_10_is_ready() {
    assert_eq!(forced_and_launched(GE_PROTON_10), PrefixState::Ready);
}

#[test]
fn every_wine_11_build_is_refused() {
    for tool in [
        PROTON_11,
        PROTON_EXPERIMENTAL,
        PROTON_HOTFIX,
        GE_PROTON_11,
        GE_PROTON_LATEST_11,
        CACHYOS_11,
    ] {
        let state = forced_and_launched(tool);
        assert!(
            matches!(state, PrefixState::UnsupportedProton { major: 11, .. }),
            "{}: {state:?}",
            tool.dir
        );
    }
}

#[test]
fn game_on_a_microsd_card_with_proton_10_on_the_internal_drive_is_ready() {
    let mut steam = Steam::new();
    let sd = steam.add_library("mmcblk0p1");
    let game = steam.install_game(&sd);
    let proton = steam.install(&steam.root, PROTON_10);
    steam.force(PROTON_10);
    steam.launch(&game, PROTON_10, &proton);

    assert_eq!(steam.state(&game), PrefixState::Ready);
}

#[test]
fn a_leftover_copy_of_proton_10_in_another_library_is_still_proton_10() {
    let mut steam = Steam::new();
    let sd = steam.add_library("mmcblk0p1");
    let game = steam.install_game(&sd);
    // Steam ran the internal copy; an older copy is still on the card.
    steam.install(&sd, PROTON_10);
    let proton = steam.install(&steam.root, PROTON_10);
    steam.force(PROTON_10);
    steam.launch(&game, PROTON_10, &proton);

    assert_eq!(steam.state(&game), PrefixState::Ready);
}

#[test]
fn switching_to_a_wine_11_build_on_another_drive_is_caught_before_launch() {
    let mut steam = Steam::new();
    let usb = steam.add_library("usb");
    let game = steam.install_game(&steam.root);
    let proton = steam.install(&steam.root, PROTON_10);
    steam.install(&usb, PROTON_EXPERIMENTAL);
    steam.launch(&game, PROTON_10, &proton);
    steam.force(PROTON_EXPERIMENTAL);

    assert_eq!(
        steam.state(&game),
        PrefixState::UnsupportedConfiguredProton {
            tool_name: "Proton - Experimental".to_owned(),
            major: 11,
        }
    );
}

#[test]
fn a_custom_tool_is_found_by_its_registered_name_not_its_folder() {
    let steam = Steam::new();
    let game = steam.install_game(&steam.root);
    let proton = steam.install(&steam.root, PROTON_10);
    steam.launch(&game, PROTON_10, &proton);
    steam.install_as(&steam.root, GE_PROTON_LATEST_11, "proton-ge-auto");
    steam.force(GE_PROTON_LATEST_11);

    assert_eq!(
        steam.state(&game),
        PrefixState::UnsupportedConfiguredProton {
            tool_name: "proton-ge-auto".to_owned(),
            major: 11,
        }
    );
}

#[test]
fn config_vdf_keys_are_read_whatever_their_case() {
    let steam = Steam::new();
    let game = steam.install_game(&steam.root);
    let proton = steam.install(&steam.root, PROTON_10);
    steam.launch(&game, PROTON_10, &proton);
    steam.install(&steam.root, GE_PROTON_11);
    std::fs::write(
        steam.root.join("config/config.vdf"),
        format!(
            "\"InstallConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"valve\"\n\t\t{{\n\t\t\t\"steam\"\n\t\t\t{{\n\t\t\t\t\"CompatToolMapping\"\n\t\t\t\t{{\n{}\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n",
            Steam::mapping(GE_PROTON_11)
        ),
    )
    .unwrap();

    assert_eq!(
        steam.state(&game),
        PrefixState::UnsupportedConfiguredProton {
            tool_name: "GE-Proton11-6".to_owned(),
            major: 11,
        }
    );
}

#[test]
fn switching_from_ge_proton_10_to_proton_10_asks_for_one_launch_by_name() {
    let steam = Steam::new();
    let game = steam.install_game(&steam.root);
    let ge = steam.install(&steam.root, GE_PROTON_10);
    steam.install(&steam.root, PROTON_10);
    steam.launch(&game, GE_PROTON_10, &ge);
    steam.force(PROTON_10);

    let state = steam.state(&game);
    assert_eq!(
        state,
        PrefixState::ConfigMismatch {
            prefix_tool: "GE-Proton10-34".to_owned(),
            configured_tool: "Proton 10.0".to_owned(),
        }
    );
    let message = steam_config_message_for_state("Sonic Adventure 2", &Ok(state));
    assert!(message.contains("GE-Proton10-34"), "{message}");
    assert!(message.contains("Proton 10.0"), "{message}");
}

#[test]
fn a_system_wide_ge_proton_10_is_ready() {
    let steam = Steam::new();
    let game = steam.install_game(&steam.root);
    let ge = steam.install_system(GE_PROTON_10);
    steam.force(GE_PROTON_10);
    steam.launch(&game, GE_PROTON_10, &ge);

    assert_eq!(steam.state(&game), PrefixState::Ready);
}

#[test]
fn a_self_updating_tool_that_moved_to_wine_11_is_refused() {
    let steam = Steam::new();
    let game = steam.install_game(&steam.root);
    // The prefix was last set up by GE-Proton10, then the same folder updated
    // itself to GE-Proton11, which is what the next launch will run.
    let latest_10 = Tool {
        prefix_version: GE_PROTON_10.prefix_version,
        ..GE_PROTON_LATEST_11
    };
    let dir = steam.install(&steam.root, GE_PROTON_LATEST_11);
    steam.force(GE_PROTON_LATEST_11);
    steam.launch(&game, latest_10, &dir);

    assert_eq!(
        steam.state(&game),
        PrefixState::UnsupportedProton {
            tool_name: "Proton-GE Latest".to_owned(),
            major: 11,
        }
    );
}

#[test]
fn messages_name_the_build_not_its_prefix_version() {
    let state = forced_and_launched(PROTON_EXPERIMENTAL);
    let message = steam_config_message_for_state("Sonic Adventure 2", &Ok(state));
    assert!(message.contains("Proton - Experimental"), "{message}");
    assert!(!message.contains("11.0-100"), "{message}");
}
