//! Big, controller-friendly building blocks shared by every screen.

use egui::{
    Align2, Color32, CornerRadius, FontSelection, Frame, Margin, Response, RichText, Sense, Stroke,
    TextStyle, Ui, Vec2, WidgetInfo, WidgetText, WidgetType,
};

use super::{motion, theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Success,
    Warning,
    Error,
    Neutral,
}

impl Tone {
    pub fn color(self) -> Color32 {
        match self {
            Self::Accent => theme::ACCENT_BRIGHT,
            Self::Success => theme::SUCCESS,
            Self::Warning => theme::WARNING,
            Self::Error => theme::ERROR,
            Self::Neutral => theme::TEXT_DIM,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Suggested,
    Normal,
    Destructive,
}

/// A large rounded button. Focus draws a bright ring around it.
pub fn button(ui: &mut Ui, text: &str, kind: ButtonKind) -> Response {
    button_sized(ui, text, kind, 0.0)
}

/// [`button`] at least `width` wide.
pub fn button_sized(ui: &mut Ui, text: &str, kind: ButtonKind, width: f32) -> Response {
    let enabled = ui.is_enabled();
    let text_color = match (enabled, kind) {
        (false, _) => theme::TEXT_DIM,
        (true, ButtonKind::Suggested) => theme::ON_ACCENT,
        (true, _) => theme::TEXT,
    };
    let galley = WidgetText::from(RichText::new(text).color(text_color)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    );
    let padding = ui.spacing().button_padding.x;
    let size = Vec2::new(
        (galley.size().x + 2.0 * padding).max(width.max(120.0)),
        theme::TARGET_HEIGHT,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, galley.text()));

    let (fill, lit) = match kind {
        ButtonKind::Suggested => (theme::ACCENT, theme::ACCENT_BRIGHT),
        ButtonKind::Normal => (Color32::from_white_alpha(30), Color32::from_white_alpha(56)),
        ButtonKind::Destructive => (theme::DESTRUCTIVE, theme::ERROR),
    };
    let highlight = motion::highlight(ui, &response);
    let press = motion::press(ui, &response);
    // Pressing pushes the button down onto its lip.
    let lip = if kind == ButtonKind::Normal { 0.0 } else { 3.0 };
    let body = rect
        .shrink2(Vec2::new(0.0, lip / 2.0))
        .translate(Vec2::new(0.0, -lip / 2.0 + press * lip));
    let radius = CornerRadius::same((body.height() / 2.0) as u8);
    let painter = ui.painter();
    if kind != ButtonKind::Normal && enabled {
        painter.add(
            egui::Shadow {
                offset: [0, 8],
                blur: 20,
                spread: 0,
                color: fill.gamma_multiply(0.2 + 0.3 * highlight),
            }
            .as_shape(body, radius),
        );
        let deep = match kind {
            ButtonKind::Suggested => theme::ACCENT_DEEP,
            _ => Color32::from_rgb(0x8a, 0x14, 0x1e),
        };
        painter.rect_filled(
            body.translate(Vec2::new(0.0, lip * (1.0 - press))),
            radius,
            deep,
        );
    }
    painter.rect_filled(body, radius, fill.lerp_to_gamma(lit, highlight * 0.7));
    painter.galley(body.center() - galley.size() / 2.0, galley, text_color);
    focus_ring(ui, &response);
    response
}

/// A round button showing only `icon`; `label` names it for hover text and
/// screen readers.
pub fn icon_button(ui: &mut Ui, icon: &str, label: &str) -> Response {
    painted_icon_button(ui, label, |painter, rect, color| {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(30.0),
            color,
        );
    })
}

/// A round "Back" button with a chevron pointing left.
pub fn back_button(ui: &mut Ui) -> Response {
    painted_icon_button(ui, "Back", |painter, rect, color| {
        let center = rect.center() + Vec2::new(-2.0, 0.0);
        let arm = 9.0;
        painter.line(
            vec![
                center + Vec2::new(arm / 2.0, -arm),
                center + Vec2::new(-arm / 2.0, 0.0),
                center + Vec2::new(arm / 2.0, arm),
            ],
            Stroke::new(3.0, color),
        );
    })
}

fn painted_icon_button(
    ui: &mut Ui,
    label: &str,
    paint: impl FnOnce(&egui::Painter, egui::Rect, Color32),
) -> Response {
    let size = Vec2::splat(theme::TARGET_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    let highlight = motion::highlight(ui, &response);
    let press = motion::press(ui, &response);
    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        rect.width() / 2.0 - press * 2.0,
        Color32::from_white_alpha(18).lerp_to_gamma(Color32::from_white_alpha(48), highlight),
    );
    paint(
        painter,
        rect,
        theme::TEXT
            .gamma_multiply(0.85)
            .lerp_to_gamma(theme::TEXT, highlight),
    );
    focus_ring(ui, &response);
    response.on_hover_text(label)
}

/// How far the focus ring reaches outside its widget, gap and stroke included.
/// Scroll areas clip their content, so lists leave this much room around it.
pub const FOCUS_RING_OUTSET: f32 = 7.0;

/// Outline the widget the controller is on. The ring fades in.
pub fn focus_ring(ui: &Ui, response: &Response) {
    let shown = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("focus"),
        response.has_focus(),
        motion::QUICK,
        motion::ease_out,
    );
    if shown > 0.0 {
        let radius = (response.rect.height() / 2.0) as u8 + 4;
        ui.painter().rect_stroke(
            response.rect.expand(2.0 + 2.0 * shown),
            CornerRadius::same(radius),
            Stroke::new(3.0, theme::ACCENT_BRIGHT.gamma_multiply(shown)),
            egui::StrokeKind::Outside,
        );
    }
}

/// A card: a rounded, filled box with room inside, floating on a soft shadow.
pub fn card() -> Frame {
    Frame::new()
        .fill(theme::CARD)
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(Margin::same(20))
        .shadow(egui::Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(110),
        })
}

/// A tinted message box with a colored stripe.
pub fn banner(ui: &mut Ui, tone: Tone, text: &str) -> Response {
    let color = tone.color();
    Frame::new()
        .fill(color.gamma_multiply(0.14))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(18, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 6.0, color);
                ui.add(egui::Label::new(text).wrap());
            });
        })
        .response
}

/// A colored dot with a short state label, such as "Mods installed".
pub fn status_dot(ui: &mut Ui, tone: Tone, label: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 6.0, tone.color());
        ui.label(RichText::new(label).strong().color(tone.color()));
    });
}

/// Dimmed, smaller text that wraps.
pub fn caption(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .text_style(TextStyle::Small)
                .color(theme::TEXT_DIM),
        )
        .wrap(),
    )
}

/// A full-width row with a check box, a title and a description. Pressing it
/// flips `checked`.
pub fn toggle_row(ui: &mut Ui, checked: &mut bool, title: &str, subtitle: &str) -> Response {
    let width = ui.available_width();
    let text_left = 64.0;
    let wrap = (width - text_left - 16.0).max(40.0);
    let title_galley = WidgetText::from(RichText::new(title).strong()).into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        wrap,
        FontSelection::Default,
    );
    let subtitle_galley = WidgetText::from(
        RichText::new(subtitle)
            .text_style(TextStyle::Small)
            .color(theme::TEXT_DIM),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        wrap,
        FontSelection::Default,
    );
    let height = (title_galley.size().y + subtitle_galley.size().y + 30.0).max(72.0);

    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let value = *checked;
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, value, title));

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let highlight = motion::highlight(ui, &response);
        let on = ui.ctx().animate_bool_with_time_and_easing(
            response.id.with("checked"),
            value,
            motion::QUICK,
            motion::ease_out,
        );
        let fill = theme::CARD.lerp_to_gamma(theme::CARD_RAISED, highlight);
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(14),
            fill,
            if response.has_focus() {
                Stroke::new(3.0, theme::ACCENT_BRIGHT)
            } else {
                Stroke::NONE
            },
            egui::StrokeKind::Inside,
        );
        let check = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 32.0, rect.center().y),
            Vec2::splat(28.0),
        );
        painter.rect(
            check,
            CornerRadius::same(8),
            theme::BACKGROUND.lerp_to_gamma(theme::ACCENT, on),
            Stroke::new(
                2.0,
                visuals.fg_stroke.color.lerp_to_gamma(theme::ACCENT, on),
            ),
            egui::StrokeKind::Inside,
        );
        if on > 0.0 {
            // The check mark grows from the box's center.
            let mark = |point: egui::Pos2| check.center() + (point - check.center()) * on;
            let points = [
                mark(check.left_center() + Vec2::new(6.0, 0.0)),
                mark(check.center_bottom() + Vec2::new(-2.0, -7.0)),
                mark(check.right_top() + Vec2::new(-6.0, 7.0)),
            ];
            painter.line(
                points.to_vec(),
                Stroke::new(3.0, theme::TEXT.gamma_multiply(on)),
            );
        }
        let top = rect.top() + (height - title_galley.size().y - subtitle_galley.size().y) / 2.0;
        painter.galley(
            egui::pos2(rect.left() + text_left, top),
            title_galley.clone(),
            theme::TEXT,
        );
        painter.galley(
            egui::pos2(rect.left() + text_left, top + title_galley.size().y + 2.0),
            subtitle_galley,
            theme::TEXT_DIM,
        );
    }
    response
}

/// A full-width row naming a setting and its value, like "Voices  Japanese ›".
/// Pressing it should open a picker.
pub fn choice_row(ui: &mut Ui, title: &str, value: &str) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::click());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(WidgetType::ComboBox, true, title);
        info.current_text_value = Some(value.to_owned());
        info
    });

    // Painting is clipped, so off-screen rows cost little.
    let highlight = motion::highlight(ui, &response);
    let painter = ui.painter();
    let fill = theme::CARD.lerp_to_gamma(theme::CARD_RAISED, highlight);
    painter.rect(
        rect,
        CornerRadius::same(14),
        fill,
        if response.has_focus() {
            Stroke::new(3.0, theme::ACCENT_BRIGHT)
        } else {
            Stroke::NONE
        },
        egui::StrokeKind::Inside,
    );
    let body = TextStyle::Body.resolve(ui.style());
    painter.text(
        rect.left_center() + Vec2::new(20.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        body.clone(),
        theme::TEXT,
    );
    painter.text(
        rect.right_center() - Vec2::new(20.0, 0.0),
        Align2::RIGHT_CENTER,
        format!("{value}  ›"),
        body,
        theme::ACCENT_BRIGHT,
    );
    response
}

/// A rounded progress bar with `text` on it. The fill glides to `fraction`;
/// `pulse` adds a moving shimmer for work without a known end.
pub fn progress_bar(ui: &mut Ui, fraction: f32, text: &str, pulse: bool) -> Response {
    let size = Vec2::new(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, text));
    let shown = ui.ctx().animate_value_with_time(
        response.id.with("fraction"),
        fraction.clamp(0.0, 1.0),
        0.35,
    );
    let radius = CornerRadius::same(18);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, theme::CARD);
    if shown > 0.0 {
        let filled = egui::Rect::from_min_size(
            rect.min,
            Vec2::new((rect.width() * shown).max(rect.height()), rect.height()),
        );
        painter.rect_filled(filled, radius, theme::ACCENT);
    }
    if pulse {
        // A soft highlight sweeping across, once every 1.6 seconds.
        let time = ui.input(|input| input.time);
        let phase = (time / 1.6).fract() as f32;
        let width = rect.width() * 0.25;
        let x = rect.left() - width + phase * (rect.width() + width);
        let sweep = egui::Rect::from_min_max(
            egui::pos2(x.max(rect.left()), rect.top()),
            egui::pos2((x + width).min(rect.right()), rect.bottom()),
        );
        if sweep.width() > 0.0 {
            painter.rect_filled(sweep, radius, Color32::from_white_alpha(28));
        }
        ui.ctx().request_repaint();
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        TextStyle::Body.resolve(ui.style()),
        theme::TEXT,
    );
    response
}

/// A controller button glyph, such as a green "A", followed by what it does.
pub fn pad_hint(ui: &mut Ui, glyph: &str, color: Color32, label: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 15.0, color);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(16.0),
            theme::BACKGROUND,
        );
        ui.label(RichText::new(label).color(theme::TEXT_DIM));
    });
}

/// A large centered icon, title and description, for screens that say one thing.
pub fn status_page(ui: &mut Ui, icon: &str, tone: Tone, title: &str, description: &str) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(640.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(112.0), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 56.0, tone.color().gamma_multiply(0.18));
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(56.0),
            tone.color(),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(title)
                .text_style(theme::title_style())
                .strong(),
        );
        ui.add(egui::Label::new(description).wrap());
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    #[test]
    fn tones_have_distinct_colors() {
        let colors: Vec<_> = [
            Tone::Accent,
            Tone::Success,
            Tone::Warning,
            Tone::Error,
            Tone::Neutral,
        ]
        .map(Tone::color)
        .to_vec();
        for (index, color) in colors.iter().enumerate() {
            assert!(!colors[index + 1..].contains(color));
        }
    }

    #[test]
    fn toggle_rows_flip_when_pressed_and_show_focus() {
        let mut harness = Harness::new_ui_state(
            |ui, checked: &mut bool| {
                theme::apply_ui(ui);
                let response = toggle_row(ui, checked, "Dreamcast Conversion", "Restores the look");
                if ui.input(|i| i.key_pressed(egui::Key::F1)) {
                    response.request_focus();
                }
            },
            false,
        );

        harness.get_by_label("Dreamcast Conversion").click();
        harness.run();
        assert!(*harness.state());

        harness.key_press(egui::Key::F1);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert!(!*harness.state(), "Enter on the focused row flips it back");
    }

    #[test]
    fn every_widget_renders_in_and_out_of_focus() {
        let mut harness = Harness::new_ui_state(
            |ui, clicks: &mut Vec<&'static str>| {
                theme::apply_ui(ui);
                for (label, kind) in [
                    ("Suggested", ButtonKind::Suggested),
                    ("Normal", ButtonKind::Normal),
                    ("Destructive", ButtonKind::Destructive),
                ] {
                    if button(ui, label, kind).clicked() {
                        clicks.push(label);
                    }
                }
                if choice_row(ui, "Voices", "Japanese").clicked() {
                    clicks.push("Voices");
                }
                banner(ui, Tone::Warning, "Needs access");
                status_dot(ui, Tone::Success, "Mods installed");
                caption(ui, "Small print");
                pad_hint(ui, "A", theme::SUCCESS, "Select");
                status_page(ui, "✔", Tone::Success, "Proton Is Ready", "All set");
            },
            Vec::new(),
        );

        harness.get_by_label("Voices").click();
        harness.get_by_label("Destructive").click();
        harness.run();
        // Tab through everything so focused states draw too.
        for _ in 0..5 {
            harness.key_press(egui::Key::Tab);
            harness.run();
        }
        harness.get_by_label("Proton Is Ready");
        harness.get_by_label("Needs access");
        let mut clicks = harness.state().clone();
        clicks.sort();
        assert_eq!(clicks, vec!["Destructive", "Voices"]);
    }
}
