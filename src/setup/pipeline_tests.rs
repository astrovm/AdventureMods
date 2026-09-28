use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use super::*;
use crate::external::test_http::{Reply, serve};
use crate::setup::common::ModSource;
use crate::steam::game::GameKind;

#[test]
fn resolves_named_mods_in_requested_order() {
    let selected = resolve_selected_mods(
        GameKind::SA2,
        None,
        &["HD GUI: SA2 Edition", "SA2 Render Fix"],
    )
    .unwrap();

    let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();
    assert_eq!(names, vec!["HD GUI: SA2 Edition", "SA2 Render Fix"]);
}

#[test]
fn resolves_preset_when_no_explicit_mods_are_given() {
    let selected =
        resolve_selected_mods(GameKind::SADX, Some("Dreamcast Restoration"), &[]).unwrap();

    assert!(
        selected
            .iter()
            .any(|entry| entry.name == "Dreamcast Characters Pack")
    );
    assert!(
        !selected
            .iter()
            .any(|entry| entry.name == "DX Characters Refined")
    );
}

#[test]
fn rejects_unknown_mod_names() {
    let error = resolve_selected_mods(GameKind::SA2, None, &["Not Real"])
        .err()
        .expect("unknown mod should fail");

    assert_eq!(
        error.to_string(),
        "Unknown mod 'Not Real' for Sonic Adventure 2"
    );
}

#[test]
fn rejects_unknown_presets() {
    // SA2 has no presets at all.
    let error = resolve_selected_mods(GameKind::SA2, Some("Dreamcast Restoration"), &[])
        .err()
        .expect("unknown preset should fail");

    assert_eq!(
        error.to_string(),
        "Unknown preset 'Dreamcast Restoration' for Sonic Adventure 2"
    );
}

#[test]
fn nothing_is_selected_without_mods_or_a_preset() {
    assert!(
        resolve_selected_mods(GameKind::SADX, None, &[])
            .unwrap()
            .is_empty()
    );
}

static CATALOG: [ModEntry; 1] = [ModEntry {
    name: "Listed Mod",
    slug: "listed-mod",
    source: ModSource::DirectUrl {
        url: "https://example.com/listed.7z",
    },
    description: "in the catalog",
    full_description: None,
    pictures: &[],
    dir_name: Some("ListedMod"),
    links: &[],
}];

#[test]
fn presets_resolve_against_the_catalog_and_report_missing_mods() {
    let presets = [
        ModPreset {
            name: "Complete",
            description: "only listed mods",
            mod_names: &["listed mod"],
        },
        ModPreset {
            name: "Stale",
            description: "names a mod the catalog dropped",
            mod_names: &["Listed Mod", "Dropped Mod"],
        },
    ];

    let complete =
        resolve_selected_mods_in(GameKind::SADX, &CATALOG, &presets, Some("complete"), &[])
            .unwrap();
    assert_eq!(complete.len(), 1);
    assert_eq!(complete[0].slug, "listed-mod");

    let error = resolve_selected_mods_in(GameKind::SADX, &CATALOG, &presets, Some("Stale"), &[])
        .err()
        .expect("a preset naming a missing mod should fail");
    assert_eq!(
        error.to_string(),
        "Preset 'Stale' references unknown mod 'Dropped Mod' for Sonic Adventure DX"
    );
}

#[test]
fn resolve_selected_mods_preserves_duplicate_entries() {
    let selected =
        resolve_selected_mods(GameKind::SA2, None, &["SA2 Render Fix", "SA2 Render Fix"]).unwrap();

    let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();
    assert_eq!(names, vec!["SA2 Render Fix", "SA2 Render Fix"]);
}

#[test]
fn reject_duplicate_install_targets_errors_on_duplicates() {
    let entry = ModEntry {
        name: "SA2 Render Fix",
        slug: "sa2-render-fix",
        dir_name: Some("sa2-render-fix"),
        source: ModSource::DirectUrl {
            url: "https://example.com/mod.zip",
        },
        description: "test",
        full_description: None,
        pictures: &[],
        links: &[],
    };
    let selected = vec![&entry, &entry];
    let result = reject_duplicate_install_targets(&selected);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Duplicate mod install target")
    );
}

/// Run the SA2 install with `progress`.
fn install(
    game_path: &Path,
    selected: &[&ModEntry],
    progress: impl FnMut(InstallProgress<'_>) -> Result<()>,
) -> Result<()> {
    install_selected_mods_and_generate_config_with_progress(
        game_path,
        GameKind::SA2,
        selected,
        1280,
        720,
        config::LanguageSelection::defaults_for(GameKind::SA2),
        progress,
    )
}

/// The file SA2 config generation writes first.
fn manager_json(game_path: &Path) -> PathBuf {
    game_path.join("SAManager/Manager.json")
}

/// A short name for each progress event.
fn event_name(progress: &InstallProgress<'_>) -> &'static str {
    match progress {
        InstallProgress::Started { .. } => "started",
        InstallProgress::DownloadingMod { .. } => "download",
        InstallProgress::Finished { .. } => "finished",
        InstallProgress::GeneratingConfig => "config",
    }
}

#[test]
fn empty_selection_generates_config_and_reports_progress() {
    let dir = tempfile::tempdir().unwrap();
    let mut events = Vec::new();

    install(dir.path(), &[], |progress| {
        events.push(event_name(&progress));
        Ok(())
    })
    .unwrap();

    assert_eq!(events, vec!["config"]);
    assert!(manager_json(dir.path()).is_file());
}

#[test]
fn empty_selection_stops_when_the_config_progress_callback_fails() {
    let dir = tempfile::tempdir().unwrap();

    let error = install(dir.path(), &[], |_| Err(anyhow!("stop before config"))).unwrap_err();

    assert_eq!(error.to_string(), "stop before config");
    assert!(!manager_json(dir.path()).exists());
}

fn complete_mod_entry() -> ModEntry {
    ModEntry {
        name: "Ready Mod",
        slug: "ready-mod",
        dir_name: Some("ReadyMod"),
        source: ModSource::DirectUrl {
            url: "http://127.0.0.1:9/unused.zip",
        },
        description: "preinstalled synthetic mod",
        full_description: None,
        pictures: &[],
        links: &[],
    }
}

/// A game folder with [`complete_mod_entry`] already installed.
fn game_with_ready_mod() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mod_dir = dir.path().join("mods/ReadyMod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(mod_dir.join("mod.ini"), b"Name=Ready Mod\n").unwrap();
    dir
}

#[test]
fn preinstalled_mod_reports_started_finished_and_config_events() {
    let dir = game_with_ready_mod();
    let entry = complete_mod_entry();
    let mut events = Vec::new();

    install(dir.path(), &[&entry], |progress| {
        events.push(event_name(&progress));
        Ok(())
    })
    .unwrap();

    assert_eq!(events, vec!["started", "finished", "config"]);
}

#[test]
fn progress_callback_errors_cancel_started_and_finished_events() {
    let dir = game_with_ready_mod();
    let entry = complete_mod_entry();

    let started_error = install(dir.path(), &[&entry], |progress| {
        if event_name(&progress) == "started" {
            anyhow::bail!("stop at start");
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(started_error.to_string(), "stop at start");

    let finished_error = install(dir.path(), &[&entry], |progress| {
        if event_name(&progress) == "finished" {
            anyhow::bail!("stop at finish");
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(finished_error.to_string(), "stop at finish");
    assert!(!manager_json(dir.path()).exists());
}

#[test]
fn config_generation_errors_are_reported_after_the_mods_install() {
    let dir = game_with_ready_mod();
    // The manager settings folder cannot be created over a file.
    std::fs::write(dir.path().join("SAManager"), "not a folder").unwrap();
    let entry = complete_mod_entry();
    let mut events = Vec::new();

    let result = install(dir.path(), &[&entry], |progress| {
        events.push(event_name(&progress));
        Ok(())
    });

    assert!(result.is_err());
    assert_eq!(events, vec!["started", "finished", "config"]);
}

/// Downloads served by a local server and extracted by a fake 7zz, with the
/// download cache in a scratch folder. Holds the environment lock.
struct DownloadEnv {
    dir: tempfile::TempDir,
    _lock: MutexGuard<'static, ()>,
}

impl DownloadEnv {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;

        let _ = rustls::crypto::ring::default_provider().install_default();
        let lock = crate::test_env::lock();
        let dir = tempfile::tempdir().unwrap();
        // Puts the archive's text into a mod.ini inside a mod folder.
        let fake_7zz = dir.path().join("fake-7zz");
        std::fs::write(
            &fake_7zz,
            r##"#!/bin/sh
for arg in "$@"; do
    case "$arg" in
        -o*) dest="${arg#-o}" ;;
        x|-y) ;;
        *) archive="$arg" ;;
    esac
done
mkdir -p "$dest/Mod"
cp "$archive" "$dest/Mod/mod.ini"
"##,
        )
        .unwrap();
        std::fs::set_permissions(&fake_7zz, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::create_dir_all(dir.path().join("game")).unwrap();
        unsafe {
            std::env::set_var("ADVENTURE_MODS_7ZZ", &fake_7zz);
            std::env::set_var("ADVENTURE_MODS_CACHE_DIR", dir.path().join("cache"));
        }
        Self { dir, _lock: lock }
    }

    fn game(&self) -> PathBuf {
        self.dir.path().join("game")
    }

    /// Complete archives in the download cache.
    fn cached_archives(&self) -> usize {
        std::fs::read_dir(self.dir.path().join("cache/downloads"))
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "archive"))
                    .count()
            })
            .unwrap_or(0)
    }
}

impl Drop for DownloadEnv {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("ADVENTURE_MODS_7ZZ");
            std::env::remove_var("ADVENTURE_MODS_CACHE_DIR");
        }
    }
}

fn served_mod(name: &'static str, dir_name: &'static str, url: String) -> ModEntry {
    ModEntry {
        name,
        slug: dir_name,
        dir_name: Some(dir_name),
        source: ModSource::DirectUrl {
            url: Box::leak(url.into_boxed_str()),
        },
        description: "served by the test server",
        full_description: None,
        pictures: &[],
        links: &[],
    }
}

fn gets(log: &Mutex<Vec<String>>) -> usize {
    log.lock()
        .unwrap()
        .iter()
        .filter(|request| request.starts_with("GET"))
        .count()
}

#[test]
fn downloaded_mods_report_their_download_progress() {
    let env = DownloadEnv::new();
    let body = "Name=Downloaded Mod";
    let (base, _) = serve(move |_| Reply::ok(body));
    let entry = served_mod("Downloaded Mod", "DownloadedMod", format!("{base}/mod.7z"));
    let mut downloads = Vec::new();
    let mut events = Vec::new();

    install(&env.game(), &[&entry], |progress| {
        if let InstallProgress::DownloadingMod {
            mod_name,
            downloaded,
            total_bytes,
        } = progress
        {
            downloads.push((mod_name.to_owned(), downloaded, total_bytes));
        }
        events.push(event_name(&progress));
        Ok(())
    })
    .unwrap();

    let total = body.len() as u64;
    assert_eq!(
        downloads.last(),
        Some(&("Downloaded Mod".to_owned(), total, Some(total)))
    );
    assert_eq!(events.first(), Some(&"started"));
    assert_eq!(events[events.len() - 2..], ["finished", "config"]);
    assert_eq!(
        std::fs::read_to_string(env.game().join("mods/DownloadedMod/mod.ini")).unwrap(),
        body
    );
}

#[test]
fn download_progress_callback_errors_stop_the_install() {
    let env = DownloadEnv::new();
    let (base, _) = serve(|_| Reply::ok("Name=Stopped Mod"));
    let entry = served_mod("Stopped Mod", "StoppedMod", format!("{base}/mod.7z"));

    let result = install(&env.game(), &[&entry], |progress| {
        if event_name(&progress) == "download" {
            anyhow::bail!("stop at download");
        }
        Ok(())
    });

    // Whether the download noticed in time or finished first, setup stops
    // before writing the config.
    assert!(result.is_err());
    assert!(!manager_json(&env.game()).exists());
}

#[test]
fn failing_mods_are_retried_then_reported_by_name() {
    let env = DownloadEnv::new();
    let (base, log) = serve(|_| Reply::ok("gone").status("500 Internal Server Error"));
    let entry = served_mod("Broken Mod", "BrokenMod", format!("{base}/broken.7z"));
    let mut events = Vec::new();

    let error = install(&env.game(), &[&entry], |progress| {
        events.push(event_name(&progress));
        Ok(())
    })
    .unwrap_err();

    assert!(
        error
            .to_string()
            .starts_with("Failed to install mods: Broken Mod: "),
        "{error}"
    );
    assert_eq!(gets(&log), MAX_MOD_INSTALL_ATTEMPTS);
    assert_eq!(events, vec!["started"]);
    assert!(!manager_json(&env.game()).exists());
}

#[test]
fn cancelling_during_a_download_stops_without_retrying() {
    let env = DownloadEnv::new();
    // The server answers slowly, so the cancellation lands mid-download.
    let (base, log) = serve(|_| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        Reply::ok("Name=Slow Mod")
    });
    let entry = served_mod("Slow Mod", "SlowMod", format!("{base}/slow.7z"));

    let error = install(&env.game(), &[&entry], |progress| {
        if event_name(&progress) == "started" {
            anyhow::bail!("user cancelled");
        }
        Ok(())
    })
    .unwrap_err();

    assert!(
        error
            .to_string()
            .starts_with("Failed to install mods: Slow Mod"),
        "{error}"
    );
    assert_eq!(gets(&log), 1);
    assert!(!env.game().join("mods/SlowMod").exists());
    assert!(!manager_json(&env.game()).exists());
}

#[test]
fn prefetch_stops_downloading_once_cancelled() {
    let env = DownloadEnv::new();
    let cancelled = Arc::new(AtomicBool::new(false));
    // The user cancels while the archive is being served.
    let (base, log) = serve({
        let cancelled = cancelled.clone();
        move |_| {
            cancelled.store(true, Ordering::SeqCst);
            Reply::ok("Name=Prefetch Mod")
        }
    });
    let entry = served_mod("Prefetch Mod", "PrefetchMod", format!("{base}/prefetch.7z"));

    prefetch_mod_archives(&env.game(), &[&entry], &cancelled);

    assert_eq!(gets(&log), 1);
    assert_eq!(env.cached_archives(), 0);
}
