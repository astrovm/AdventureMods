//! The app's text in the user's language.
//!
//! Wrap user-facing text in [`tr!`](crate::tr), [`trc!`](crate::trc),
//! [`trf!`](crate::trf) or [`trn!`](crate::trn). Text in `const` data, such as
//! the mod catalogs, is marked with [`tr_noop!`](crate::tr_noop) and passed
//! through [`translate`] where it is shown.
//!
//! The translations are the gettext catalogs in `po/`, built into the binary,
//! and the language follows the locale the way gettext picks it. Logs and the
//! technical details of errors stay in English, so they can be searched for.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;

/// A language with a catalog in `po/`.
pub(crate) struct Language {
    pub(crate) code: &'static str,
    source: &'static str,
    /// The plural form for a count, as the `Plural-Forms` of the catalog says.
    plural: fn(u64) -> usize,
    catalog: OnceLock<Catalog>,
}

impl Language {
    fn catalog(&self) -> &Catalog {
        self.catalog.get_or_init(|| Catalog::parse(self.source))
    }
}

fn one_or_other(n: u64) -> usize {
    usize::from(n != 1)
}

fn zero_or_one_or_other(n: u64) -> usize {
    usize::from(n > 1)
}

fn single_form(_: u64) -> usize {
    0
}

pub(crate) static LANGUAGES: [Language; 5] = [
    Language {
        code: "de",
        source: include_str!("../po/de.po"),
        plural: one_or_other,
        catalog: OnceLock::new(),
    },
    Language {
        code: "es",
        source: include_str!("../po/es.po"),
        plural: one_or_other,
        catalog: OnceLock::new(),
    },
    Language {
        code: "fr",
        source: include_str!("../po/fr.po"),
        plural: zero_or_one_or_other,
        catalog: OnceLock::new(),
    },
    Language {
        code: "it",
        source: include_str!("../po/it.po"),
        plural: one_or_other,
        catalog: OnceLock::new(),
    },
    Language {
        code: "ja",
        source: include_str!("../po/ja.po"),
        plural: single_form,
        catalog: OnceLock::new(),
    },
];

/// The language in use, or `None` for English.
fn active() -> Option<&'static Language> {
    #[cfg(test)]
    {
        // Tests read English unless they ask for a language, whatever the
        // locale of the machine running them.
        ACTIVE_IN_TEST.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        static ACTIVE: OnceLock<Option<&'static Language>> = OnceLock::new();
        *ACTIVE.get_or_init(|| language_from(|name| std::env::var(name).ok()))
    }
}

/// The language gettext would pick from the environment `var` reads: the
/// first one `LANGUAGE` lists that has a catalog, then the locale's. In the
/// C locale, as in no locale at all, everything stays in English.
fn language_from(var: impl Fn(&str) -> Option<String>) -> Option<&'static Language> {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| var(name).filter(|value| !value.is_empty()))?;
    if is_c_locale(&locale) {
        return None;
    }
    let preferences = var("LANGUAGE").unwrap_or_default();
    preferences
        .split(':')
        .chain([locale.as_str()])
        .map(language_code)
        .find_map(|code| {
            // English is built in, so a preference for it wins too.
            if code == "en" {
                Some(None)
            } else {
                LANGUAGES
                    .iter()
                    .find(|language| language.code == code)
                    .map(Some)
            }
        })
        .flatten()
}

fn is_c_locale(locale: &str) -> bool {
    locale == "POSIX" || locale == "C" || locale.starts_with("C.")
}

/// The language part of a locale like `es_AR.UTF-8@euro`.
fn language_code(locale: &str) -> &str {
    locale
        .split(['_', '-', '.', '@'])
        .next()
        .unwrap_or_default()
}

/// The translation of `msgid`, or `msgid` itself when there is none.
pub fn translate(msgid: &'static str) -> &'static str {
    translate_in("", msgid)
}

/// The translation of `msgid` where `context` tells it apart from the same
/// English text meaning something else.
pub fn translate_in(context: &str, msgid: &'static str) -> &'static str {
    active()
        .and_then(|language| language.catalog().get(context, msgid))
        .map_or(msgid, |forms| &forms[0])
}

/// The translation of `singular` or `plural` that fits a count of `n`.
pub fn translate_plural(singular: &'static str, plural: &'static str, n: u64) -> &'static str {
    let english = if n == 1 { singular } else { plural };
    let Some(language) = active() else {
        return english;
    };
    language
        .catalog()
        .get("", singular)
        .and_then(|forms| forms.get((language.plural)(n)))
        .map_or(english, String::as_str)
}

/// `n` as a count for [`translate_plural`]; counts past `u64` are just many.
#[doc(hidden)]
pub fn count(n: impl TryInto<u64>) -> u64 {
    n.try_into().unwrap_or(u64::MAX)
}

/// Replaces each `{name}` in `template` with its value, and `{{` and `}}`
/// with single braces, as `format!` does. Values are never read as
/// templates, and unknown or broken placeholders stay as they are.
#[doc(hidden)]
pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut text = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find(['{', '}']) {
        text.push_str(&rest[..start]);
        let tail = &rest[start..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            text.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        let value = tail
            .strip_prefix('{')
            .and_then(|after| after.split_once('}'))
            .and_then(|(name, after)| {
                let (_, value) = args.iter().find(|(key, _)| *key == name)?;
                Some((value, after))
            });
        match value {
            Some((value, after)) => {
                text.push_str(&value.to_string());
                rest = after;
            }
            None => {
                text.push_str(&tail[..1]);
                rest = &tail[1..];
            }
        }
    }
    text.push_str(rest);
    text
}

/// Translated text, by context ("" for none) and then by English text. Each
/// has its plural forms, or just the one form.
#[derive(Debug, Default)]
struct Catalog {
    messages: HashMap<String, HashMap<String, Vec<String>>>,
}

impl Catalog {
    /// Reads a `.po` file. Untranslated, fuzzy and obsolete messages are left
    /// out, so their English text shows.
    fn parse(source: &str) -> Self {
        let mut catalog = Self::default();
        for entry in entries(source) {
            if entry.fuzzy
                || entry.msgid.is_empty()
                || entry.msgstr.is_empty()
                || entry.msgstr.iter().any(String::is_empty)
            {
                continue;
            }
            catalog
                .messages
                .entry(entry.msgctxt.unwrap_or_default())
                .or_default()
                .insert(entry.msgid, entry.msgstr);
        }
        catalog
    }

    fn get(&self, context: &str, msgid: &str) -> Option<&[String]> {
        self.messages.get(context)?.get(msgid).map(Vec::as_slice)
    }
}

/// One message of a `.po` file.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Entry {
    pub(crate) msgctxt: Option<String>,
    pub(crate) msgid: String,
    pub(crate) msgid_plural: Option<String>,
    pub(crate) msgstr: Vec<String>,
    pub(crate) fuzzy: bool,
}

#[derive(Clone, Copy)]
enum Field {
    Context,
    Id,
    Plural,
    Str(usize),
}

/// The messages of a `.po` file, the header included as the message with an
/// empty `msgid`. Obsolete messages (`#~`) are skipped.
pub(crate) fn entries(source: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut entry = Entry::default();
    let mut started = false;
    let mut field = None;
    for line in source.lines().map(str::trim) {
        let keyword = line.split_once(' ').map(|(keyword, _)| keyword);
        let new_entry = line.is_empty()
            || line.starts_with('#')
            || matches!(keyword, Some("msgctxt" | "msgid"));
        if new_entry && started {
            entries.push(std::mem::take(&mut entry));
            started = false;
            field = None;
        }
        if line.starts_with("#,") {
            entry.fuzzy |= line.split([',', ' ']).any(|flag| flag == "fuzzy");
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (target, text) = match line.split_once(' ') {
            _ if line.starts_with('"') => (field, line),
            Some(("msgctxt", text)) => (Some(Field::Context), text),
            Some(("msgid", text)) => (Some(Field::Id), text),
            Some(("msgid_plural", text)) => (Some(Field::Plural), text),
            Some((keyword, text)) => (msgstr_index(keyword).map(Field::Str), text),
            None => (None, ""),
        };
        let Some(target) = target else {
            continue;
        };
        field = Some(target);
        started |= matches!(target, Field::Id);
        let text = unquote(text);
        match target {
            Field::Context => entry.msgctxt.get_or_insert_default().push_str(&text),
            Field::Id => entry.msgid.push_str(&text),
            Field::Plural => entry.msgid_plural.get_or_insert_default().push_str(&text),
            Field::Str(index) => {
                if entry.msgstr.len() <= index {
                    entry.msgstr.resize(index + 1, String::new());
                }
                entry.msgstr[index].push_str(&text);
            }
        }
    }
    if started {
        entries.push(entry);
    }
    entries
}

/// The form index of `msgstr` (0) or `msgstr[N]`.
fn msgstr_index(keyword: &str) -> Option<usize> {
    if keyword == "msgstr" {
        return Some(0);
    }
    keyword
        .strip_prefix("msgstr[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// The text of a quoted `.po` string, its escapes undone.
fn unquote(quoted: &str) -> String {
    let inner = quoted.strip_prefix('"').unwrap_or(quoted);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    let mut text = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            text.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => text.push('\n'),
            Some('t') => text.push('\t'),
            Some(other) => text.push(other),
            None => text.push('\\'),
        }
    }
    text
}

/// Text in the user's language. `tr!("Set Up")` is a `&'static str`.
#[macro_export]
macro_rules! tr {
    ($msgid:literal) => {
        $crate::i18n::translate($msgid)
    };
}

/// [`tr!`] for English text that means more than one thing, told apart by a
/// context: `trc!("install location", "Install")`.
#[macro_export]
macro_rules! trc {
    ($context:literal, $msgid:literal) => {
        $crate::i18n::translate_in($context, $msgid)
    };
}

/// Text with values in it, as a `String`:
/// `trf!("Restore the original {game}?", game = kind.name())`.
/// Placeholders without an argument, and arguments without a placeholder,
/// fail to compile.
#[macro_export]
macro_rules! trf {
    ($msgid:literal, $($name:ident = $value:expr),+ $(,)?) => {
        match ($(&$value,)+) {
            ($($name,)+) => {
                let _ = ::std::format_args!($msgid, $($name = $name),+);
                $crate::i18n::fill(
                    $crate::i18n::translate($msgid),
                    &[$((::std::stringify!($name), $name as &dyn ::std::fmt::Display)),+],
                )
            }
        }
    };
}

/// Text with a count, as a `String`, in the plural form the count needs:
/// `trn!("{n} mod", "{n} mods", count)`. Both forms show the count as `{n}`,
/// and more values can follow as in [`trf!`].
#[macro_export]
macro_rules! trn {
    ($singular:literal, $plural:literal, $n:expr $(, $name:ident = $value:expr)* $(,)?) => {
        match (&$n, $(&$value,)*) {
            (n, $($name,)*) => {
                let _ = ::std::format_args!($singular, n = n $(, $name = $name)*);
                let _ = ::std::format_args!($plural, n = n $(, $name = $name)*);
                $crate::i18n::fill(
                    $crate::i18n::translate_plural(
                        $singular,
                        $plural,
                        $crate::i18n::count(*n),
                    ),
                    &[
                        ("n", n as &dyn ::std::fmt::Display)
                        $(, (::std::stringify!($name), $name as &dyn ::std::fmt::Display))*
                    ],
                )
            }
        }
    };
}

/// Marks text in `const` data for translation without translating it yet.
/// Pass it through [`translate`] where it is shown.
#[macro_export]
macro_rules! tr_noop {
    ($msgid:literal) => {
        $msgid
    };
}

#[cfg(test)]
thread_local! {
    static ACTIVE_IN_TEST: std::cell::Cell<Option<&'static Language>> =
        const { std::cell::Cell::new(None) };
}

/// Show this thread's text in the language with `code` until the guard
/// drops, then in English again.
#[cfg(test)]
pub(crate) fn use_language(code: &str) -> impl Drop {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE_IN_TEST.with(|active| active.set(None));
        }
    }
    let language = LANGUAGES
        .iter()
        .find(|language| language.code == code)
        .unwrap_or_else(|| panic!("no catalog for {code}"));
    ACTIVE_IN_TEST.with(|active| active.set(Some(language)));
    Restore
}

/// Every translation of every catalog, with its language's code.
#[cfg(test)]
pub(crate) fn all_translations() -> impl Iterator<Item = (&'static str, &'static str)> {
    LANGUAGES.iter().flat_map(|language| {
        language
            .catalog()
            .messages
            .values()
            .flat_map(HashMap::values)
            .flatten()
            .map(move |text| (language.code, text.as_str()))
    })
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
