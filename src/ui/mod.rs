pub mod game_card;
pub mod setup_page;
pub mod welcome_page;

#[cfg(test)]
pub mod test_util;

pub const WIZARD_DEFAULT_WIDTH: i32 = 872;
pub const WIZARD_DEFAULT_HEIGHT: i32 = 666;

/// Opens a URI; replaced in tests so links are checked, never launched.
pub(crate) type UriOpener = std::rc::Rc<dyn Fn(Option<&gtk::Window>, &str)>;

/// Where a page opens links: the system handler unless a test swapped it.
#[derive(Default)]
pub(crate) struct UriOpenerSlot(std::cell::RefCell<Option<UriOpener>>);

impl std::fmt::Debug for UriOpenerSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UriOpenerSlot")
    }
}

impl UriOpenerSlot {
    pub(crate) fn open(&self, window: Option<&gtk::Window>, uri: &str) {
        let opener = self.0.borrow().clone();
        match opener {
            Some(open) => open(window, uri),
            None => launch_uri(window, uri),
        }
    }

    #[cfg(test)]
    pub(crate) fn replace(&self, opener: UriOpener) {
        self.0.replace(Some(opener));
    }
}

/// Open `uri` with the default handler, such as Steam for `steam://` links.
pub(crate) fn launch_uri(window: Option<&gtk::Window>, uri: &str) {
    let target = uri.to_owned();
    gtk::UriLauncher::new(uri).launch(window, gtk::gio::Cancellable::NONE, move |result| {
        report_launch_failure(&target, result);
    });
}

/// Log why `target` could not be opened; there is no one else to tell.
fn report_launch_failure(target: &str, result: Result<(), gtk::glib::Error>) {
    if let Err(err) = result {
        tracing::warn!("Could not open {target}: {err}");
    }
}

pub(crate) fn catch_ui_panic(label: &'static str, action: impl FnOnce()) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)).map_err(|payload| {
        let message = panic_message(payload.as_ref());
        tracing::error!("UI callback panicked in {label}: {message}");
        message
    })
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn catch_ui_panic_reports_panics_without_unwinding() {
        let result = super::catch_ui_panic("test callback", || panic!("boom"));

        assert_eq!(result, Err("boom".to_string()));
    }

    #[test]
    fn catch_ui_panic_returns_ok_for_successful_callbacks() {
        assert_eq!(super::catch_ui_panic("test callback", || {}), Ok(()));
    }

    #[test]
    fn catch_ui_panic_reports_owned_and_opaque_payloads() {
        let (owned, logs) = crate::test_log::capture_logs(|| {
            super::catch_ui_panic("owned callback", || panic!("{} failed", "load"))
        });
        assert_eq!(owned, Err("load failed".to_string()));
        assert!(logs.contains("UI callback panicked in owned callback: load failed"));

        let opaque = super::catch_ui_panic("opaque callback", || std::panic::panic_any(7));
        assert_eq!(opaque, Err("unknown panic payload".to_string()));
    }

    #[test]
    fn uri_opener_slot_debug_names_the_slot() {
        assert_eq!(
            format!("{:?}", super::UriOpenerSlot::default()),
            "UriOpenerSlot"
        );
    }

    #[test]
    fn only_failed_launches_are_logged() {
        let ((), logs) = crate::test_log::capture_logs(|| {
            super::report_launch_failure("steam://validate/71250", Ok(()));
        });
        assert_eq!(logs, "");

        let ((), logs) = crate::test_log::capture_logs(|| {
            super::report_launch_failure(
                "steam://validate/71250",
                Err(gtk::glib::Error::new(
                    gtk::gio::IOErrorEnum::NotFound,
                    "no handler",
                )),
            );
        });
        assert!(logs.contains("Could not open steam://validate/71250: no handler"));
    }

    #[gtk::test]
    fn uri_opener_slot_falls_back_to_the_system_launcher_and_logs_failures() {
        let capture = crate::test_log::LogCapture::start();
        // No handler is registered for this scheme, so nothing is launched.
        let uri = "adventure-mods-test-unhandled://nowhere";

        super::UriOpenerSlot::default().open(None, uri);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !capture.contents().contains("Could not open") {
            assert!(
                std::time::Instant::now() < deadline,
                "launch failure was not reported"
            );
            gtk::glib::MainContext::default().iteration(false);
        }
        assert!(
            capture
                .contents()
                .contains(&format!("Could not open {uri}: "))
        );
    }
}
