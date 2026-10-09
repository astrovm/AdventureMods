//! App preferences, kept as JSON in the user's config directory.
//!
//! Earlier versions stored these in GSettings. The first load without a JSON
//! file imports them, so granted Steam libraries and language choices survive
//! the move.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const WINDOW_WIDTH_KEY: &str = "window-width";
pub const WINDOW_HEIGHT_KEY: &str = "window-height";
pub const WINDOW_MAXIMIZED_KEY: &str = "window-maximized";
pub const EXTRA_LIBRARY_PATHS_KEY: &str = "extra-library-paths";

const SETTINGS_FILE: &str = "settings.json";

/// One stored preference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    String(String),
    Strings(Vec<String>),
}

/// Preferences backed by `settings.json`. Every change is written right away.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    path: PathBuf,
    values: BTreeMap<String, Value>,
}

impl Settings {
    /// The app's settings, importing GSettings from older versions once.
    pub fn load() -> Option<Self> {
        let dir = config_dir()?;
        Some(Self::load_from(&dir, legacy_gsettings))
    }

    /// Settings stored in `dir`, or the ones `legacy` finds when none are.
    pub fn load_from(dir: &Path, legacy: impl FnOnce() -> Option<String>) -> Self {
        let path = dir.join(SETTINGS_FILE);
        let values = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
                tracing::warn!("Ignoring unreadable settings {}: {err}", path.display());
                BTreeMap::new()
            }),
            Err(_) => {
                let imported = legacy().map(|text| parse_gsettings_dump(&text));
                let settings = Self {
                    path,
                    values: imported.unwrap_or_default(),
                };
                if !settings.values.is_empty() {
                    tracing::info!("Imported settings from GSettings");
                    settings.save();
                }
                return settings;
            }
        };
        Self { path, values }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        match self.values.get(key) {
            Some(Value::String(value)) => Some(value),
            _ => None,
        }
    }

    pub fn strings(&self, key: &str) -> Vec<String> {
        match self.values.get(key) {
            Some(Value::Strings(values)) => values.clone(),
            _ => Vec::new(),
        }
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        match self.values.get(key) {
            Some(Value::Int(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn boolean(&self, key: &str) -> Option<bool> {
        match self.values.get(key) {
            Some(Value::Bool(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        if self.values.get(key) == Some(&value) {
            return;
        }
        self.values.insert(key.to_owned(), value);
        self.save();
    }

    fn save(&self) {
        if let Err(err) = write_json(&self.path, &self.values) {
            tracing::warn!(
                "Could not save settings to {}: {err:#}",
                self.path.display()
            );
        }
    }
}

fn write_json(path: &Path, values: &BTreeMap<String, Value>) -> anyhow::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(values)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// `$XDG_CONFIG_HOME/<app id>`. Flatpak points that into the app's sandbox.
fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(crate::config::APP_ID))
}

/// The GSettings path older versions used, such as `/io/github/astrovm/AdventureMods/`.
fn gsettings_path() -> String {
    format!("/{}/", crate::config::APP_ID.replace('.', "/"))
}

/// Older settings, as `[section]` and `key=GVariant` lines: the keyfile
/// backend Flatpak uses, or `dconf dump` outside it.
fn legacy_gsettings() -> Option<String> {
    let keyfile = dirs::config_dir()?.join("glib-2.0/settings/keyfile");
    legacy_gsettings_from(&keyfile, &gsettings_path(), "dconf")
}

fn legacy_gsettings_from(keyfile: &Path, path: &str, dconf: &str) -> Option<String> {
    if let Ok(text) = std::fs::read_to_string(keyfile) {
        let section = format!("[{}]", path.trim_matches('/'));
        return Some(keyfile_section(&text, &section));
    }
    let output = std::process::Command::new(dconf)
        .args(["dump", path])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The lines of `section` in an INI-style `text`.
fn keyfile_section(text: &str, section: &str) -> String {
    let mut inside = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == section;
        } else if inside {
            lines.push(trimmed);
        }
    }
    lines.join("\n")
}

/// Read `key=value` lines whose values are GVariant text.
fn parse_gsettings_dump(text: &str) -> BTreeMap<String, Value> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter_map(|(key, value)| Some((key.trim().to_owned(), parse_gvariant(value.trim())?)))
        .collect()
}

fn parse_gvariant(text: &str) -> Option<Value> {
    match text {
        "true" => return Some(Value::Bool(true)),
        "false" => return Some(Value::Bool(false)),
        _ => {}
    }
    if let Ok(int) = text.parse() {
        return Some(Value::Int(int));
    }
    let text = text.strip_prefix("@as ").unwrap_or(text);
    if let Some(inner) = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        let mut rest = inner.trim();
        let mut items = Vec::new();
        while !rest.is_empty() {
            let (item, tail) = parse_gvariant_string(rest)?;
            items.push(item);
            rest = tail.trim_start().trim_start_matches(',').trim_start();
        }
        return Some(Value::Strings(items));
    }
    let (string, tail) = parse_gvariant_string(text)?;
    tail.is_empty().then_some(Value::String(string))
}

/// One quoted GVariant string at the start of `text`, and what follows it.
fn parse_gvariant_string(text: &str) -> Option<(String, &str)> {
    let mut chars = text.char_indices();
    let (_, quote) = chars.next().filter(|(_, c)| *c == '\'' || *c == '"')?;
    let mut value = String::new();
    while let Some((index, c)) = chars.next() {
        match c {
            '\\' => value.push(chars.next()?.1),
            c if c == quote => return Some((value, &text[index + 1..])),
            c => value.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_legacy() -> Option<String> {
        None
    }

    #[test]
    fn values_round_trip_through_the_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::load_from(dir.path(), no_legacy);
        assert_eq!(settings.path(), dir.path().join("settings.json"));
        assert!(!settings.path().exists(), "nothing to save yet");

        settings.set("name", Value::String("english".into()));
        settings.set("list", Value::Strings(vec!["/a".into(), "/b".into()]));
        settings.set("width", Value::Int(1280));
        settings.set("maximized", Value::Bool(true));

        let reloaded = Settings::load_from(dir.path(), || panic!("not a first run"));
        assert_eq!(reloaded, settings);
        assert_eq!(reloaded.string("name"), Some("english"));
        assert_eq!(reloaded.strings("list"), vec!["/a", "/b"]);
        assert_eq!(reloaded.int("width"), Some(1280));
        assert_eq!(reloaded.boolean("maximized"), Some(true));
    }

    #[test]
    fn missing_or_mistyped_values_read_as_unset() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::load_from(dir.path(), no_legacy);
        settings.set("key", Value::Int(1));

        assert_eq!(settings.string("key"), None);
        assert!(settings.strings("key").is_empty());
        assert_eq!(settings.boolean("key"), None);
        settings.set("key", Value::Bool(false));
        assert_eq!(settings.int("key"), None);
        assert_eq!(settings.string("missing"), None);
    }

    #[test]
    fn unchanged_values_are_not_written_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::load_from(dir.path(), no_legacy);
        settings.set("key", Value::Int(1));
        std::fs::remove_file(settings.path()).unwrap();

        settings.set("key", Value::Int(1));

        assert!(!settings.path().exists());
    }

    #[test]
    fn unreadable_settings_files_start_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "{not json").unwrap();

        let (settings, logs) = crate::test_log::capture_logs(|| {
            Settings::load_from(dir.path(), || panic!("the file exists"))
        });

        assert_eq!(settings.string(EXTRA_LIBRARY_PATHS_KEY), None);
        assert!(logs.contains("Ignoring unreadable settings"), "{logs}");
    }

    #[test]
    fn failed_saves_are_logged() {
        let dir = tempfile::tempdir().unwrap();
        // A file where the settings directory should be.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "").unwrap();
        let mut settings = Settings::load_from(&blocker.join("app"), no_legacy);

        let ((), logs) = crate::test_log::capture_logs(|| settings.set("key", Value::Bool(true)));

        assert!(logs.contains("Could not save settings"), "{logs}");
    }

    #[test]
    fn first_load_imports_gsettings_and_saves_them() {
        let dir = tempfile::tempdir().unwrap();
        let dump = "window-width=1111\nwindow-maximized=true\n\
                    extra-library-paths=['/run/user/1000/doc/a/Steam Library', \"/mnt/it's\"]\n\
                    sadx-voice-language='english'\nbroken=[\n";

        let settings = Settings::load_from(dir.path(), || Some(dump.to_owned()));

        assert_eq!(settings.int(WINDOW_WIDTH_KEY), Some(1111));
        assert_eq!(settings.boolean(WINDOW_MAXIMIZED_KEY), Some(true));
        assert_eq!(
            settings.strings(EXTRA_LIBRARY_PATHS_KEY),
            vec!["/run/user/1000/doc/a/Steam Library", "/mnt/it's"]
        );
        assert_eq!(settings.string("sadx-voice-language"), Some("english"));
        assert!(settings.path().exists());

        let empty = tempfile::tempdir().unwrap();
        let nothing = Settings::load_from(empty.path(), || Some(String::new()));
        assert!(!nothing.path().exists());
    }

    #[test]
    fn gvariant_values_cover_every_stored_type() {
        assert_eq!(parse_gvariant("false"), Some(Value::Bool(false)));
        assert_eq!(parse_gvariant("-3"), Some(Value::Int(-3)));
        assert_eq!(parse_gvariant("@as []"), Some(Value::Strings(Vec::new())));
        assert_eq!(
            parse_gvariant(r"'a\'b'"),
            Some(Value::String("a'b".to_owned()))
        );
        assert_eq!(parse_gvariant("'trailing' junk"), None);
        assert_eq!(parse_gvariant("'unterminated"), None);
        assert_eq!(parse_gvariant(r"'escape at end\"), None);
        assert_eq!(parse_gvariant("['a' 'b'"), None);
        assert_eq!(parse_gvariant("[oops]"), None);
        assert_eq!(parse_gvariant("bare"), None);
    }

    #[test]
    fn legacy_settings_come_from_the_keyfile_or_dconf() {
        let dir = tempfile::tempdir().unwrap();
        let keyfile = dir.path().join("keyfile");
        std::fs::write(
            &keyfile,
            "[org/other]\nwindow-width=1\n\n[io/github/astrovm/AdventureMods]\nwindow-width=2\n",
        )
        .unwrap();

        assert_eq!(
            legacy_gsettings_from(&keyfile, "/io/github/astrovm/AdventureMods/", "false")
                .as_deref(),
            Some("window-width=2")
        );

        // Without a keyfile, `dconf dump` prints the same format.
        let missing = dir.path().join("missing");
        let echo = legacy_gsettings_from(&missing, "/x/", "echo").unwrap();
        assert_eq!(echo, "dump /x/\n");
        assert_eq!(legacy_gsettings_from(&missing, "/x/", "false"), None);
        assert_eq!(
            legacy_gsettings_from(&missing, "/x/", "/nonexistent/dconf"),
            None
        );
    }

    #[test]
    fn app_settings_live_in_the_config_directory() {
        let _env = crate::test_env::lock();
        let home = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", home.path()) };

        let settings = Settings::load().unwrap();
        let path = gsettings_path();
        // No keyfile in this home; whatever dconf has is fine to read.
        let _ = legacy_gsettings();

        crate::test_env::set_var("XDG_CONFIG_HOME", previous);
        assert_eq!(
            settings.path(),
            home.path()
                .join(crate::config::APP_ID)
                .join("settings.json")
        );
        assert_eq!(path, "/io/github/astrovm/AdventureMods/");
    }
}
