//! Monitor resolution, which the mod configs use as the game's resolution.

/// The physical resolution of the monitor egui reports for the window.
pub fn resolution_from_viewport(viewport: &egui::ViewportInfo) -> Option<(u32, u32)> {
    let size = viewport.monitor_size?;
    let scale = viewport.native_pixels_per_point.unwrap_or(1.0);
    physical_resolution(size.x, size.y, scale)
}

/// Pick the landscape monitor with the largest area, or any monitor when all
/// of them are portrait.
pub fn preferred_resolution(monitors: impl IntoIterator<Item = (u32, u32)>) -> Option<(u32, u32)> {
    monitors
        .into_iter()
        .filter(|&(width, height)| width > 0 && height > 0)
        .max_by_key(|&(width, height)| (width >= height, u64::from(width) * u64::from(height)))
}

/// Scale a monitor's logical size to physical pixels.
fn physical_resolution(width: f32, height: f32, scale: f32) -> Option<(u32, u32)> {
    let physical_width = (width * scale).round() as u32;
    let physical_height = (height * scale).round() as u32;

    if physical_width == 0 || physical_height == 0 {
        return None;
    }

    tracing::info!(
        "Detected resolution: {physical_width}x{physical_height} (logical: {width}x{height}, scale: {scale:.2})",
    );
    Some((physical_width, physical_height))
}

/// Ask the display server for its monitors, for the command line where no
/// window exists. winit only allows one event loop per process, so the first
/// answer is kept.
pub fn resolution_from_display_server() -> Option<(u32, u32)> {
    static RESOLUTION: std::sync::OnceLock<Option<(u32, u32)>> = std::sync::OnceLock::new();
    *RESOLUTION.get_or_init(probe_display_server)
}

fn probe_display_server() -> Option<(u32, u32)> {
    use winit::event::Event;
    use winit::event_loop::EventLoop;
    use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
    use winit::platform::x11::EventLoopBuilderExtX11;

    // Tests and the CLI may not run on the main thread.
    let mut event_loop = EventLoop::builder().with_any_thread(true).build().ok()?;
    let mut resolution = None;
    // A closure instead of an `ApplicationHandler`: the probe opens no window,
    // so a handler's required `window_event` would never run.
    #[allow(deprecated)]
    event_loop
        .run_on_demand(|event, event_loop| {
            if let Event::Resumed = event {
                resolution = preferred_resolution(event_loop.available_monitors().map(|monitor| {
                    let size = monitor.size();
                    (size.width, size.height)
                }));
                event_loop.exit();
            }
        })
        .ok()?;
    resolution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_resolution_applies_the_scale_and_logs_both_sizes() {
        let viewport = egui::ViewportInfo {
            monitor_size: Some(egui::vec2(1280.0, 720.0)),
            native_pixels_per_point: Some(1.5),
            ..Default::default()
        };

        let (resolution, logs) =
            crate::test_log::capture_logs(|| resolution_from_viewport(&viewport));

        assert_eq!(resolution, Some((1920, 1080)));
        assert!(
            logs.contains("Detected resolution: 1920x1080 (logical: 1280x720, scale: 1.50)"),
            "logs were: {logs}"
        );
    }

    #[test]
    fn viewport_resolution_needs_a_monitor_size() {
        assert_eq!(
            resolution_from_viewport(&egui::ViewportInfo::default()),
            None
        );

        let unscaled = egui::ViewportInfo {
            monitor_size: Some(egui::vec2(1280.0, 800.0)),
            ..Default::default()
        };
        assert_eq!(resolution_from_viewport(&unscaled), Some((1280, 800)));
    }

    #[test]
    fn physical_resolution_rejects_empty_sizes() {
        assert_eq!(physical_resolution(0.0, 720.0, 1.0), None);
        assert_eq!(physical_resolution(1280.0, 0.0, 2.0), None);
        assert_eq!(physical_resolution(1280.0, 720.0, 0.0), None);
    }

    #[test]
    fn preferred_resolution_picks_the_largest_landscape_monitor() {
        assert_eq!(
            preferred_resolution([(1080, 1920), (1280, 800), (2560, 1440), (0, 0)]),
            Some((2560, 1440))
        );
        assert_eq!(preferred_resolution([(1080, 1920)]), Some((1080, 1920)));
        assert_eq!(preferred_resolution([]), None);
    }

    #[test]
    fn display_server_reports_a_monitor_and_keeps_the_answer() {
        // CI runs the tests under Xvfb, so a display server is present.
        let first = resolution_from_display_server();
        assert!(first.is_some_and(|(width, height)| width > 0 && height > 0));

        assert_eq!(resolution_from_display_server(), first);
        // A second event loop cannot be created in the same process.
        assert_eq!(probe_display_server(), None);
    }
}
