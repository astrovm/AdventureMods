//! A dark, big-picture style: large text, large targets and a bright focus
//! ring, so the app reads from a couch and works with a controller.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x10, 0x13, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x17, 0x1b, 0x22);
pub const CARD: Color32 = Color32::from_rgb(0x1f, 0x24, 0x2d);
pub const CARD_RAISED: Color32 = Color32::from_rgb(0x2a, 0x30, 0x3c);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x3a, 0x42, 0x52);
pub const TEXT: Color32 = Color32::from_rgb(0xf2, 0xf4, 0xf8);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0xa3, 0xb2);
pub const ACCENT: Color32 = Color32::from_rgb(0x35, 0x84, 0xe4);
pub const ACCENT_BRIGHT: Color32 = Color32::from_rgb(0x62, 0xa0, 0xea);
pub const SUCCESS: Color32 = Color32::from_rgb(0x2e, 0xc2, 0x7e);
pub const WARNING: Color32 = Color32::from_rgb(0xf5, 0xc2, 0x11);
pub const ERROR: Color32 = Color32::from_rgb(0xff, 0x6b, 0x5f);
pub const DESTRUCTIVE: Color32 = Color32::from_rgb(0xc0, 0x1c, 0x28);

/// Corner radius of cards, banners and dialogs.
pub const CARD_RADIUS: u8 = 18;
/// Smallest height of anything you can press.
pub const TARGET_HEIGHT: f32 = 52.0;

/// Paint the window background: a dark vertical gradient with a faint blue
/// glow at the top, under every panel.
pub fn paint_background(ctx: &egui::Context) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let mut mesh = egui::Mesh::default();
    let top = Color32::from_rgb(0x16, 0x1c, 0x27);
    let bottom = Color32::from_rgb(0x0c, 0x0e, 0x12);
    for (pos, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
    // A soft glow: a wide, very blurred shadow above the top edge.
    let glow =
        egui::Rect::from_center_size(rect.center_top(), Vec2::new(rect.width() * 0.6, 120.0));
    painter.add(
        egui::Shadow {
            offset: [0, 0],
            blur: 255,
            spread: 40,
            color: ACCENT.gamma_multiply(0.18),
        }
        .as_shape(glow, CornerRadius::same(60)),
    );
}

/// Text larger than a heading, for screen titles.
pub fn title_style() -> TextStyle {
    TextStyle::Name("Title".into())
}

/// Style `ui` and everything after it, applying the theme on first use.
pub fn apply_ui(ui: &mut egui::Ui) {
    if !ui.style().text_styles.contains_key(&title_style()) {
        apply(ui.ctx());
        ui.set_style(ui.ctx().global_style());
    }
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(16.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(20.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(21.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(28.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(16.0, FontFamily::Monospace),
            ),
            (title_style(), FontId::new(38.0, FontFamily::Proportional)),
        ]
        .into();

        let spacing = &mut style.spacing;
        spacing.item_spacing = Vec2::new(14.0, 14.0);
        spacing.button_padding = Vec2::new(22.0, 12.0);
        spacing.interact_size = Vec2::new(TARGET_HEIGHT, TARGET_HEIGHT);
        spacing.icon_width = 28.0;
        spacing.icon_width_inner = 16.0;
        spacing.icon_spacing = 12.0;
        spacing.window_margin = Margin::same(24);
        spacing.scroll.bar_width = 10.0;

        let visuals = &mut style.visuals;
        visuals.dark_mode = true;
        visuals.override_text_color = Some(TEXT);
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = PANEL;
        visuals.window_stroke = Stroke::new(1.0, CARD_RAISED);
        visuals.window_corner_radius = CornerRadius::same(CARD_RADIUS);
        visuals.extreme_bg_color = Color32::from_rgb(0x0b, 0x0d, 0x11);
        visuals.faint_bg_color = CARD;
        visuals.hyperlink_color = ACCENT_BRIGHT;
        visuals.warn_fg_color = WARNING;
        visuals.error_fg_color = ERROR;
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke = Stroke::new(2.0, TEXT);

        let radius = CornerRadius::same(12);
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = CARD;
        widgets.noninteractive.weak_bg_fill = CARD;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, CARD_RAISED);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.noninteractive.corner_radius = radius;

        widgets.inactive.bg_fill = CARD_RAISED;
        widgets.inactive.weak_bg_fill = CARD_RAISED;
        widgets.inactive.bg_stroke = Stroke::NONE;
        widgets.inactive.fg_stroke = Stroke::new(1.5, TEXT);
        widgets.inactive.corner_radius = radius;

        widgets.hovered.bg_fill = Color32::from_rgb(0x35, 0x3d, 0x4b);
        widgets.hovered.weak_bg_fill = Color32::from_rgb(0x35, 0x3d, 0x4b);
        widgets.hovered.bg_stroke = Stroke::new(1.5, ACCENT_BRIGHT);
        widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);
        widgets.hovered.corner_radius = radius;
        widgets.hovered.expansion = 1.0;

        // Focused and pressed widgets: what the controller is on must be obvious.
        widgets.active.bg_fill = Color32::from_rgb(0x3b, 0x45, 0x56);
        widgets.active.weak_bg_fill = Color32::from_rgb(0x3b, 0x45, 0x56);
        widgets.active.bg_stroke = Stroke::new(3.0, ACCENT_BRIGHT);
        widgets.active.fg_stroke = Stroke::new(2.0, TEXT);
        widgets.active.corner_radius = radius;
        widgets.active.expansion = 3.0;

        widgets.open = widgets.active;
    });
}

/// egui's fonts, plus the glyphs of "日本語" for the Japanese language option.
fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "japanese-label".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../data/fonts/NotoSansCJK-JapaneseLabel.otf"
        ))),
    );
    for family in fonts.families.values_mut() {
        family.push("japanese-label".to_owned());
    }
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_uses_large_text_and_targets() {
        let ctx = egui::Context::default();
        apply(&ctx);

        let style = ctx.global_style();
        assert_eq!(style.text_styles[&TextStyle::Body].size, 20.0);
        assert_eq!(style.text_styles[&title_style()].size, 38.0);
        assert_eq!(style.spacing.interact_size.y, TARGET_HEIGHT);
        assert_eq!(style.visuals.widgets.active.bg_stroke.width, 3.0);
        assert!(style.visuals.dark_mode);
    }

    #[test]
    fn the_japanese_label_has_glyphs() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();

        let has_glyphs = ctx.fonts_mut(|fonts| {
            "日本語"
                .chars()
                .all(|c| fonts.has_glyph(&FontId::new(20.0, FontFamily::Proportional), c))
        });
        assert!(has_glyphs);
    }
}
