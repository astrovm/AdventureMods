//! Short, eased animations. Everything settles within a few frames and only
//! asks for repaints while it moves, so an idle screen costs nothing.

use egui::{Context, Id, Response, Ui};

/// How long hover, focus and press effects take, in seconds.
pub const QUICK: f32 = 0.12;
/// How long a screen takes to fade and slide in, in seconds.
pub const SCREEN: f64 = 0.22;
/// How far a new screen slides up while it fades in, in points.
pub const SCREEN_SLIDE: f32 = 18.0;

/// Cubic ease-out: fast at first, gentle at the end.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// 0 at rest, 1 while `response` is hovered or focused.
pub fn highlight(ui: &Ui, response: &Response) -> f32 {
    let on = response.hovered() || response.has_focus();
    ui.ctx()
        .animate_bool_with_time_and_easing(response.id.with("highlight"), on, QUICK, ease_out)
}

/// 0 at rest, 1 while `response` is held down.
pub fn press(ui: &Ui, response: &Response) -> f32 {
    ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("press"),
        response.is_pointer_button_down_on(),
        QUICK,
        ease_out,
    )
}

/// Fades a screen in whenever its key changes.
#[derive(Default)]
pub struct ScreenTransition {
    key: Option<u64>,
    started: f64,
}

impl ScreenTransition {
    /// Progress from 0 (just changed) to 1 (settled) for the screen `key`.
    pub fn progress(&mut self, ctx: &Context, key: u64) -> f32 {
        let now = ctx.input(|input| input.time);
        if self.key != Some(key) {
            // The first screen is already there; later ones animate in.
            self.started = if self.key.is_some() {
                now
            } else {
                now - SCREEN
            };
            self.key = Some(key);
        }
        let t = ((now - self.started) / SCREEN) as f32;
        if t < 1.0 {
            ctx.request_repaint();
        }
        ease_out(t)
    }

    /// Draw `add` faded and slid in by the transition for `key`.
    pub fn show<R>(&mut self, ui: &mut Ui, key: u64, add: impl FnOnce(&mut Ui) -> R) -> R {
        let t = self.progress(ui.ctx(), key);
        ui.multiply_opacity(t);
        ui.add_space((1.0 - t) * SCREEN_SLIDE);
        add(ui)
    }
}

/// A stable key for `value`, for [`ScreenTransition`].
pub fn key(value: impl std::hash::Hash + std::fmt::Debug) -> u64 {
    Id::new(value).value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_starts_fast_and_ends_at_one() {
        assert_eq!(ease_out(-1.0), 0.0);
        assert_eq!(ease_out(0.0), 0.0);
        assert!(ease_out(0.5) > 0.5);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(2.0), 1.0);
    }

    #[test]
    fn the_first_screen_shows_at_once_and_later_ones_fade_in() {
        let ctx = Context::default();
        let mut transition = ScreenTransition::default();
        let mut progress = Vec::new();
        let mut input = egui::RawInput::default();
        for (time, key) in [(0.0, 1), (0.05, 2), (0.1, 2), (1.0, 2)] {
            input.time = Some(time);
            let mut output = ctx.run_ui(input.clone(), |ui| {
                progress.push(transition.progress(ui.ctx(), key));
            });
            output.textures_delta.clear();
        }

        assert_eq!(progress[0], 1.0, "nothing to fade from");
        assert_eq!(progress[1], 0.0, "a new screen starts hidden");
        assert!(progress[2] > 0.0 && progress[2] < 1.0);
        assert_eq!(progress[3], 1.0);
    }

    #[test]
    fn keys_are_stable_and_distinct() {
        assert_eq!(key(("setup", 2)), key(("setup", 2)));
        assert_ne!(key(("setup", 2)), key(("setup", 3)));
    }
}
