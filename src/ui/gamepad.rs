//! Controller input. The D-pad and left stick move focus like the arrow keys,
//! A presses the focused button, B goes back, and the other buttons run the
//! screen's shortcuts shown in the footer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gilrs::{Axis, Button, EventType};

/// Hold a direction this long before it repeats.
const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_INTERVAL: Duration = Duration::from_millis(120);
/// How far the stick must tilt to count as a press, and come back to release.
const STICK_PRESS: f32 = 0.6;
const STICK_RELEASE: f32 = 0.3;

/// What a controller button means in the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    Up,
    Down,
    Left,
    Right,
    /// A (south): press the focused button.
    Confirm,
    /// B (east): close a dialog or go back.
    Back,
    /// X (west): the screen's secondary action.
    Action,
    /// Y (north): scan the Steam libraries again.
    Refresh,
    /// LB: previous screenshot.
    PreviousPage,
    /// RB: next screenshot.
    NextPage,
    /// Start: continue to the next screen.
    Start,
}

impl PadButton {
    fn from_button(button: Button) -> Option<Self> {
        Some(match button {
            Button::DPadUp => Self::Up,
            Button::DPadDown => Self::Down,
            Button::DPadLeft => Self::Left,
            Button::DPadRight => Self::Right,
            Button::South => Self::Confirm,
            Button::East => Self::Back,
            Button::West => Self::Action,
            Button::North => Self::Refresh,
            Button::LeftTrigger => Self::PreviousPage,
            Button::RightTrigger => Self::NextPage,
            Button::Start => Self::Start,
            _ => return None,
        })
    }

    pub fn is_direction(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::Left | Self::Right)
    }

    /// The key egui already understands for this button, if any.
    pub fn key(self) -> Option<egui::Key> {
        Some(match self {
            Self::Up => egui::Key::ArrowUp,
            Self::Down => egui::Key::ArrowDown,
            Self::Left => egui::Key::ArrowLeft,
            Self::Right => egui::Key::ArrowRight,
            Self::Confirm => egui::Key::Enter,
            Self::Back => egui::Key::Escape,
            _ => return None,
        })
    }
}

/// Turns raw controller events into presses, with key-like repeat for
/// directions that are held.
#[derive(Default)]
pub struct PadInput {
    held: Option<(PadButton, Instant)>,
    stick_x: Option<PadButton>,
    stick_y: Option<PadButton>,
}

impl PadInput {
    pub fn handle(&mut self, event: EventType, now: Instant) -> Option<PadButton> {
        match event {
            EventType::ButtonPressed(button, _) => {
                let pressed = PadButton::from_button(button)?;
                if pressed.is_direction() {
                    self.held = Some((pressed, now + REPEAT_DELAY));
                }
                Some(pressed)
            }
            EventType::ButtonReleased(button, _) => {
                let released = PadButton::from_button(button)?;
                self.release(released);
                None
            }
            EventType::AxisChanged(Axis::LeftStickX, value, _) => self.stick(
                value,
                PadButton::Left,
                PadButton::Right,
                |input| &mut input.stick_x,
                now,
            ),
            // Stick up is positive.
            EventType::AxisChanged(Axis::LeftStickY, value, _) => self.stick(
                value,
                PadButton::Down,
                PadButton::Up,
                |input| &mut input.stick_y,
                now,
            ),
            _ => None,
        }
    }

    fn stick(
        &mut self,
        value: f32,
        negative: PadButton,
        positive: PadButton,
        axis: fn(&mut Self) -> &mut Option<PadButton>,
        now: Instant,
    ) -> Option<PadButton> {
        let direction = if value <= -STICK_PRESS {
            Some(negative)
        } else if value >= STICK_PRESS {
            Some(positive)
        } else if value.abs() < STICK_RELEASE {
            None
        } else {
            // Between the thresholds nothing changes.
            return None;
        };
        let previous = std::mem::replace(axis(self), direction);
        if previous == direction {
            return None;
        }
        if let Some(previous) = previous {
            self.release(previous);
        }
        let pressed = direction?;
        self.held = Some((pressed, now + REPEAT_DELAY));
        Some(pressed)
    }

    fn release(&mut self, button: PadButton) {
        if self.held.is_some_and(|(held, _)| held == button) {
            self.held = None;
        }
    }

    /// A repeat of the held direction, if one is due.
    pub fn repeat(&mut self, now: Instant) -> Option<PadButton> {
        let (button, due) = self.held?;
        if now < due {
            return None;
        }
        self.held = Some((button, now + REPEAT_INTERVAL));
        Some(button)
    }

    /// How long to wait for events before a repeat is due.
    pub fn wait(&self, now: Instant) -> Duration {
        self.held
            .map(|(_, due)| due.saturating_duration_since(now))
            .unwrap_or(Duration::from_millis(250))
    }
}

/// Controller presses from a background thread.
pub struct Gamepad {
    presses: std::sync::mpsc::Receiver<PadButton>,
    connected: Arc<AtomicBool>,
}

impl Gamepad {
    /// Watch controllers, waking `ctx` on every press.
    pub fn spawn(ctx: &egui::Context) -> Self {
        let (tx, presses) = std::sync::mpsc::channel();
        let connected = Arc::new(AtomicBool::new(false));
        let thread_connected = connected.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut gilrs = match gilrs::Gilrs::new() {
                Ok(gilrs) => gilrs,
                Err(err) => return tracing::info!("Controllers are unavailable: {err}"),
            };
            pump(
                |timeout| {
                    let event = gilrs.next_event_blocking(Some(timeout));
                    let any = gilrs.gamepads().next().is_some();
                    thread_connected.store(any, Ordering::Relaxed);
                    event.map(|event| event.event)
                },
                forward(tx, ctx),
            );
        });
        Self { presses, connected }
    }

    #[cfg(test)]
    pub fn fake(presses: std::sync::mpsc::Receiver<PadButton>, connected: bool) -> Self {
        Self {
            presses,
            connected: Arc::new(AtomicBool::new(connected)),
        }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn presses(&self) -> Vec<PadButton> {
        self.presses.try_iter().collect()
    }
}

/// Hand presses to the app and wake it; `false` once the app is gone.
fn forward(
    tx: std::sync::mpsc::Sender<PadButton>,
    ctx: egui::Context,
) -> impl FnMut(PadButton) -> bool {
    move |press| {
        let sent = tx.send(press).is_ok();
        ctx.request_repaint();
        sent
    }
}

/// Read events from `next` and report presses to `send` until it fails.
fn pump(
    mut next: impl FnMut(Duration) -> Option<EventType>,
    mut send: impl FnMut(PadButton) -> bool,
) {
    let mut input = PadInput::default();
    loop {
        let event = next(input.wait(Instant::now()));
        let now = Instant::now();
        let press = event
            .and_then(|event| input.handle(event, now))
            .or_else(|| input.repeat(now));
        if let Some(press) = press
            && !send(press)
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gilrs::ev::Code;

    fn code() -> Code {
        // SAFETY: `Code` wraps two integers on Linux, so all zeroes is a valid
        // value. gilrs offers no other way to build one without a controller.
        unsafe { std::mem::zeroed() }
    }

    #[test]
    fn buttons_map_to_app_roles() {
        let mut input = PadInput::default();
        let now = Instant::now();
        let cases = [
            (Button::South, PadButton::Confirm),
            (Button::East, PadButton::Back),
            (Button::West, PadButton::Action),
            (Button::North, PadButton::Refresh),
            (Button::LeftTrigger, PadButton::PreviousPage),
            (Button::RightTrigger, PadButton::NextPage),
            (Button::Start, PadButton::Start),
            (Button::DPadUp, PadButton::Up),
            (Button::DPadDown, PadButton::Down),
            (Button::DPadLeft, PadButton::Left),
            (Button::DPadRight, PadButton::Right),
        ];
        for (button, expected) in cases {
            assert_eq!(
                input.handle(EventType::ButtonPressed(button, code()), now),
                Some(expected)
            );
        }
        assert_eq!(
            input.handle(EventType::ButtonPressed(Button::Mode, code()), now),
            None
        );
        assert_eq!(
            input.handle(EventType::ButtonReleased(Button::Mode, code()), now),
            None
        );
        assert_eq!(input.handle(EventType::Connected, now), None);
    }

    #[test]
    fn only_navigation_buttons_become_keys() {
        assert_eq!(PadButton::Up.key(), Some(egui::Key::ArrowUp));
        assert_eq!(PadButton::Down.key(), Some(egui::Key::ArrowDown));
        assert_eq!(PadButton::Left.key(), Some(egui::Key::ArrowLeft));
        assert_eq!(PadButton::Right.key(), Some(egui::Key::ArrowRight));
        assert_eq!(PadButton::Confirm.key(), Some(egui::Key::Enter));
        assert_eq!(PadButton::Back.key(), Some(egui::Key::Escape));
        assert_eq!(PadButton::Refresh.key(), None);
    }

    #[test]
    fn held_directions_repeat_until_released() {
        let mut input = PadInput::default();
        let start = Instant::now();
        input.handle(EventType::ButtonPressed(Button::DPadDown, code()), start);

        assert_eq!(input.wait(start), REPEAT_DELAY);
        assert_eq!(input.repeat(start), None);
        let first = start + REPEAT_DELAY;
        assert_eq!(input.repeat(first), Some(PadButton::Down));
        assert_eq!(input.repeat(first + REPEAT_INTERVAL), Some(PadButton::Down));

        // Releasing another button keeps the repeat going.
        input.handle(EventType::ButtonReleased(Button::DPadUp, code()), first);
        assert!(input.held.is_some());
        input.handle(EventType::ButtonReleased(Button::DPadDown, code()), first);
        assert_eq!(input.repeat(first + Duration::from_secs(5)), None);
        assert_eq!(input.wait(first), Duration::from_millis(250));

        // Buttons other than directions never repeat.
        input.handle(EventType::ButtonPressed(Button::South, code()), first);
        assert_eq!(input.repeat(first + Duration::from_secs(5)), None);
    }

    #[test]
    fn the_left_stick_acts_like_the_d_pad() {
        let mut input = PadInput::default();
        let now = Instant::now();
        let x = |value| EventType::AxisChanged(Axis::LeftStickX, value, code());
        let y = |value| EventType::AxisChanged(Axis::LeftStickY, value, code());

        assert_eq!(input.handle(x(0.9), now), Some(PadButton::Right));
        assert_eq!(input.handle(x(0.95), now), None, "still held");
        assert_eq!(input.handle(x(0.4), now), None, "between thresholds");
        assert_eq!(input.handle(x(-0.8), now), Some(PadButton::Left));
        assert_eq!(input.handle(x(0.0), now), None);
        assert!(input.held.is_none());

        assert_eq!(input.handle(y(0.7), now), Some(PadButton::Up));
        assert_eq!(input.handle(y(-0.7), now), Some(PadButton::Down));
        assert_eq!(
            input.handle(EventType::AxisChanged(Axis::RightStickX, 1.0, code()), now),
            None
        );
    }

    #[test]
    fn pumping_sends_presses_and_repeats_until_the_app_stops_listening() {
        let mut events = vec![
            Some(EventType::ButtonPressed(Button::DPadRight, code())),
            None,
            Some(EventType::ButtonPressed(Button::South, code())),
        ]
        .into_iter();
        let mut sent = Vec::new();

        pump(
            |_| {
                let event = events.next().unwrap_or(None);
                if event.is_none() {
                    std::thread::sleep(REPEAT_DELAY);
                }
                event
            },
            |press| {
                sent.push(press);
                sent.len() < 3
            },
        );

        assert_eq!(
            sent,
            vec![PadButton::Right, PadButton::Right, PadButton::Confirm]
        );
    }

    #[test]
    fn the_controller_thread_reports_presses_without_blocking() {
        let ctx = egui::Context::default();
        let gamepad = Gamepad::spawn(&ctx);
        // Let the thread look for controllers once; test machines have none.
        std::thread::sleep(Duration::from_millis(400));
        assert!(gamepad.presses().is_empty());
        let _ = gamepad.connected();

        let (tx, rx) = std::sync::mpsc::channel();
        let mut send = forward(tx, ctx.clone());
        assert!(send(PadButton::Back));
        assert_eq!(rx.recv().unwrap(), PadButton::Back);
        drop(rx);
        assert!(!send(PadButton::Back), "the app stopped listening");

        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(PadButton::Start).unwrap();
        let fake = Gamepad::fake(rx, true);
        assert!(fake.connected());
        assert_eq!(fake.presses(), vec![PadButton::Start]);
    }
}
