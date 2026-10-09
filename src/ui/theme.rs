//! A bold, big-picture style in Sonic blue and ring gold: large rounded text,
//! large targets and a bright focus ring, so the app reads from a couch and
//! works with a controller.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x06, 0x15, 0x3a);
pub const PANEL: Color32 = Color32::from_rgb(0x0b, 0x22, 0x55);
pub const CARD: Color32 = Color32::from_rgb(0x0e, 0x29, 0x66);
pub const CARD_RAISED: Color32 = Color32::from_rgb(0x18, 0x38, 0x80);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x22, 0x47, 0x99);
pub const TEXT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xa9, 0xbb, 0xe0);
/// Ring gold, for the main action and what is selected.
pub const ACCENT: Color32 = Color32::from_rgb(0xf5, 0xa3, 0x00);
pub const ACCENT_BRIGHT: Color32 = Color32::from_rgb(0xff, 0xd2, 0x3f);
/// The darker lip under gold buttons.
pub const ACCENT_DEEP: Color32 = Color32::from_rgb(0xb9, 0x74, 0x00);
/// Text on gold.
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x3a, 0x23, 0x00);
pub const SUCCESS: Color32 = Color32::from_rgb(0x5c, 0xe6, 0x8a);
pub const WARNING: Color32 = Color32::from_rgb(0xff, 0x8f, 0x3f);
pub const ERROR: Color32 = Color32::from_rgb(0xff, 0x6b, 0x5f);
pub const DESTRUCTIVE: Color32 = Color32::from_rgb(0xd6, 0x2e, 0x3a);

/// Rubik Bold, for buttons and headings.
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Rubik ExtraBold, for titles and game names.
pub fn heavy() -> FontFamily {
    FontFamily::Name("heavy".into())
}

/// Corner radius of cards, banners and dialogs.
pub const CARD_RADIUS: u8 = 18;
/// Smallest height of anything you can press.
pub const TARGET_HEIGHT: f32 = 52.0;

/// Paint the window background: a deep blue gradient with a bright blue glow
/// at the top, under every panel.
pub fn paint_background(ctx: &egui::Context) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let mut mesh = egui::Mesh::default();
    let top = Color32::from_rgb(0x0f, 0x33, 0x86);
    let bottom = BACKGROUND;
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
            color: Color32::from_rgb(0x2a, 0x6c, 0xe6).gamma_multiply(0.55),
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
        let mut style = (*ui.ctx().global_style()).clone();
        if !fonts_ready(ui.ctx()) {
            // New fonts load with the next frame; until then use the default.
            for font in style.text_styles.values_mut() {
                font.family = FontFamily::Proportional;
            }
            ui.ctx().request_discard("load the theme's fonts");
        }
        ui.set_style(style);
    }
}

/// Whether Rubik is loaded yet.
fn fonts_ready(ctx: &egui::Context) -> bool {
    ctx.fonts(|fonts| fonts.families().contains(&heavy()))
}

/// A font of `family`, or the default one on the frame before Rubik loads.
pub fn font(ctx: &egui::Context, size: f32, family: FontFamily) -> FontId {
    if fonts_ready(ctx) {
        FontId::new(size, family)
    } else {
        FontId::proportional(size)
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
            (TextStyle::Button, FontId::new(20.0, bold())),
            (TextStyle::Heading, FontId::new(26.0, bold())),
            (
                TextStyle::Monospace,
                FontId::new(16.0, FontFamily::Monospace),
            ),
            (title_style(), FontId::new(36.0, heavy())),
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
        visuals.extreme_bg_color = Color32::from_rgb(0x04, 0x0e, 0x28);
        visuals.faint_bg_color = CARD;
        visuals.hyperlink_color = ACCENT_BRIGHT;
        visuals.warn_fg_color = WARNING;
        visuals.error_fg_color = ERROR;
        visuals.selection.bg_fill = ACCENT.gamma_multiply(0.6);
        visuals.selection.stroke = Stroke::new(2.0, ACCENT_BRIGHT);

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

        widgets.hovered.bg_fill = CARD_HOVER;
        widgets.hovered.weak_bg_fill = CARD_HOVER;
        widgets.hovered.bg_stroke = Stroke::new(1.5, ACCENT_BRIGHT);
        widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);
        widgets.hovered.corner_radius = radius;
        widgets.hovered.expansion = 1.0;

        // Focused and pressed widgets: what the controller is on must be obvious.
        widgets.active.bg_fill = CARD_HOVER;
        widgets.active.weak_bg_fill = CARD_HOVER;
        widgets.active.bg_stroke = Stroke::new(3.0, ACCENT_BRIGHT);
        widgets.active.fg_stroke = Stroke::new(2.0, TEXT);
        widgets.active.corner_radius = radius;
        widgets.active.expansion = 3.0;

        widgets.open = widgets.active;
    });
}

/// Rubik in three weights, with egui's fonts behind it for symbols, plus the
/// glyphs of "日本語" for the Japanese language option.
fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "rubik-medium",
            &include_bytes!("../../data/fonts/Rubik-Medium.ttf")[..],
        ),
        (
            "rubik-bold",
            &include_bytes!("../../data/fonts/Rubik-Bold.ttf")[..],
        ),
        (
            "rubik-extrabold",
            &include_bytes!("../../data/fonts/Rubik-ExtraBold.ttf")[..],
        ),
        (
            "japanese-label",
            &include_bytes!("../../data/fonts/NotoSansCJK-JapaneseLabel.otf")[..],
        ),
    ] {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    for (family, first) in [
        (FontFamily::Proportional, "rubik-medium"),
        (bold(), "rubik-bold"),
        (heavy(), "rubik-extrabold"),
    ] {
        let mut chain = vec![first.to_owned()];
        chain.extend(fallbacks.iter().cloned());
        fonts.families.insert(family, chain);
    }
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
        assert_eq!(style.text_styles[&title_style()].size, 36.0);
        assert_eq!(style.text_styles[&title_style()].family, heavy());
        assert_eq!(style.text_styles[&TextStyle::Button].family, bold());
        assert_eq!(style.spacing.interact_size.y, TARGET_HEIGHT);
        assert_eq!(style.visuals.widgets.active.bg_stroke.width, 3.0);
        assert!(style.visuals.dark_mode);
    }

    #[test]
    fn a_ui_styled_after_the_fonts_load_uses_rubik_at_once() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let mut families = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                // A fresh style, as a new window would have.
                ui.set_style(egui::Style::default());
                apply_ui(ui);
                families.push(ui.style().text_styles[&title_style()].family.clone());
            });
            output.textures_delta.clear();
        }

        assert_eq!(families, vec![heavy(), heavy()]);
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
