//! Modal dialogs. Each opens with a sensible button focused, so A and B on
//! a controller answer them.

use egui::{Id, Modal, RichText};

use super::theme;
use super::widgets::{self, ButtonKind};

/// How a dialog was answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The button at this index was pressed.
    Button(usize),
    /// Closed with B, Escape or a click outside.
    Dismissed,
}

/// A dialog with a heading, some text and a row of buttons.
pub struct MessageDialog {
    id: Id,
    heading: String,
    body: String,
    buttons: Vec<(String, ButtonKind)>,
    /// The button focused when the dialog opens.
    default: usize,
    opened: bool,
}

impl MessageDialog {
    pub fn new(id: &str, heading: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: Id::new(id),
            heading: heading.into(),
            body: body.into(),
            buttons: Vec::new(),
            default: 0,
            opened: false,
        }
    }

    pub fn button(mut self, label: &str, kind: ButtonKind) -> Self {
        self.buttons.push((label.to_owned(), kind));
        self
    }

    pub fn default_button(mut self, index: usize) -> Self {
        self.default = index;
        self
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Answer> {
        let focus = !std::mem::replace(&mut self.opened, true);
        let modal = Modal::new(self.id)
            .frame(widgets::card().inner_margin(28).fill(theme::PANEL))
            .show(ctx, |ui| {
                ui.set_max_width(560.0);
                ui.label(RichText::new(&self.heading).heading().strong());
                ui.add_space(4.0);
                ui.add(egui::Label::new(&self.body).wrap());
                ui.add_space(12.0);
                let mut pressed = None;
                ui.horizontal(|ui| {
                    for (index, (label, kind)) in self.buttons.iter().enumerate() {
                        let response = widgets::button(ui, label, *kind);
                        if focus && index == self.default {
                            response.request_focus();
                        }
                        if response.clicked() {
                            pressed = Some(index);
                        }
                    }
                });
                pressed
            });
        if let Some(index) = modal.inner {
            return Some(Answer::Button(index));
        }
        modal.should_close().then_some(Answer::Dismissed)
    }
}

/// A list of options to pick one from, such as a language.
pub struct ChoiceDialog {
    id: Id,
    title: String,
    options: Vec<String>,
    selected: Option<usize>,
    opened: bool,
}

impl ChoiceDialog {
    pub fn new(
        id: &str,
        title: impl Into<String>,
        options: Vec<String>,
        selected: Option<usize>,
    ) -> Self {
        Self {
            id: Id::new(id),
            title: title.into(),
            options,
            selected,
            opened: false,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// `Button(index)` when an option was picked.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Answer> {
        let focus = !std::mem::replace(&mut self.opened, true);
        let modal = Modal::new(self.id)
            .frame(widgets::card().inner_margin(28).fill(theme::PANEL))
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(RichText::new(&self.title).heading().strong());
                ui.add_space(4.0);
                let mut picked = None;
                egui::ScrollArea::vertical()
                    .max_height(ctx.content_rect().height() * 0.6)
                    .show(ui, |ui| {
                        for (index, option) in self.options.iter().enumerate() {
                            let label = if Some(index) == self.selected {
                                format!("✔  {option}")
                            } else {
                                option.clone()
                            };
                            let kind = if Some(index) == self.selected {
                                ButtonKind::Suggested
                            } else {
                                ButtonKind::Normal
                            };
                            let response =
                                widgets::button_sized(ui, &label, kind, ui.available_width());
                            if focus && index == self.selected.unwrap_or(0) {
                                response.request_focus();
                            }
                            if response.clicked() {
                                picked = Some(index);
                            }
                        }
                    });
                ui.add_space(8.0);
                if widgets::button(ui, "Cancel", ButtonKind::Normal).clicked() {
                    picked = Some(usize::MAX);
                }
                picked
            });
        match modal.inner {
            Some(usize::MAX) => Some(Answer::Dismissed),
            Some(index) => Some(Answer::Button(index)),
            None => modal.should_close().then_some(Answer::Dismissed),
        }
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    fn message_harness() -> Harness<'static, (MessageDialog, Vec<Answer>)> {
        let dialog = MessageDialog::new("test", "Restore the original game?", "Mods stay.")
            .button("Cancel", ButtonKind::Normal)
            .button("Restore", ButtonKind::Destructive)
            .default_button(0);
        Harness::new_ui_state(
            |ui, (dialog, answers): &mut (MessageDialog, Vec<Answer>)| {
                if let Some(answer) = dialog.show(ui.ctx()) {
                    answers.push(answer);
                }
            },
            (dialog, Vec::new()),
        )
    }

    #[test]
    fn message_dialogs_report_the_pressed_button() {
        let mut harness = message_harness();
        harness.get_by_label("Mods stay.");

        harness.get_by_label("Restore").click();
        harness.run();

        assert_eq!(harness.state().1, vec![Answer::Button(1)]);
    }

    #[test]
    fn message_dialogs_focus_the_default_button_and_close_on_escape() {
        let mut harness = message_harness();

        // The default button has focus, so Enter (A) presses it.
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(harness.state().1, vec![Answer::Button(0)]);

        harness.key_press(egui::Key::Escape);
        harness.run();
        assert_eq!(harness.state().1.last(), Some(&Answer::Dismissed));
    }

    fn choice_harness() -> Harness<'static, (ChoiceDialog, Vec<Answer>)> {
        let dialog = ChoiceDialog::new(
            "voices",
            "Voices",
            vec!["Japanese".into(), "English".into()],
            Some(1),
        );
        assert_eq!(dialog.title(), "Voices");
        Harness::new_ui_state(
            |ui, (dialog, answers): &mut (ChoiceDialog, Vec<Answer>)| {
                if let Some(answer) = dialog.show(ui.ctx()) {
                    answers.push(answer);
                }
            },
            (dialog, Vec::new()),
        )
    }

    #[test]
    fn choice_dialogs_mark_and_focus_the_current_option() {
        let mut harness = choice_harness();
        harness.get_by_label("✔  English");

        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(harness.state().1, vec![Answer::Button(1)]);

        harness.get_by_label("Japanese").click();
        harness.run();
        assert_eq!(harness.state().1.last(), Some(&Answer::Button(0)));
    }

    #[test]
    fn choice_dialogs_without_a_selection_focus_the_first_unmarked_option() {
        let dialog = ChoiceDialog::new(
            "presets",
            "Preset",
            vec!["First".into(), "Second".into()],
            None,
        );
        let mut harness = Harness::new_ui_state(
            |ui, (dialog, answers): &mut (ChoiceDialog, Vec<Answer>)| {
                if let Some(answer) = dialog.show(ui.ctx()) {
                    answers.push(answer);
                }
            },
            (dialog, Vec::new()),
        );
        harness.get_by_label("First");
        harness.get_by_label("Second");
        assert!(harness.query_by_label("✔  First").is_none());
        assert!(harness.query_by_label("✔  Second").is_none());
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(harness.state().1, vec![Answer::Button(0)]);
    }

    #[test]
    fn choice_dialogs_can_be_cancelled() {
        let mut harness = choice_harness();

        harness.get_by_label("Cancel").click();
        harness.run();
        harness.key_press(egui::Key::Escape);
        harness.run();

        assert_eq!(
            harness.state().1,
            vec![Answer::Dismissed, Answer::Dismissed]
        );
    }
}
