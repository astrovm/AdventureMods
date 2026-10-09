//! Log capture for tests that assert on the diagnostics users see.
//!
//! winit's Wayland stack turns on tracing's `log` feature, so every `tracing`
//! macro also expands a fallback that forwards to the `log` crate while no
//! tracing subscriber exists. Coverage credits the macro arguments to that
//! fallback, so tests never install a tracing subscriber: a `log` logger,
//! enabled for every level, records the events instead.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<String>>);

thread_local! {
    /// Captures active on this thread; the innermost one records.
    static CAPTURES: RefCell<Vec<Buffer>> = const { RefCell::new(Vec::new()) };
}

struct TestLogger;

impl log::Log for TestLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        CAPTURES.with_borrow(|captures| {
            if let Some(buffer) = captures.last() {
                let line = format!("{} {}\n", record.level(), record.args());
                buffer.0.lock().unwrap().push_str(&line);
            }
        });
    }

    fn flush(&self) {}
}

/// Every test logs at every level, so log arguments are always evaluated.
/// Output is dropped unless a [`LogCapture`] is active on the thread.
#[ctor::ctor(unsafe)]
fn log_everything() {
    static LOGGER: TestLogger = TestLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}

/// Records every log line emitted on the current thread until dropped.
pub(crate) struct LogCapture {
    buffer: Buffer,
}

impl LogCapture {
    pub(crate) fn start() -> Self {
        let buffer = Buffer::default();
        CAPTURES.with_borrow_mut(|captures| captures.push(buffer.clone()));
        Self { buffer }
    }

    pub(crate) fn contents(&self) -> String {
        self.buffer.0.lock().unwrap().clone()
    }
}

impl Drop for LogCapture {
    fn drop(&mut self) {
        CAPTURES.with_borrow_mut(|captures| {
            captures.retain(|buffer| !Arc::ptr_eq(&buffer.0, &self.buffer.0));
        });
    }
}

/// Run `test` while recording its logs, returning its result and the logs.
pub(crate) fn capture_logs<T>(test: impl FnOnce() -> T) -> (T, String) {
    let capture = LogCapture::start();
    let result = test();
    (result, capture.contents())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_logs_records_messages_at_every_level() {
        let (value, logs) = capture_logs(|| {
            tracing::trace!("trace detail");
            tracing::warn!("warning detail");
            7
        });

        assert_eq!(value, 7);
        assert!(logs.contains("TRACE trace detail"));
        assert!(logs.contains("WARN warning detail"));
    }

    #[test]
    fn log_capture_stops_recording_when_dropped() {
        let capture = LogCapture::start();
        tracing::info!("while capturing");
        let logs = capture.contents();
        drop(capture);
        tracing::info!("after capture");
        log::logger().flush();

        assert!(logs.contains("while capturing"));
        assert!(!logs.contains("after capture"));
    }

    #[test]
    fn the_innermost_capture_records() {
        let outer = LogCapture::start();
        let inner = LogCapture::start();
        tracing::info!("inner line");
        drop(inner);
        tracing::info!("outer line");

        assert_eq!(outer.contents(), "INFO outer line\n");
    }

    #[test]
    fn logs_from_other_threads_are_not_recorded() {
        let (_, logs) = capture_logs(|| {
            std::thread::spawn(|| tracing::info!("elsewhere"))
                .join()
                .unwrap();
        });

        assert!(logs.is_empty());
    }

    #[test]
    fn multi_line_log_arguments_are_evaluated() {
        let mut evaluated = 0;
        tracing::info!("evaluated {}", {
            evaluated += 1;
            evaluated
        });

        assert_eq!(evaluated, 1);
        assert!(!tracing::dispatcher::has_been_set());
    }
}
