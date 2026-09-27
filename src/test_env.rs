//! Process environment is global, so every test that sets environment
//! variables must hold this one lock rather than a per-module one.

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serialize environment changes across the whole test binary. A panicking
/// test must not fail unrelated tests, so poisoning is ignored.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
