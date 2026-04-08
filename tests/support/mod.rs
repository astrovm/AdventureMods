pub mod http_server;
pub mod scripts;
pub mod steam_fixture;

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap()
}

pub struct EnvGuard {
    vars: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    pub fn set(pairs: &[(&'static str, String)]) -> Self {
        let mut vars = Vec::with_capacity(pairs.len());
        for (key, value) in pairs {
            vars.push((*key, std::env::var(key).ok()));
            unsafe {
                std::env::set_var(key, value);
            }
        }
        Self { vars }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, previous) in self.vars.drain(..).rev() {
            match previous {
                Some(value) => unsafe {
                    std::env::set_var(key, value);
                },
                None => unsafe {
                    std::env::remove_var(key);
                },
            }
        }
    }
}
