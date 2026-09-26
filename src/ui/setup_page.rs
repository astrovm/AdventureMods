use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::blocking;
use crate::setup::steps::StepId;
use crate::setup::{common, config, pipeline, sadx, steps};
use crate::steam::game::Game;

const MOD_PREVIEW_DESCRIPTION_HEIGHT: i32 = 150;
const MOD_PREVIEW_TEXT_WIDTH_CHARS: i32 = 42;
const MOD_PREVIEW_HOVER_DELAY: Duration = Duration::from_millis(50);
/// Mods whose preview pages (and decoded screenshots) stay cached. Each decoded
/// screenshot is a few MB, so the cache is bounded instead of growing per hover.
const MOD_PREVIEW_CACHE_LIMIT: usize = 6;
const PROGRESS_UPDATE_INTERVAL: Duration = Duration::from_millis(16);
/// Fade-in only duration for step body swaps. Kept short so Continue/Back feel snappy.
const CONTENT_FADE_MS: u32 = 100;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/setup_page.ui")]
    pub struct AdventureModsSetupPage {
        #[template_child]
        pub body_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub step_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub step_description: TemplateChild<gtk::Label>,
        #[template_child]
        pub content_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub content_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_button: TemplateChild<gtk::Button>,

        pub game: RefCell<Option<Game>>,
        pub current_step: Cell<usize>,
        pub all_steps: RefCell<Vec<steps::SetupStep>>,
        pub selected_mods: RefCell<Vec<usize>>,
        pub language_selection: RefCell<Option<config::LanguageSelection>>,
        // Steam/Proton status for the current visit to the Steam step. Refreshed
        // whenever the user navigates or asks to check again.
        pub steam_config_status: RefCell<Option<common::SteamConfigStatus>>,
        // Per-mod download sizes for the mod list, fetched once in the background.
        pub(super) download_estimates: RefCell<Option<Rc<Vec<ModDownloadEstimate>>>>,
        pub download_estimates_requested: Cell<bool>,
        pub download_size_label: RefCell<Option<gtk::Label>>,
        pub cancel_flag: RefCell<Option<Arc<AtomicBool>>>,
        pub is_error: Cell<bool>,
        pub step_busy: Cell<bool>,
        // True while a blocking download task is running; cleared on completion.
        // The cancel handler polls this before re-showing the step.
        pub task_running: Cell<bool>,
        // SourceId of the cancel poll timer; removed by the task completion path
        // to prevent the poll from firing after a normal (non-cancelled) finish.
        pub poll_source: RefCell<Option<glib::SourceId>>,
    }

    impl std::fmt::Debug for AdventureModsSetupPage {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AdventureModsSetupPage").finish()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsSetupPage {
        const NAME: &'static str = "AdventureModsSetupPage";
        type Type = super::AdventureModsSetupPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AdventureModsSetupPage {
        fn constructed(&self) {
            self.parent_constructed();

            sync_content_fade_duration(&self.content_revealer);
            if let Some(settings) = gtk::Settings::default() {
                let revealer = self.content_revealer.get();
                settings.connect_gtk_enable_animations_notify(move |_| {
                    sync_content_fade_duration(&revealer);
                });
            }

            let obj = self.obj().clone();
            self.next_button.connect_clicked(move |_| {
                if crate::ui::catch_ui_panic("setup next button", || obj.on_next_clicked()).is_err()
                {
                    obj.show_error(
                        "Something went wrong while continuing setup. Please try again.",
                    );
                }
            });

            let obj = self.obj().clone();
            self.back_button.connect_clicked(move |_| {
                if crate::ui::catch_ui_panic("setup back button", || obj.on_back_clicked()).is_err()
                {
                    obj.show_error("Something went wrong while going back. Please try again.");
                }
            });
        }
    }

    impl WidgetImpl for AdventureModsSetupPage {}
    impl BinImpl for AdventureModsSetupPage {}
}

enum ProgressMsg {
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
struct ProgressSamples {
    step: Mutex<Option<(u64, Option<u64>, String)>>,
    mods: Mutex<HashMap<String, (u64, Option<u64>)>>,
    mod_total: AtomicUsize,
}

impl ProgressSamples {
    fn set_step_bytes(&self, downloaded: u64, total: Option<u64>, status: impl Into<String>) {
        *self.step.lock().expect("progress samples lock") =
            Some((downloaded, total, status.into()));
    }

    fn set_mod_bytes(
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

    fn clear_mod(&self, mod_name: &str) {
        self.mods
            .lock()
            .expect("progress samples lock")
            .remove(mod_name);
    }

    fn clear(&self) {
        *self.step.lock().expect("progress samples lock") = None;
        self.mods.lock().expect("progress samples lock").clear();
        self.mod_total.store(0, Ordering::Relaxed);
    }

    fn apply_to(&self, state: &mut ProgressState) {
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

fn wake_progress_ui(tx: &async_channel::Sender<ProgressMsg>) {
    let _ = tx.try_send(ProgressMsg::Refresh);
}

fn publish_step_bytes(
    tx: &async_channel::Sender<ProgressMsg>,
    samples: &ProgressSamples,
    downloaded: u64,
    total: Option<u64>,
    status: &str,
) {
    samples.set_step_bytes(downloaded, total, status);
    wake_progress_ui(tx);
}

fn publish_mod_bytes(
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

fn content_animations_enabled() -> bool {
    gtk::Settings::default()
        .map(|settings| settings.is_gtk_enable_animations())
        .unwrap_or(true)
}

fn sync_content_fade_duration(revealer: &gtk::Revealer) {
    revealer.set_transition_duration(if content_animations_enabled() {
        CONTENT_FADE_MS
    } else {
        0
    });
}

glib::wrapper! {
    pub struct AdventureModsSetupPage(ObjectSubclass<imp::AdventureModsSetupPage>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[derive(Clone, Debug, PartialEq)]
struct ProgressDisplay {
    fraction: Option<f64>,
    pulse: bool,
    text: String,
}

fn fraction_needs_update(previous: Option<&ProgressDisplay>, fraction: f64) -> bool {
    previous.is_none_or(|previous| {
        previous.pulse
            || previous
                .fraction
                .is_none_or(|previous| (previous - fraction).abs() >= 0.001)
    })
}

#[derive(Default)]
struct ProgressState {
    completed_mods: usize,
    active_downloads: HashMap<String, (u64, Option<u64>)>,
    display: Option<ProgressDisplay>,
}

impl ProgressState {
    fn apply(&mut self, msg: ProgressMsg) {
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

    fn render(&self, progress_bar: &gtk::ProgressBar, previous: &mut Option<ProgressDisplay>) {
        let Some(display) = self.display.as_ref() else {
            return;
        };

        if display.pulse {
            progress_bar.pulse();
        } else if let Some(fraction) = display.fraction {
            let changed = fraction_needs_update(previous.as_ref(), fraction);
            if changed {
                progress_bar.set_fraction(fraction);
            }
        }

        if previous.as_ref().map(|previous| previous.text.as_str()) != Some(display.text.as_str()) {
            progress_bar.set_text(Some(&display.text));
        }

        *previous = Some(display.clone());
    }
}

fn drain_progress_updates(
    receiver: &async_channel::Receiver<ProgressMsg>,
    state: &mut ProgressState,
) {
    while let Ok(msg) = receiver.try_recv() {
        state.apply(msg);
    }
}

fn spawn_progress_receiver(
    progress_bar: gtk::ProgressBar,
    receiver: async_channel::Receiver<ProgressMsg>,
    samples: Arc<ProgressSamples>,
) {
    glib::spawn_future_local(async move {
        let mut state = ProgressState::default();
        let mut previous = None;
        let mut last_render: Option<Instant> = None;

        while let Ok(msg) = receiver.recv().await {
            state.apply(msg);

            if let Some(last_render) = last_render {
                let elapsed = last_render.elapsed();
                if elapsed < PROGRESS_UPDATE_INTERVAL {
                    glib::timeout_future(PROGRESS_UPDATE_INTERVAL - elapsed).await;
                }
            }

            drain_progress_updates(&receiver, &mut state);
            samples.apply_to(&mut state);

            state.render(&progress_bar, &mut previous);
            last_render = Some(Instant::now());
        }
    });
}

fn initial_preview_index(mod_count: usize, selected_mods: &[usize]) -> Option<usize> {
    selected_mods
        .iter()
        .copied()
        .find(|&idx| idx < mod_count)
        .or_else(|| (mod_count > 0).then_some(0))
}

fn subtitle_language_labels(game_kind: crate::steam::game::GameKind) -> Vec<&'static str> {
    config::SubtitleLanguage::supported_for(game_kind)
        .iter()
        .map(|language| language.label())
        .collect()
}

fn voice_language_labels() -> Vec<&'static str> {
    config::VoiceLanguage::all()
        .iter()
        .map(|language| language.label())
        .collect()
}

fn completed_mod_fraction(completed: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }

    completed as f64 / total as f64
}

fn format_mb_value(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
}

fn format_mb(bytes: u64) -> String {
    format!("{} MB", format_mb_value(bytes))
}

fn mod_download_fraction(
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

fn format_step_download_text(status: &str, downloaded: u64, total: Option<u64>) -> String {
    let bytes_text = format_download_bytes_text(downloaded, total);

    if status.is_empty() {
        bytes_text
    } else {
        format!("{status} - {bytes_text}")
    }
}

fn mod_download_start_text(mod_name: &str) -> String {
    format!("Starting {mod_name}...")
}

fn mod_download_finished_text(mod_name: &str, completed: usize, total: usize) -> String {
    format!("Installed {mod_name} ({completed}/{total})")
}

fn format_download_bytes_text(downloaded: u64, total: Option<u64>) -> String {
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

struct ModDownloadProgressUpdate {
    fraction: f64,
    pulse: bool,
    text: String,
}

fn mod_download_progress_update(
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

fn subtitle_language_index(
    game_kind: crate::steam::game::GameKind,
    language: config::SubtitleLanguage,
) -> u32 {
    config::SubtitleLanguage::supported_for(game_kind)
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0) as u32
}

fn voice_language_index(language: config::VoiceLanguage) -> u32 {
    config::VoiceLanguage::all()
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0) as u32
}

#[derive(Clone)]
struct ModPreviewPage {
    widget: gtk::Widget,
    picture: gtk::Picture,
    resource: &'static str,
}

#[derive(Default)]
struct ModPreviewState {
    current_index: Option<usize>,
    pages: HashMap<usize, Vec<ModPreviewPage>>,
    // Least recently shown first.
    recent: VecDeque<usize>,
}

impl ModPreviewState {
    fn cached_pages(&mut self, index: usize) -> Option<Vec<ModPreviewPage>> {
        let pages = self.pages.get(&index)?.clone();
        self.touch(index);
        Some(pages)
    }

    fn insert_pages(&mut self, index: usize, pages: Vec<ModPreviewPage>) {
        self.pages.insert(index, pages);
        self.touch(index);
        while self.recent.len() > MOD_PREVIEW_CACHE_LIMIT {
            if let Some(evicted) = self.recent.pop_front() {
                self.pages.remove(&evicted);
            }
        }
    }

    fn touch(&mut self, index: usize) {
        self.recent.retain(|&cached| cached != index);
        self.recent.push_back(index);
    }
}

#[derive(Clone)]
struct ModPreview {
    game_kind: crate::steam::game::GameKind,
    title_label: gtk::Label,
    carousel: adw::Carousel,
    carousel_frame: gtk::Frame,
    description_label: gtk::Label,
    links_box: gtk::FlowBox,
    state: Rc<RefCell<ModPreviewState>>,
    hover_source: Rc<RefCell<Option<glib::SourceId>>>,
    // Bumped whenever another mod is shown so stale texture loads stop early.
    load_generation: Rc<Cell<u64>>,
}

impl ModPreview {
    fn new(
        game_kind: crate::steam::game::GameKind,
        title_label: &gtk::Label,
        carousel: &adw::Carousel,
        carousel_frame: &gtk::Frame,
        description_label: &gtk::Label,
        links_box: &gtk::FlowBox,
    ) -> Self {
        Self {
            game_kind,
            title_label: title_label.clone(),
            carousel: carousel.clone(),
            carousel_frame: carousel_frame.clone(),
            description_label: description_label.clone(),
            links_box: links_box.clone(),
            state: Rc::new(RefCell::new(ModPreviewState::default())),
            hover_source: Rc::new(RefCell::new(None)),
            load_generation: Rc::new(Cell::new(0)),
        }
    }

    fn show(&self, index: usize) {
        let mods = common::recommended_mods_for_game(self.game_kind);
        self.show_entry(Some(index), mods.get(index));
    }

    fn show_entry(&self, index: Option<usize>, mod_entry: Option<&common::ModEntry>) {
        if self.state.borrow().current_index == index {
            return;
        }

        clear_carousel(&self.carousel);
        let (name, description, links) = if let Some(mod_entry) = mod_entry {
            (
                mod_entry.name,
                mod_entry.full_description.unwrap_or(mod_entry.description),
                mod_entry.links,
            )
        } else {
            ("", "", &[][..])
        };

        let pages = match (index, mod_entry) {
            (Some(index), Some(mod_entry)) => {
                let cached_pages = self.state.borrow_mut().cached_pages(index);
                if let Some(pages) = cached_pages {
                    pages
                } else {
                    let pages = build_mod_preview_pages(mod_entry);
                    self.state.borrow_mut().insert_pages(index, pages.clone());
                    pages
                }
            }
            _ => Vec::new(),
        };

        self.title_label.set_label(name);
        self.carousel_frame.set_visible(!pages.is_empty());
        for page in &pages {
            self.carousel.append(&page.widget);
        }
        self.load_textures(&pages);
        self.description_label.set_label(description);

        while let Some(child) = self.links_box.first_child() {
            self.links_box.remove(&child);
        }
        for link in links {
            let button = gtk::LinkButton::builder()
                .label(link.label)
                .uri(link.url)
                .halign(gtk::Align::Start)
                .build();
            self.links_box.insert(&button, -1);
        }

        self.state.borrow_mut().current_index = index;
    }

    fn queue_hover(&self, index: usize) {
        self.cancel_hover();

        let preview = self.clone();
        let source_id = glib::timeout_add_local(MOD_PREVIEW_HOVER_DELAY, move || {
            preview.hover_source.borrow_mut().take();
            preview.show(index);
            glib::ControlFlow::Break
        });
        self.hover_source.borrow_mut().replace(source_id);
    }

    fn cancel_hover(&self) {
        if let Some(source_id) = self.hover_source.borrow_mut().take() {
            source_id.remove();
        }
    }

    /// Decode the shown mod's screenshots off the UI thread, first page first.
    /// Hovering through the list stays responsive because only the visible mod
    /// keeps decoding; pages already decoded are reused from the cache.
    fn load_textures(&self, pages: &[ModPreviewPage]) {
        let generation = self.load_generation.get().wrapping_add(1);
        self.load_generation.set(generation);

        let pending: Vec<_> = pages
            .iter()
            .filter(|page| page.picture.paintable().is_none())
            .map(|page| (page.picture.clone(), page.resource))
            .collect();
        if pending.is_empty() {
            return;
        }

        let current_generation = self.load_generation.clone();
        glib::spawn_future_local(async move {
            for (picture, resource) in pending {
                if current_generation.get() != generation {
                    return;
                }
                match blocking::flatten_spawn_result(
                    gio::spawn_blocking(move || load_preview_texture(resource)).await,
                ) {
                    Ok(texture) => picture.set_paintable(Some(&texture)),
                    Err(err) => tracing::warn!("Failed to load mod preview {resource}: {err}"),
                }
            }
        });
    }
}

/// Decode a bundled screenshot. Safe to call from a worker thread.
fn load_preview_texture(resource: &str) -> anyhow::Result<gdk::Texture> {
    use glib::translate::{FromGlibPtrFull, ToGlibPtr};

    let bytes = gio::resources_lookup_data(resource, gio::ResourceLookupFlags::NONE)?;
    let mut error = std::ptr::null_mut();
    // SAFETY: gdk_texture_new_from_bytes is documented as threadsafe so images can
    // be decoded off the main thread; the gtk-rs wrapper only asserts the main
    // thread as a blanket rule. Ownership of the returned texture/error is full.
    let texture =
        unsafe { gdk::ffi::gdk_texture_new_from_bytes(bytes.to_glib_none().0, &mut error) };
    if error.is_null() {
        Ok(unsafe { gdk::Texture::from_glib_full(texture) })
    } else {
        Err(unsafe { glib::Error::from_glib_full(error) }.into())
    }
}

fn clear_carousel(carousel: &adw::Carousel) {
    let mut children = Vec::new();
    let mut child = carousel.first_child();
    while let Some(widget) = child {
        children.push(widget.clone());
        child = widget.next_sibling();
    }
    for child in children {
        carousel.remove(&child);
    }
}

fn build_mod_preview_pages(mod_entry: &common::ModEntry) -> Vec<ModPreviewPage> {
    mod_entry
        .pictures
        .iter()
        .map(|pic| {
            let image = gtk::Picture::builder()
                .can_shrink(true)
                .content_fit(gtk::ContentFit::Contain)
                .hexpand(true)
                .vexpand(true)
                .build();

            let badge_text = if pic.contains("_before") {
                Some("Before")
            } else if pic.contains("_after") {
                Some("After")
            } else {
                None
            };

            let widget = if let Some(text) = badge_text {
                let badge = gtk::Label::builder()
                    .label(text)
                    .halign(gtk::Align::Center)
                    .valign(gtk::Align::End)
                    .margin_bottom(8)
                    .css_classes(vec!["caption".to_string(), "osd".to_string()])
                    .build();
                let overlay = gtk::Overlay::builder()
                    .child(&image)
                    .hexpand(true)
                    .vexpand(true)
                    .build();
                overlay.add_overlay(&badge);
                overlay.upcast::<gtk::Widget>()
            } else {
                image.clone().upcast::<gtk::Widget>()
            };

            ModPreviewPage {
                widget,
                picture: image,
                resource: pic,
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModDownloadEstimate {
    size: Option<u64>,
    installed: bool,
}

/// Look up every mod's download size (in parallel) and whether it is installed.
fn estimate_mod_downloads(
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

fn remote_mod_download_size(mod_entry: &common::ModEntry) -> Option<u64> {
    // Unit tests render this page without a network.
    if cfg!(test) {
        return None;
    }
    common::mod_download_size(mod_entry).unwrap_or_else(|err| {
        tracing::debug!("No download size for {}: {err:#}", mod_entry.name);
        None
    })
}

fn format_download_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else {
        format!("{} MB", (bytes as f64 / 1_000_000.0).ceil().max(1.0) as u64)
    }
}

/// The line under the mod list saying how much the selection will download.
fn download_size_text(estimates: Option<&[ModDownloadEstimate]>, selected: &[usize]) -> String {
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

fn apply_install_progress(
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

impl AdventureModsSetupPage {
    pub fn new(game: Game) -> Self {
        let obj: Self = glib::Object::builder().build();
        let game_kind = game.kind;
        let all_steps = steps::steps_for_game(game.kind);
        obj.imp().all_steps.replace(all_steps);
        obj.imp().game.replace(Some(game));
        obj.imp()
            .language_selection
            .replace(Some(config::load_language_selection(
                config::app_settings().as_ref(),
                game_kind,
            )));

        let initial_step = obj.skip_completed_steps(0);
        obj.imp().current_step.set(initial_step);
        obj.show_current_step();
        obj
    }

    fn show_current_step(&self) {
        let imp = self.imp();
        let step_idx = imp.current_step.get();
        let all_steps = imp.all_steps.borrow();
        imp.is_error.set(false);
        imp.step_busy.set(false);

        if let Some(flag) = imp.cancel_flag.borrow().as_ref() {
            flag.store(true, Ordering::Relaxed);
        }
        imp.cancel_flag.replace(None);

        let Some(step) = all_steps.get(step_idx) else {
            return;
        };

        // Gate navigation immediately for steps that start work as soon as they render.
        if matches!(step.kind, steps::StepKind::Auto | steps::StepKind::Download) {
            imp.next_button.set_sensitive(false);
        }

        let is_last_step = step_idx + 1 >= all_steps.len();
        imp.back_button.set_visible(!is_last_step);
        imp.back_button
            .set_sensitive(!is_last_step && !imp.step_busy.get());

        drop(all_steps);
        self.transition_step_content();
    }

    fn current_step_prefers_instant_transition(&self) -> bool {
        let imp = self.imp();
        imp.all_steps
            .borrow()
            .get(imp.current_step.get())
            .is_some_and(|step| {
                matches!(step.kind, steps::StepKind::Auto | steps::StepKind::Download)
            })
    }

    /// Swap step body with a short fade-in. Work steps (Auto/Download) skip motion
    /// so progress UI appears immediately.
    fn transition_step_content(&self) {
        let imp = self.imp();
        let revealer = &imp.content_revealer;

        let instant = self.current_step_prefers_instant_transition()
            || !self.is_mapped()
            || !content_animations_enabled();

        if instant {
            revealer.set_transition_duration(0);
            self.render_current_step_content();
            revealer.set_reveal_child(true);
            sync_content_fade_duration(revealer);
            return;
        }

        // Fade-in only: hide without animating, rebuild, then crossfade in (~100 ms).
        revealer.set_transition_duration(0);
        revealer.set_reveal_child(false);
        self.render_current_step_content();
        sync_content_fade_duration(revealer);
        revealer.set_reveal_child(true);
    }

    /// Title, description, and layout stay in sync with the step body.
    fn apply_step_chrome(&self) {
        let imp = self.imp();
        let step_idx = imp.current_step.get();
        let all_steps = imp.all_steps.borrow();
        let Some(step) = all_steps.get(step_idx) else {
            return;
        };

        let centered_layout = !matches!(step.kind, steps::StepKind::ModSelection);
        imp.body_box.set_valign(if centered_layout {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        });
        imp.content_box.set_halign(if centered_layout {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        });
        imp.content_box.set_valign(if centered_layout {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        });
        imp.content_box.set_vexpand(!centered_layout);

        imp.step_title.set_label(step.title);
        let step_description = if step.id == StepId::SteamConfig {
            self.steam_config_status()
                .map(|status| status.message)
                .unwrap_or_else(|| step.description.to_string())
        } else {
            step.description.to_string()
        };
        imp.step_description.set_label(&step_description);
    }

    fn render_current_step_content(&self) {
        self.apply_step_chrome();

        let imp = self.imp();
        let step_idx = imp.current_step.get();
        let all_steps = imp.all_steps.borrow();
        let Some(step) = all_steps.get(step_idx) else {
            return;
        };
        let is_last_step = step_idx + 1 >= all_steps.len();

        while let Some(child) = imp.content_box.first_child() {
            imp.content_box.remove(&child);
        }
        self.render_step(step, is_last_step, &imp.content_box);
    }

    fn render_step(&self, step: &steps::SetupStep, is_last_step: bool, content_box: &gtk::Box) {
        let imp = self.imp();
        match &step.kind {
            steps::StepKind::Auto => {
                imp.next_button.set_label("Continue");
                imp.next_button.set_sensitive(false);

                let spinner = gtk::Spinner::builder()
                    .spinning(true)
                    .halign(gtk::Align::Center)
                    .build();
                content_box.append(&spinner);

                self.set_step_busy(true);
                self.run_auto_step(step.id);
            }
            steps::StepKind::Info => {
                let steam_config_ready =
                    step.id == StepId::SteamConfig && self.steam_config_ready();
                imp.next_button.set_label(
                    if step.id == StepId::SteamConfig && !steam_config_ready {
                        "Check Again"
                    } else if is_last_step {
                        "Finish"
                    } else {
                        "Continue"
                    },
                );
                imp.next_button.set_sensitive(true);

                if step.id == StepId::LanguageOptions {
                    let selection = self.current_language_selection();
                    let game_kind = imp.game.borrow().as_ref().map(|game| game.kind);
                    let subtitle_languages = game_kind
                        .map(config::SubtitleLanguage::supported_for)
                        .unwrap_or(config::SubtitleLanguage::supported_for(
                            crate::steam::game::GameKind::SADX,
                        ));

                    let form_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Vertical)
                        .spacing(12)
                        .hexpand(true)
                        .build();
                    let subtitle_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(12)
                        .build();
                    let subtitle_dropdown = gtk::DropDown::from_strings(&subtitle_language_labels(
                        game_kind.unwrap_or(crate::steam::game::GameKind::SADX),
                    ));
                    subtitle_dropdown.set_selected(subtitle_language_index(
                        game_kind.unwrap_or(crate::steam::game::GameKind::SADX),
                        selection.subtitle,
                    ));
                    subtitle_box.append(
                        &gtk::Label::builder()
                            .label("Subtitles")
                            .halign(gtk::Align::Start)
                            .hexpand(true)
                            .build(),
                    );
                    subtitle_box.append(&subtitle_dropdown);

                    let voice_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(12)
                        .build();
                    let voice_dropdown = gtk::DropDown::from_strings(&voice_language_labels());
                    voice_dropdown.set_selected(voice_language_index(selection.voice));
                    voice_box.append(
                        &gtk::Label::builder()
                            .label("Voice Language")
                            .halign(gtk::Align::Start)
                            .hexpand(true)
                            .build(),
                    );
                    voice_box.append(&voice_dropdown);

                    let obj = self.clone();
                    let subtitle_languages = subtitle_languages.to_vec();
                    subtitle_dropdown.connect_selected_notify(move |dropdown| {
                        let _ = crate::ui::catch_ui_panic("subtitle language selector", || {
                            let language = subtitle_languages
                                .get(dropdown.selected() as usize)
                                .copied()
                                .unwrap_or(config::SubtitleLanguage::English);
                            let mut selection = obj.current_language_selection();
                            selection.subtitle = language;
                            obj.imp().language_selection.replace(Some(selection));
                        });
                    });

                    let obj = self.clone();
                    voice_dropdown.connect_selected_notify(move |dropdown| {
                        let _ = crate::ui::catch_ui_panic("voice language selector", || {
                            let language = config::VoiceLanguage::all()
                                .get(dropdown.selected() as usize)
                                .copied()
                                .unwrap_or(config::VoiceLanguage::English);
                            let mut selection = obj.current_language_selection();
                            selection.voice = language;
                            obj.imp().language_selection.replace(Some(selection));
                        });
                    });

                    form_box.append(&subtitle_box);
                    form_box.append(&voice_box);
                    content_box.append(&form_box);
                }
            }
            steps::StepKind::Download => {
                imp.next_button.set_label("Continue");
                imp.next_button.set_sensitive(false);

                let progress_bar = gtk::ProgressBar::builder()
                    .show_text(true)
                    .hexpand(true)
                    .build();

                let cancel_flag = Arc::new(AtomicBool::new(false));
                imp.cancel_flag.replace(Some(cancel_flag.clone()));

                content_box.append(&progress_bar);

                if step.id == StepId::DownloadMods {
                    let cancel_button = gtk::Button::builder()
                        .label("Cancel")
                        .halign(gtk::Align::Center)
                        .css_classes(vec!["destructive-action".to_string()])
                        .build();

                    let flag = cancel_flag.clone();
                    let obj = self.clone();
                    cancel_button.connect_clicked(move |btn| {
                        if crate::ui::catch_ui_panic("download cancel button", || {
                            flag.store(true, Ordering::Relaxed);
                            btn.set_sensitive(false);
                            btn.set_label("Cancelling...");
                            // Poll until the blocking task has finished before re-showing
                            // the step, so we don't start a new task while the old one is
                            // still writing to disk.
                            let obj2 = obj.clone();
                            let source_id =
                                glib::timeout_add_local(Duration::from_millis(50), move || {
                                    if obj2.imp().task_running.get() {
                                        return glib::ControlFlow::Continue;
                                    }
                                    obj2.imp().poll_source.borrow_mut().take();
                                    obj2.show_current_step();
                                    glib::ControlFlow::Break
                                });
                            obj.imp().poll_source.replace(Some(source_id));
                        })
                        .is_err()
                        {
                            obj.show_error(
                                "Something went wrong while cancelling. Please try again.",
                            );
                        }
                    });

                    content_box.append(&cancel_button);
                }

                self.set_step_busy(true);
                self.run_download_step(step.id, progress_bar, cancel_flag);
            }
            steps::StepKind::ModSelection => self.render_mod_selection(content_box),
        }
    }

    fn render_mod_selection(&self, content_box: &gtk::Box) {
        let imp = self.imp();
        imp.next_button.set_label("Install Selected");
        imp.next_button.set_sensitive(true);

        let game_kind = imp.game.borrow().as_ref().map(|g| g.kind);
        let presets = game_kind.map(common::presets_for_game).unwrap_or(&[]);

        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .homogeneous(true)
            .spacing(24)
            .hexpand(true)
            .vexpand(true)
            .valign(gtk::Align::Fill)
            .halign(gtk::Align::Fill)
            .build();

        let left_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .vexpand(true)
            .valign(gtk::Align::Fill)
            .build();

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .valign(gtk::Align::Fill)
            .build();

        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(vec!["boxed-list".to_string()])
            .build();

        let checks: Rc<RefCell<Vec<gtk::CheckButton>>> = Rc::new(RefCell::new(Vec::new()));
        if !presets.is_empty() {
            let preset_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .margin_bottom(6)
                .build();

            let preset_label = gtk::Label::builder()
                .label("Preset:")
                .css_classes(vec!["heading".to_string()])
                .build();

            let preset_names: Vec<&str> = presets.iter().map(|p| p.name).collect();
            let dropdown = gtk::DropDown::from_strings(&preset_names);
            dropdown.set_hexpand(true);

            preset_box.append(&preset_label);
            preset_box.append(&dropdown);
            left_box.append(&preset_box);

            let preset_desc_label = gtk::Label::builder()
                .label(presets[0].description)
                .wrap(true)
                .halign(gtk::Align::Start)
                .css_classes(vec!["caption".to_string()])
                .margin_bottom(12)
                .build();
            left_box.append(&preset_desc_label);

            let obj_clone = self.clone();
            let presets_clone = presets;
            let checks_clone = checks.clone();
            let desc_label_clone = preset_desc_label.clone();
            dropdown.connect_selected_notify(move |dd| {
                let _ = crate::ui::catch_ui_panic("mod preset selector", || {
                    let idx = dd.selected() as usize;
                    if let Some(preset) = presets_clone.get(idx) {
                        desc_label_clone.set_label(preset.description);

                        let mut sel = obj_clone.imp().selected_mods.borrow_mut();
                        sel.clear();

                        let game_kind = obj_clone.imp().game.borrow().as_ref().map(|g| g.kind);
                        let mods_list = game_kind
                            .map(common::recommended_mods_for_game)
                            .unwrap_or(&[]);

                        for (i, check) in checks_clone.borrow().iter().enumerate() {
                            if let Some(mod_entry) = mods_list.get(i) {
                                let active = preset.mod_names.contains(&mod_entry.name);
                                check.set_active(active);
                                if active {
                                    sel.push(i);
                                }
                            }
                        }
                        drop(sel);
                        obj_clone.update_download_size_label();
                    }
                });
            });
        }

        let preview_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .vexpand(true)
            .valign(gtk::Align::Fill)
            .halign(gtk::Align::Fill)
            .build();

        let carousel = adw::Carousel::builder()
            .interactive(true)
            .allow_scroll_wheel(true)
            .vexpand(true)
            .build();

        let indicator = adw::CarouselIndicatorDots::builder()
            .carousel(&carousel)
            .margin_top(6)
            .margin_bottom(6)
            .build();

        let carousel_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        carousel_box.append(&carousel);
        carousel_box.append(&indicator);

        let carousel_aspect = gtk::AspectFrame::builder()
            .ratio(16.0_f32 / 9.0_f32)
            .obey_child(false)
            .xalign(0.5)
            .yalign(0.5)
            .hexpand(true)
            .vexpand(true)
            .child(&carousel_box)
            .build();

        let carousel_frame = gtk::Frame::builder()
            .child(&carousel_aspect)
            .hexpand(true)
            .vexpand(true)
            .build();

        let full_desc_label = gtk::Label::builder()
            .wrap(true)
            .max_width_chars(MOD_PREVIEW_TEXT_WIDTH_CHARS)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .css_classes(vec!["body".to_string()])
            .build();

        let desc_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .max_content_height(MOD_PREVIEW_DESCRIPTION_HEIGHT)
            .hexpand(true)
            .vexpand(true)
            .child(&full_desc_label)
            .build();

        let preview_title_label = gtk::Label::builder()
            .wrap(true)
            .max_width_chars(MOD_PREVIEW_TEXT_WIDTH_CHARS)
            .halign(gtk::Align::Start)
            .css_classes(vec!["title-3".to_string()])
            .build();

        let links_box = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(1)
            .max_children_per_line(3)
            .column_spacing(24)
            .row_spacing(6)
            .halign(gtk::Align::Start)
            .build();

        preview_box.append(&preview_title_label);
        preview_box.append(&carousel_frame);
        preview_box.append(&desc_scrolled);
        preview_box.append(&links_box);

        let mods_list = game_kind
            .map(common::recommended_mods_for_game)
            .unwrap_or(&[]);
        let preview_game_kind = game_kind.unwrap_or(crate::steam::game::GameKind::SADX);
        let preview = ModPreview::new(
            preview_game_kind,
            &preview_title_label,
            &carousel,
            &carousel_frame,
            &full_desc_label,
            &links_box,
        );
        let mut initial_selected = Vec::new();

        {
            let preview = preview.clone();
            list_box.connect_row_selected(move |_, row| {
                let _ = crate::ui::catch_ui_panic("mod row selection", || {
                    let Some(row) = row else { return };
                    preview.cancel_hover();
                    preview.show(row.index() as usize);
                });
            });
        }

        let default_preset = presets.first();

        for (i, mod_entry) in mods_list.iter().enumerate() {
            let row_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .margin_start(12)
                .margin_end(12)
                .margin_top(12)
                .margin_bottom(12)
                .hexpand(true)
                .build();

            let is_active = default_preset
                .map(|preset| preset.mod_names.contains(&mod_entry.name))
                .unwrap_or(true);

            let check = gtk::CheckButton::builder().active(is_active).build();
            checks.borrow_mut().push(check.clone());

            let text_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(2)
                .hexpand(true)
                .build();

            let name_label = gtk::Label::builder()
                .label(mod_entry.name)
                .halign(gtk::Align::Start)
                .css_classes(vec!["heading".to_string()])
                .build();

            let desc_label = gtk::Label::builder()
                .label(mod_entry.description)
                .halign(gtk::Align::Start)
                .wrap(true)
                .css_classes(vec!["caption".to_string()])
                .build();

            text_box.append(&name_label);
            text_box.append(&desc_label);

            row_box.append(&check);
            row_box.append(&text_box);

            let list_row = gtk::ListBoxRow::builder().child(&row_box).build();

            let obj_clone = self.clone();
            let idx = i;
            check.connect_toggled(move |btn| {
                let _ = crate::ui::catch_ui_panic("mod checkbox", || {
                    if let Ok(mut sel) = obj_clone.imp().selected_mods.try_borrow_mut() {
                        if btn.is_active() {
                            if !sel.contains(&idx) {
                                sel.push(idx);
                            }
                        } else {
                            sel.retain(|&x| x != idx);
                        }
                    } else {
                        // A preset is being applied; it refreshes the size once done.
                        return;
                    }
                    obj_clone.update_download_size_label();
                });
            });

            let preview_focus = preview.clone();
            check.connect_has_focus_notify(move |btn| {
                let _ = crate::ui::catch_ui_panic("mod checkbox focus", || {
                    if btn.has_focus() {
                        preview_focus.cancel_hover();
                        preview_focus.show(idx);
                    }
                });
            });

            let preview_enter = preview.clone();
            let preview_leave = preview.clone();
            let gesture = gtk::EventControllerMotion::new();
            gesture.connect_enter(move |_, _, _| {
                let _ = crate::ui::catch_ui_panic("mod row hover", || {
                    preview_enter.queue_hover(idx);
                });
            });
            gesture.connect_leave(move |_| preview_leave.cancel_hover());
            list_row.add_controller(gesture);

            list_box.append(&list_row);
            if is_active {
                initial_selected.push(i);
            }
        }
        imp.selected_mods.replace(initial_selected);

        if let Some(initial_index) =
            initial_preview_index(mods_list.len(), &imp.selected_mods.borrow())
        {
            preview.show(initial_index);
            if let Some(row) = list_box.row_at_index(initial_index as i32) {
                list_box.select_row(Some(&row));
            }
        }

        scrolled.set_child(Some(&list_box));
        left_box.append(&scrolled);

        let download_size_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(vec!["caption".to_string(), "dim-label".to_string()])
            .build();
        left_box.append(&download_size_label);
        imp.download_size_label.replace(Some(download_size_label));
        self.update_download_size_label();
        self.request_download_estimates();

        main_box.append(&left_box);
        main_box.append(&preview_box);
        content_box.append(&main_box);
    }

    fn update_download_size_label(&self) {
        let imp = self.imp();
        let Some(label) = imp.download_size_label.borrow().clone() else {
            return;
        };
        let estimates = imp.download_estimates.borrow().clone();
        label.set_label(&download_size_text(
            estimates.as_deref().map(Vec::as_slice),
            &imp.selected_mods.borrow(),
        ));
    }

    fn request_download_estimates(&self) {
        let imp = self.imp();
        if imp.download_estimates_requested.replace(true) {
            return;
        }
        let Some(game) = imp.game.borrow().clone() else {
            return;
        };

        let obj = self.clone();
        glib::spawn_future_local(async move {
            let mods = common::recommended_mods_for_game(game.kind);
            match blocking::spawn_result(
                gio::spawn_blocking(move || {
                    estimate_mod_downloads(&game.path, mods, remote_mod_download_size)
                })
                .await,
            ) {
                Ok(estimates) => {
                    obj.imp()
                        .download_estimates
                        .replace(Some(Rc::new(estimates)));
                }
                Err(err) => {
                    tracing::warn!("Failed to estimate mod downloads: {err}");
                    obj.imp()
                        .download_estimates
                        .replace(Some(Rc::new(Vec::new())));
                }
            }
            obj.update_download_size_label();
        });
    }

    fn run_auto_step(&self, step_id: StepId) {
        let obj = self.clone();
        let game = self.imp().game.borrow().clone();

        glib::spawn_future_local(async move {
            let result = match step_id {
                StepId::Dotnet => {
                    if let Some(ref game) = game {
                        common::install_runtimes(game.path.clone(), game.kind.app_id()).await
                    } else {
                        Ok(())
                    }
                }
                _ => Ok(()),
            };

            match result {
                Ok(()) => {
                    obj.set_step_busy(false);
                    obj.imp().next_button.set_sensitive(true);
                    obj.advance_step();
                }
                Err(e) => {
                    obj.set_step_busy(false);
                    obj.show_error(&format!("{e}"));
                }
            }
        });
    }

    fn run_download_step(
        &self,
        step_id: StepId,
        progress_bar: gtk::ProgressBar,
        cancel_flag: Arc<AtomicBool>,
    ) {
        let obj = self.clone();
        let game = self.imp().game.borrow().clone();
        self.imp().task_running.set(true);

        glib::spawn_future_local(async move {
            let Some(ref game) = game else {
                obj.set_step_busy(false);
                obj.imp().task_running.set(false);
                return;
            };

            let (tx, rx) = async_channel::bounded::<ProgressMsg>(32);
            let samples = Arc::new(ProgressSamples::default());

            spawn_progress_receiver(progress_bar.clone(), rx, samples.clone());

            let game_kind = game.kind;
            let (width, height) = obj.get_resolution();
            let result: anyhow::Result<()> = match step_id {
                StepId::ConvertSteam => {
                    let game_path = game.path.clone();
                    let tx_clone = tx.clone();
                    let samples = samples.clone();
                    let progress_fn: Option<crate::external::download::ProgressFn> =
                        Some(Box::new(move |dl, total| {
                            publish_step_bytes(&tx_clone, &samples, dl, total, "Downloading...");
                        }));
                    blocking::flatten_spawn_result(
                        gio::spawn_blocking(move || {
                            sadx::convert_steam_to_2004(&game_path, progress_fn)
                        })
                        .await,
                    )
                }
                StepId::InstallModManager => {
                    let game_path = game.path.clone();
                    let game_kind = game.kind;
                    let tx_clone = tx.clone();
                    let samples = samples.clone();
                    let progress_fn: Option<crate::external::download::ProgressFn> =
                        Some(Box::new(move |dl, total| {
                            publish_step_bytes(&tx_clone, &samples, dl, total, "Downloading...");
                        }));
                    blocking::flatten_spawn_result(
                        gio::spawn_blocking(move || {
                            common::install_mod_manager(&game_path, game_kind, progress_fn)
                        })
                        .await,
                    )
                }
                StepId::DownloadMods => {
                    let selected: Vec<usize> = obj.imp().selected_mods.borrow().clone();
                    let total_count = selected.len();
                    let game_path = game.path.clone();
                    let language_selection = obj.current_language_selection();
                    let was_cancelled = cancel_flag.clone();
                    let cancel_during_install = cancel_flag.clone();
                    let mods_list = common::recommended_mods_for_game(game_kind);
                    match blocking::flatten_spawn_result(
                        gio::spawn_blocking(move || {
                            let selected_entries: Vec<&common::ModEntry> = selected
                                .iter()
                                .filter_map(|idx| mods_list.get(*idx))
                                .collect();
                            pipeline::install_selected_mods_and_generate_config_with_progress(
                                &game_path,
                                game_kind,
                                &selected_entries,
                                width,
                                height,
                                language_selection,
                                |progress| {
                                    apply_install_progress(
                                        progress,
                                        &cancel_during_install,
                                        &tx,
                                        &samples,
                                        total_count,
                                    )
                                },
                            )
                        })
                        .await,
                    ) {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            if was_cancelled.load(Ordering::Relaxed) {
                                obj.set_step_busy(false);
                                obj.imp().task_running.set(false);
                                return; // Was cancelled
                            }
                            Err(e)
                        }
                    }
                }
                _ => Ok(()),
            };

            obj.set_step_busy(false);
            obj.imp().task_running.set(false);
            // If the task finished without cancellation, stop the cancel poll so
            // it doesn't re-run the current step after advance_step() has already
            // moved on.
            if let Some(source_id) = obj.imp().poll_source.borrow_mut().take() {
                source_id.remove();
            }
            match result {
                Ok(()) => {
                    obj.imp().next_button.set_sensitive(true);
                    obj.advance_step();
                }
                Err(e) => {
                    obj.show_error(&format!("{e}"));
                }
            }
        });
    }

    fn set_step_busy(&self, busy: bool) {
        let imp = self.imp();
        imp.step_busy.set(busy);
        if imp.back_button.is_visible() {
            imp.back_button.set_sensitive(!busy);
        }
    }

    fn get_resolution(&self) -> (u32, u32) {
        let surface = self.native().and_then(|n| n.surface());
        let display = self.display();
        crate::display::resolution_from_display(&display, surface.as_ref()).unwrap_or_else(|| {
            tracing::warn!("Could not detect monitor resolution, using fallback 1920x1080");
            (1920, 1080)
        })
    }

    fn steam_config_ready(&self) -> bool {
        self.steam_config_status()
            .is_some_and(|status| status.ready)
    }

    fn steam_config_status(&self) -> Option<common::SteamConfigStatus> {
        let imp = self.imp();
        if let Some(status) = imp.steam_config_status.borrow().clone() {
            return Some(status);
        }
        let status = imp
            .game
            .borrow()
            .as_ref()
            .map(common::steam_config_status)?;
        imp.steam_config_status.replace(Some(status.clone()));
        Some(status)
    }

    fn invalidate_steam_config_status(&self) {
        self.imp().steam_config_status.replace(None);
    }

    fn advance_step(&self) {
        let imp = self.imp();
        let next = imp.current_step.get() + 1;
        let total = imp.all_steps.borrow().len();

        if next < total {
            let next_skipped = self.skip_completed_steps(next);
            imp.current_step.set(next_skipped);
            self.show_current_step();
        }
    }

    fn skip_completed_steps(&self, start_idx: usize) -> usize {
        let imp = self.imp();
        let all_steps = imp.all_steps.borrow();
        let Some(game) = imp.game.borrow().clone() else {
            tracing::warn!("Cannot skip completed setup steps without a selected game");
            return start_idx;
        };
        let Some(last_step_idx) = all_steps.len().checked_sub(1) else {
            return start_idx;
        };
        let mut idx = start_idx;

        // Skip steps that are already complete, but NEVER skip the last step
        // (the completion screen).
        while idx < last_step_idx {
            let step = &all_steps[idx];
            if common::is_step_complete(step.id, &game) {
                tracing::info!("Auto-skipping completed step: {}", step.id);
                idx += 1;
            } else {
                break;
            }
        }
        idx
    }

    fn on_next_clicked(&self) {
        let imp = self.imp();

        // Ignore clicks while a step is running work (Auto/Download).
        if imp.step_busy.get() {
            return;
        }
        // Continue / Check Again should see what Steam looks like right now.
        self.invalidate_steam_config_status();

        if imp.is_error.get() {
            // Retry: re-run the current step
            self.show_current_step();
            return;
        }

        let Some(current_step_id) = imp
            .all_steps
            .borrow()
            .get(imp.current_step.get())
            .map(|step| step.id)
        else {
            tracing::warn!(
                "Setup next button clicked with invalid step index {}",
                imp.current_step.get()
            );
            self.show_error("Setup state is out of date. Please try again.");
            return;
        };
        let next = imp.current_step.get() + 1;
        let total = imp.all_steps.borrow().len();
        if next >= total {
            // Last step: navigate back to the welcome page
            self.go_back_to_welcome();
        } else {
            if current_step_id == StepId::SteamConfig && !self.steam_config_ready() {
                self.show_current_step();
                return;
            }
            if current_step_id == StepId::LanguageOptions {
                self.persist_language_selection();
            }
            self.advance_step();
        }
    }

    fn current_language_selection(&self) -> config::LanguageSelection {
        self.imp().language_selection.borrow().unwrap_or_else(|| {
            self.imp()
                .game
                .borrow()
                .as_ref()
                .map(|game| config::LanguageSelection::defaults_for(game.kind))
                .unwrap_or(config::LanguageSelection::defaults_for(
                    crate::steam::game::GameKind::SADX,
                ))
        })
    }

    fn persist_language_selection(&self) {
        let Some(game) = self.imp().game.borrow().clone() else {
            return;
        };

        config::save_language_selection(
            config::app_settings().as_ref(),
            game.kind,
            self.current_language_selection(),
        );
    }

    fn on_back_clicked(&self) {
        self.invalidate_steam_config_status();
        let imp = self.imp();
        let current = imp.current_step.get();
        if current == 0 {
            self.go_back_to_welcome();
            return;
        }

        let all_steps = imp.all_steps.borrow();
        let Some(game) = imp.game.borrow().clone() else {
            tracing::warn!("Setup back button clicked without a selected game");
            self.go_back_to_welcome();
            return;
        };
        let mut prev = current - 1;

        // Skip steps backwards that are either automatic or already complete
        while prev > 0 {
            let Some(step) = all_steps.get(prev) else {
                tracing::warn!("Setup back button clicked with invalid step index {prev}");
                self.show_error("Setup state is out of date. Please try again.");
                return;
            };
            if matches!(step.kind, steps::StepKind::Auto)
                || common::is_step_complete(step.id, &game)
            {
                prev -= 1;
            } else {
                break;
            }
        }

        // Final check for the step we landed on: if it's still something that should be skipped,
        // it means we've reached the beginning of the list and everything before 'current' was skippable.
        let Some(step) = all_steps.get(prev) else {
            tracing::warn!("Setup back button landed on invalid step index {prev}");
            self.show_error("Setup state is out of date. Please try again.");
            return;
        };
        if matches!(step.kind, steps::StepKind::Auto) || common::is_step_complete(step.id, &game) {
            self.go_back_to_welcome();
        } else {
            imp.current_step.set(prev);
            self.show_current_step();
        }
    }

    fn go_back_to_welcome(&self) {
        if let Some(nav_view) = self.ancestor(adw::NavigationView::static_type()) {
            if let Ok(nav_view) = nav_view.downcast::<adw::NavigationView>() {
                nav_view.pop();
            } else {
                tracing::warn!("Setup page ancestor was not a NavigationView");
            }
        }
    }

    fn show_error(&self, message: &str) {
        let imp = self.imp();
        imp.is_error.set(true);

        // Show errors immediately; do not wait on the step fade-in.
        imp.content_revealer.set_transition_duration(0);
        imp.content_revealer.set_reveal_child(true);
        sync_content_fade_duration(&imp.content_revealer);

        let content_box = &imp.content_box;
        while let Some(child) = content_box.first_child() {
            content_box.remove(&child);
        }
        let label = gtk::Label::builder()
            .label(message)
            .wrap(true)
            .css_classes(vec!["error".to_string()])
            .build();
        content_box.append(&label);

        imp.next_button.set_label("Retry");
        imp.next_button.set_sensitive(true);
    }
}

#[cfg(test)]
mod tests {
    use adw::prelude::*;
    use adw::subclass::prelude::ObjectSubclassIsExt;
    use gtk::glib;

    use super::AdventureModsSetupPage;
    use super::{
        MOD_PREVIEW_CACHE_LIMIT, ModDownloadEstimate, ModPreview, ProgressDisplay, ProgressMsg,
        ProgressSamples, ProgressState, apply_install_progress, completed_mod_fraction,
        download_size_text, drain_progress_updates, estimate_mod_downloads,
        format_download_bytes_text, format_download_size, format_step_download_text,
        fraction_needs_update, initial_preview_index, load_preview_texture,
        mod_download_finished_text, mod_download_fraction, mod_download_progress_update,
        mod_download_start_text, publish_mod_bytes, publish_step_bytes, spawn_progress_receiver,
        subtitle_language_index, subtitle_language_labels, voice_language_index,
        voice_language_labels,
    };
    use crate::setup::config::{SubtitleLanguage, VoiceLanguage};
    use crate::setup::steps::StepId;
    use crate::setup::{common, pipeline, steps};
    use crate::steam::game::Game;
    use crate::steam::game::GameKind;
    use crate::ui::test_util::init_resource_overlay;

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
    fn determinate_progress_refreshes_after_pulse_mode() {
        let previous = ProgressDisplay {
            fraction: Some(0.25),
            pulse: true,
            text: "Downloading mods - 1.0 MB".to_string(),
        };

        assert!(fraction_needs_update(Some(&previous), 0.25));
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
        assert!(fraction_needs_update(None, 0.5));
        assert!(!fraction_needs_update(
            Some(&ProgressDisplay {
                fraction: Some(0.5),
                pulse: false,
                text: String::new(),
            }),
            0.5005
        ));
        assert!(fraction_needs_update(
            Some(&ProgressDisplay {
                fraction: None,
                pulse: false,
                text: String::new(),
            }),
            0.5
        ));
    }

    #[gtk::test]
    fn progress_render_updates_pulse_and_determinate_bars() {
        init_resource_overlay();

        let progress_bar = gtk::ProgressBar::new();
        let mut previous = None;
        let mut state = ProgressState {
            display: Some(ProgressDisplay {
                fraction: None,
                pulse: true,
                text: "Working".to_string(),
            }),
            ..ProgressState::default()
        };
        state.render(&progress_bar, &mut previous);
        assert!(previous.is_some());

        state.display = Some(ProgressDisplay {
            fraction: Some(0.75),
            pulse: false,
            text: "Done".to_string(),
        });
        state.render(&progress_bar, &mut previous);
        assert_eq!(progress_bar.fraction(), 0.75);

        state.display = None;
        state.render(&progress_bar, &mut previous);
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

    #[gtk::test]
    fn progress_receiver_drains_messages_and_samples() {
        init_resource_overlay();

        let (sender, receiver) = async_channel::bounded(4);
        let samples = std::sync::Arc::new(ProgressSamples::default());
        let progress_bar = gtk::ProgressBar::new();
        spawn_progress_receiver(progress_bar.clone(), receiver, samples.clone());
        sender
            .send_blocking(ProgressMsg::Bytes {
                downloaded: 1,
                total: Some(2),
                status: "First".to_string(),
            })
            .unwrap();
        samples.set_step_bytes(1_048_576, Some(2_097_152), "Latest");
        sender.send_blocking(ProgressMsg::Refresh).unwrap();
        drop(sender);

        for _ in 0..100 {
            while glib::MainContext::default().iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(progress_bar.text().is_some());
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

    #[gtk::test]
    fn mod_preview_decodes_screenshots_off_the_ui_thread() {
        init_resource_overlay();

        let carousel = adw::Carousel::new();
        let preview = ModPreview::new(
            GameKind::SADX,
            &gtk::Label::new(None),
            &carousel,
            &gtk::Frame::new(None),
            &gtk::Label::new(None),
            &gtk::FlowBox::new(),
        );
        let mods = common::recommended_mods_for_game(GameKind::SADX);
        let (index, mod_entry) = mods
            .iter()
            .enumerate()
            .find(|(_, entry)| !entry.pictures.is_empty())
            .expect("a SADX mod with screenshots");

        preview.show_entry(Some(index), Some(mod_entry));
        let pages = preview.state.borrow().pages[&index].clone();
        assert_eq!(pages.len(), mod_entry.pictures.len());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while pages.iter().any(|page| page.picture.paintable().is_none()) {
            assert!(
                std::time::Instant::now() < deadline,
                "screenshots did not load"
            );
            glib::MainContext::default().iteration(false);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // Showing another mod and coming back reuses the decoded textures.
        preview.show_entry(None, None);
        preview.show_entry(Some(index), Some(mod_entry));
        let cached = preview.state.borrow().pages[&index].clone();
        assert!(cached[0].picture.paintable().is_some());
        assert_eq!(cached[0].picture, pages[0].picture);
    }

    #[gtk::test]
    fn mod_preview_cache_is_bounded() {
        init_resource_overlay();

        let preview = ModPreview::new(
            GameKind::SADX,
            &gtk::Label::new(None),
            &adw::Carousel::new(),
            &gtk::Frame::new(None),
            &gtk::Label::new(None),
            &gtk::FlowBox::new(),
        );
        let mods = common::recommended_mods_for_game(GameKind::SADX);
        assert!(mods.len() > MOD_PREVIEW_CACHE_LIMIT + 1);

        for (index, mod_entry) in mods.iter().enumerate().take(MOD_PREVIEW_CACHE_LIMIT + 1) {
            preview.show_entry(Some(index), Some(mod_entry));
        }
        // Revisit the oldest so it becomes the most recently used entry.
        preview.show_entry(Some(1), mods.get(1));
        preview.show_entry(Some(0), mods.first());

        let state = preview.state.borrow();
        assert_eq!(state.pages.len(), MOD_PREVIEW_CACHE_LIMIT);
        assert_eq!(state.recent.len(), MOD_PREVIEW_CACHE_LIMIT);
        assert!(state.pages.contains_key(&0));
        assert!(state.pages.contains_key(&1));
        assert!(!state.pages.contains_key(&2));
        assert_eq!(state.recent.back(), Some(&0));
    }

    #[gtk::test]
    fn load_preview_texture_reports_missing_and_invalid_resources() {
        init_resource_overlay();

        assert!(load_preview_texture("/io/github/astrovm/AdventureMods/missing.jpg").is_err());
        assert!(
            load_preview_texture("/io/github/astrovm/AdventureMods/resources/ui/window.ui")
                .is_err()
        );
    }

    #[gtk::test]
    fn mod_preview_handles_empty_cached_and_hover_states() {
        init_resource_overlay();

        let title = gtk::Label::new(None);
        let carousel = adw::Carousel::new();
        let carousel_frame = gtk::Frame::new(None);
        let description = gtk::Label::new(None);
        let links = gtk::FlowBox::new();
        let preview = ModPreview::new(
            GameKind::SADX,
            &title,
            &carousel,
            &carousel_frame,
            &description,
            &links,
        );
        let mods = common::recommended_mods_for_game(GameKind::SADX);

        preview.show_entry(Some(0), mods.first());
        assert!(!carousel_frame.is_visible() || carousel.first_child().is_some());
        assert_eq!(title.label().as_str(), mods[0].name);

        preview.show_entry(Some(1), mods.get(1));
        preview.show_entry(Some(0), mods.first());
        preview.show_entry(None, None);
        assert_eq!(title.label().as_str(), "");
        assert!(!carousel_frame.is_visible());

        preview.queue_hover(0);
        preview.cancel_hover();
        assert!(preview.hover_source.borrow().is_none());
        preview.queue_hover(0);
        std::thread::sleep(std::time::Duration::from_millis(70));
        while glib::MainContext::default().iteration(false) {}
        assert_eq!(preview.state.borrow().current_index, Some(0));
    }

    #[gtk::test]
    fn setup_page_renders_info_language_and_download_controls() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        let all_steps = page.imp().all_steps.borrow().clone();

        let steam_step = all_steps
            .iter()
            .find(|step| step.id == StepId::SteamConfig)
            .unwrap()
            .clone();
        let steam_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.render_step(&steam_step, false, &steam_content);
        assert_eq!(
            page.imp().next_button.label().as_deref(),
            Some("Check Again")
        );

        let language_step = all_steps
            .iter()
            .find(|step| step.id == StepId::LanguageOptions)
            .unwrap()
            .clone();
        let language_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.render_step(&language_step, false, &language_content);

        let form = language_content
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let subtitle_box = form.first_child().unwrap().downcast::<gtk::Box>().unwrap();
        let subtitle_dropdown = subtitle_box
            .last_child()
            .unwrap()
            .downcast::<gtk::DropDown>()
            .unwrap();
        subtitle_dropdown.set_selected(1);

        let voice_box = form.last_child().unwrap().downcast::<gtk::Box>().unwrap();
        let voice_dropdown = voice_box
            .last_child()
            .unwrap()
            .downcast::<gtk::DropDown>()
            .unwrap();
        voice_dropdown.set_selected(0);
        voice_dropdown.set_selected(1);
        assert_eq!(
            page.imp().language_selection.borrow().unwrap().voice,
            VoiceLanguage::English
        );

        page.imp().game.replace(None);
        page.imp().language_selection.replace(None);
        let fallback_language_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.render_step(&language_step, false, &fallback_language_content);
        assert_eq!(
            fallback_language_content.first_child().unwrap().type_(),
            gtk::Box::static_type()
        );

        let complete_step = all_steps
            .iter()
            .find(|step| step.id == StepId::Complete)
            .unwrap()
            .clone();
        let complete_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.render_step(&complete_step, true, &complete_content);
        assert_eq!(page.imp().next_button.label().as_deref(), Some("Finish"));

        let download_step = all_steps
            .iter()
            .find(|step| step.id == StepId::DownloadMods)
            .unwrap()
            .clone();
        let download_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.render_step(&download_step, false, &download_content);
        assert_eq!(download_content.observe_children().n_items(), 2);
        let cancel_button = download_content
            .last_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap();
        cancel_button.emit_clicked();
        assert_eq!(cancel_button.label().as_deref(), Some("Cancelling..."));
        page.imp().task_running.set(false);
        page.imp().current_step.set(
            page.imp()
                .all_steps
                .borrow()
                .iter()
                .position(|step| step.id == StepId::Complete)
                .unwrap(),
        );
        std::thread::sleep(std::time::Duration::from_millis(60));
        while glib::MainContext::default().iteration(false) {}
    }

    #[gtk::test]
    fn no_game_auto_steps_complete_without_running_external_work() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        page.imp().game.replace(None);
        page.imp().all_steps.replace(vec![
            steps::SetupStep {
                id: StepId::Dotnet,
                title: "Runtime",
                description: "Runtime",
                kind: steps::StepKind::Auto,
            },
            steps::SetupStep {
                id: StepId::Complete,
                title: "Complete",
                description: "Complete",
                kind: steps::StepKind::Info,
            },
        ]);
        page.imp().current_step.set(0);

        page.run_auto_step(StepId::Dotnet);
        while glib::MainContext::default().iteration(false) {}
        assert_eq!(page.imp().current_step.get(), 1);

        page.run_auto_step(StepId::ConvertSteam);
        while glib::MainContext::default().iteration(false) {}
        assert_eq!(page.imp().current_step.get(), 1);
    }

    #[gtk::test]
    fn setup_page_navigation_handles_errors_and_backtracking() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        let info = |id| steps::SetupStep {
            id,
            title: "Info",
            description: "Info",
            kind: steps::StepKind::Info,
        };
        let auto = steps::SetupStep {
            id: StepId::Dotnet,
            title: "Auto",
            description: "Auto",
            kind: steps::StepKind::Auto,
        };

        page.imp().all_steps.replace(vec![info(StepId::Complete)]);
        page.imp().step_busy.set(true);
        page.on_next_clicked();
        page.imp().step_busy.set(false);
        page.imp().is_error.set(true);
        page.on_next_clicked();
        page.imp().current_step.set(usize::MAX);
        page.imp().is_error.set(false);
        page.on_next_clicked();
        assert!(page.imp().is_error.get());

        page.imp()
            .all_steps
            .replace(vec![info(StepId::SteamConfig), info(StepId::Complete)]);
        page.imp().current_step.set(0);
        page.imp().is_error.set(false);
        page.on_next_clicked();
        assert_eq!(
            page.imp().next_button.label().as_deref(),
            Some("Check Again")
        );

        page.imp()
            .all_steps
            .replace(vec![info(StepId::LanguageOptions), info(StepId::Complete)]);
        page.imp().current_step.set(0);
        page.imp().game.replace(None);
        page.on_next_clicked();
        assert_eq!(page.imp().current_step.get(), 1);

        page.imp()
            .all_steps
            .replace(vec![auto.clone(), info(StepId::Complete)]);
        page.imp().current_step.set(1);
        page.imp().game.replace(Some(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        }));
        page.on_back_clicked();

        page.imp().all_steps.replace(vec![
            info(StepId::SteamConfig),
            auto,
            info(StepId::Complete),
        ]);
        page.imp().current_step.set(2);
        page.on_back_clicked();
        assert_eq!(page.imp().current_step.get(), 0);

        page.imp()
            .all_steps
            .replace(vec![info(StepId::Complete), info(StepId::Complete)]);
        page.imp().current_step.set(3);
        page.on_back_clicked();
        assert!(page.imp().is_error.get());

        page.imp().all_steps.replace(Vec::new());
        page.imp().current_step.set(1);
        page.on_back_clicked();
        assert!(page.imp().is_error.get());

        page.imp().all_steps.replace(vec![info(StepId::Complete)]);
        page.imp().current_step.set(0);
        page.imp().cancel_flag.replace(Some(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        )));
        page.show_current_step();
        assert!(page.imp().cancel_flag.borrow().is_none());
        assert!(page.get_resolution().0 > 0);
        page.imp().game.replace(None);
        page.imp().language_selection.replace(None);
        assert_eq!(
            page.current_language_selection().voice,
            VoiceLanguage::Japanese
        );
        page.persist_language_selection();
        assert_eq!(page.skip_completed_steps(4), 4);

        page.imp().game.replace(Some(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        }));
        page.imp().all_steps.replace(Vec::new());
        assert_eq!(page.skip_completed_steps(4), 4);

        page.imp().all_steps.replace(vec![
            info(StepId::Complete),
            info(StepId::Complete),
            info(StepId::Complete),
        ]);
        page.imp().current_step.set(2);
        page.imp().is_error.set(false);
        page.on_next_clicked();

        page.imp().current_step.set(usize::MAX);
        page.apply_step_chrome();
        page.render_current_step_content();
        page.show_current_step();

        page.imp().all_steps.replace(vec![
            info(StepId::Complete),
            info(StepId::Complete),
            info(StepId::Complete),
        ]);
        page.imp().current_step.set(2);
        page.imp().is_error.set(false);
        page.on_back_clicked();

        let nav = adw::NavigationView::new();
        let welcome = adw::NavigationPage::builder()
            .title("Welcome")
            .child(&gtk::Label::new(Some("Welcome")))
            .build();
        nav.push(&welcome);
        let nav_page = adw::NavigationPage::builder().child(&page).build();
        nav.push(&nav_page);
        page.go_back_to_welcome();
        assert_eq!(nav.visible_page().unwrap().title().as_str(), "Welcome");
    }

    #[gtk::test]
    fn mapped_setup_page_uses_fade_transition() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        let window = gtk::Window::builder()
            .default_width(800)
            .default_height(600)
            .child(&page)
            .build();
        window.present();
        while glib::MainContext::default().iteration(false) {}

        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_enable_animations(false);
            super::sync_content_fade_duration(&page.imp().content_revealer);
            assert_eq!(page.imp().content_revealer.transition_duration(), 0);
            settings.set_gtk_enable_animations(true);
        }
        page.imp().current_step.set(0);
        page.show_current_step();
        assert!(page.imp().content_revealer.reveals_child());
        window.close();
    }

    #[gtk::test]
    fn set_step_busy_disables_back_button() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });

        assert!(page.imp().back_button.is_sensitive());
        page.set_step_busy(true);
        assert!(!page.imp().back_button.is_sensitive());
        page.set_step_busy(false);
        assert!(page.imp().back_button.is_sensitive());
    }

    #[gtk::test]
    fn setup_button_panic_handlers_show_recoverable_errors() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        assert!(format!("{:?}", page).starts_with("AdventureModsSetupPage"));

        let all_steps_guard = page.imp().all_steps.borrow_mut();
        page.imp().next_button.emit_clicked();
        drop(all_steps_guard);
        assert_eq!(page.imp().next_button.label().as_deref(), Some("Retry"));
        assert!(page.imp().is_error.get());

        page.imp().is_error.set(false);
        page.imp().current_step.set(1);
        let all_steps_guard = page.imp().all_steps.borrow_mut();
        page.imp().back_button.emit_clicked();
        drop(all_steps_guard);
        assert_eq!(page.imp().next_button.label().as_deref(), Some("Retry"));
    }

    #[gtk::test]
    fn setup_page_covers_selector_callbacks_and_motion_handlers() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SADX,
            path: tmp.path().to_path_buf(),
        });
        let select_mods_index = page
            .imp()
            .all_steps
            .borrow()
            .iter()
            .position(|step| step.id == StepId::SelectMods)
            .unwrap();
        page.imp().current_step.set(select_mods_index);
        page.show_current_step();

        let main_box = page
            .imp()
            .content_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let left_box = main_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let preset_box = left_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let preset_dropdown = preset_box
            .last_child()
            .unwrap()
            .downcast::<gtk::DropDown>()
            .unwrap();
        preset_dropdown.set_selected(1);

        let scrolled = left_box
            .last_child()
            .and_then(|size_label| size_label.prev_sibling())
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let list_box = scrolled
            .child()
            .unwrap()
            .downcast::<gtk::Viewport>()
            .unwrap()
            .child()
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        let row = list_box.row_at_index(0).unwrap();
        let row_box = row.child().unwrap().downcast::<gtk::Box>().unwrap();
        let check = row_box
            .first_child()
            .unwrap()
            .downcast::<gtk::CheckButton>()
            .unwrap();
        check.set_active(false);
        check.set_active(true);

        let controllers = row.observe_controllers();
        for index in 0..controllers.n_items() {
            if let Some(controller) = controllers.item(index)
                && let Ok(motion) = controller.downcast::<gtk::EventControllerMotion>()
            {
                motion.emit_by_name::<()>("enter", &[&0.0f64, &0.0f64]);
                motion.emit_by_name::<()>("leave", &[]);
            }
        }
        while glib::MainContext::default().iteration(false) {}
        assert!(!page.imp().selected_mods.borrow().is_empty());
    }

    #[gtk::test]
    fn run_download_steps_cover_conversion_manager_and_empty_mods() {
        init_resource_overlay();

        let sadx_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(sadx_dir.path().join("system")).unwrap();
        std::fs::write(sadx_dir.path().join("system/CHRMODELS_orig.dll"), b"orig").unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SADX,
            path: sadx_dir.path().to_path_buf(),
        });
        page.imp().all_steps.replace(Vec::new());

        page.run_download_step(
            StepId::ConvertSteam,
            gtk::ProgressBar::new(),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        for _ in 0..100 {
            while glib::MainContext::default().iteration(false) {}
            if !page.imp().task_running.get() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        std::fs::write(
            sadx_dir.path().join("Sonic Adventure DX.exe.bak"),
            b"backup",
        )
        .unwrap();
        std::fs::create_dir_all(sadx_dir.path().join("mods/.modloader")).unwrap();
        std::fs::write(
            sadx_dir.path().join("mods/.modloader/SADXModLoader.dll"),
            b"loader",
        )
        .unwrap();
        page.run_download_step(
            StepId::InstallModManager,
            gtk::ProgressBar::new(),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        for _ in 0..100 {
            while glib::MainContext::default().iteration(false) {}
            if !page.imp().task_running.get() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let sa2_dir = tempfile::tempdir().unwrap();
        let sa2_page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: sa2_dir.path().to_path_buf(),
        });
        sa2_page.imp().all_steps.replace(Vec::new());
        sa2_page.run_download_step(
            StepId::DownloadMods,
            gtk::ProgressBar::new(),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        for _ in 0..100 {
            while glib::MainContext::default().iteration(false) {}
            if !sa2_page.imp().task_running.get() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!sa2_page.imp().task_running.get());
    }

    #[gtk::test]
    fn selecting_mod_row_updates_preview_title() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        let select_mods_index = page
            .imp()
            .all_steps
            .borrow()
            .iter()
            .position(|step| step.id == StepId::SelectMods)
            .unwrap();

        page.imp().current_step.set(select_mods_index);
        page.show_current_step();

        let main_box = page
            .imp()
            .content_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        while glib::MainContext::default().iteration(false) {}
        let left_box = main_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let scrolled = left_box
            .first_child()
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let viewport = scrolled
            .child()
            .unwrap()
            .downcast::<gtk::Viewport>()
            .unwrap();
        let list_box = viewport
            .child()
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        let preview_box = main_box
            .last_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let preview_title = preview_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();

        let row = list_box.row_at_index(1).unwrap();
        list_box.select_row(Some(&row));
        while glib::MainContext::default().iteration(false) {}

        assert_eq!(
            preview_title.label().as_str(),
            "Retranslated Story -COMPLETE-"
        );
    }

    #[gtk::test]
    fn mod_selection_columns_keep_width_when_preview_content_changes() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SADX,
            path: tmp.path().to_path_buf(),
        });
        let select_mods_index = page
            .imp()
            .all_steps
            .borrow()
            .iter()
            .position(|step| step.id == StepId::SelectMods)
            .unwrap();

        page.imp().current_step.set(select_mods_index);
        page.show_current_step();

        let window = gtk::Window::builder()
            .default_width(1100)
            .default_height(820)
            .child(&page)
            .build();
        window.present();
        while glib::MainContext::default().iteration(false) {}

        let main_box = page
            .imp()
            .content_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let left_box = main_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let scrolled = left_box
            .last_child()
            .and_then(|size_label| size_label.prev_sibling())
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let viewport = scrolled
            .child()
            .unwrap()
            .downcast::<gtk::Viewport>()
            .unwrap();
        let list_box = viewport
            .child()
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        let preview_box = main_box
            .last_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();

        let row = list_box.row_at_index(8).unwrap();
        list_box.select_row(Some(&row));
        while glib::MainContext::default().iteration(false) {}

        let initial_left_width = left_box.width();
        let initial_preview_width = preview_box.width();
        assert!(initial_left_width > 0);
        assert!(initial_preview_width > 0);

        for row_index in [9, 10, 11] {
            let row = list_box.row_at_index(row_index).unwrap();
            list_box.select_row(Some(&row));
            while glib::MainContext::default().iteration(false) {}

            assert_eq!(left_box.width(), initial_left_width);
            assert_eq!(preview_box.width(), initial_preview_width);
        }
    }

    #[gtk::test]
    fn mod_selection_keeps_two_columns_in_narrow_window() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SADX,
            path: tmp.path().to_path_buf(),
        });
        let select_mods_index = page
            .imp()
            .all_steps
            .borrow()
            .iter()
            .position(|step| step.id == StepId::SelectMods)
            .unwrap();

        page.imp().current_step.set(select_mods_index);
        page.show_current_step();

        let window = gtk::Window::builder()
            .default_width(700)
            .default_height(820)
            .child(&page)
            .build();
        window.present();
        while glib::MainContext::default().iteration(false) {}

        let main_box = page
            .imp()
            .content_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();

        assert_eq!(main_box.orientation(), gtk::Orientation::Horizontal);
        assert!(main_box.is_homogeneous());
    }

    #[gtk::test]
    fn run_download_step_clears_task_running_when_game_is_missing() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        let progress_bar = gtk::ProgressBar::new();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        page.imp().game.replace(None);
        page.run_download_step(StepId::DownloadMods, progress_bar, cancel_flag);
        while glib::MainContext::default().iteration(false) {}

        assert!(!page.imp().task_running.get());
    }

    #[gtk::test]
    fn run_download_step_marks_task_running_before_main_loop_spins() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        let progress_bar = gtk::ProgressBar::new();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        page.run_download_step(StepId::Complete, progress_bar, cancel_flag);

        assert!(page.imp().task_running.get());
    }
}
