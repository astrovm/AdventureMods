//! Translated text. Wrap every user-facing string in [`tr`], [`trn`] or [`tr_fmt`].

use std::fmt::Display;

use gettextrs::{LocaleCategory, bindtextdomain, gettext, ngettext, setlocale, textdomain};

use crate::config;

pub const TEXT_DOMAIN: &str = "adventure-mods";

/// Picks the language from the environment. Call once, before showing text.
/// Without a catalog, the English text is used.
pub fn init() {
    // SAFETY: called once at startup, before any other thread is spawned.
    unsafe { setlocale(LocaleCategory::LcAll, "") };
    let _ = bindtextdomain(TEXT_DOMAIN, config::LOCALEDIR);
    let _ = textdomain(TEXT_DOMAIN);
}

/// Returns the translation of `msgid`, or `msgid` itself when there is none.
pub fn tr(msgid: &str) -> String {
    gettext(msgid)
}

/// Marks a static string for extraction. Returns it unchanged; call [`tr`] where it is shown.
pub fn mark(msgid: &'static str) -> &'static str {
    msgid
}

/// Picks the singular or plural translation for `n`.
pub fn trn(singular: &str, plural: &str, n: u64) -> String {
    ngettext(singular, plural, u32::try_from(n).unwrap_or(u32::MAX))
}

/// Translates `msgid`, then replaces each `{name}` with its value.
/// Translators keep the names; the values are never translated.
pub fn tr_fmt(msgid: &str, args: &[(&str, &dyn Display)]) -> String {
    fill(&tr(msgid), args)
}

/// Replaces each `{name}` in `template` with its value in one pass, so a
/// value that contains braces is never expanded again. Unknown names stay.
fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut text = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        text.push_str(&rest[..start]);
        match args.iter().find(|(key, _)| *key == &after[..end]) {
            Some((_, value)) => text.push_str(&value.to_string()),
            None => text.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    text.push_str(rest);
    text
}

#[cfg(test)]
mod tests {
    use super::fill;

    #[test]
    fn fill_replaces_named_placeholders_and_keeps_other_braces() {
        assert_eq!(
            fill(
                "{count} mods in {path} {}",
                &[("count", &3), ("path", &"/games")]
            ),
            "3 mods in /games {}"
        );
    }

    #[test]
    fn fill_leaves_unknown_placeholders_alone() {
        assert_eq!(fill("{missing} {", &[("count", &3)]), "{missing} {");
    }

    #[test]
    fn fill_does_not_expand_braces_inside_values() {
        assert_eq!(fill("{a} {b}", &[("a", &"{b}"), ("b", &"x")]), "{b} x");
    }
}
