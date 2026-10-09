//! The first screen: one card per game, saying whether it is ready for mods.

use std::path::{Path, PathBuf};

use egui::{Color32, RichText, Ui, Vec2};

use super::dialogs::{Answer, ChoiceDialog};
use super::images::{ImageCache, cover_resource};
use super::theme;
use super::widgets::{self, ButtonKind, Tone};
use crate::path_display::display_path;
use crate::setup::restore;
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{DetectionResult, InaccessibleGame};

const CARD_WIDTH: f32 = 380.0;
/// Covers are 460x215 Steam headers.
const COVER_HEIGHT: f32 = CARD_WIDTH * 215.0 / 460.0;

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

    /// Badge, its tone and the line under it.
    fn status(&self) -> (&'static str, Tone, Option<&'static str>) {
        let multiple = self.options.len() > 1;
        match self.selected_option() {
            None => (
                "Not installed",
                Tone::Neutral,
                Some("Install it in Steam, then scan again."),
            ),
            Some(GameInstallOption::Inaccessible(_)) => (
                "Needs access",
                Tone::Warning,
                Some("Allow access to this Steam library to set it up."),
            ),
            Some(GameInstallOption::Detected(_)) if self.steam_repair => (
                "Needs Steam repair",
                Tone::Warning,
                Some("Let Steam verify the game files to finish restoring it."),
            ),
            Some(GameInstallOption::Detected(_)) => {
                let note = multiple.then_some("Choose which install to set up.");
                if self.modded {
                    ("Mods installed", Tone::Success, note)
                } else {
                    ("Ready to set up", Tone::Accent, note)
                }
            }
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
    /// Focus the first card's main button on the next frame.
    focus_first: bool,
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
        let mut action = None;
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

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(ui.available_width().min(2.0 * CARD_WIDTH + 40.0));
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new("Choose a Game")
                            .text_style(theme::title_style())
                            .strong(),
                    );
                    ui.add_space(8.0);

                    let Some(result) = &self.result else {
                        ui.add_space(48.0);
                        ui.add(egui::Spinner::new().size(48.0));
                        ui.label(RichText::new("Looking for your games…").color(theme::TEXT_DIM));
                        return;
                    };

                    if let Some(alert) = inaccessible_alert(&result.inaccessible) {
                        widgets::banner(ui, Tone::Warning, &alert);
                        ui.add_space(8.0);
                    }

                    let side_by_side = ui.available_width() >= 2.0 * CARD_WIDTH + 24.0;
                    let layout = if side_by_side {
                        egui::Layout::left_to_right(egui::Align::Min)
                    } else {
                        egui::Layout::top_down(egui::Align::Center)
                    };
                    let width = if side_by_side {
                        2.0 * CARD_WIDTH + 24.0
                    } else {
                        CARD_WIDTH
                    };
                    ui.allocate_ui_with_layout(Vec2::new(width, 0.0), layout, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::splat(24.0);
                        for index in 0..self.cards.len() {
                            // Focus lands on the first card that has a button.
                            let focus = self.focus_first && self.cards[index].actions().is_some();
                            self.focus_first &= !focus;
                            if let Some(card_action) = self.show_card(ui, images, index, focus) {
                                action = Some(card_action);
                            }
                        }
                    });
                    ui.add_space(24.0);
                });
            });
        action
    }

    fn show_card(
        &mut self,
        ui: &mut Ui,
        images: &mut ImageCache,
        index: usize,
        focus: bool,
    ) -> Option<WelcomeAction> {
        let card = &self.cards[index];
        let mut action = None;
        let mut open_picker = false;
        // Stack the card's parts even when the cards sit side by side.
        ui.vertical(|ui| {
            widgets::card().inner_margin(0).show(ui, |ui| {
                ui.set_width(CARD_WIDTH);
                ui.spacing_mut().item_spacing.y = 10.0;
                let cover = cover_resource(card.kind);
                let size = Vec2::new(CARD_WIDTH, COVER_HEIGHT);
                match images.get(ui.ctx(), cover) {
                    Some(texture) => {
                        let tint = if card.state == CardState::Missing {
                            Color32::from_gray(90)
                        } else {
                            Color32::WHITE
                        };
                        ui.add(
                            egui::Image::new(&texture)
                                .fit_to_exact_size(size)
                                .tint(tint)
                                .corner_radius(egui::CornerRadius {
                                    nw: theme::CARD_RADIUS,
                                    ne: theme::CARD_RADIUS,
                                    sw: 0,
                                    se: 0,
                                }),
                        );
                    }
                    None => {
                        ui.allocate_exact_size(size, egui::Sense::hover());
                    }
                }
                egui::Frame::new().inner_margin(20).show(ui, |ui| {
                    ui.set_width(CARD_WIDTH - 40.0);
                    ui.label(RichText::new(card.kind.name()).heading().strong());
                    let (badge, tone, note) = card.status();
                    widgets::status_dot(ui, tone, badge);
                    if card.options.len() > 1
                        && let Some(option) = card.selected_option()
                        && widgets::choice_row(ui, "Install", &option.selector_label())
                            .on_hover_text("Choose which install to set up")
                            .clicked()
                    {
                        open_picker = true;
                    }
                    if let Some(note) = note {
                        widgets::caption(ui, note);
                    }
                    if let Some((primary, secondary)) = card.actions() {
                        ui.horizontal(|ui| {
                            let width = if secondary.is_some() {
                                CARD_WIDTH - 40.0 - 150.0
                            } else {
                                CARD_WIDTH - 40.0
                            };
                            let response =
                                widgets::button_sized(ui, primary, ButtonKind::Suggested, width);
                            let response = match card.selected_option() {
                                Some(option) => response.on_hover_text(display_path(option.path())),
                                None => response,
                            };
                            if focus {
                                response.request_focus();
                            }
                            if response.clicked() {
                                action = card.primary_action();
                            }
                            if let Some(secondary) = secondary
                                && widgets::button_sized(ui, secondary, ButtonKind::Normal, 136.0)
                                    .clicked()
                            {
                                action = card.secondary_action();
                            }
                        });
                    }
                });
            });
        });
        if open_picker {
            let card = &self.cards[index];
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
        assert_eq!(cards[0].status().0, "Not installed");
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
        assert_eq!(card.status().0, "Needs access");
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
        assert_eq!(
            card.status(),
            (
                "Ready to set up",
                Tone::Accent,
                Some("Choose which install to set up.")
            )
        );
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
        assert_eq!(card.status().0, "Needs Steam repair");
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
        assert_eq!(card.status().0, "Mods installed");
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

        harness.get_by_label("Set Up").click();
        harness.get_by_label("Grant Access").click();
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
    fn narrow_windows_stack_the_cards() {
        let mut harness = harness(Some(DetectionResult::default()));
        harness.set_size(Vec2::new(480.0, 900.0));
        harness.run_steps(4);
        let sadx = harness.get_by_label("Sonic Adventure DX").rect();
        let sa2 = harness.get_by_label("Sonic Adventure 2").rect();
        assert!(sa2.min.y > sadx.max.y, "{sadx:?} {sa2:?}");
    }
}
