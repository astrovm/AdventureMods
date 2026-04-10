use std::path::{Path, PathBuf};

use crate::support::scripts;

#[allow(dead_code)]
pub struct SteamFixture {
    pub _temp_dir: tempfile::TempDir,
    pub libraryfolders_vdf: PathBuf,
    pub game_path: PathBuf,
    pub prefix_path: PathBuf,
    pub wine_log: PathBuf,
    pub fake_7zz: PathBuf,
    pub fake_hpatchz: PathBuf,
    pub extract_root: PathBuf,
}

#[allow(dead_code)]
pub fn create_sa2_fixture() -> SteamFixture {
    let temp_dir = tempfile::tempdir().unwrap();
    let steam_root = temp_dir.path().join("Steam");
    let steamapps = steam_root.join("steamapps");
    let compatdata = steamapps.join("compatdata/213610");
    let proton_dir = steam_root.join("compatibilitytools.d/TestProton");
    let game_path = steamapps.join("common/Sonic Adventure 2");
    let extract_root = temp_dir.path().join("extract-fixtures");
    let patched_root = temp_dir.path().join("patched-sadx");
    let script_dir = temp_dir.path().join("scripts");
    let wine_log = temp_dir.path().join("wine.log");

    std::fs::create_dir_all(game_path.join("resource/gd_PC/DLL/Win32")).unwrap();
    std::fs::create_dir_all(game_path.join("mods")).unwrap();
    std::fs::write(game_path.join("Launcher.exe"), b"launcher").unwrap();
    std::fs::write(game_path.join("sonic2app.exe"), b"game").unwrap();
    std::fs::write(
        game_path.join("resource/gd_PC/DLL/Win32/Data_DLL.dll"),
        b"original-dll",
    )
    .unwrap();

    std::fs::create_dir_all(compatdata.join("pfx/drive_c")).unwrap();
    std::fs::write(compatdata.join("version"), b"TestProton\n").unwrap();
    std::fs::write(
        compatdata.join("config_info"),
        format!(
            "TestProton\n{}/files/share/fonts/\n{}/files/lib/\n{}\n0\n0\n0\n{}/files/share/default_pfx/\n0\nFalse\n",
            proton_dir.display(),
            proton_dir.display(),
            steam_root.display(),
            proton_dir.display(),
        ),
    )
    .unwrap();

    std::fs::create_dir_all(steam_root.join("config")).unwrap();
    std::fs::write(
        steam_root.join("config/config.vdf"),
        r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "CompatToolMapping"
                {
                    "0"
                    {
                        "name"  "TestProton"
                    }
                }
            }
        }
    }
}"#,
    )
    .unwrap();

    let libraryfolders_vdf = steamapps.join("libraryfolders.vdf");
    std::fs::create_dir_all(&steamapps).unwrap();
    std::fs::write(
        &libraryfolders_vdf,
        format!(
            "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n        \"apps\"\n        {{\n            \"213610\"\t\"0\"\n        }}\n    }}\n}}\n",
            steam_root.display()
        ),
    )
    .unwrap();

    std::fs::create_dir_all(proton_dir.join("files/bin")).unwrap();
    scripts::install_fake_wine(&proton_dir.join("files/bin/wine64"), &wine_log);

    std::fs::create_dir_all(&extract_root).unwrap();
    write_file(
        &extract_root.join("samodmanager/SAModManager.exe"),
        b"manager",
    );
    write_file(&extract_root.join("sa2-loader/SA2ModLoader.dll"), b"loader");
    write_file(
        &extract_root.join("render-fix/Render Fix/mod.ini"),
        b"Name=Render Fix",
    );
    write_file(&extract_root.join("render-fix/Render Fix/data.bin"), b"rf");
    write_file(
        &extract_root.join("hd-gui/HD GUI for SA2/mod.ini"),
        b"Name=HD GUI for SA2",
    );
    write_file(
        &extract_root.join("test-flat/Test Flat/mod.ini"),
        b"Name=Test Flat",
    );
    write_file(&extract_root.join("test-flat/Test Flat/asset.txt"), b"flat");

    std::fs::create_dir_all(&patched_root).unwrap();

    std::fs::create_dir_all(&script_dir).unwrap();
    let fake_7zz = script_dir.join("fake-7zz");
    scripts::install_fake_7zz(&fake_7zz, &extract_root);
    let fake_hpatchz = script_dir.join("fake-hpatchz");
    scripts::install_fake_hpatchz(&fake_hpatchz, &patched_root);

    SteamFixture {
        _temp_dir: temp_dir,
        libraryfolders_vdf,
        game_path,
        prefix_path: compatdata.join("pfx"),
        wine_log,
        fake_7zz,
        fake_hpatchz,
        extract_root,
    }
}

#[allow(dead_code)]
pub fn create_sadx_fixture() -> SteamFixture {
    let temp_dir = tempfile::tempdir().unwrap();
    let steam_root = temp_dir.path().join("Steam");
    let steamapps = steam_root.join("steamapps");
    let compatdata = steamapps.join("compatdata/71250");
    let proton_dir = steam_root.join("compatibilitytools.d/TestProton");
    let game_path = steamapps.join("common/Sonic Adventure DX");
    let extract_root = temp_dir.path().join("extract-fixtures");
    let patched_root = temp_dir.path().join("patched-sadx");
    let script_dir = temp_dir.path().join("scripts");
    let wine_log = temp_dir.path().join("wine.log");

    std::fs::create_dir_all(game_path.join("system")).unwrap();
    std::fs::create_dir_all(game_path.join("mods")).unwrap();
    std::fs::create_dir_all(game_path.join("SoundData/VOICE_JP/WMA")).unwrap();
    std::fs::create_dir_all(game_path.join("SoundData/VOICE_US/WMA")).unwrap();
    std::fs::create_dir_all(game_path.join("SoundData/SE")).unwrap();
    std::fs::write(game_path.join("Sonic Adventure DX.exe"), b"sadx").unwrap();
    std::fs::write(game_path.join("system/CHRMODELS.dll"), b"original-dll").unwrap();

    std::fs::create_dir_all(compatdata.join("pfx/drive_c")).unwrap();
    std::fs::write(compatdata.join("version"), b"TestProton\n").unwrap();
    std::fs::write(
        compatdata.join("config_info"),
        format!(
            "TestProton\n{}/files/share/fonts/\n{}/files/lib/\n{}\n0\n0\n0\n{}/files/share/default_pfx/\n0\nFalse\n",
            proton_dir.display(),
            proton_dir.display(),
            steam_root.display(),
            proton_dir.display(),
        ),
    )
    .unwrap();

    std::fs::create_dir_all(steam_root.join("config")).unwrap();
    std::fs::write(
        steam_root.join("config/config.vdf"),
        r#""InstallConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "CompatToolMapping"
                {
                    "0"
                    {
                        "name"  "TestProton"
                    }
                }
            }
        }
    }
}"#,
    )
    .unwrap();

    let libraryfolders_vdf = steamapps.join("libraryfolders.vdf");
    std::fs::create_dir_all(&steamapps).unwrap();
    std::fs::write(
        &libraryfolders_vdf,
        format!(
            "\"libraryfolders\"\n{{\n    \"0\"\n    {{\n        \"path\"\t\"{}\"\n        \"apps\"\n        {{\n            \"71250\"\t\"0\"\n        }}\n    }}\n}}\n",
            steam_root.display()
        ),
    )
    .unwrap();

    std::fs::create_dir_all(proton_dir.join("files/bin")).unwrap();
    scripts::install_fake_wine(&proton_dir.join("files/bin/wine64"), &wine_log);

    std::fs::create_dir_all(&extract_root).unwrap();
    write_file(
        &extract_root.join("samodmanager/SAModManager.exe"),
        b"manager",
    );
    write_file(
        &extract_root.join("sadx-loader/SADXModLoader.dll"),
        b"loader",
    );
    write_file(
        &extract_root.join("steam-tools/patch_steam_inst.dat"),
        b"patch",
    );
    write_file(
        &extract_root.join("dreamcast-test/Dreamcast Test/mod.ini"),
        b"Name=Dreamcast Test",
    );
    write_file(
        &extract_root.join("dreamcast-test/Dreamcast Test/data.bin"),
        b"dc",
    );
    write_file(
        &extract_root.join("test-flat/Test Flat/mod.ini"),
        b"Name=Test Flat",
    );
    write_file(&extract_root.join("test-flat/Test Flat/asset.txt"), b"flat");

    std::fs::create_dir_all(&patched_root).unwrap();
    write_file(&patched_root.join("sonic.exe"), b"converted");
    write_file(&patched_root.join("system/CHRMODELS_orig.dll"), b"backup");

    std::fs::create_dir_all(&script_dir).unwrap();
    let fake_7zz = script_dir.join("fake-7zz");
    scripts::install_fake_7zz(&fake_7zz, &extract_root);
    let fake_hpatchz = script_dir.join("fake-hpatchz");
    scripts::install_fake_hpatchz(&fake_hpatchz, &patched_root);

    SteamFixture {
        _temp_dir: temp_dir,
        libraryfolders_vdf,
        game_path,
        prefix_path: compatdata.join("pfx"),
        wine_log,
        fake_7zz,
        fake_hpatchz,
        extract_root,
    }
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}
