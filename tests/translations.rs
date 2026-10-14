//! Keeps the translation catalogs in `po/` in step with the strings in the source.

use std::collections::BTreeSet;
use std::fs;
use std::iter::Peekable;
use std::path::Path;
use std::str::CharIndices;

/// A string literal from a translation call. `continued` is true when the
/// literal has a `\` line continuation, which puts the next line's indentation
/// into the text.
struct Literal {
    text: String,
    continued: bool,
}

/// One `msgid`/`msgstr` pair from a catalog.
struct Entry {
    msgid: String,
    msgstr: String,
    fuzzy: bool,
}

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn unquote(text: &str) -> String {
    let inner = text
        .trim()
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_else(|| panic!("not a quoted po string: {text}"));
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            other => panic!("unsupported escape in po string: {other:?}"),
        }
    }
    out
}

/// Parses a `.po` or `.pot` file. The header entry (empty `msgid`) is skipped.
fn parse_po(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    for block in text.split("\n\n") {
        let (mut msgid, mut msgstr, mut fuzzy) = (String::new(), String::new(), false);
        let mut field = ' ';
        for line in block.lines().map(str::trim) {
            if let Some(flags) = line.strip_prefix("#,") {
                fuzzy |= flags.contains("fuzzy");
            } else if line.starts_with('#') || line.is_empty() {
                continue;
            } else if let Some(rest) = line.strip_prefix("msgid ") {
                field = 'i';
                msgid.push_str(&unquote(rest));
            } else if let Some(rest) = line.strip_prefix("msgstr ") {
                field = 's';
                msgstr.push_str(&unquote(rest));
            } else if line.starts_with("msgid_plural") {
                panic!("plural entries are not supported");
            } else if line.starts_with('"') {
                match field {
                    'i' => msgid.push_str(&unquote(line)),
                    's' => msgstr.push_str(&unquote(line)),
                    _ => panic!("continuation line outside a field: {line}"),
                }
            }
        }
        if !msgid.is_empty() {
            entries.push(Entry {
                msgid,
                msgstr,
                fuzzy,
            });
        }
    }
    entries
}

/// Names in `{name}` placeholders. Braces with other contents are ignored.
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        let name = &after[..end];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            names.insert(name.to_string());
        }
        rest = &after[end + 1..];
    }
    names
}

fn skip_whitespace(source: &str, mut index: usize) -> usize {
    while let Some(c) = source[index..].chars().next() {
        if !c.is_whitespace() {
            break;
        }
        index += c.len_utf8();
    }
    index
}

/// Reads the Rust string literal that starts at `start` (the opening quote).
/// Returns it and the index just past its closing quote.
fn read_literal(source: &str, start: usize) -> (Literal, usize) {
    let mut text = String::new();
    let mut continued = false;
    let mut chars: Peekable<CharIndices> = source[start + 1..].char_indices().peekable();
    loop {
        let (offset, c) = chars.next().expect("unterminated string literal");
        match c {
            '"' => {
                return (Literal { text, continued }, start + 1 + offset + 1);
            }
            '\\' => match chars.next().expect("unterminated escape").1 {
                'n' => text.push('\n'),
                't' => text.push('\t'),
                '"' => text.push('"'),
                '\\' => text.push('\\'),
                '\n' => {
                    continued = true;
                    while chars.peek().is_some_and(|(_, next)| next.is_whitespace()) {
                        chars.next();
                    }
                }
                other => panic!("unsupported escape \\{other} in source string"),
            },
            c => text.push(c),
        }
    }
}

/// Finds the string literals passed to `tr`, `tr_fmt`, `trn`, and `mark`.
/// `trn` takes two literals; the others take one.
fn scan_literals(source: &str) -> Vec<Literal> {
    let mut found = Vec::new();
    for name in ["tr_fmt", "trn", "tr", "mark"] {
        for (index, _) in source.match_indices(name) {
            let preceded_by_word = source[..index]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            if preceded_by_word {
                continue;
            }
            let mut pos = skip_whitespace(source, index + name.len());
            if !source[pos..].starts_with('(') {
                continue;
            }
            pos = skip_whitespace(source, pos + 1);
            let count = if name == "trn" { 2 } else { 1 };
            for n in 0..count {
                if n > 0 {
                    if !source[pos..].starts_with(',') {
                        break;
                    }
                    pos = skip_whitespace(source, pos + 1);
                }
                if !source[pos..].starts_with('"') {
                    break;
                }
                let (literal, end) = read_literal(source, pos);
                found.push(literal);
                pos = skip_whitespace(source, end);
            }
        }
    }
    found
}

/// Every translatable literal in the files listed in `po/POTFILES`.
fn source_literals() -> Vec<Literal> {
    read("po/POTFILES")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .flat_map(|file| scan_literals(&read(file)))
        .collect()
}

fn template_msgids() -> BTreeSet<String> {
    let entries = parse_po(&read("po/adventure-mods.pot"));
    let msgids: BTreeSet<String> = entries.into_iter().map(|entry| entry.msgid).collect();
    assert!(!msgids.is_empty(), "the template has no messages");
    msgids
}

#[test]
fn scanner_finds_translation_calls_and_flags_continuations() {
    let sample = r#"tr("Plain"); tr_fmt(
        "Hi {name}", &[]); mark("A \
        B"); str("ignored"); trn("one", "many", 2)"#;
    let mut literals = scan_literals(sample);
    literals.sort_by(|a, b| a.text.cmp(&b.text));
    let texts: Vec<&str> = literals.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["A B", "Hi {name}", "Plain", "many", "one"]);
    let continued: Vec<bool> = literals.iter().map(|l| l.continued).collect();
    assert_eq!(continued, [true, false, false, false, false]);
}

#[test]
fn placeholders_are_named_and_braces_are_ignored() {
    let names = placeholders("{count} mods in {path}, {} and {x y} {}");
    assert_eq!(
        names,
        BTreeSet::from(["count".to_string(), "path".to_string()])
    );
}

#[test]
fn translatable_strings_stay_on_one_line() {
    let broken: Vec<String> = source_literals()
        .into_iter()
        .filter(|literal| literal.continued)
        .map(|literal| literal.text)
        .collect();
    assert!(
        broken.is_empty(),
        "a `\\` line continuation puts the next line's indentation into the message: {broken:?}"
    );
}

#[test]
fn template_has_every_translatable_string_and_nothing_stale() {
    let in_source: BTreeSet<String> = source_literals()
        .into_iter()
        .map(|literal| literal.text)
        .collect();
    let in_template = template_msgids();
    let missing: Vec<&String> = in_source.difference(&in_template).collect();
    let stale: Vec<&String> = in_template.difference(&in_source).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "po/adventure-mods.pot is out of date. Regenerate it.\nmissing: {missing:?}\nstale: {stale:?}"
    );
}

#[test]
fn template_header_does_not_publish_a_bug_report_address() {
    let header = read("po/adventure-mods.pot");
    let header = header.split("\n\n").next().unwrap_or_default();
    let report_line = header
        .lines()
        .find(|line| line.starts_with("\"Report-Msgid-Bugs-To:"))
        .expect("the template header has no Report-Msgid-Bugs-To line");
    assert_eq!(
        report_line, "\"Report-Msgid-Bugs-To: \\n\"",
        "the template header publishes an address"
    );
}

#[test]
fn every_catalog_translates_every_message_with_the_same_placeholders() {
    let template = template_msgids();
    for lang in read("po/LINGUAS").split_whitespace() {
        let entries = parse_po(&read(&format!("po/{lang}.po")));
        let mut translated: BTreeSet<&str> = BTreeSet::new();
        for entry in &entries {
            assert!(!entry.fuzzy, "{lang}: fuzzy entry {:?}", entry.msgid);
            assert!(
                template.contains(&entry.msgid),
                "{lang}: message not in the template: {:?}",
                entry.msgid
            );
            assert!(
                translated.insert(&entry.msgid),
                "{lang}: duplicate {:?}",
                entry.msgid
            );
            assert!(
                !entry.msgstr.is_empty(),
                "{lang}: untranslated {:?}",
                entry.msgid
            );
            assert_eq!(
                placeholders(&entry.msgstr),
                placeholders(&entry.msgid),
                "{lang}: placeholders differ for {:?}",
                entry.msgid
            );
        }
        let untranslated: Vec<&String> = template
            .iter()
            .filter(|msgid| !translated.contains(msgid.as_str()))
            .collect();
        assert!(untranslated.is_empty(), "{lang}: missing {untranslated:?}");
    }
}
