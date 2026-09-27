#![allow(dead_code)]

pub mod http_server;
pub mod scripts;
pub mod steam_fixture;

use std::sync::{Mutex, MutexGuard};

use adventure_mods::setup::common::{ModEntry, ModSource};

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct EnvGuard {
    vars: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    /// Set `pairs` until the guard drops. Downloads are always cached in a
    /// temporary directory so tests never write to the user's real cache.
    pub fn set(pairs: &[(&'static str, String)]) -> Self {
        let cache_dir = (
            "ADVENTURE_MODS_CACHE_DIR",
            std::env::temp_dir()
                .join("adventure-mods-tests")
                .display()
                .to_string(),
        );
        let mut vars = Vec::with_capacity(pairs.len() + 1);
        for (key, value) in std::iter::once(&cache_dir).chain(pairs) {
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

pub fn leak_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

pub const TEST_FLAT: ModEntry = ModEntry {
    name: "Test Flat",
    slug: "test-flat",
    source: ModSource::GameBananaItem {
        item_type: "Mod",
        item_id: 2,
    },
    description: "test mod",
    full_description: None,
    pictures: &[],
    dir_name: Some("Test Flat"),
    links: &[],
};
