//! Process environment is global, so every test that sets environment
//! variables must hold this one lock rather than a per-module one.

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serialize environment changes across the whole test binary. A panicking
/// test must not fail unrelated tests, so poisoning is ignored.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_still_granted_after_a_holder_panics() {
        let holder = std::thread::spawn(|| {
            let _guard = lock();
            panic!("test failed while changing the environment");
        });
        assert!(holder.join().is_err());
        assert!(ENV_LOCK.is_poisoned());

        let _guard = lock();
    }
}
