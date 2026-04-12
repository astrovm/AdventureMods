use gtk::gdk;
use gtk::prelude::{Cast, DisplayExt, ListModelExt, MonitorExt, SurfaceExt};

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
        let monitors = display.monitors();
        let m = (0..monitors.n_items())
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
            })?;
        let scale = m.scale();
        (m, scale)
    };

    let geometry = monitor.geometry();
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
