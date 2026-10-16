//! Steam's libraryfolders.vdf and config.vdf come from outside the app, so
//! the parser must handle any text without crashing.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let _ = adventure_mods::steam::vdf::parse(text);
});
