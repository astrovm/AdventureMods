//! The window: header, status banner, the current screen, dialogs and toasts.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use egui::{Align2, RichText, Ui, Vec2};

use super::dialogs::{Answer, MessageDialog};
use super::gamepad::{Gamepad, PadButton};
use super::images::{self, ImageCache};
use super::setup::{SetupEvent, SetupFlow, SetupWork};
use super::welcome::{WelcomeAction, WelcomeScreen};
use super::widgets::{self, ButtonKind, Tone};
use super::{FolderPicker, UriOpener};
use super::{motion, theme};
use crate::path_display::display_path;
use crate::settings::{Settings, Value, WINDOW_HEIGHT_KEY, WINDOW_MAXIMIZED_KEY, WINDOW_WIDTH_KEY};
use crate::setup::config;
use crate::setup::restore::{self, RestoreReport};
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{DetectionResult, resolve_granted_steam_library};

const TOAST_SECONDS: f64 = 4.0;
const ISSUES_URL: &str = "https://github.com/astrovm/AdventureMods/issues";

/// Everything the app does outside itself. Tests swap these for fakes.
#[derive(Clone)]
pub struct Services {
    pub open_uri: UriOpener,
    pub pick_folder: FolderPicker,
    pub detect_games: fn(&[PathBuf]) -> DetectionResult,
    pub restore_game: fn(&Path, GameKind) -> anyhow::Result<RestoreReport>,
    pub setup: SetupWork,
}

impl Services {
    pub fn real() -> Self {
        Self {
            open_uri: std::rc::Rc::new(super::launch_uri),
            pick_folder: std::rc::Rc::new(super::pick_library_folder),
            detect_games: crate::steam::library::detect_games_with_extra_libraries,
            restore_game: restore::restore_original_game,
            setup: SetupWork::real(),
        }
    }
}

/// The smallest the UI shrinks to in a small window, so text stays readable.
const MIN_ZOOM: f32 = 0.8;
/// The biggest the UI grows to in a big window.
const MAX_ZOOM: f32 = 2.0;
/// Zoom steps per 100%. Every zoom lays out and draws text anew, so a few
/// steps keep resizing smooth.
const ZOOM_STEPS: f32 = 40.0;

/// Scale the UI with the window, so it keeps the layout instead of cutting
/// off the mod preview in a small window or looking tiny in a big one.
fn fit_zoom_to_window(ctx: &egui::Context) {
    let zoom = ctx.zoom_factor();
    let wanted = zoom_for_window(ctx.content_rect().size() * zoom);
    if wanted != zoom {
        ctx.set_zoom_factor(wanted);
        // Lay out again at the new zoom, so the old one never shows.
        ctx.request_discard("zoom follows the window");
    }
}

/// The zoom that fits the layout in a window of `size`, unzoomed.
fn zoom_for_window(size: Vec2) -> f32 {
    let fit = (size.x / super::LAYOUT_WIDTH).min(size.y / super::LAYOUT_HEIGHT);
    // Rounded down, so the layout still fits.
    ((fit * ZOOM_STEPS).floor() / ZOOM_STEPS).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// The first window's size on a `monitor`: two thirds of its height in the
/// layout's shape, so it looks the same on every screen, but at least the
/// default size and never bigger than the monitor.
fn first_window_size(monitor: Vec2) -> Vec2 {
    let height = (monitor.y * 2.0 / 3.0).max(super::DEFAULT_HEIGHT);
    let size = Vec2::new(height * super::LAYOUT_WIDTH / super::LAYOUT_HEIGHT, height);
    size.min(monitor)
}

/// `size` shrunk to fit on a `monitor`, or `None` when it already fits.
fn fit_to_monitor(size: Vec2, monitor: Vec2) -> Option<Vec2> {
    let fitted = size.min(monitor);
    (fitted != size).then_some(fitted)
}

/// The window size to restore next time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowState {
    pub size: Vec2,
    pub maximized: bool,
    /// Whether the size came from an earlier run.
    pub saved: bool,
}

impl WindowState {
    pub fn load(settings: Option<&Settings>) -> Self {
        let int = |key| settings.and_then(|settings| settings.int(key));
        Self {
            size: Vec2::new(
                int(WINDOW_WIDTH_KEY).map_or(super::DEFAULT_WIDTH, |width| width as f32),
                int(WINDOW_HEIGHT_KEY).map_or(super::DEFAULT_HEIGHT, |height| height as f32),
            ),
            maximized: settings
                .and_then(|settings| settings.boolean(WINDOW_MAXIMIZED_KEY))
                .unwrap_or(false),
            saved: int(WINDOW_WIDTH_KEY).is_some() && int(WINDOW_HEIGHT_KEY).is_some(),
        }
    }
}

enum Dialog {
    ConfirmRestore(GameKind, PathBuf, MessageDialog),
    SteamVerify(GameKind, MessageDialog),
    About(MessageDialog),
}

struct Toast {
    text: String,
    until: f64,
}

/// A scan of the Steam libraries running in the background.
struct Scan {
    id: u64,
    result: Receiver<anyhow::Result<DetectionResult>>,
}

pub struct AdventureModsApp {
    services: Services,
    settings: Option<Settings>,
    extra_library_paths: Vec<PathBuf>,
    latest_scan: u64,
    scan: Option<Scan>,
    welcome: WelcomeScreen,
    setup: Option<SetupFlow>,
    status: Option<(String, bool)>,
    toasts: Vec<Toast>,
    dialog: Option<Dialog>,
    restoring: Option<(GameKind, Receiver<anyhow::Result<RestoreReport>>)>,
    granting: Option<(PathBuf, Receiver<Option<PathBuf>>)>,
    images: Option<ImageCache>,
    gamepad: Option<Gamepad>,
    pad_presses: Vec<PadButton>,
    /// Keyboard or controller in use: keep something focused.
    navigating: bool,
    window: WindowState,
    /// Whether the window was checked against its monitor yet.
    fitted: bool,
    transition: motion::ScreenTransition,
    logo: Option<egui::TextureHandle>,
}

impl AdventureModsApp {
    pub fn new(services: Services, settings: Option<Settings>) -> Self {
        let extra_library_paths = config::load_extra_library_paths(settings.as_ref());
        let window = WindowState::load(settings.as_ref());
        let mut app = Self {
            services,
            settings,
            extra_library_paths,
            latest_scan: 0,
            scan: None,
            welcome: WelcomeScreen::default(),
            setup: None,
            status: None,
            toasts: Vec::new(),
            dialog: None,
            restoring: None,
            granting: None,
            images: None,
            gamepad: None,
            pad_presses: Vec::new(),
            navigating: false,
            window,
            fitted: false,
            transition: motion::ScreenTransition::default(),
            logo: None,
        };
        app.detect_games();
        app
    }

    pub fn set_gamepad(&mut self, gamepad: Gamepad) {
        self.gamepad = Some(gamepad);
    }

    pub fn window_state(&self) -> WindowState {
        self.window
    }

    /// Start a scan. A newer scan replaces the result of an older one.
    fn detect_games(&mut self) {
        self.latest_scan = self.latest_scan.wrapping_add(1);
        let id = self.latest_scan;
        let extra = self.extra_library_paths.clone();
        let detect = self.services.detect_games;
        let (tx, result) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(|| detect(&extra));
            let _ = tx.send(crate::blocking::spawn_result(outcome));
        });
        self.scan = Some(Scan { id, result });
    }

    fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Show the outcome of scan `id`, unless a newer scan replaced it.
    fn apply_scan(&mut self, id: u64, result: anyhow::Result<DetectionResult>) {
        if id != self.latest_scan {
            return;
        }
        match result {
            Ok(result) => {
                self.welcome.set_result(result);
                self.status = None;
            }
            Err(err) => {
                tracing::error!("Failed to detect games: {err}");
                self.show_status(format!("Failed to detect Steam libraries: {err}"), true);
            }
        }
    }

    pub fn show_status(&mut self, message: impl Into<String>, is_error: bool) {
        self.status = Some((message.into(), is_error));
    }

    fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        let now = ctx.input(|input| input.time);
        self.toasts.push(Toast {
            text: text.into(),
            until: now + TOAST_SECONDS,
        });
    }

    /// Remember a newly granted library and scan again.
    fn library_access_granted(&mut self, path: PathBuf) {
        if !self.extra_library_paths.contains(&path) {
            self.extra_library_paths.push(path);
            config::save_extra_library_paths(self.settings.as_mut(), &self.extra_library_paths);
        }
        self.detect_games();
    }

    fn request_library_access(&mut self, expected: PathBuf) {
        let answer = (self.services.pick_folder)(&expected);
        self.granting = Some((expected, answer));
    }

    fn folder_chosen(&mut self, expected: &Path, folder: Option<PathBuf>) {
        let Some(folder) = folder else {
            tracing::info!("Library access dialog cancelled or failed");
            return;
        };
        let Some(resolved) = resolve_granted_steam_library(&folder, expected) else {
            tracing::warn!(
                selected = %folder.display(),
                expected = %expected.display(),
                "Granted folder is not a usable Steam library"
            );
            self.show_status(
                format!(
                    "That folder is not the requested Steam library. Select {} (it must contain a steamapps folder).",
                    display_path(expected)
                ),
                true,
            );
            return;
        };
        tracing::info!(
            selected = %folder.display(),
            expected = %expected.display(),
            resolved = %resolved.display(),
            "Granted Steam library access"
        );
        self.library_access_granted(resolved);
    }

    fn restore_game(&mut self, kind: GameKind, path: PathBuf) {
        let restore = self.services.restore_game;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(|| restore(&path, kind));
            let _ = tx.send(crate::blocking::flatten_spawn_result(outcome));
        });
        self.restoring = Some((kind, rx));
    }

    fn restored(
        &mut self,
        ctx: &egui::Context,
        kind: GameKind,
        result: anyhow::Result<RestoreReport>,
    ) {
        match result {
            Ok(report) => {
                self.toast(ctx, format!("{} was restored.", kind.name()));
                self.detect_games();
                if report.needs_steam_verify {
                    self.dialog = Some(Dialog::SteamVerify(kind, steam_verify_dialog(kind)));
                }
            }
            Err(err) => {
                tracing::error!("Failed to restore {}: {err:#}", kind.name());
                self.show_status(format!("Could not restore {}: {err}", kind.name()), true);
            }
        }
    }

    fn verify_in_steam(&self, kind: GameKind) {
        (self.services.open_uri)(&restore::steam_verify_uri(kind));
    }

    fn open_setup(&mut self, game: Game) {
        let languages = config::load_language_selection(self.settings.as_ref(), game.kind);
        self.setup = Some(SetupFlow::new(game, self.services.setup, languages));
    }

    fn close_setup(&mut self) {
        self.setup = None;
        self.welcome.focus_first();
        // Setup changes what each game card shows.
        self.detect_games();
    }

    fn handle_welcome_action(&mut self, action: WelcomeAction) {
        match action {
            WelcomeAction::SetUp(game) => self.open_setup(game),
            WelcomeAction::GrantAccess(path) => self.request_library_access(path),
            WelcomeAction::VerifyInSteam(kind) => self.verify_in_steam(kind),
            WelcomeAction::ConfirmRestore(kind, path) => {
                self.dialog = Some(Dialog::ConfirmRestore(kind, path, restore_dialog(kind)));
            }
        }
    }

    fn handle_setup_events(&mut self) {
        let Some(setup) = &mut self.setup else {
            return;
        };
        for event in setup.take_events() {
            match event {
                SetupEvent::Exit => self.close_setup(),
                SetupEvent::OpenUri(uri) => (self.services.open_uri)(&uri),
                SetupEvent::SaveLanguages(kind, languages) => {
                    config::save_language_selection(self.settings.as_mut(), kind, languages);
                }
            }
        }
    }

    /// Pick up results from background work.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(scan) = &self.scan
            && let Ok(result) = scan.result.try_recv()
        {
            let id = scan.id;
            self.scan = None;
            self.apply_scan(id, result);
        }
        if let Some((kind, rx)) = &self.restoring
            && let Ok(result) = rx.try_recv()
        {
            let kind = *kind;
            self.restoring = None;
            self.restored(ctx, kind, result);
        }
        if let Some((expected, rx)) = &self.granting
            && let Ok(folder) = rx.try_recv()
        {
            let expected = expected.clone();
            self.granting = None;
            self.folder_chosen(&expected, folder);
        }
        if let Some(setup) = &mut self.setup {
            setup.poll();
        }
        self.handle_setup_events();
        let now = ctx.input(|input| input.time);
        self.toasts.retain(|toast| toast.until > now);
    }

    /// Turn controller presses into keys egui understands, and keep the rest
    /// for the screens.
    pub fn feed_gamepad(&mut self, input: &mut egui::RawInput) {
        let presses = self
            .gamepad
            .as_ref()
            .map(Gamepad::presses)
            .unwrap_or_default();
        for press in presses {
            self.navigating = true;
            match press.key() {
                Some(key) => {
                    for pressed in [true, false] {
                        input.events.push(egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
                None => self.pad_presses.push(press),
            }
        }
    }

    /// Track whether keyboard or controller navigation is in use.
    fn track_navigation(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Key {
                        key:
                            egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::Tab,
                        pressed: true,
                        ..
                    } => self.navigating = true,
                    egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. } => {
                        self.navigating = false;
                    }
                    _ => {}
                }
            }
        });
        // Nothing focused: put focus where a press makes sense.
        if self.navigating && ctx.memory(|memory| memory.focused().is_none()) {
            match &mut self.setup {
                Some(setup) => setup.focus_primary(),
                None => self.welcome.focus_first(),
            }
        }
    }

    fn handle_pad_presses(&mut self) {
        for press in std::mem::take(&mut self.pad_presses) {
            if self.dialog.is_some() {
                continue;
            }
            if let Some(setup) = &mut self.setup {
                setup.handle_pad(press);
            } else if press == PadButton::Refresh && !self.scanning() {
                self.detect_games();
            }
        }
    }

    /// Returns whether it resized the window.
    fn remember_window(&mut self, ctx: &egui::Context) -> bool {
        let zoom = ctx.zoom_factor();
        let fitted = ctx.input(|input| {
            let viewport = input.viewport();
            // eframe's own check misreads fractional scaling on Wayland, so
            // shrink a window bigger than its monitor once the monitor is known.
            // The first run sizes it for the monitor instead.
            let mut fitted = None;
            if !self.fitted
                && let (Some(monitor), Some(rect)) = (viewport.monitor_size, viewport.inner_rect)
            {
                self.fitted = true;
                let (monitor, size) = (monitor * zoom, rect.size() * zoom);
                let filling = viewport.fullscreen == Some(true) || viewport.maximized == Some(true);
                fitted = if self.window.saved || filling {
                    fit_to_monitor(size, monitor)
                } else {
                    let first = first_window_size(monitor);
                    (first != size).then_some(first)
                }
                .map(|size| size / zoom);
            }
            if let Some(maximized) = viewport.maximized {
                self.window.maximized = maximized;
            }
            if !self.window.maximized
                && let Some(rect) = viewport.inner_rect
            {
                // Saved without the zoom, which only follows the window.
                self.window.size = rect.size() * zoom;
            }
            fitted
        });
        if let Some(size) = fitted {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }
        fitted.is_some()
    }

    /// Save the window size so the next start uses it.
    pub fn save_window_state(&mut self) {
        let Some(settings) = &mut self.settings else {
            return;
        };
        if !self.window.maximized {
            settings.set(
                WINDOW_WIDTH_KEY,
                Value::Int(self.window.size.x.round() as i64),
            );
            settings.set(
                WINDOW_HEIGHT_KEY,
                Value::Int(self.window.size.y.round() as i64),
            );
        }
        settings.set(WINDOW_MAXIMIZED_KEY, Value::Bool(self.window.maximized));
    }

    /// Draw one frame.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        // The new size is in points at this zoom, so the zoom waits a frame.
        if !self.remember_window(&ctx) {
            fit_zoom_to_window(&ctx);
        }
        theme::apply_ui(ui);
        self.poll(&ctx);
        self.track_navigation(&ctx);
        self.handle_pad_presses();
        if ctx.input(|input| input.key_pressed(egui::Key::F11)) {
            let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        if let Some(setup) = &mut self.setup {
            setup.resolution =
                ctx.input(|input| crate::display::resolution_from_viewport(input.viewport()));
        }
        self.images.get_or_insert_with(|| ImageCache::new(&ctx));
        // B on a controller, or Escape: go back, unless a dialog takes it.
        if ctx.input(|input| input.key_pressed(egui::Key::Escape))
            && self.dialog.is_none()
            && let Some(setup) = &mut self.setup
            && !setup.has_dialog()
        {
            setup.back();
        }

        theme::paint_background(&ctx);
        // The chosen game's art stays behind setup too.
        self.welcome.paint_backdrops(&ctx);
        self.show_header(ui);
        if let Some((message, is_error)) = self.status.clone() {
            egui::Panel::top("status")
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 8)))
                .show_separator_line(false)
                .show(ui, |ui| {
                    widgets::banner(
                        ui,
                        if is_error { Tone::Error } else { Tone::Accent },
                        &message,
                    );
                });
        }
        self.show_footer(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| self.show_screen(ui));

        self.show_dialog(&ctx);
        self.show_toasts(&ctx);
        self.handle_setup_events();

        let busy = self.scan.is_some()
            || self.restoring.is_some()
            || self.granting.is_some()
            || self.setup.as_ref().is_some_and(SetupFlow::busy)
            || !self.toasts.is_empty();
        if busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn show_screen(&mut self, ui: &mut Ui) {
        let images = self.images.as_mut().expect("created before the screen");
        let key = self.setup.as_ref().map_or(0, SetupFlow::screen_key);
        let welcome = &mut self.welcome;
        let setup = &mut self.setup;
        let action = self.transition.show(ui, key, |ui| match setup {
            Some(setup) => {
                setup.show_body(ui, images);
                None
            }
            None => welcome.show(ui, images),
        });
        if let Some(action) = action {
            self.handle_welcome_action(action);
        }
    }

    fn show_header(&mut self, ui: &mut Ui) {
        let (title, subtitle) = match &self.setup {
            Some(setup) => setup.title(),
            None => (crate::config::APP_NAME.to_owned(), String::new()),
        };
        egui::Panel::top("header")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 12)))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(theme::TARGET_HEIGHT);
                    if let Some(setup) = &mut self.setup {
                        let back = ui.add_enabled_ui(setup.can_go_back(), widgets::back_button);
                        if back.inner.clicked() {
                            setup.back();
                        }
                    }
                    if self.setup.is_none() {
                        let logo = self.logo.get_or_insert_with(|| {
                            let image = images::decode(images::APP_ICON)
                                .expect("the bundled app icon decodes");
                            ui.ctx()
                                .load_texture("app-icon", image, egui::TextureOptions::LINEAR)
                        });
                        ui.add(egui::Image::new(&*logo).fit_to_exact_size(Vec2::splat(40.0)));
                    }
                    if self.setup.is_none() {
                        ui.label(
                            RichText::new(title.to_uppercase())
                                .font(theme::font(ui.ctx(), 20.0, theme::heavy()))
                                .extra_letter_spacing(1.5),
                        );
                    } else if subtitle.is_empty() {
                        ui.label(RichText::new(&title).heading().strong());
                    } else {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(RichText::new(&title).heading().strong());
                            widgets::caption(ui, &subtitle);
                        });
                    }
                    // Development builds say so, to tell them apart.
                    ui.label(
                        RichText::new(profile_badge(crate::config::PROFILE))
                            .strong()
                            .color(theme::WARNING),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.setup.is_some() {
                            return;
                        }
                        if widgets::icon_button(ui, "ℹ", "About").clicked() {
                            self.dialog = Some(Dialog::About(about_dialog()));
                        }
                        if self.scanning() {
                            ui.add_sized(
                                Vec2::splat(theme::TARGET_HEIGHT),
                                egui::Spinner::new().size(26.0),
                            );
                        } else if widgets::icon_button(ui, "⟳", "Scan Again").clicked() {
                            self.detect_games();
                        }
                    });
                });
            });
    }

    fn show_footer(&mut self, ui: &mut Ui) {
        let hints = self.gamepad.as_ref().is_some_and(Gamepad::connected);
        if self.setup.is_none() && !hints {
            return;
        }
        egui::Panel::bottom("footer")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 14)))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(theme::TARGET_HEIGHT);
                    if hints {
                        widgets::pad_hint(ui, "A", theme::SUCCESS, "Select");
                        widgets::pad_hint(ui, "B", theme::ERROR, "Back");
                        if self.setup.is_some() {
                            widgets::pad_hint(ui, "X", theme::ACCENT_BRIGHT, "Other action");
                            widgets::pad_hint(ui, "≡", theme::TEXT_DIM, "Continue");
                        } else {
                            widgets::pad_hint(ui, "Y", theme::WARNING, "Scan again");
                        }
                    }
                    if let Some(setup) = &mut self.setup {
                        setup.show_footer(ui);
                    }
                });
            });
    }

    fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let answer = match &mut dialog {
            Dialog::ConfirmRestore(_, _, message)
            | Dialog::SteamVerify(_, message)
            | Dialog::About(message) => message.show(ctx),
        };
        let Some(answer) = answer else {
            // Still open.
            self.dialog = Some(dialog);
            return;
        };
        self.welcome.focus_first();
        match (dialog, answer) {
            (Dialog::ConfirmRestore(kind, path, _), Answer::Button(1)) => {
                self.restore_game(kind, path);
            }
            (Dialog::SteamVerify(kind, _), Answer::Button(1)) => self.verify_in_steam(kind),
            (Dialog::About(_), Answer::Button(0)) => (self.services.open_uri)(ISSUES_URL),
            _ => {}
        }
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        if self.toasts.is_empty() {
            return;
        }
        let now = ctx.input(|input| input.time);
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -110.0))
            .interactable(false)
            .show(ctx, |ui| {
                for toast in &self.toasts {
                    // Fade in when shown and out before leaving.
                    let shown = now - (toast.until - TOAST_SECONDS);
                    let left = toast.until - now;
                    let fade = (shown.min(left) / motion::SCREEN).clamp(0.0, 1.0) as f32;
                    ui.set_opacity(motion::ease_out(fade));
                    egui::Frame::new()
                        .fill(theme::CARD_RAISED)
                        .corner_radius(24)
                        .inner_margin(egui::Margin::symmetric(24, 12))
                        .shadow(egui::Shadow {
                            offset: [0, 6],
                            blur: 20,
                            spread: 0,
                            color: egui::Color32::from_black_alpha(120),
                        })
                        .show(ui, |ui| {
                            ui.label(&toast.text);
                        });
                }
            });
        ctx.request_repaint();
    }
}

fn profile_badge(profile: &str) -> &'static str {
    if profile == "development" {
        "DEVEL"
    } else {
        ""
    }
}

fn restore_dialog(kind: GameKind) -> MessageDialog {
    MessageDialog::new(
        "restore",
        format!("Restore the original {}?", kind.name()),
        "This puts back the game's original launcher and removes the mod loader, \
         so Steam starts the unmodded game. Downloaded mods stay in the mods folder \
         and are reused if you set the game up again.",
    )
    .button("Cancel", ButtonKind::Normal)
    .button("Restore", ButtonKind::Destructive)
    .default_button(0)
}

/// Offers Steam's file verification once setup converted `kind` and it was restored.
fn steam_verify_dialog(kind: GameKind) -> MessageDialog {
    MessageDialog::new(
        "steam-verify",
        "Finish in Steam",
        format!(
            "Setup converted {} to the 2004 version. Let Steam verify the game files to \
             get the original Steam version back.",
            kind.name()
        ),
    )
    .button("Later", ButtonKind::Normal)
    .button("Verify in Steam", ButtonKind::Suggested)
    .default_button(1)
}

fn about_dialog() -> MessageDialog {
    MessageDialog::new(
        "about",
        format!("{} {}", crate::config::APP_NAME, env!("CARGO_PKG_VERSION")),
        "The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux.",
    )
    .footer(made_by())
    .button("Report an Issue", ButtonKind::Normal)
    .button("Close", ButtonKind::Suggested)
    .default_button(1)
}

/// "Made with ❤ by astro", with a red heart.
fn made_by() -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let font = egui::FontId::proportional(20.0);
    for (text, color) in [
        ("Made with ", theme::TEXT_DIM),
        ("❤", theme::ERROR),
        (" by astro", theme::TEXT_DIM),
    ] {
        job.append(text, 0.0, egui::TextFormat::simple(font.clone(), color));
    }
    job
}

/// The eframe side: controller input, the frame, and saving on exit.
pub struct EframeApp {
    app: AdventureModsApp,
    /// Close after one frame: the startup smoke test sets this.
    quit_after_first_frame: bool,
}

impl EframeApp {
    pub fn new(app: AdventureModsApp) -> Self {
        Self {
            app,
            quit_after_first_frame: std::env::var_os("ADVENTURE_MODS_QUIT_AFTER_FIRST_FRAME")
                .is_some(),
        }
    }
}

impl eframe::App for EframeApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.app.show(ui);
        if self.quit_after_first_frame {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.app.feed_gamepad(raw_input);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.app.save_window_state();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
