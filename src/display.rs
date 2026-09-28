use gtk::prelude::{Cast, DisplayExt, ListModelExt, MonitorExt, SurfaceExt};
use gtk::{gdk, gio};

/// Detect the physical resolution of the relevant monitor.
///
/// When `surface` is provided, the monitor containing that surface is used and
/// the fractional scale is read from the surface (GTK 4.12+).  When no surface
/// is available the monitor with the largest landscape area is chosen and its
/// fractional scale is read via `Monitor::scale()` (GDK 4.14+).
///
/// Returns `None` if no monitor is found or if the computed dimensions are zero.
pub fn resolution_from_display(
    display: &gdk::Display,
    surface: Option<&gdk::Surface>,
) -> Option<(u32, u32)> {
    let (monitor, scale) = if let Some(s) = surface {
        let m = display.monitor_at_surface(s)?;
        let scale = s.scale();
        (m, scale)
    } else {
        let m = preferred_monitor(&display.monitors())?;
        let scale = m.scale();
        (m, scale)
    };

    physical_resolution(&monitor.geometry(), scale)
}

/// Pick the landscape monitor with the largest area, or any monitor when all
/// of them are portrait.
fn preferred_monitor(monitors: &gio::ListModel) -> Option<gdk::Monitor> {
    (0..monitors.n_items())
        .filter_map(|i| {
            monitors
                .item(i)
                .and_then(|m| m.downcast::<gdk::Monitor>().ok())
        })
        .max_by_key(|m| {
            let g = m.geometry();
            let (w, h) = (g.width() as i64, g.height() as i64);
            // Prefer landscape monitors (width >= height), then largest area.
            (if w >= h { 1i64 } else { 0i64 }, w * h)
        })
}

/// Scale a monitor's logical geometry to physical pixels.
fn physical_resolution(geometry: &gdk::Rectangle, scale: f64) -> Option<(u32, u32)> {
    let width = (geometry.width() as f64 * scale).round() as u32;
    let height = (geometry.height() as f64 * scale).round() as u32;

    if width == 0 || height == 0 {
        return None;
    }

    tracing::info!(
        "Detected resolution: {width}x{height} (logical: {}x{}, scale: {scale:.2})",
        geometry.width(),
        geometry.height(),
    );
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::prelude::*;

    #[gtk::test]
    fn resolution_from_display_uses_the_default_monitor() {
        let display = gdk::Display::default().expect("test display");
        let resolution = resolution_from_display(&display, None);
        assert!(resolution.is_some_and(|(width, height)| width > 0 && height > 0));
    }

    #[gtk::test]
    fn resolution_from_display_uses_a_surface_monitor_when_available() {
        let display = gdk::Display::default().expect("test display");
        let window = gtk::Window::new();
        window.set_default_size(320, 240);
        window.present();
        while gtk::glib::MainContext::default().iteration(false) {}

        let surface = window.surface().expect("realized test window");
        let resolution = resolution_from_display(&display, Some(&surface));
        assert!(resolution.is_some_and(|(width, height)| width > 0 && height > 0));
        window.close();
    }

    #[gtk::test]
    fn preferred_monitor_needs_at_least_one_monitor() {
        let display = gdk::Display::default().expect("test display");
        let monitors = display.monitors();
        assert!(monitors.n_items() > 0);

        let preferred = preferred_monitor(&monitors).expect("a test monitor");
        assert!(preferred.geometry().width() > 0);
        let no_monitors = gio::ListStore::new::<gdk::Monitor>();
        assert!(preferred_monitor(no_monitors.upcast_ref()).is_none());
    }

    #[test]
    fn physical_resolution_applies_the_scale_and_logs_both_sizes() {
        let (resolution, logs) = crate::test_log::capture_logs(|| {
            physical_resolution(&gdk::Rectangle::new(0, 0, 1280, 720), 1.5)
        });

        assert_eq!(resolution, Some((1920, 1080)));
        assert!(
            logs.contains("Detected resolution: 1920x1080 (logical: 1280x720, scale: 1.50)"),
            "logs were: {logs}"
        );
    }

    #[test]
    fn physical_resolution_rejects_empty_geometry() {
        assert_eq!(
            physical_resolution(&gdk::Rectangle::new(0, 0, 0, 720), 1.0),
            None
        );
        assert_eq!(
            physical_resolution(&gdk::Rectangle::new(0, 0, 1280, 0), 2.0),
            None
        );
        assert_eq!(
            physical_resolution(&gdk::Rectangle::new(0, 0, 1280, 720), 0.0),
            None
        );
    }
}
