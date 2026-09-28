use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use anyhow::{Result, anyhow};

use crate::steam::game::GameKind;

use super::common::{self, ModEntry, ModPreset};
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

/// Download the selected mods' archives into the cache, four at a time, so a
/// later install mostly extracts. Runs alongside the other setup work.
///
/// Failures are only logged: the install retries them and reports errors.
/// Stops early once `cancelled` is set.
pub fn prefetch_mod_archives(
    game_path: &Path,
    selected_mods: &[&ModEntry],
    cancelled: &AtomicBool,
) {
    let queue = Mutex::new(selected_mods.iter().collect::<VecDeque<_>>());
    thread::scope(|scope| {
        for _ in 0..selected_mods.len().min(MAX_CONCURRENT_MOD_INSTALLS) {
            scope.spawn(|| {
                while !cancelled.load(Ordering::Relaxed) {
                    let Some(mod_entry) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let mut stop_when_cancelled =
                        |_: u64, _: Option<u64>| ensure_not_cancelled(cancelled);
                    if let Err(err) = common::prefetch_mod_archive(
                        game_path,
                        mod_entry,
                        Some(&mut stop_when_cancelled),
                    ) {
                        tracing::info!("Prefetching '{}' failed: {err:#}", mod_entry.name);
                    }
                }
            });
        }
    });
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
    install_mods_and_generate_config(
        game_path,
        game_kind,
        selected_mods,
        width,
        height,
        language_selection,
        &mut progress,
    )
}

/// The body of [`install_selected_mods_and_generate_config_with_progress`],
/// compiled once rather than for every progress callback type.
fn install_mods_and_generate_config(
    game_path: &Path,
    game_kind: GameKind,
    selected_mods: &[&ModEntry],
    width: u32,
    height: u32,
    language_selection: config::LanguageSelection,
    progress: &mut dyn FnMut(InstallProgress<'_>) -> Result<()>,
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
                            ensure_not_cancelled(&cancelled)?;

                            // The receiver outlives every worker, so this
                            // cannot fail.
                            let _ = tx.send(WorkerMessage::DownloadingMod {
                                job_index,
                                downloaded,
                                total_bytes,
                            });

                            ensure_not_cancelled(&cancelled)
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

    if let Some(error) = callback_error {
        return Err(error);
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

/// Stop a download once the install was cancelled.
fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        anyhow::bail!("cancelled")
    }
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
    resolve_selected_mods_in(
        game_kind,
        common::recommended_mods_for_game(game_kind),
        common::presets_for_game(game_kind),
        preset_name,
        mod_names,
    )
}

/// [`resolve_selected_mods`] against a given mod catalog and its presets.
fn resolve_selected_mods_in(
    game_kind: GameKind,
    mods: &'static [ModEntry],
    presets: &[ModPreset],
    preset_name: Option<&str>,
    mod_names: &[&str],
) -> Result<Vec<&'static ModEntry>> {
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
        let preset = presets
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
#[path = "pipeline_tests.rs"]
mod tests;
