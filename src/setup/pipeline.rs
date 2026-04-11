use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use anyhow::{Result, anyhow};

use crate::steam::game::GameKind;

use super::common::{self, ModEntry};
use super::config;

pub enum InstallProgress<'a> {
    Started {
        mod_name: &'a str,
    },
    DownloadingMod {
        mod_name: &'a str,
        downloaded: u64,
        total_bytes: Option<u64>,
    },
    Finished {
        mod_name: &'a str,
        completed: usize,
        total: usize,
    },
    GeneratingConfig,
}

const MAX_CONCURRENT_MOD_INSTALLS: usize = 4;
const MAX_MOD_INSTALL_ATTEMPTS: usize = 3;

enum WorkerMessage {
    InstallingMod {
        job_index: usize,
    },
    DownloadingMod {
        job_index: usize,
        downloaded: u64,
        total_bytes: Option<u64>,
    },
    Completed {
        job_index: usize,
    },
    Failed {
        job_index: usize,
        error: String,
    },
}

pub fn install_selected_mods_and_generate_config_with_progress(
    game_path: &Path,
    game_kind: GameKind,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
    language_selection: config::LanguageSelection,
    mut progress: impl FnMut(InstallProgress<'_>) -> Result<()>,
) -> Result<()> {
    reject_duplicate_install_targets(selected_mods)?;

    let mod_total = selected_mods.len();
    if mod_total == 0 {
        progress(InstallProgress::GeneratingConfig)?;
        return config::generate_config(
            game_path,
            game_kind,
            selected_mods,
            width,
            height,
            language_selection,
        );
    }

    let worker_count = mod_total.min(MAX_CONCURRENT_MOD_INSTALLS);
    let queue = Arc::new(Mutex::new((0..mod_total).collect::<VecDeque<_>>()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<WorkerMessage>();
    let mut callback_error = None;
    let mut completed = 0;
    let mut failures = vec![None; mod_total];

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = queue.clone();
            let cancelled = cancelled.clone();
            let tx = tx.clone();

            scope.spawn(move || {
                loop {
                    if cancelled.load(Ordering::Relaxed) {
                        break;
                    }

                    let Some(job_index) = queue.lock().unwrap().pop_front() else {
                        break;
                    };

                    let mod_entry = selected_mods[job_index];
                    let _ = tx.send(WorkerMessage::InstallingMod { job_index });

                    let mut attempt = 0;
                    let result = loop {
                        let mut download_progress = |downloaded: u64, total_bytes: Option<u64>| {
                            if cancelled.load(Ordering::Relaxed) {
                                anyhow::bail!("cancelled")
                            }

                            tx.send(WorkerMessage::DownloadingMod {
                                job_index,
                                downloaded,
                                total_bytes,
                            })
                            .map_err(|_| anyhow!("cancelled"))?;

                            if cancelled.load(Ordering::Relaxed) {
                                anyhow::bail!("cancelled")
                            }

                            Ok(())
                        };

                        let result = common::install_mod_with_progress(
                            game_path,
                            mod_entry,
                            Some(&mut download_progress),
                        );

                        attempt += 1;
                        match result {
                            Ok(()) => break Ok(()),
                            Err(e) if attempt < MAX_MOD_INSTALL_ATTEMPTS => {
                                if cancelled.load(Ordering::Relaxed) {
                                    break Err(e);
                                }
                                // brief pause before retry
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            Err(e) => break Err(e),
                        }
                    };

                    match result {
                        Ok(()) => {
                            let _ = tx.send(WorkerMessage::Completed { job_index });
                        }
                        Err(error) => {
                            let _ = tx.send(WorkerMessage::Failed {
                                job_index,
                                error: error.to_string(),
                            });
                        }
                    }
                }
            });
        }

        drop(tx);

        while let Ok(message) = rx.recv() {
            match message {
                WorkerMessage::InstallingMod { job_index } => {
                    if callback_error.is_none()
                        && let Err(error) = progress(InstallProgress::Started {
                            mod_name: selected_mods[job_index].name,
                        })
                    {
                        cancelled.store(true, Ordering::Relaxed);
                        callback_error = Some(error);
                    }
                }
                WorkerMessage::DownloadingMod {
                    job_index,
                    downloaded,
                    total_bytes,
                } => {
                    if callback_error.is_none()
                        && let Err(error) = progress(InstallProgress::DownloadingMod {
                            mod_name: selected_mods[job_index].name,
                            downloaded,
                            total_bytes,
                        })
                    {
                        cancelled.store(true, Ordering::Relaxed);
                        callback_error = Some(error);
                    }
                }
                WorkerMessage::Completed { job_index } => {
                    completed += 1;
                    if callback_error.is_none()
                        && let Err(error) = progress(InstallProgress::Finished {
                            mod_name: selected_mods[job_index].name,
                            completed,
                            total: mod_total,
                        })
                    {
                        cancelled.store(true, Ordering::Relaxed);
                        callback_error = Some(error);
                    }
                }
                WorkerMessage::Failed { job_index, error } => {
                    failures[job_index] = Some(error);
                    cancelled.store(true, Ordering::Relaxed);
                }
            }
        }
    });

    if let Some(error) = callback_error {
        return Err(error);
    }

    let failed_mods: Vec<String> = failures
        .into_iter()
        .enumerate()
        .filter_map(|(index, error)| {
            error.map(|error| format!("{}: {error}", selected_mods[index].name))
        })
        .collect();

    if !failed_mods.is_empty() {
        return Err(anyhow!(
            "Failed to install mods: {}",
            failed_mods.join("; ")
        ));
    }

    progress(InstallProgress::GeneratingConfig)?;
    config::generate_config(
        game_path,
        game_kind,
        selected_mods,
        width,
        height,
        language_selection,
    )?;

    Ok(())
}

fn reject_duplicate_install_targets(selected_mods: &[&ModEntry]) -> Result<()> {
    let mut seen = HashSet::new();

    for mod_entry in selected_mods {
        let target = mod_entry.dir_name.unwrap_or(mod_entry.name);
        if !seen.insert(target) {
            return Err(anyhow!("Duplicate mod install target '{target}' requested"));
        }
    }

    Ok(())
}

pub fn resolve_selected_mods(
    game_kind: GameKind,
    preset_name: Option<&str>,
    mod_names: &[&str],
) -> Result<Vec<&'static ModEntry>> {
    let mods = common::recommended_mods_for_game(game_kind);

    if !mod_names.is_empty() {
        return mod_names
            .iter()
            .map(|name| {
                mods.iter()
                    .find(|entry| entry.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| anyhow!("Unknown mod '{}' for {}", name, game_kind.name()))
            })
            .collect();
    }

    if let Some(preset_name) = preset_name {
        let preset = common::presets_for_game(game_kind)
            .iter()
            .find(|preset| preset.name.eq_ignore_ascii_case(preset_name))
            .ok_or_else(|| anyhow!("Unknown preset '{}' for {}", preset_name, game_kind.name()))?;

        return preset
            .mod_names
            .iter()
            .map(|name| {
                mods.iter()
                    .find(|entry| entry.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| {
                        anyhow!(
                            "Preset '{}' references unknown mod '{}' for {}",
                            preset_name,
                            name,
                            game_kind.name()
                        )
                    })
            })
            .collect();
    }

    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::resolve_selected_mods;
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
        let error = match resolve_selected_mods(GameKind::SA2, None, &["Not Real"]) {
            Ok(_) => panic!("expected unknown mod to fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("Unknown mod"));
    }

    #[test]
    fn preserves_duplicate_named_mods() {
        let selected =
            resolve_selected_mods(GameKind::SA2, None, &["SA2 Render Fix", "SA2 Render Fix"])
                .unwrap();

        let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["SA2 Render Fix", "SA2 Render Fix"]);
    }
}
