//! Setup progress: messages from install workers, the bar's text and
//! fraction, and the download size of the chosen mods.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::setup::{common, config, pipeline};

pub(crate) enum ProgressMsg {
    /// Wakes the UI so it can read the latest byte samples. Safe to drop when full.
    Refresh,
    Bytes {
        downloaded: u64,
        total: Option<u64>,
        status: String,
    },
    ModInstall {
        mod_name: String,
        total: usize,
    },
    ModFinished {
        mod_name: String,
        completed: usize,
        total: usize,
    },
    Configuring {
        total: usize,
    },
}

/// Latest high-frequency byte samples. Download workers overwrite these without blocking;
/// the UI merges them each frame so a full progress channel cannot leave stale per-mod totals.
#[derive(Default)]
pub(crate) struct ProgressSamples {
    step: Mutex<Option<(u64, Option<u64>, String)>>,
    mods: Mutex<HashMap<String, (u64, Option<u64>)>>,
    mod_total: AtomicUsize,
}

impl ProgressSamples {
    pub(crate) fn set_step_bytes(
        &self,
        downloaded: u64,
        total: Option<u64>,
        status: impl Into<String>,
    ) {
        *self.step.lock().expect("progress samples lock") =
            Some((downloaded, total, status.into()));
    }

    pub(crate) fn set_mod_bytes(
        &self,
        mod_name: impl Into<String>,
        downloaded: u64,
        total_bytes: Option<u64>,
        total_mods: usize,
    ) {
        self.mod_total.store(total_mods, Ordering::Relaxed);
        self.mods
            .lock()
            .expect("progress samples lock")
            .insert(mod_name.into(), (downloaded, total_bytes));
    }

    pub(crate) fn clear_mod(&self, mod_name: &str) {
        self.mods
            .lock()
            .expect("progress samples lock")
            .remove(mod_name);
    }

    pub(crate) fn clear(&self) {
        *self.step.lock().expect("progress samples lock") = None;
        self.mods.lock().expect("progress samples lock").clear();
        self.mod_total.store(0, Ordering::Relaxed);
    }

    pub(crate) fn apply_to(&self, state: &mut ProgressState) {
        if let Some((downloaded, total, status)) =
            self.step.lock().expect("progress samples lock").clone()
        {
            state.apply(ProgressMsg::Bytes {
                downloaded,
                total,
                status,
            });
        }

        let mods = self.mods.lock().expect("progress samples lock").clone();
        if mods.is_empty() {
            return;
        }

        let total_mods = self.mod_total.load(Ordering::Relaxed);
        state.active_downloads = mods;
        let (downloaded, total_bytes) = state.active_downloads.values().fold(
            (0u64, Some(0u64)),
            |(downloaded, total), (item_downloaded, item_total)| {
                let total = match (total, item_total) {
                    (Some(total), Some(item_total)) => Some(total + item_total),
                    _ => None,
                };
                (downloaded + item_downloaded, total)
            },
        );
        let update =
            mod_download_progress_update(state.completed_mods, total_mods, downloaded, total_bytes);
        state.display = Some(ProgressDisplay {
            fraction: Some(update.fraction),
            pulse: update.pulse,
            text: update.text,
        });
    }
}

pub(crate) fn wake_progress_ui(tx: &async_channel::Sender<ProgressMsg>) {
    let _ = tx.try_send(ProgressMsg::Refresh);
}

pub(crate) fn publish_step_bytes(
    tx: &async_channel::Sender<ProgressMsg>,
    samples: &ProgressSamples,
    downloaded: u64,
    total: Option<u64>,
    status: &str,
) {
    samples.set_step_bytes(downloaded, total, status);
    wake_progress_ui(tx);
}

pub(crate) fn publish_mod_bytes(
    tx: &async_channel::Sender<ProgressMsg>,
    samples: &ProgressSamples,
    mod_name: &str,
    total_mods: usize,
    downloaded: u64,
    total_bytes: Option<u64>,
) {
    samples.set_mod_bytes(mod_name, downloaded, total_bytes, total_mods);
    wake_progress_ui(tx);
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProgressDisplay {
    pub(crate) fraction: Option<f64>,
    pub(crate) pulse: bool,
    pub(crate) text: String,
}

#[derive(Default)]
pub(crate) struct ProgressState {
    pub(crate) completed_mods: usize,
    pub(crate) active_downloads: HashMap<String, (u64, Option<u64>)>,
    pub(crate) display: Option<ProgressDisplay>,
}

impl ProgressState {
    pub(crate) fn apply(&mut self, msg: ProgressMsg) {
        let display = match msg {
            ProgressMsg::Refresh => return,
            ProgressMsg::Bytes {
                downloaded,
                total,
                status,
            } => ProgressDisplay {
                fraction: total
                    .filter(|total| *total > 0)
                    .map(|total| downloaded as f64 / total as f64),
                pulse: total.is_none(),
                text: format_step_download_text(&status, downloaded, total),
            },
            ProgressMsg::ModInstall { mod_name, total } => ProgressDisplay {
                fraction: Some(completed_mod_fraction(self.completed_mods, total)),
                pulse: false,
                text: mod_download_start_text(&mod_name),
            },
            ProgressMsg::ModFinished {
                mod_name,
                completed,
                total,
            } => {
                self.completed_mods = completed;
                self.active_downloads.remove(&mod_name);
                ProgressDisplay {
                    fraction: Some(completed_mod_fraction(completed, total)),
                    pulse: false,
                    text: mod_download_finished_text(&mod_name, completed, total),
                }
            }
            ProgressMsg::Configuring { total } => {
                self.active_downloads.clear();
                ProgressDisplay {
                    fraction: Some(1.0),
                    pulse: false,
                    text: format!("Generating config... ({total}/{total})"),
                }
            }
        };
        self.display = Some(display);
    }
}

pub(crate) fn drain_progress_updates(
    receiver: &async_channel::Receiver<ProgressMsg>,
    state: &mut ProgressState,
) {
    while let Ok(msg) = receiver.try_recv() {
        state.apply(msg);
    }
}

pub(crate) fn initial_preview_index(mod_count: usize, selected_mods: &[usize]) -> Option<usize> {
    selected_mods
        .iter()
        .copied()
        .find(|&idx| idx < mod_count)
        .or_else(|| (mod_count > 0).then_some(0))
}

pub(crate) fn subtitle_language_labels(
    game_kind: crate::steam::game::GameKind,
) -> Vec<&'static str> {
    config::SubtitleLanguage::supported_for(game_kind)
        .iter()
        .map(|language| language.label())
        .collect()
}

pub(crate) fn voice_language_labels() -> Vec<&'static str> {
    config::VoiceLanguage::all()
        .iter()
        .map(|language| language.label())
        .collect()
}

pub(crate) fn completed_mod_fraction(completed: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }

    completed as f64 / total as f64
}

pub(crate) fn format_mb_value(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
}

pub(crate) fn format_mb(bytes: u64) -> String {
    format!("{} MB", format_mb_value(bytes))
}

pub(crate) fn mod_download_fraction(
    completed: usize,
    total: usize,
    downloaded: u64,
    total_bytes: Option<u64>,
) -> f64 {
    let completed = completed_mod_fraction(completed, total);
    let Some(total_bytes) = total_bytes else {
        return completed;
    };
    if total == 0 || total_bytes == 0 {
        return completed;
    }

    (completed + (downloaded as f64 / total_bytes as f64 / total as f64)).min(1.0)
}

pub(crate) fn format_step_download_text(
    status: &str,
    downloaded: u64,
    total: Option<u64>,
) -> String {
    let bytes_text = format_download_bytes_text(downloaded, total);

    if status.is_empty() {
        bytes_text
    } else {
        format!("{status} - {bytes_text}")
    }
}

pub(crate) fn mod_download_start_text(mod_name: &str) -> String {
    format!("Starting {mod_name}...")
}

pub(crate) fn mod_download_finished_text(mod_name: &str, completed: usize, total: usize) -> String {
    format!("Installed {mod_name} ({completed}/{total})")
}

pub(crate) fn format_download_bytes_text(downloaded: u64, total: Option<u64>) -> String {
    if let Some(total) = total {
        format!(
            "{} / {} MB",
            format_mb_value(downloaded),
            format_mb_value(total)
        )
    } else {
        format_mb(downloaded)
    }
}

pub(crate) struct ModDownloadProgressUpdate {
    fraction: f64,
    pulse: bool,
    text: String,
}

pub(crate) fn mod_download_progress_update(
    completed: usize,
    total: usize,
    downloaded: u64,
    total_bytes: Option<u64>,
) -> ModDownloadProgressUpdate {
    let bytes_text = format_download_bytes_text(downloaded, total_bytes);

    ModDownloadProgressUpdate {
        fraction: mod_download_fraction(completed, total, downloaded, total_bytes),
        pulse: total_bytes.is_none(),
        text: format!("Downloading mods - {bytes_text}"),
    }
}

pub(crate) fn subtitle_language_index(
    game_kind: crate::steam::game::GameKind,
    language: config::SubtitleLanguage,
) -> u32 {
    config::SubtitleLanguage::supported_for(game_kind)
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0) as u32
}

pub(crate) fn voice_language_index(language: config::VoiceLanguage) -> u32 {
    config::VoiceLanguage::all()
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0) as u32
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModDownloadEstimate {
    pub(crate) size: Option<u64>,
    pub(crate) installed: bool,
}

/// Look up every mod's download size (in parallel) and whether it is installed.
pub(crate) fn estimate_mod_downloads(
    game_path: &std::path::Path,
    mods: &[common::ModEntry],
    size_of: impl Fn(&common::ModEntry) -> Option<u64> + Sync,
) -> Vec<ModDownloadEstimate> {
    const WORKERS: usize = 4;
    let next = AtomicUsize::new(0);
    let estimates = Mutex::new(vec![ModDownloadEstimate::default(); mods.len()]);

    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(mods.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(mod_entry) = mods.get(index) else {
                        break;
                    };
                    let installed = common::is_mod_installed(game_path, mod_entry);
                    let size = if installed { None } else { size_of(mod_entry) };
                    estimates.lock().expect("download estimates lock")[index] =
                        ModDownloadEstimate { size, installed };
                }
            });
        }
    });

    estimates.into_inner().expect("download estimates lock")
}

pub(crate) fn remote_mod_download_size(mod_entry: &common::ModEntry) -> Option<u64> {
    common::mod_download_size(mod_entry).unwrap_or_else(|err| {
        tracing::debug!("No download size for {}: {err:#}", mod_entry.name);
        None
    })
}

/// The monitor resolution, or 1080p when no monitor reports one.
pub(crate) fn resolution_or_fallback(resolution: Option<(u32, u32)>) -> (u32, u32) {
    resolution.unwrap_or_else(|| {
        tracing::warn!("Could not detect monitor resolution, using fallback 1920x1080");
        (1920, 1080)
    })
}

pub(crate) fn format_download_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else {
        format!("{} MB", (bytes as f64 / 1_000_000.0).ceil().max(1.0) as u64)
    }
}

/// The line under the mod list saying how much the selection will download.
pub(crate) fn download_size_text(
    estimates: Option<&[ModDownloadEstimate]>,
    selected: &[usize],
) -> String {
    if selected.is_empty() {
        return "No mods selected".to_owned();
    }
    let Some(estimates) = estimates else {
        return "Checking download size…".to_owned();
    };

    let chosen: Vec<_> = selected
        .iter()
        .filter_map(|&index| estimates.get(index))
        .collect();
    let installed = chosen.iter().filter(|estimate| estimate.installed).count();
    let to_download: Vec<_> = chosen
        .iter()
        .filter(|estimate| !estimate.installed)
        .collect();
    let known: u64 = to_download
        .iter()
        .filter_map(|estimate| estimate.size)
        .sum();
    let unknown = to_download.iter().any(|estimate| estimate.size.is_none());

    let installed_note = match installed {
        0 => String::new(),
        1 => ", 1 already installed".to_owned(),
        n => format!(", {n} already installed"),
    };

    if to_download.is_empty() {
        "Everything selected is already installed".to_owned()
    } else if known == 0 {
        format!("Download size unknown{installed_note}")
    } else if unknown {
        format!(
            "At least {} to download{installed_note}",
            format_download_size(known)
        )
    } else {
        format!(
            "{} to download{installed_note}",
            format_download_size(known)
        )
    }
}

pub(crate) fn apply_install_progress(
    progress: pipeline::InstallProgress<'_>,
    cancel_flag: &AtomicBool,
    tx: &async_channel::Sender<ProgressMsg>,
    samples: &ProgressSamples,
    total_count: usize,
) -> anyhow::Result<()> {
    match progress {
        pipeline::InstallProgress::Started { mod_name } => {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(anyhow::anyhow!("cancelled"));
            }
            let _ = tx.send_blocking(ProgressMsg::ModInstall {
                mod_name: mod_name.to_string(),
                total: total_count,
            });
        }
        pipeline::InstallProgress::DownloadingMod {
            mod_name,
            downloaded,
            total_bytes,
        } => {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(anyhow::anyhow!("cancelled"));
            }
            publish_mod_bytes(tx, samples, mod_name, total_count, downloaded, total_bytes);
        }
        pipeline::InstallProgress::Finished {
            mod_name,
            completed,
            total,
        } => {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(anyhow::anyhow!("cancelled"));
            }
            samples.clear_mod(mod_name);
            let _ = tx.send_blocking(ProgressMsg::ModFinished {
                mod_name: mod_name.to_string(),
                completed,
                total,
            });
        }
        pipeline::InstallProgress::GeneratingConfig => {
            samples.clear();
            let _ = tx.send_blocking(ProgressMsg::Configuring { total: total_count });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::config::{SubtitleLanguage, VoiceLanguage};
    use crate::steam::game::GameKind;
    use crate::test_log::LogCapture;

    #[test]
    fn initial_preview_prefers_first_selected_mod() {
        assert_eq!(initial_preview_index(5, &[3, 1]), Some(3));
    }

    #[test]
    fn initial_preview_falls_back_to_first_mod_when_none_selected() {
        assert_eq!(initial_preview_index(5, &[]), Some(0));
    }

    #[test]
    fn initial_preview_skips_out_of_range_selection() {
        assert_eq!(initial_preview_index(2, &[4, 1]), Some(1));
    }

    #[test]
    fn initial_preview_is_none_when_no_mods_exist() {
        assert_eq!(initial_preview_index(0, &[0]), None);
    }

    #[test]
    fn initial_preview_is_none_when_no_mods_and_no_selection() {
        assert_eq!(initial_preview_index(0, &[]), None);
    }

    #[test]
    fn initial_preview_falls_back_to_zero_when_all_selections_out_of_range() {
        // All selected indices are out of range, but mods exist
        assert_eq!(initial_preview_index(3, &[5, 10, 99]), Some(0));
    }

    #[test]
    fn sadx_subtitle_options_include_expected_values() {
        assert_eq!(
            subtitle_language_labels(GameKind::SADX),
            vec!["日本語", "English", "Français", "Español", "Deutsch"]
        );
    }

    #[test]
    fn sa2_subtitle_options_include_expected_values() {
        assert_eq!(
            subtitle_language_labels(GameKind::SA2),
            vec![
                "English",
                "Deutsch",
                "Español",
                "Français",
                "Italiano",
                "日本語"
            ]
        );
    }

    #[test]
    fn voice_options_include_expected_values() {
        assert_eq!(voice_language_labels(), vec!["日本語", "English"]);
    }

    #[test]
    fn mod_download_fraction_starts_at_completed_items() {
        assert_eq!(mod_download_fraction(0, 2, 0, Some(100)), 0.0);
        assert_eq!(mod_download_fraction(1, 2, 0, Some(100)), 0.5);
    }

    #[test]
    fn mod_download_fraction_reaches_completion_at_end_of_current_item() {
        assert_eq!(mod_download_fraction(1, 2, 100, Some(100)), 1.0);
    }

    #[test]
    fn mod_download_progress_pulses_when_total_size_is_unknown() {
        let update = mod_download_progress_update(1, 4, 1_048_576, None);

        assert_eq!(update.fraction, completed_mod_fraction(1, 4));
        assert!(update.pulse);
        assert_eq!(update.text, "Downloading mods - 1.0 MB");
    }

    #[test]
    fn mod_download_progress_uses_generic_label_for_aggregate_bytes() {
        let update = mod_download_progress_update(1, 4, 1_048_576, Some(2_097_152));

        assert_eq!(update.text, "Downloading mods - 1.0 / 2.0 MB");
    }

    #[test]
    fn draining_progress_updates_keeps_the_latest_value() {
        let (sender, receiver) = async_channel::bounded(4);
        sender
            .try_send(ProgressMsg::Bytes {
                downloaded: 1_048_576,
                total: Some(4_194_304),
                status: "Downloading...".to_string(),
            })
            .unwrap();
        sender
            .try_send(ProgressMsg::Bytes {
                downloaded: 2_097_152,
                total: Some(4_194_304),
                status: "Downloading...".to_string(),
            })
            .unwrap();

        let mut state = ProgressState::default();
        drain_progress_updates(&receiver, &mut state);

        let display = state.display.unwrap();
        assert_eq!(display.fraction, Some(0.5));
        assert_eq!(display.text, "Downloading... - 2.0 / 4.0 MB");
    }

    #[test]
    fn progress_samples_keep_latest_bytes_per_mod() {
        let samples = ProgressSamples::default();
        samples.set_mod_bytes("Alpha", 1_048_576, Some(2_097_152), 2);
        samples.set_mod_bytes("Beta", 524_288, Some(2_097_152), 2);
        samples.set_mod_bytes("Alpha", 1_572_864, Some(2_097_152), 2);

        let mut state = ProgressState::default();
        samples.apply_to(&mut state);

        assert_eq!(
            state.active_downloads.get("Alpha"),
            Some(&(1_572_864, Some(2_097_152)))
        );
        assert_eq!(
            state.active_downloads.get("Beta"),
            Some(&(524_288, Some(2_097_152)))
        );
        let display = state.display.unwrap();
        assert_eq!(display.text, "Downloading mods - 2.0 / 4.0 MB");
        assert!(!display.pulse);
    }

    #[test]
    fn progress_samples_overwrite_step_bytes() {
        let samples = ProgressSamples::default();
        samples.set_step_bytes(1_048_576, Some(4_194_304), "Downloading...");
        samples.set_step_bytes(2_097_152, Some(4_194_304), "Downloading...");

        let mut state = ProgressState::default();
        samples.apply_to(&mut state);

        let display = state.display.unwrap();
        assert_eq!(display.fraction, Some(0.5));
        assert_eq!(display.text, "Downloading... - 2.0 / 4.0 MB");
    }

    #[test]
    fn mod_download_start_text_is_uniform() {
        assert_eq!(
            mod_download_start_text("Render Fix"),
            "Starting Render Fix..."
        );
    }

    #[test]
    fn mod_download_finished_text_is_uniform() {
        assert_eq!(
            mod_download_finished_text("Render Fix", 1, 4),
            "Installed Render Fix (1/4)"
        );
    }

    #[test]
    fn step_download_text_uses_dash_separator() {
        assert_eq!(
            format_step_download_text("Downloading...", 1_048_576, Some(2_097_152)),
            "Downloading... - 1.0 / 2.0 MB"
        );
    }

    #[test]
    fn progress_state_handles_every_message_kind() {
        let mut state = ProgressState::default();

        state.apply(ProgressMsg::Refresh);
        assert!(state.display.is_none());

        state.apply(ProgressMsg::Bytes {
            downloaded: 1_048_576,
            total: Some(0),
            status: String::new(),
        });
        assert_eq!(
            state.display,
            Some(ProgressDisplay {
                fraction: None,
                pulse: false,
                text: "1.0 / 0.0 MB".to_string(),
            })
        );

        state.apply(ProgressMsg::Bytes {
            downloaded: 1_048_576,
            total: None,
            status: "Downloading".to_string(),
        });
        assert!(state.display.as_ref().unwrap().pulse);

        state.apply(ProgressMsg::ModInstall {
            mod_name: "Alpha".to_string(),
            total: 2,
        });
        assert_eq!(state.display.as_ref().unwrap().text, "Starting Alpha...");

        state
            .active_downloads
            .insert("Alpha".to_string(), (1, None));
        state.apply(ProgressMsg::ModFinished {
            mod_name: "Alpha".to_string(),
            completed: 1,
            total: 2,
        });
        assert!(state.active_downloads.is_empty());

        state
            .active_downloads
            .insert("Beta".to_string(), (1, Some(2)));
        state.apply(ProgressMsg::Configuring { total: 2 });
        assert!(state.active_downloads.is_empty());
        assert_eq!(state.display.as_ref().unwrap().fraction, Some(1.0));
    }

    #[test]
    fn progress_samples_publish_latest_values_and_can_be_cleared() {
        let samples = ProgressSamples::default();
        let (sender, receiver) = async_channel::bounded(8);

        publish_step_bytes(&sender, &samples, 1_048_576, Some(2_097_152), "Step");
        publish_mod_bytes(&sender, &samples, "Alpha", 2, 1_048_576, Some(2_097_152));
        publish_mod_bytes(&sender, &samples, "Beta", 2, 1_048_576, None);

        assert!(receiver.try_recv().is_ok());
        let mut state = ProgressState::default();
        samples.apply_to(&mut state);
        assert_eq!(state.active_downloads.len(), 2);
        assert!(state.display.as_ref().unwrap().pulse);

        samples.clear_mod("Alpha");
        samples.clear();
        let mut cleared = ProgressState::default();
        samples.apply_to(&mut cleared);
        assert!(cleared.display.is_none());
    }

    #[test]
    fn progress_format_helpers_cover_empty_and_zero_cases() {
        assert_eq!(format_download_bytes_text(1_048_576, None), "1.0 MB");
        assert_eq!(
            format_step_download_text("", 1_048_576, Some(2_097_152)),
            "1.0 / 2.0 MB"
        );
        assert_eq!(completed_mod_fraction(0, 0), 0.0);
        assert_eq!(mod_download_fraction(1, 0, 1, Some(100)), 0.0);
        assert_eq!(mod_download_fraction(1, 2, 1, Some(0)), 0.5);
        assert_eq!(
            subtitle_language_index(GameKind::SADX, SubtitleLanguage::Italian),
            0
        );
        assert_eq!(
            subtitle_language_index(GameKind::SA2, SubtitleLanguage::Italian),
            4
        );
        assert_eq!(voice_language_index(VoiceLanguage::Japanese), 0);
        assert_eq!(voice_language_index(VoiceLanguage::English), 1);
    }

    #[test]
    fn install_progress_helper_publishes_each_event_and_honors_cancel() {
        let (sender, receiver) = async_channel::bounded(8);
        let samples = ProgressSamples::default();
        let cancel = std::sync::atomic::AtomicBool::new(false);

        apply_install_progress(
            pipeline::InstallProgress::Started { mod_name: "Alpha" },
            &cancel,
            &sender,
            &samples,
            2,
        )
        .unwrap();
        apply_install_progress(
            pipeline::InstallProgress::DownloadingMod {
                mod_name: "Alpha",
                downloaded: 10,
                total_bytes: Some(20),
            },
            &cancel,
            &sender,
            &samples,
            2,
        )
        .unwrap();
        apply_install_progress(
            pipeline::InstallProgress::Finished {
                mod_name: "Alpha",
                completed: 1,
                total: 2,
            },
            &cancel,
            &sender,
            &samples,
            2,
        )
        .unwrap();
        apply_install_progress(
            pipeline::InstallProgress::GeneratingConfig,
            &cancel,
            &sender,
            &samples,
            2,
        )
        .unwrap();

        let mut state = ProgressState::default();
        drain_progress_updates(&receiver, &mut state);
        assert_eq!(state.completed_mods, 1);
        assert_eq!(state.display.unwrap().fraction, Some(1.0));

        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        for progress in [
            pipeline::InstallProgress::Started { mod_name: "Beta" },
            pipeline::InstallProgress::DownloadingMod {
                mod_name: "Beta",
                downloaded: 1,
                total_bytes: None,
            },
            pipeline::InstallProgress::Finished {
                mod_name: "Beta",
                completed: 2,
                total: 2,
            },
        ] {
            assert!(apply_install_progress(progress, &cancel, &sender, &samples, 2).is_err());
        }

        // Config generation is deliberately allowed through after cancellation.
        apply_install_progress(
            pipeline::InstallProgress::GeneratingConfig,
            &cancel,
            &sender,
            &samples,
            2,
        )
        .unwrap();
    }

    #[test]
    fn download_size_text_summarizes_the_selection() {
        let estimate = |size, installed| ModDownloadEstimate { size, installed };
        let estimates = [
            estimate(Some(150_000_000), false),
            estimate(Some(1_000_000_000), false),
            estimate(None, false),
            estimate(None, true),
            estimate(None, true),
        ];

        assert_eq!(
            download_size_text(Some(&estimates), &[]),
            "No mods selected"
        );
        assert_eq!(download_size_text(None, &[0]), "Checking download size…");
        assert_eq!(
            download_size_text(Some(&estimates), &[0]),
            "150 MB to download"
        );
        assert_eq!(
            download_size_text(Some(&estimates), &[1, 3]),
            "1.0 GB to download, 1 already installed"
        );
        assert_eq!(
            download_size_text(Some(&estimates), &[0, 2, 3, 4]),
            "At least 150 MB to download, 2 already installed"
        );
        assert_eq!(
            download_size_text(Some(&estimates), &[2]),
            "Download size unknown"
        );
        assert_eq!(
            download_size_text(Some(&estimates), &[3, 4]),
            "Everything selected is already installed"
        );
        assert_eq!(format_download_size(1), "1 MB");
    }

    #[test]
    fn estimate_mod_downloads_skips_installed_mods() {
        let tmp = tempfile::tempdir().unwrap();
        let mods = common::recommended_mods_for_game(GameKind::SA2);
        let installed = mods[1].dir_name.unwrap();
        std::fs::create_dir_all(tmp.path().join("mods").join(installed)).unwrap();
        std::fs::write(
            tmp.path().join("mods").join(installed).join("mod.ini"),
            "[mod]",
        )
        .unwrap();

        let estimates = estimate_mod_downloads(tmp.path(), mods, |_| Some(42));

        assert_eq!(estimates.len(), mods.len());
        assert_eq!(
            estimates[1],
            ModDownloadEstimate {
                size: None,
                installed: true
            }
        );
        assert_eq!(
            estimates[0],
            ModDownloadEstimate {
                size: Some(42),
                installed: false
            }
        );
    }

    #[test]
    fn remote_download_size_is_unknown_when_the_lookup_fails() {
        use crate::external::test_http::{Reply, serve};

        let _ = rustls::crypto::ring::default_provider().install_default();
        let _guard = crate::test_env::lock();
        let logs = LogCapture::start();
        let (base, requests) = serve(|_| Reply::ok("down").status("500 Internal Server Error"));
        let mod_entry = common::recommended_mods_for_game(GameKind::SA2)
            .iter()
            .find(|entry| matches!(entry.source, common::ModSource::GameBananaItem { .. }))
            .unwrap();

        unsafe { std::env::set_var("ADVENTURE_MODS_GAMEBANANA_API_BASE", format!("{base}/api?")) };
        let size = remote_mod_download_size(mod_entry);
        unsafe { std::env::remove_var("ADVENTURE_MODS_GAMEBANANA_API_BASE") };

        assert_eq!(size, None);
        assert!(!requests.lock().unwrap().is_empty());
        assert!(
            logs.contents()
                .contains(&format!("No download size for {}", mod_entry.name))
        );
    }
}
