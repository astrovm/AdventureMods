//! Process environment is global, so every test that sets environment
//! variables must hold this one lock rather than a per-module one.

use std::ffi::OsStr;
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serialize environment changes across the whole test binary. A panicking
/// test must not fail unrelated tests, so poisoning is ignored.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Set `name` to `value`, or remove it for `None`, e.g. to restore a value
/// saved with `std::env::var_os`. Hold [`lock`] while calling this.
pub(crate) fn set_var(name: &str, value: Option<impl AsRef<OsStr>>) {
    match value {
        Some(value) => unsafe { std::env::set_var(name, value) },
        None => unsafe { std::env::remove_var(name) },
    }
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

    #[test]
    fn set_var_sets_or_removes_a_variable() {
        let _guard = lock();
        let name = "ADVENTURE_MODS_TEST_ENV_SET_VAR";

        set_var(name, Some("value"));
        assert_eq!(std::env::var(name).as_deref(), Ok("value"));

        set_var(name, None::<&str>);
        assert!(std::env::var_os(name).is_none());
    }
}
