//! The first screen: one card per game, saying whether it is ready for mods.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{Color32, RichText, Ui, Vec2};

use super::dialogs::{Answer, ChoiceDialog};
use super::images::{self, ImageCache, cover_resource};
use super::widgets::{self, ButtonKind, Tone};
use super::{motion, theme};
use crate::path_display::display_path;
use crate::setup::restore;
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{DetectionResult, InaccessibleGame};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GameInstallOption {
    Detected(PathBuf),
    Inaccessible(PathBuf),
}

impl GameInstallOption {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Detected(path) | Self::Inaccessible(path) => path,
        }
    }

    fn is_accessible(&self) -> bool {
        matches!(self, Self::Detected(_))
    }

    fn selector_label(&self) -> String {
        if self.is_accessible() {
            display_path(self.path())
        } else {
            format!("{} (Needs access)", display_path(self.path()))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CardState {
    Detected,
    Missing,
    Inaccessible,
}

/// What a card shows. Rebuilt when the scan or the chosen install changes,
/// so the disk is not checked every frame.
#[derive(Clone, Debug)]
struct GameCard {
    kind: GameKind,
    state: CardState,
    options: Vec<GameInstallOption>,
    selected: usize,
    modded: bool,
    steam_repair: bool,
}

impl GameCard {
    fn new(kind: GameKind, state: CardState, options: Vec<GameInstallOption>) -> Self {
        let selected = options
            .iter()
            .position(GameInstallOption::is_accessible)
            .unwrap_or(0);
        let mut card = Self {
            kind,
            state,
            options,
            selected,
            modded: false,
            steam_repair: false,
        };
        card.refresh();
        card
    }

    fn select(&mut self, index: usize) {
        if index < self.options.len() {
            self.selected = index;
            self.refresh();
        }
    }

    fn refresh(&mut self) {
        let accessible = self
            .selected_option()
            .filter(|option| option.is_accessible())
            .map(|option| option.path().to_path_buf());
        let accessible = accessible.as_deref();
        self.modded = accessible.is_some_and(|path| restore::is_modded(path, self.kind));
        self.steam_repair = accessible.is_some_and(restore::needs_steam_repair);
    }

    fn selected_option(&self) -> Option<&GameInstallOption> {
        self.options.get(self.selected)
    }

    /// A line under the name, only when the user has to do something the
    /// buttons don't already say.
    fn note(&self) -> Option<&'static str> {
        match self.selected_option() {
            None => Some("Install it in Steam, then scan again."),
            Some(GameInstallOption::Detected(_)) if self.steam_repair => {
                Some("Let Steam verify the game files to finish restoring it.")
            }
            Some(_) => None,
        }
    }

    /// The main button's label and, when there is one, the second button's.
    fn actions(&self) -> Option<(&'static str, Option<&'static str>)> {
        match self.selected_option()? {
            GameInstallOption::Inaccessible(_) => Some(("Grant Access", None)),
            GameInstallOption::Detected(_) if self.steam_repair => {
                Some(("Verify in Steam", Some("Set Up")))
            }
            GameInstallOption::Detected(_) if self.modded => Some(("Change Mods", Some("Restore"))),
            GameInstallOption::Detected(_) => Some(("Set Up", None)),
        }
    }

    fn primary_action(&self) -> Option<WelcomeAction> {
        Some(match self.selected_option()?.clone() {
            GameInstallOption::Inaccessible(path) => WelcomeAction::GrantAccess(path),
            GameInstallOption::Detected(_) if self.steam_repair => {
                WelcomeAction::VerifyInSteam(self.kind)
            }
            GameInstallOption::Detected(path) => WelcomeAction::SetUp(Game {
                kind: self.kind,
                path,
            }),
        })
    }

    fn secondary_action(&self) -> Option<WelcomeAction> {
        let GameInstallOption::Detected(path) = self.selected_option()?.clone() else {
            return None;
        };
        // Waiting for a Steam repair, the second button sets the game up again.
        if self.steam_repair {
            Some(WelcomeAction::SetUp(Game {
                kind: self.kind,
                path,
            }))
        } else {
            self.modded
                .then_some(WelcomeAction::ConfirmRestore(self.kind, path))
        }
    }
}

/// What the user asked for on the welcome screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WelcomeAction {
    SetUp(Game),
    GrantAccess(PathBuf),
    VerifyInSteam(GameKind),
    ConfirmRestore(GameKind, PathBuf),
}

#[derive(Default)]
pub struct WelcomeScreen {
    result: Option<DetectionResult>,
    cards: Vec<GameCard>,
    install_picker: Option<(usize, ChoiceDialog)>,
    /// The game in focus: its tile is large and its buttons show.
    selected: usize,
    /// Focus the selected game's main button on the next frame.
    focus_first: bool,
    /// Height of the content last frame, to center it vertically.
    content_height: f32,
    /// Each game's blurred cover, behind everything while it is selected.
    backdrops: HashMap<GameKind, egui::TextureHandle>,
}

impl WelcomeScreen {
    pub fn set_result(&mut self, result: DetectionResult) {
        // Keep the install each card had chosen, when it is still there.
        let previous: Vec<_> = self
            .cards
            .iter()
            .filter_map(|card| Some((card.kind, card.selected_option()?.clone())))
            .collect();
        self.cards = build_game_cards(&result);
        if self.result.is_none() {
            // Start on the first game there is something to do with.
            self.selected = self
                .cards
                .iter()
                .position(|card| card.actions().is_some())
                .unwrap_or(0);
        }
        for card in &mut self.cards {
            if let Some((_, option)) = previous.iter().find(|(kind, _)| *kind == card.kind)
                && let Some(index) = card
                    .options
                    .iter()
                    .position(|candidate| candidate == option)
            {
                card.select(index);
            }
        }
        self.result = Some(result);
    }

    pub fn has_result(&self) -> bool {
        self.result.is_some()
    }

    pub fn focus_first(&mut self) {
        self.focus_first = true;
    }

    pub fn show(&mut self, ui: &mut Ui, images: &mut ImageCache) -> Option<WelcomeAction> {
        if let Some((card_index, picker)) = &mut self.install_picker {
            match picker.show(ui.ctx()) {
                Some(Answer::Button(option)) => {
                    let card_index = *card_index;
                    self.install_picker = None;
                    self.cards[card_index].select(option);
                }
                Some(Answer::Dismissed) => self.install_picker = None,
                None => {}
            }
        }

        let Some(result) = &self.result else {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() / 2.0 - 60.0);
                ui.add(egui::Spinner::new().size(48.0));
                ui.label(RichText::new("Looking for your games…").color(theme::TEXT_DIM));
            });
            return None;
        };
        let alert = inaccessible_alert(&result.inaccessible);

        let mut action = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let free = ui.available_height() - self.content_height;
                ui.add_space((free / 2.0).max(16.0));
                let content = ui.vertical(|ui| {
                    let margin = (ui.available_width() * 0.05).clamp(16.0, 48.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(margin as i8, 0))
                        .show(ui, |ui| {
                            if let Some(alert) = &alert {
                                widgets::banner(ui, Tone::Warning, alert);
                                ui.add_space(12.0);
                            }
                            self.show_tiles(ui, images);
                            ui.add_space(28.0);
                            action = self.show_selected(ui);
                        });
                });
                self.content_height = content.response.rect.height();
                ui.add_space(16.0);
            });
        action
    }

    /// The selected game's blurred cover fills the window, fading between
    /// games, with the bottom darkened so text stays readable. Call it before
    /// anything else is drawn, as it paints over the whole window.
    pub fn paint_backdrops(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        for (index, card) in self.cards.iter().enumerate() {
            let shown = ctx.animate_bool_with_time_and_easing(
                egui::Id::new(("backdrop", card.kind)),
                index == self.selected && card.state != CardState::Missing,
                motion::SCREEN as f32 * 2.0,
                motion::ease_out,
            );
            if shown <= 0.0 {
                continue;
            }
            let texture = self.backdrops.entry(card.kind).or_insert_with(|| {
                let image = images::decode_backdrop(cover_resource(card.kind))
                    .expect("bundled covers decode");
                ctx.load_texture(
                    format!("backdrop-{:?}", card.kind),
                    image,
                    egui::TextureOptions::LINEAR,
                )
            });
            painter.image(
                texture.id(),
                screen,
                cover_uv(texture.aspect_ratio(), screen.aspect_ratio()),
                Color32::WHITE.gamma_multiply(0.42 * shown),
            );
        }
        let mut shade = egui::Mesh::default();
        let clear = theme::BACKGROUND.gamma_multiply(0.15);
        let dark = theme::BACKGROUND.gamma_multiply(0.92);
        for (pos, color) in [
            (screen.left_top(), clear),
            (screen.right_top(), clear),
            (screen.right_bottom(), dark),
            (screen.left_bottom(), dark),
        ] {
            shade.colored_vertex(pos, color);
        }
        shade.add_triangle(0, 1, 2);
        shade.add_triangle(0, 2, 3);
        painter.add(shade);
    }

    /// One tile per game; the selected one is larger and outlined in gold.
    fn show_tiles(&mut self, ui: &mut Ui, images: &mut ImageCache) {
        let gap = 24.0;
        let count = self.cards.len() as f32;
        let room = ui.available_width() - gap * (count - 1.0);
        // The selected tile takes a bigger share of the row.
        let small = (room / (count + 0.25)).min(560.0);
        let large = small * 1.25;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Max), |ui| {
                for index in 0..self.cards.len() {
                    let card = &self.cards[index];
                    let grow = ui.ctx().animate_bool_with_time_and_easing(
                        egui::Id::new(("tile-grow", card.kind)),
                        index == self.selected,
                        motion::QUICK * 2.0,
                        motion::ease_out,
                    );
                    let width = small + (large - small) * grow;
                    let size = Vec2::new(width, width * 215.0 / 460.0);
                    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                    let name = card.kind.name();
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name)
                    });
                    let highlight = motion::highlight(ui, &response);
                    paint_tile(ui, rect, card, images, grow, highlight);
                    if response.gained_focus() {
                        self.selected = index;
                    }
                    if response.clicked() {
                        self.selected = index;
                        // A, or a click, moves on to the game's buttons.
                        self.focus_first = true;
                    }
                }
            });
        });
    }

    /// The selected game's name, what it needs, and its buttons.
    fn show_selected(&mut self, ui: &mut Ui) -> Option<WelcomeAction> {
        let index = self.selected.min(self.cards.len() - 1);
        let card = &self.cards[index];
        let mut action = None;
        let mut open_picker = false;
        ui.label(
            RichText::new(card.kind.name().to_uppercase())
                .font(theme::font(ui.ctx(), 34.0, theme::heavy()))
                .extra_letter_spacing(1.0),
        );
        if let Some(note) = card.note() {
            ui.label(RichText::new(note).color(theme::TEXT_DIM));
        }
        if card.options.len() > 1
            && let Some(option) = card.selected_option()
        {
            ui.allocate_ui(Vec2::new(ui.available_width().min(520.0), 0.0), |ui| {
                if widgets::choice_row(ui, "Install", &option.selector_label())
                    .on_hover_text("Choose which install to set up")
                    .clicked()
                {
                    open_picker = true;
                }
            });
        }
        // Actions exist only for a selected install.
        if let (Some((primary, secondary)), Some(option)) = (card.actions(), card.selected_option())
        {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let response = widgets::button_sized(ui, primary, ButtonKind::Suggested, 220.0)
                    .on_hover_text(display_path(option.path()));
                if std::mem::take(&mut self.focus_first) {
                    response.request_focus();
                }
                if response.clicked() {
                    action = card.primary_action();
                }
                if let Some(secondary) = secondary
                    && widgets::button_sized(ui, secondary, ButtonKind::Normal, 160.0).clicked()
                {
                    action = card.secondary_action();
                }
            });
        }
        if open_picker {
            let options = card
                .options
                .iter()
                .map(GameInstallOption::selector_label)
                .collect();
            self.install_picker = Some((
                index,
                ChoiceDialog::new(
                    "install-picker",
                    format!("{} Install", card.kind.name()),
                    options,
                    card.selected,
                ),
            ));
        }
        action
    }

    #[cfg(test)]
    fn card(&self, kind: GameKind) -> &GameCard {
        self.cards.iter().find(|card| card.kind == kind).unwrap()
    }
}

/// A game's cover, dimmed unless selected, outlined in gold as it grows.
fn paint_tile(
    ui: &Ui,
    rect: egui::Rect,
    card: &GameCard,
    images: &mut ImageCache,
    selected: f32,
    highlight: f32,
) {
    let radius = egui::CornerRadius::same(14);
    let painter = ui.painter();
    if selected > 0.0 {
        painter.add(
            egui::Shadow {
                offset: [0, 12],
                blur: 30,
                spread: 4,
                color: theme::ACCENT.gamma_multiply(0.35 * selected),
            }
            .as_shape(rect, radius),
        );
    }
    painter.rect_filled(rect, radius, theme::CARD);
    if let Some(texture) = images.get(ui.ctx(), cover_resource(card.kind)) {
        let light = if card.state == CardState::Missing {
            90.0
        } else {
            170.0 + 85.0 * selected.max(highlight)
        };
        egui::Image::new(&texture)
            .corner_radius(radius)
            .tint(Color32::from_gray(light as u8))
            .paint_at(ui, rect);
    }
    if selected > 0.0 {
        painter.rect_stroke(
            rect.expand(3.0),
            egui::CornerRadius::same(17),
            egui::Stroke::new(3.0, theme::ACCENT_BRIGHT.gamma_multiply(selected)),
            egui::StrokeKind::Outside,
        );
    }
}

/// The part of an image with aspect ratio `image` that covers an area with
/// aspect ratio `area`, centered.
fn cover_uv(image: f32, area: f32) -> egui::Rect {
    if area > image {
        let height = image / area;
        egui::Rect::from_min_max(
            egui::pos2(0.0, (1.0 - height) / 2.0),
            egui::pos2(1.0, (1.0 + height) / 2.0),
        )
    } else {
        let width = area / image;
        egui::Rect::from_min_max(
            egui::pos2((1.0 - width) / 2.0, 0.0),
            egui::pos2((1.0 + width) / 2.0, 1.0),
        )
    }
}

fn inaccessible_alert(inaccessible: &[InaccessibleGame]) -> Option<String> {
    let mut names = Vec::new();
    for game in inaccessible {
        let name = game.kind.name();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    (!names.is_empty()).then(|| {
        format!(
            "Adventure Mods needs access to the Steam library with {}. Use Grant Access on the game below.",
            names.join(" and ")
        )
    })
}

fn build_game_cards(result: &DetectionResult) -> Vec<GameCard> {
    [GameKind::SADX, GameKind::SA2]
        .into_iter()
        .map(|kind| {
            let mut options: Vec<GameInstallOption> = result
                .games
                .iter()
                .filter(|game| game.kind == kind)
                .map(|game| GameInstallOption::Detected(game.path.clone()))
                .collect();
            let detected = !options.is_empty();
            options.extend(
                result
                    .inaccessible
                    .iter()
                    .filter(|game| game.kind == kind)
                    .map(|game| GameInstallOption::Inaccessible(game.library_path.clone())),
            );
            let state = if detected {
                CardState::Detected
            } else if options.is_empty() {
                CardState::Missing
            } else {
                CardState::Inaccessible
            };
            GameCard::new(kind, state, options)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    fn game(kind: GameKind, path: &str) -> Game {
        Game {
            kind,
            path: path.into(),
        }
    }

    fn inaccessible(kind: GameKind, path: &str) -> InaccessibleGame {
        InaccessibleGame {
            kind,
            library_path: path.into(),
        }
    }

    struct State {
        screen: WelcomeScreen,
        images: Option<ImageCache>,
        actions: Vec<WelcomeAction>,
    }

    fn harness(result: Option<DetectionResult>) -> Harness<'static, State> {
        let mut screen = WelcomeScreen::default();
        if let Some(result) = result {
            screen.set_result(result);
        }
        Harness::builder()
            .with_size(Vec2::new(1280.0, 800.0))
            .build_ui_state(
                |ui, state: &mut State| {
                    theme::apply_ui(ui);
                    let images = state
                        .images
                        .get_or_insert_with(|| ImageCache::new(ui.ctx()));
                    state.screen.paint_backdrops(ui.ctx());
                    if let Some(action) = state.screen.show(ui, images) {
                        state.actions.push(action);
                    }
                },
                State {
                    screen,
                    images: None,
                    actions: Vec::new(),
                },
            )
    }

    #[test]
    fn cards_always_include_missing_games() {
        let cards = build_game_cards(&DetectionResult {
            games: vec![game(GameKind::SA2, "/games/sa2")],
            inaccessible: vec![],
        });

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].state, CardState::Missing);
        assert_eq!(cards[1].state, CardState::Detected);
        assert_eq!(
            cards[0].note(),
            Some("Install it in Steam, then scan again.")
        );
        assert!(cards[0].actions().is_none());
        assert!(cards[0].primary_action().is_none());
        assert!(cards[0].secondary_action().is_none());
    }

    #[test]
    fn inaccessible_libraries_ask_for_access() {
        let cards = build_game_cards(&DetectionResult {
            games: vec![],
            inaccessible: vec![inaccessible(GameKind::SADX, "/mnt/steam")],
        });

        let card = &cards[0];
        assert_eq!(card.state, CardState::Inaccessible);
        assert_eq!(card.note(), None, "the Grant Access button says it");
        assert_eq!(card.actions(), Some(("Grant Access", None)));
        assert_eq!(
            card.primary_action(),
            Some(WelcomeAction::GrantAccess("/mnt/steam".into()))
        );
        assert_eq!(card.secondary_action(), None);
        assert_eq!(
            card.options[0].selector_label(),
            "/mnt/steam (Needs access)"
        );
    }

    #[test]
    fn mixed_installs_prefer_one_that_can_be_set_up() {
        let cards = build_game_cards(&DetectionResult {
            games: vec![
                game(GameKind::SADX, "/games/sadx-1"),
                game(GameKind::SADX, "/games/sadx-2"),
            ],
            inaccessible: vec![
                inaccessible(GameKind::SADX, "/mnt/steam-1"),
                inaccessible(GameKind::SADX, "/mnt/steam-2"),
            ],
        });

        let card = &cards[0];
        assert_eq!(card.options.len(), 4);
        assert_eq!(card.state, CardState::Detected);
        assert_eq!(card.note(), None, "the install picker says it");
        assert_eq!(card.selected, 0);
    }

    #[test]
    fn the_alert_names_each_game_once() {
        assert_eq!(inaccessible_alert(&[]), None);
        assert_eq!(
            inaccessible_alert(&[
                inaccessible(GameKind::SADX, "/a"),
                inaccessible(GameKind::SADX, "/b"),
                inaccessible(GameKind::SA2, "/c"),
            ])
            .unwrap(),
            "Adventure Mods needs access to the Steam library with Sonic Adventure DX and Sonic Adventure 2. Use Grant Access on the game below."
        );
    }

    #[test]
    fn modded_games_offer_restore_and_repairs_offer_verification() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        std::fs::write(path.join("Sonic Adventure DX.exe"), "game").unwrap();
        std::fs::write(path.join(".adventure-mods-steam-repair"), "").unwrap();

        let mut card = GameCard::new(
            GameKind::SADX,
            CardState::Detected,
            vec![GameInstallOption::Detected(path.clone())],
        );
        assert!(card.steam_repair);
        assert_eq!(
            card.note(),
            Some("Let Steam verify the game files to finish restoring it.")
        );
        assert_eq!(card.actions(), Some(("Verify in Steam", Some("Set Up"))));
        assert_eq!(
            card.primary_action(),
            Some(WelcomeAction::VerifyInSteam(GameKind::SADX))
        );
        assert_eq!(
            card.secondary_action(),
            Some(WelcomeAction::SetUp(Game {
                kind: GameKind::SADX,
                path: path.clone()
            }))
        );

        // Pretend setup ran: the card offers to change mods or restore.
        card.steam_repair = false;
        card.modded = true;
        assert_eq!(card.note(), None);
        assert_eq!(card.actions(), Some(("Change Mods", Some("Restore"))));
        assert_eq!(
            card.secondary_action(),
            Some(WelcomeAction::ConfirmRestore(GameKind::SADX, path))
        );

        card.modded = false;
        assert_eq!(card.secondary_action(), None);
    }

    #[test]
    fn the_screen_waits_for_the_first_scan() {
        let harness = harness(None);
        harness.get_by_label("Looking for your games…");
        assert!(!harness.state().screen.has_result());
    }

    #[test]
    fn pressing_a_card_button_reports_its_action() {
        let mut harness = harness(Some(DetectionResult {
            games: vec![game(GameKind::SA2, "/games/sa2")],
            inaccessible: vec![inaccessible(GameKind::SADX, "/mnt/steam")],
        }));
        harness.get_by_label(
            "Adventure Mods needs access to the Steam library with Sonic Adventure DX. Use Grant Access on the game below.",
        );

        // The first game with a button starts selected.
        harness.get_by_label("Grant Access").click();
        harness.run_steps(4);
        harness.get_by_label("Sonic Adventure 2").click();
        harness.run_steps(8);
        harness.get_by_label("Set Up").click();
        harness.run_steps(4);

        assert!(
            harness
                .state()
                .actions
                .contains(&WelcomeAction::SetUp(game(GameKind::SA2, "/games/sa2")))
        );
        assert!(
            harness
                .state()
                .actions
                .contains(&WelcomeAction::GrantAccess("/mnt/steam".into()))
        );
    }

    #[test]
    fn the_install_picker_switches_the_card_to_another_install() {
        let mut harness = harness(Some(DetectionResult {
            games: vec![game(GameKind::SA2, "/games/sa2")],
            inaccessible: vec![inaccessible(GameKind::SA2, "/mnt/steam")],
        }));

        harness.get_by_label("Install").click();
        harness.run_steps(4);
        harness.get_by_label("/mnt/steam (Needs access)").click();
        harness.run_steps(4);
        assert_eq!(harness.state().screen.card(GameKind::SA2).selected, 1);
        harness.get_by_label("Grant Access");

        // A dismissed picker changes nothing.
        harness.get_by_label("Install").click();
        harness.run_steps(4);
        harness.key_press(egui::Key::Escape);
        harness.run_steps(4);
        assert_eq!(harness.state().screen.card(GameKind::SA2).selected, 1);
        assert!(harness.state().screen.install_picker.is_none());
    }

    #[test]
    fn rescanning_keeps_the_chosen_install_and_focus_lands_on_the_first_card() {
        let result = DetectionResult {
            games: vec![
                game(GameKind::SADX, "/games/a"),
                game(GameKind::SADX, "/games/b"),
            ],
            inaccessible: vec![],
        };
        let mut screen = WelcomeScreen::default();
        screen.set_result(result.clone());
        screen.cards[0].select(1);
        screen.cards[0].select(9);
        screen.set_result(result);
        assert_eq!(screen.card(GameKind::SADX).selected, 1);

        let mut harness = harness(Some(DetectionResult {
            games: vec![game(GameKind::SADX, "/games/a")],
            inaccessible: vec![],
        }));
        harness.state_mut().screen.focus_first();
        harness.run_steps(4);
        harness.key_press(egui::Key::Enter);
        harness.run_steps(4);
        assert_eq!(
            harness.state().actions,
            vec![WelcomeAction::SetUp(game(GameKind::SADX, "/games/a"))]
        );
    }

    #[test]
    fn selecting_a_tile_shows_its_game_and_focuses_its_button() {
        let mut harness = harness(Some(DetectionResult {
            games: vec![
                game(GameKind::SADX, "/games/sadx"),
                game(GameKind::SA2, "/games/sa2"),
            ],
            inaccessible: vec![],
        }));
        harness.run_steps(4);
        harness.get_by_label("SONIC ADVENTURE DX");

        // Moving focus onto a tile selects it, as the controller does.
        harness.get_by_label("Sonic Adventure 2").focus();
        harness.run_steps(4);
        harness.get_by_label("SONIC ADVENTURE 2");
        harness.get_by_label("Sonic Adventure DX").focus();
        harness.run_steps(4);
        harness.get_by_label("SONIC ADVENTURE DX");

        harness.get_by_label("Sonic Adventure 2").click();
        harness.run_steps(30);
        harness.get_by_label("SONIC ADVENTURE 2");
        assert!(harness.get_by_label("Set Up").is_focused());
        let sadx = harness.get_by_label("Sonic Adventure DX").rect();
        let sa2 = harness.get_by_label("Sonic Adventure 2").rect();
        assert!(sa2.width() > sadx.width(), "the selected tile is larger");
    }

    #[test]
    fn missing_games_show_dimmed_with_a_hint_and_no_buttons() {
        let mut harness = harness(Some(DetectionResult::default()));
        harness.run_steps(4);
        harness.get_by_label("Install it in Steam, then scan again.");
        assert!(harness.query_by_label("Set Up").is_none());
    }

    #[test]
    fn narrow_windows_shrink_the_tiles_to_fit() {
        let mut harness = harness(Some(DetectionResult::default()));
        harness.set_size(Vec2::new(480.0, 900.0));
        harness.run_steps(4);
        let sa2 = harness.get_by_label("Sonic Adventure 2").rect();
        assert!(sa2.max.x <= 480.0, "{sa2:?}");
    }

    #[test]
    fn covers_are_cropped_to_fill_any_area() {
        let wide = cover_uv(2.0, 4.0);
        assert_eq!(wide.width(), 1.0);
        assert_eq!(wide.height(), 0.5);
        let tall = cover_uv(2.0, 1.0);
        assert_eq!(tall.height(), 1.0);
        assert_eq!(tall.width(), 0.5);
    }
}
