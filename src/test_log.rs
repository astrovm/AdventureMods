//! Log capture for tests that assert on the diagnostics users see.

use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Records every log line emitted on the current thread until dropped.
///
/// Unlike [`capture_logs`], the capture stays active across GTK main-loop
/// iterations, so logs from local futures and signal handlers are recorded.
pub(crate) struct LogCapture {
    buffer: Buffer,
    _guard: tracing::subscriber::DefaultGuard,
}

impl LogCapture {
    pub(crate) fn start() -> Self {
        let buffer = Buffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        Self {
            buffer,
            _guard: tracing::subscriber::set_default(subscriber),
        }
    }

    pub(crate) fn contents(&self) -> String {
        String::from_utf8(self.buffer.0.lock().unwrap().clone()).unwrap()
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
        assert!(logs.contains("TRACE"));
        assert!(logs.contains("trace detail"));
        assert!(logs.contains("WARN"));
        assert!(logs.contains("warning detail"));
    }

    #[test]
    fn log_capture_stops_recording_when_dropped() {
        let capture = LogCapture::start();
        tracing::info!("while capturing");
        let logs = capture.contents();
        drop(capture);
        tracing::info!("after capture");

        assert!(logs.contains("while capturing"));
        assert!(!logs.contains("after capture"));
    }
}
