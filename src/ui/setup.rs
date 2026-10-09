//! The setup flow for one game: every choice first, then all the work on one
//! install screen, then a finish screen.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, mpsc};

use egui::{RichText, Ui, Vec2};

use super::dialogs::{Answer, ChoiceDialog};
use super::gamepad::PadButton;
use super::images::{ImageCache, cover_resource};
use super::progress::{
    ModDownloadEstimate, ProgressDisplay, ProgressMsg, ProgressSamples, ProgressState,
    apply_install_progress, download_size_text, drain_progress_updates, estimate_mod_downloads,
    initial_preview_index, publish_step_bytes, remote_mod_download_size, resolution_or_fallback,
    subtitle_language_index, subtitle_language_labels, voice_language_index, voice_language_labels,
};
use super::theme;
use super::widgets::{self, ButtonKind, Tone};
use crate::blocking;
use crate::external::download::ProgressFn;
use crate::setup::common::{self, ModEntry, SteamConfigStatus};
use crate::setup::config::{LanguageSelection, SubtitleLanguage, VoiceLanguage};
use crate::setup::pipeline::{self, InstallProgress};
use crate::setup::steps::{self, SetupStep, StepId, StepKind};
use crate::steam::game::{Game, GameKind};

/// Looks up how many bytes installing a mod downloads.
pub type DownloadSizeFn = fn(&ModEntry) -> Option<u64>;
/// Downloads the selected mods' archives ahead of the install step.
pub type PrefetchFn = fn(&Path, &[&ModEntry], &AtomicBool);
/// Installs the chosen mods and writes the configs.
pub type InstallModsFn = fn(
    &Path,
    GameKind,
    &[&ModEntry],
    u32,
    u32,
    LanguageSelection,
    &mut dyn FnMut(InstallProgress<'_>) -> anyhow::Result<()>,
) -> anyhow::Result<()>;

/// The work behind each step. Tests swap in fakes so nothing touches the
/// network or a real Proton prefix.
#[derive(Clone, Copy)]
pub struct SetupWork {
    pub install_runtimes: fn(&Path, u32) -> anyhow::Result<()>,
    pub convert_steam: fn(&Path, Option<ProgressFn>) -> anyhow::Result<()>,
    pub install_mod_manager: fn(&Path, GameKind, Option<ProgressFn>) -> anyhow::Result<()>,
    pub install_mods: InstallModsFn,
    pub download_size_of: Option<DownloadSizeFn>,
    pub prefetch_archives: Option<PrefetchFn>,
    pub steam_config_status: fn(&Game) -> SteamConfigStatus,
    pub is_step_complete: fn(StepId, &Game) -> bool,
}

impl SetupWork {
    pub fn real() -> Self {
        Self {
            install_runtimes: crate::external::runtime_installer::install_runtimes,
            convert_steam: crate::setup::sadx::convert_steam_to_2004,
            install_mod_manager: common::install_mod_manager,
            install_mods: install_mods_with_pipeline,
            download_size_of: Some(remote_mod_download_size),
            prefetch_archives: Some(pipeline::prefetch_mod_archives),
            steam_config_status: common::steam_config_status,
            is_step_complete: common::is_step_complete,
        }
    }
}

fn install_mods_with_pipeline(
    game_path: &Path,
    game_kind: GameKind,
    mods: &[&ModEntry],
    width: u32,
    height: u32,
    languages: LanguageSelection,
    progress: &mut dyn FnMut(InstallProgress<'_>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    pipeline::install_selected_mods_and_generate_config_with_progress(
        game_path, game_kind, mods, width, height, languages, progress,
    )
}

/// What the setup flow asks of the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupEvent {
    /// Leave setup for the game list.
    Exit,
    OpenUri(String),
    SaveLanguages(GameKind, LanguageSelection),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskState {
    Pending,
    Running,
    Done,
    Failed,
}

/// One checklist row for a work step.
struct InstallTask {
    step: usize,
    title: &'static str,
    description: &'static str,
    /// Shown instead of the description while the task waits its turn.
    pending_note: Option<&'static str>,
    state: TaskState,
}

impl InstallTask {
    fn subtitle(&self) -> &'static str {
        match self.state {
            TaskState::Done => "Done",
            TaskState::Failed => "Failed",
            TaskState::Pending => self.pending_note.unwrap_or(self.description),
            TaskState::Running => self.description,
        }
    }
}

/// The install screen: a checklist of every work step, the current step's
/// progress, and the error if one failed.
struct InstallView {
    tasks: Vec<InstallTask>,
    show_progress: bool,
    progress: ProgressState,
    error: Option<String>,
}

impl InstallView {
    fn new(all_steps: &[SetupStep]) -> Self {
        let tasks = all_steps
            .iter()
            .enumerate()
            .filter(|(_, step)| step.kind.is_work())
            .map(|(step, info)| InstallTask {
                step,
                title: info.title,
                description: info.description,
                pending_note: None,
                state: TaskState::Pending,
            })
            .collect();
        Self {
            tasks,
            show_progress: false,
            progress: ProgressState::default(),
            error: None,
        }
    }

    /// Mark tasks before `current` done, `current` running and the rest pending.
    fn show_step(&mut self, all_steps: &[SetupStep], current: usize) {
        for task in &mut self.tasks {
            task.state = match task.step.cmp(&current) {
                std::cmp::Ordering::Less => TaskState::Done,
                std::cmp::Ordering::Equal => TaskState::Running,
                std::cmp::Ordering::Greater => TaskState::Pending,
            };
        }
        self.error = None;
        self.show_progress = all_steps
            .get(current)
            .is_some_and(|step| matches!(step.kind, StepKind::Download));
        self.progress = ProgressState::default();
    }

    fn show_error(&mut self, current: usize, message: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.step == current) {
            task.state = TaskState::Failed;
        }
        self.show_progress = false;
        self.error = Some(message.to_owned());
    }

    /// Say what a waiting task is already doing in the background.
    fn set_pending_note(&mut self, step: usize, note: &'static str) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.step == step) {
            task.pending_note = Some(note);
        }
    }
}

/// Background download of the selected mods' archives. `done` closes when
/// the downloads stop, finished or cancelled.
struct Prefetch {
    cancel: Arc<AtomicBool>,
    done: async_channel::Receiver<()>,
}

/// Work running for a step on another thread.
struct RunningTask {
    cancel: Arc<AtomicBool>,
    result: Receiver<anyhow::Result<()>>,
    progress: async_channel::Receiver<ProgressMsg>,
    samples: Arc<ProgressSamples>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picker {
    Preset,
    Subtitles,
    Voices,
}

/// Which mod the preview shows, and which of its screenshots.
#[derive(Default)]
struct Preview {
    index: Option<usize>,
    page: usize,
}

pub struct SetupFlow {
    game: Game,
    work: SetupWork,
    steps: Vec<SetupStep>,
    current: usize,
    /// The Proton check is only part of the flow when it was needed.
    steam_check_needed: bool,
    selected_mods: Vec<usize>,
    preset: usize,
    languages: LanguageSelection,
    // Steam/Proton status for the current visit to the Steam step. Refreshed
    // whenever the user navigates or asks to check again.
    steam_status: Option<SteamConfigStatus>,
    estimates: Option<Vec<ModDownloadEstimate>>,
    estimates_rx: Option<Receiver<anyhow::Result<Vec<ModDownloadEstimate>>>>,
    preview: Preview,
    install: Option<InstallView>,
    task: Option<RunningTask>,
    prefetch: Option<Prefetch>,
    error: Option<String>,
    cancelling: bool,
    picker: Option<(Picker, ChoiceDialog)>,
    /// The game's monitor resolution, for the mod configs.
    pub resolution: Option<(u32, u32)>,
    focus_primary: bool,
    events: Vec<SetupEvent>,
}

impl SetupFlow {
    pub fn new(game: Game, work: SetupWork, languages: LanguageSelection) -> Self {
        let mods = common::recommended_mods_for_game(game.kind);
        let default_preset = common::presets_for_game(game.kind).first();
        let selected_mods = mods
            .iter()
            .enumerate()
            .filter(|(_, mod_entry)| {
                default_preset.is_none_or(|preset| preset.mod_names.contains(&mod_entry.name))
            })
            .map(|(index, _)| index)
            .collect();
        let mut flow = Self {
            steps: steps::steps_for_game(game.kind),
            game,
            work,
            current: 0,
            steam_check_needed: false,
            selected_mods,
            preset: 0,
            languages,
            steam_status: None,
            estimates: None,
            estimates_rx: None,
            preview: Preview::default(),
            install: None,
            task: None,
            prefetch: None,
            error: None,
            cancelling: false,
            picker: None,
            resolution: None,
            focus_primary: true,
            events: Vec::new(),
        };
        flow.current = flow.skip_completed_steps(0);
        flow.steam_check_needed = flow.current == 0;
        flow.enter_step();
        flow
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn take_events(&mut self) -> Vec<SetupEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn focus_primary(&mut self) {
        self.focus_primary = true;
    }

    /// Whether a picker dialog is open, so B closes it instead of going back.
    pub fn has_dialog(&self) -> bool {
        self.picker.is_some()
    }

    fn step(&self) -> Option<&SetupStep> {
        self.steps.get(self.current)
    }

    fn is_last_step(&self) -> bool {
        self.current + 1 >= self.steps.len()
    }

    fn on_work_step(&self) -> bool {
        self.step().is_some_and(|step| step.kind.is_work())
    }

    pub fn busy(&self) -> bool {
        self.task.is_some()
    }

    /// Show the current step and start its work, if it has any.
    fn enter_step(&mut self) {
        self.error = None;
        self.cancelling = false;
        self.focus_primary = true;
        // Steps only change while nothing runs.
        debug_assert!(self.task.is_none());

        let Some(step) = self.step().cloned() else {
            return;
        };
        if !step.kind.is_work() {
            // Leaving the install screen: the next one builds it again.
            self.install = None;
            if step.id == StepId::SelectMods {
                self.request_download_estimates();
                if self.preview.index.is_none() {
                    let mods = common::recommended_mods_for_game(self.game.kind);
                    self.show_preview(initial_preview_index(mods.len(), &self.selected_mods));
                }
            }
            return;
        }

        if self.install.is_none() {
            self.install = Some(InstallView::new(&self.steps));
            self.start_prefetch();
        }
        let current = self.current;
        let install = self
            .install
            .as_mut()
            .expect("install view was just created");
        install.show_step(&self.steps, current);
        self.start_task(step.id);
    }

    /// Start downloading the selected mods now, so they arrive while the
    /// runtime, conversion and mod manager install.
    fn start_prefetch(&mut self) {
        let mods = common::recommended_mods_for_game(self.game.kind);
        let selected: Vec<&'static ModEntry> = self
            .selected_mods
            .iter()
            .filter_map(|index| mods.get(*index))
            .collect();
        if selected.is_empty() {
            return;
        }

        let previous = self.prefetch.take();
        let prefetch_archives = self.work.prefetch_archives;
        let cancel = Arc::new(AtomicBool::new(false));
        let (done_tx, done) = async_channel::bounded::<()>(1);
        let worker_cancel = cancel.clone();
        let game_path = self.game.path.clone();
        std::thread::spawn(move || {
            // Never two writers for one archive: let a cancelled run stop first.
            if let Some(previous) = previous {
                let _ = previous.done.recv_blocking();
            }
            if let Some(prefetch_archives) = prefetch_archives {
                prefetch_archives(&game_path, &selected, &worker_cancel);
            }
            drop(done_tx);
        });
        self.prefetch = Some(Prefetch { cancel, done });

        if let Some(mods_step) = self
            .steps
            .iter()
            .position(|step| step.id == StepId::DownloadMods)
            && let Some(install) = &mut self.install
        {
            install.set_pending_note(mods_step, "Downloading in the background");
        }
    }

    fn cancel_prefetch(&self) {
        if let Some(prefetch) = &self.prefetch {
            prefetch.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn start_task(&mut self, step_id: StepId) {
        let cancel = Arc::new(AtomicBool::new(false));
        let (result_tx, result) = mpsc::channel();
        let (progress_tx, progress) = async_channel::bounded::<ProgressMsg>(32);
        let samples = Arc::new(ProgressSamples::default());
        let game = self.game.clone();
        let work = self.work;

        let job: Box<dyn FnOnce() -> anyhow::Result<()> + Send> = match step_id {
            StepId::Dotnet => {
                Box::new(move || (work.install_runtimes)(&game.path, game.kind.app_id()))
            }
            StepId::ConvertSteam => {
                let progress = step_progress(progress_tx, samples.clone());
                Box::new(move || (work.convert_steam)(&game.path, Some(progress)))
            }
            StepId::InstallModManager => {
                let progress = step_progress(progress_tx, samples.clone());
                Box::new(move || (work.install_mod_manager)(&game.path, game.kind, Some(progress)))
            }
            // Only work steps run, and this is the last one.
            other => {
                debug_assert_eq!(other, StepId::DownloadMods);
                self.install_mods_job(&cancel, progress_tx, &samples)
            }
        };

        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            let _ = result_tx.send(blocking::flatten_spawn_result(outcome));
        });
        self.task = Some(RunningTask {
            cancel,
            result,
            progress,
            samples,
        });
    }

    /// Wait for the background downloads, then install the chosen mods.
    fn install_mods_job(
        &mut self,
        cancel: &Arc<AtomicBool>,
        progress_tx: async_channel::Sender<ProgressMsg>,
        samples: &Arc<ProgressSamples>,
    ) -> Box<dyn FnOnce() -> anyhow::Result<()> + Send> {
        // Never write an archive the prefetch is still writing.
        let prefetch_done = self.prefetch.as_ref().map(|prefetch| prefetch.done.clone());
        if prefetch_done.is_some()
            && let Some(install) = &mut self.install
        {
            install.progress.display = Some(ProgressDisplay {
                fraction: None,
                pulse: true,
                text: "Finishing downloads…".to_owned(),
            });
        }
        let mods = common::recommended_mods_for_game(self.game.kind);
        let selected: Vec<&'static ModEntry> = self
            .selected_mods
            .iter()
            .filter_map(|index| mods.get(*index))
            .collect();
        let (width, height) = resolution_or_fallback(self.resolution);
        let languages = self.languages;
        let game = self.game.clone();
        let work = self.work;
        let cancel = cancel.clone();
        let samples = samples.clone();
        Box::new(move || {
            if let Some(done) = prefetch_done {
                let _ = done.recv_blocking();
            }
            // Cancel may have ended the wait; install nothing then.
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let total = selected.len();
            (work.install_mods)(
                &game.path,
                game.kind,
                &selected,
                width,
                height,
                languages,
                &mut |progress| {
                    apply_install_progress(progress, &cancel, &progress_tx, &samples, total)
                },
            )
        })
    }

    /// Pick up background results. Call once per frame.
    pub fn poll(&mut self) {
        if let Some(Ok(result)) = self.estimates_rx.as_ref().map(Receiver::try_recv) {
            self.estimates = Some(result.unwrap_or_else(|err| {
                tracing::warn!("Failed to estimate mod downloads: {err}");
                let mods = common::recommended_mods_for_game(self.game.kind);
                vec![ModDownloadEstimate::default(); mods.len()]
            }));
            self.estimates_rx = None;
        }

        let Some(task) = &self.task else {
            return;
        };
        if let Some(install) = &mut self.install {
            drain_progress_updates(&task.progress, &mut install.progress);
            task.samples.apply_to(&mut install.progress);
        }
        // The worker always reports, even when the work panicked.
        let Ok(result) = task.result.try_recv() else {
            return;
        };
        let cancelled = task.cancel.load(Ordering::Relaxed);
        self.task = None;

        // A cancelled task never moves setup on, even if it finished.
        if cancelled {
            self.go_back_to_choices();
            return;
        }
        match result {
            Ok(()) => self.advance_step(),
            Err(err) => self.show_error(&format!("{err}")),
        }
    }

    fn request_download_estimates(&mut self) {
        if self.estimates.is_some() || self.estimates_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let path = self.game.path.clone();
        let mods = common::recommended_mods_for_game(self.game.kind);
        let size_of = self.work.download_size_of;
        std::thread::spawn(move || {
            let estimates = std::panic::catch_unwind(|| {
                estimate_mod_downloads(&path, mods, |mod_entry| {
                    size_of.and_then(|size_of| size_of(mod_entry))
                })
            });
            let _ = tx.send(blocking::spawn_result(estimates));
        });
        self.estimates_rx = Some(rx);
    }

    fn advance_step(&mut self) {
        let next = self.current + 1;
        if next < self.steps.len() {
            self.current = self.skip_completed_steps(next);
            self.enter_step();
        }
    }

    fn skip_completed_steps(&self, start: usize) -> usize {
        let Some(last) = self.steps.len().checked_sub(1) else {
            return start;
        };
        let mut index = start;
        // Never skip the last step, the finish screen.
        while index < last && (self.work.is_step_complete)(self.steps[index].id, &self.game) {
            tracing::info!("Auto-skipping completed step: {}", self.steps[index].id);
            index += 1;
        }
        index
    }

    fn steam_status(&mut self) -> &SteamConfigStatus {
        let work = self.work;
        let game = &self.game;
        self.steam_status
            .get_or_insert_with(|| (work.steam_config_status)(game))
    }

    pub fn on_next(&mut self) {
        // Ignore presses while a step is running.
        if self.busy() {
            return;
        }
        // Continue / Check Again should see what Steam looks like right now.
        self.steam_status = None;

        if self.error.is_some() {
            // Retry the current step.
            self.enter_step();
            return;
        }
        let Some(step_id) = self.step().map(|step| step.id) else {
            return;
        };
        if self.is_last_step() {
            self.exit();
            return;
        }
        if step_id == StepId::SteamConfig && !self.steam_status().ready {
            self.focus_primary = true;
            return;
        }
        if step_id == StepId::LanguageOptions {
            self.events
                .push(SetupEvent::SaveLanguages(self.game.kind, self.languages));
        }
        self.advance_step();
    }

    pub fn on_back(&mut self) {
        self.steam_status = None;
        if self.busy() || self.is_last_step() {
            return;
        }
        if self.on_work_step() {
            self.go_back_to_choices();
            return;
        }
        if self.current == 0 {
            self.exit();
            return;
        }

        // Skip steps backwards that are automatic or already complete.
        let skippable = |step: &SetupStep| {
            matches!(step.kind, StepKind::Auto) || (self.work.is_step_complete)(step.id, &self.game)
        };
        let mut previous = self.current - 1;
        while previous > 0 && skippable(&self.steps[previous]) {
            previous -= 1;
        }
        // Everything before here could be skipped: leave setup.
        if skippable(&self.steps[previous]) {
            self.exit();
        } else {
            self.current = previous;
            self.enter_step();
        }
    }

    /// Leave the install screen for the last screen that asked something.
    fn go_back_to_choices(&mut self) {
        // The choices may change, so stop downloading the current ones.
        self.cancel_prefetch();
        match self.steps[..self.current]
            .iter()
            .rposition(|step| !step.kind.is_work())
        {
            Some(index) => {
                self.current = index;
                self.enter_step();
            }
            None => self.exit(),
        }
    }

    fn cancel_download(&mut self) {
        if let Some(task) = &self.task {
            task.cancel.store(true, Ordering::Relaxed);
        }
        self.cancel_prefetch();
        self.cancelling = true;
    }

    fn exit(&mut self) {
        self.cancel_prefetch();
        self.events.push(SetupEvent::Exit);
    }

    fn show_error(&mut self, message: &str) {
        self.error = Some(message.to_owned());
        self.focus_primary = true;
        let current = self.current;
        if let Some(install) = &mut self.install {
            install.show_error(current, message);
        }
    }

    fn play_in_steam(&mut self) {
        self.events.push(SetupEvent::OpenUri(format!(
            "steam://rungameid/{}",
            self.game.kind.app_id()
        )));
    }

    /// The secondary button's label and whether it can be pressed.
    fn secondary(&self) -> Option<(&'static str, bool)> {
        if self.error.is_some() {
            return None;
        }
        let step = self.step()?;
        if self.is_last_step() {
            Some(("Play in Steam", true))
        } else if step.id == StepId::DownloadMods && self.busy() {
            Some(if self.cancelling {
                ("Cancelling…", false)
            } else {
                ("Cancel", true)
            })
        } else {
            None
        }
    }

    fn on_secondary(&mut self) {
        match self.secondary() {
            Some(("Play in Steam", _)) => self.play_in_steam(),
            Some(("Cancel", true)) => self.cancel_download(),
            _ => {}
        }
    }

    /// The main button's label, or `None` while work runs.
    fn next_label(&mut self) -> Option<&'static str> {
        if self.error.is_some() {
            return Some("Try Again");
        }
        let step = self.step()?.clone();
        Some(match step.kind {
            StepKind::Auto | StepKind::Download => return None,
            _ if self.is_last_step() => "Done",
            StepKind::Info if step.id == StepId::SteamConfig => {
                if self.steam_status().ready {
                    "Continue"
                } else {
                    "Check Again"
                }
            }
            StepKind::Info if self.next_screen_is_work(step.id) => "Install",
            StepKind::Info | StepKind::ModSelection => "Continue",
        })
    }

    /// Whether the screen after `step_id` starts installing.
    fn next_screen_is_work(&self, step_id: StepId) -> bool {
        self.steps
            .iter()
            .position(|step| step.id == step_id)
            .and_then(|index| self.steps.get(index + 1))
            .is_some_and(|step| step.kind.is_work())
    }

    /// Position of the current screen among the screens the user moves
    /// through, counting all work steps as one install screen.
    fn screen_position(&self) -> Option<(usize, usize)> {
        let mut screens = 0;
        let mut position = None;
        let mut previous_was_work = false;
        for (index, step) in self.steps.iter().enumerate() {
            let counted = match step.id {
                StepId::SteamConfig => self.steam_check_needed,
                StepId::Complete => false,
                _ => !(step.kind.is_work() && previous_was_work),
            };
            previous_was_work = step.kind.is_work();
            if counted {
                screens += 1;
            }
            if index == self.current && step.id != StepId::Complete {
                position = Some(screens);
            }
        }
        position.map(|position| (position, screens))
    }

    /// Header title and subtitle for the current step.
    pub fn title(&self) -> (String, String) {
        let game_name = self.game.kind.name();
        let title = match self.step() {
            Some(step) if step.id == StepId::Complete => game_name,
            Some(step) if step.kind.is_work() => "Installing",
            Some(step) => step.title,
            None => game_name,
        };
        let subtitle = match self.screen_position() {
            Some((position, total)) => format!("{game_name} · Step {position} of {total}"),
            None => String::new(),
        };
        (title.to_owned(), subtitle)
    }

    /// Whether the header's back button can be used.
    pub fn can_go_back(&self) -> bool {
        !self.is_last_step() && !self.busy()
    }

    pub fn handle_pad(&mut self, button: PadButton) {
        match button {
            PadButton::Start => {
                if self.next_label().is_some() {
                    self.on_next();
                }
            }
            PadButton::Action => self.on_secondary(),
            PadButton::PreviousPage => self.turn_page(-1),
            PadButton::NextPage => self.turn_page(1),
            _ => {}
        }
    }

    fn show_preview(&mut self, index: Option<usize>) {
        if self.preview.index != index {
            self.preview = Preview { index, page: 0 };
        }
    }

    fn turn_page(&mut self, delta: isize) {
        let Some(mod_entry) = self.preview_entry() else {
            return;
        };
        let pages = mod_entry.pictures.len();
        if pages > 0 {
            self.preview.page =
                (self.preview.page as isize + delta).rem_euclid(pages as isize) as usize;
        }
    }

    fn preview_entry(&self) -> Option<&'static ModEntry> {
        common::recommended_mods_for_game(self.game.kind).get(self.preview.index?)
    }

    fn apply_preset(&mut self, index: usize) {
        let Some(preset) = common::presets_for_game(self.game.kind).get(index) else {
            return;
        };
        self.preset = index;
        self.selected_mods = common::recommended_mods_for_game(self.game.kind)
            .iter()
            .enumerate()
            .filter(|(_, mod_entry)| preset.mod_names.contains(&mod_entry.name))
            .map(|(index, _)| index)
            .collect();
    }

    fn set_mod_selected(&mut self, index: usize, selected: bool) {
        self.selected_mods.retain(|&other| other != index);
        if selected {
            self.selected_mods.push(index);
        }
    }
}

// --- Rendering ---

impl SetupFlow {
    fn focus_starts_in_body(&self) -> bool {
        self.error.is_none()
            && self
                .step()
                .is_some_and(|step| matches!(step.kind, StepKind::ModSelection))
    }

    /// Go back a screen, as the header's back button or B does.
    pub fn back(&mut self) {
        self.guarded(
            "setup back button",
            "Something went wrong while going back. Please try again.",
            Self::on_back,
        );
    }

    /// Run a button's `action`, turning a panic into an error the user can retry.
    fn guarded(&mut self, label: &'static str, message: &str, action: fn(&mut Self)) {
        if super::catch_ui_panic(label, || action(self)).is_err() {
            self.show_error(message);
        }
    }

    /// The footer: the secondary and main buttons.
    pub fn show_footer(&mut self, ui: &mut Ui) {
        let secondary = self.secondary();
        let next = self.next_label();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(label) = next {
                let response = widgets::button_sized(ui, label, ButtonKind::Suggested, 200.0);
                // On the mod list, focus starts on the first mod instead.
                if !self.focus_starts_in_body() && std::mem::take(&mut self.focus_primary) {
                    response.request_focus();
                }
                if response.clicked() {
                    self.guarded(
                        "setup next button",
                        "Something went wrong while continuing setup. Please try again.",
                        Self::on_next,
                    );
                }
            }
            if let Some((label, enabled)) = secondary {
                let response = ui.add_enabled_ui(enabled, |ui| {
                    widgets::button_sized(ui, label, ButtonKind::Normal, 180.0)
                });
                if response.inner.clicked() {
                    self.guarded(
                        "setup secondary button",
                        "Something went wrong. Please try again.",
                        Self::on_secondary,
                    );
                }
            }
        });
    }

    pub fn show_body(&mut self, ui: &mut Ui, images: &mut ImageCache) {
        self.show_picker(ui.ctx());

        if self.error.is_some() && !self.on_work_step() {
            let message = self.error.clone().unwrap_or_default();
            centered(ui, |ui| {
                widgets::status_page(ui, "✖", Tone::Error, "Something Went Wrong", &message);
            });
            return;
        }
        let Some(step) = self.step().cloned() else {
            return;
        };
        match step.kind {
            StepKind::Auto | StepKind::Download => self.show_install(ui),
            StepKind::ModSelection => self.show_mod_selection(ui, images),
            StepKind::Info if step.id == StepId::SteamConfig => {
                let description = step.description;
                let status = self.steam_status().clone();
                let message = if status.message.is_empty() {
                    description.to_owned()
                } else {
                    status.message
                };
                centered(ui, |ui| {
                    if status.ready {
                        widgets::status_page(ui, "✔", Tone::Success, "Proton Is Ready", &message);
                    } else {
                        widgets::status_page(ui, "⚠", Tone::Warning, "Proton 10 Needed", &message);
                    }
                });
            }
            StepKind::Info if self.is_last_step() => self.show_complete(ui, images, &step),
            StepKind::Info => {
                if step.id == StepId::LanguageOptions {
                    self.show_languages(ui, step.description);
                }
            }
        }
    }

    fn show_picker(&mut self, ctx: &egui::Context) {
        let Some((target, dialog)) = &mut self.picker else {
            return;
        };
        let target = *target;
        let Some(answer) = dialog.show(ctx) else {
            return;
        };
        self.picker = None;
        let Answer::Button(index) = answer else {
            return;
        };
        match target {
            Picker::Preset => self.apply_preset(index),
            Picker::Subtitles => {
                if let Some(language) = SubtitleLanguage::supported_for(self.game.kind).get(index) {
                    self.languages.subtitle = *language;
                }
            }
            Picker::Voices => {
                if let Some(language) = VoiceLanguage::all().get(index) {
                    self.languages.voice = *language;
                }
            }
        }
    }

    fn open_picker(&mut self, target: Picker) {
        let (title, options, selected): (&str, Vec<&str>, usize) = match target {
            Picker::Preset => {
                let presets = common::presets_for_game(self.game.kind);
                (
                    "Preset",
                    presets.iter().map(|preset| preset.name).collect(),
                    self.preset,
                )
            }
            Picker::Subtitles => (
                "Subtitles",
                subtitle_language_labels(self.game.kind),
                subtitle_language_index(self.game.kind, self.languages.subtitle) as usize,
            ),
            Picker::Voices => (
                "Voices",
                voice_language_labels(),
                voice_language_index(self.languages.voice) as usize,
            ),
        };
        let options = options.into_iter().map(String::from).collect();
        self.picker = Some((
            target,
            ChoiceDialog::new(&format!("setup-picker-{title}"), title, options, selected),
        ));
    }

    fn show_languages(&mut self, ui: &mut Ui, description: &str) {
        let subtitle = self.languages.subtitle.label();
        let voice = self.languages.voice.label();
        let mut open = None;
        centered(ui, |ui| {
            ui.set_max_width(560.0);
            ui.add(egui::Label::new(description).wrap());
            ui.add_space(8.0);
            if widgets::choice_row(ui, "Subtitles", subtitle).clicked() {
                open = Some(Picker::Subtitles);
            }
            if widgets::choice_row(ui, "Voices", voice).clicked() {
                open = Some(Picker::Voices);
            }
        });
        if let Some(target) = open {
            self.open_picker(target);
        }
    }

    fn show_install(&mut self, ui: &mut Ui) {
        let install = self
            .install
            .as_ref()
            .expect("work steps always have an install view");
        let busy = self.task.is_some();
        centered(ui, |ui| {
            ui.set_max_width(620.0);
            widgets::card().inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for task in &install.tasks {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(ui.available_width() - 48.0);
                                    ui.label(RichText::new(task.title).strong());
                                    widgets::caption(ui, task.subtitle());
                                });
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                                match task.state {
                                    TaskState::Pending => {}
                                    TaskState::Running => {
                                        ui.put(rect, egui::Spinner::new().size(28.0));
                                    }
                                    TaskState::Done => icon(ui, rect, "✔", theme::SUCCESS),
                                    TaskState::Failed => icon(ui, rect, "✖", theme::ERROR),
                                }
                            });
                        });
                }
            });
            if install.show_progress && busy {
                let (fraction, text, animate) = match &install.progress.display {
                    Some(display) => (
                        display.fraction.unwrap_or(0.0) as f32,
                        display.text.clone(),
                        display.pulse,
                    ),
                    None => (0.0, "Starting…".to_owned(), true),
                };
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .text(text)
                        .animate(animate)
                        .desired_height(36.0)
                        .corner_radius(18),
                );
            }
            if let Some(error) = &install.error {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Setup Didn't Finish")
                        .heading()
                        .color(theme::ERROR),
                );
                ui.label("Try again, or go back to change your choices.");
                egui::CollapsingHeader::new("Details")
                    .id_salt("setup-error-details")
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(error).color(theme::TEXT_DIM))
                                .wrap()
                                .selectable(true),
                        );
                    });
            }
        });
    }

    fn show_complete(&mut self, ui: &mut Ui, images: &mut ImageCache, step: &SetupStep) {
        centered(ui, |ui| {
            ui.set_max_width(560.0);
            if let Some(texture) = images.get(ui.ctx(), cover_resource(self.game.kind)) {
                ui.add(
                    egui::Image::new(&texture)
                        .fit_to_exact_size(Vec2::new(380.0, 380.0 * 215.0 / 460.0))
                        .corner_radius(theme::CARD_RADIUS),
                );
            }
            ui.label(
                RichText::new(step.title)
                    .text_style(theme::title_style())
                    .strong(),
            );
            ui.add(egui::Label::new(step.description).wrap());
        });
    }

    fn show_mod_selection(&mut self, ui: &mut Ui, images: &mut ImageCache) {
        let mods = common::recommended_mods_for_game(self.game.kind);
        let presets = common::presets_for_game(self.game.kind);
        let wide = ui.available_width() >= 900.0;
        let column_width = if wide {
            (ui.available_width() - 72.0) / 2.0
        } else {
            ui.available_width() - 48.0
        };
        let mut open_preset = false;
        let mut focused = None;
        let mut hovered = None;

        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.set_width(column_width);
                if let Some(preset) = presets.get(self.preset) {
                    if widgets::choice_row(ui, "Preset", preset.name).clicked() {
                        open_preset = true;
                    }
                    widgets::caption(ui, preset.description);
                }
                let list_height = ui.available_height() - 40.0;
                egui::ScrollArea::vertical()
                    .id_salt("mod-list")
                    .max_height(list_height.max(120.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        for (index, mod_entry) in mods.iter().enumerate() {
                            let mut checked = self.selected_mods.contains(&index);
                            let response = widgets::toggle_row(
                                ui,
                                &mut checked,
                                mod_entry.name,
                                mod_entry.description,
                            )
                            .on_hover_text("Install this mod");
                            if response.changed() {
                                self.set_mod_selected(index, checked);
                            }
                            if response.has_focus() {
                                focused = Some(index);
                            }
                            if response.hovered() {
                                hovered = Some(index);
                            }
                            if index == 0 && std::mem::take(&mut self.focus_primary) {
                                response.request_focus();
                            }
                        }
                    });
                widgets::caption(
                    ui,
                    &download_size_text(self.estimates.as_deref(), &self.selected_mods),
                );
            });
            if wide {
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    egui::ScrollArea::vertical()
                        .id_salt("mod-preview")
                        .auto_shrink([false, true])
                        .show(ui, |ui| self.show_preview_panel(ui, images));
                });
            }
        });

        if let Some(index) = focused.or(hovered) {
            self.show_preview(Some(index));
        }
        if let Some(mod_entry) = self.preview_entry() {
            images.retain_pending(mod_entry.pictures);
        }
        if open_preset {
            self.open_picker(Picker::Preset);
        }
    }

    fn show_preview_panel(&mut self, ui: &mut Ui, images: &mut ImageCache) {
        let Some(mod_entry) = self.preview_entry() else {
            return;
        };
        ui.label(RichText::new(mod_entry.name).heading().strong());
        let pages = mod_entry.pictures;
        if let Some(picture) = pages.get(self.preview.page) {
            let width = ui.available_width();
            let size = Vec2::new(width, width * 9.0 / 16.0);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, theme::CARD_RADIUS, theme::CARD);
            match images.get(ui.ctx(), picture) {
                Some(texture) => {
                    egui::Image::new(&texture)
                        .fit_to_exact_size(size)
                        .maintain_aspect_ratio(true)
                        .corner_radius(theme::CARD_RADIUS)
                        .paint_at(ui, rect);
                }
                None if images.failed(picture) => {}
                None => {
                    ui.put(rect, egui::Spinner::new().size(40.0));
                }
            }
            let badge = if picture.contains("_before") {
                Some("Before")
            } else if picture.contains("_after") {
                Some("After")
            } else {
                None
            };
            if let Some(badge) = badge {
                let center = rect.center_bottom() - Vec2::new(0.0, 28.0);
                let badge_rect = egui::Rect::from_center_size(center, Vec2::new(96.0, 32.0));
                ui.painter()
                    .rect_filled(badge_rect, 16, egui::Color32::from_black_alpha(180));
                ui.painter().text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    badge,
                    egui::FontId::proportional(16.0),
                    theme::TEXT,
                );
            }
            if pages.len() > 1 {
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new("‹").min_size(Vec2::splat(44.0)))
                        .clicked()
                    {
                        self.turn_page(-1);
                    }
                    for page in 0..pages.len() {
                        let (dot, _) =
                            ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                        let color = if page == self.preview.page {
                            theme::TEXT
                        } else {
                            theme::CARD_RAISED
                        };
                        ui.painter().circle_filled(dot.center(), 5.0, color);
                    }
                    if ui
                        .add(egui::Button::new("›").min_size(Vec2::splat(44.0)))
                        .clicked()
                    {
                        self.turn_page(1);
                    }
                });
            }
        }
        egui::ScrollArea::vertical()
            .id_salt("mod-description")
            .max_height(150.0)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(mod_entry.full_description.unwrap_or(mod_entry.description))
                        .wrap(),
                );
            });
        ui.horizontal_wrapped(|ui| {
            for link in mod_entry.links {
                if ui
                    .add(
                        egui::Button::new(format!("{}  ↗", link.label))
                            .min_size(Vec2::new(0.0, 44.0)),
                    )
                    .on_hover_text(link.url)
                    .clicked()
                {
                    self.events.push(SetupEvent::OpenUri(link.url.to_owned()));
                }
            }
        });
    }
}

/// A download progress callback that updates the install screen's bar.
fn step_progress(
    tx: async_channel::Sender<ProgressMsg>,
    samples: Arc<ProgressSamples>,
) -> ProgressFn {
    Box::new(move |downloaded, total| {
        publish_step_bytes(&tx, &samples, downloaded, total, "Downloading...");
    })
}

fn icon(ui: &Ui, rect: egui::Rect, glyph: &str, color: egui::Color32) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(28.0),
        color,
    );
}

/// Center `add` in the space left, scrolling when it does not fit.
fn centered(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.add_space(24.0);
            ui.vertical_centered(add);
            ui.add_space(24.0);
        });
}

#[cfg(test)]
#[path = "setup_tests.rs"]
pub(crate) mod tests;
