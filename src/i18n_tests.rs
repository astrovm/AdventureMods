use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::*;

const TEMPLATE: &str = include_str!("../po/adventure-mods.pot");
const LINGUAS: &str = include_str!("../po/LINGUAS");

fn env<'a>(vars: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<String> + 'a {
    |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

fn code_from(vars: &[(&str, &str)]) -> Option<&'static str> {
    language_from(env(vars)).map(|language| language.code)
}

/// The `{name}` placeholders of `text`, escaped braces left out.
fn placeholders(text: &str) -> BTreeSet<String> {
    let unescaped = text.replace("{{", "").replace("}}", "");
    unescaped
        .split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}'))
        .map(|(name, _)| name.to_owned())
        .collect()
}

/// The value of a header field, as in `Plural-Forms: nplurals=2; ...`.
fn header_field(source: &str, field: &str) -> Option<String> {
    let header = entries(source)
        .into_iter()
        .find(|entry| entry.msgid.is_empty())?;
    header.msgstr[0].lines().find_map(|line| {
        line.strip_prefix(field)?
            .strip_prefix(": ")
            .map(str::to_owned)
    })
}

fn messages(source: &str) -> BTreeMap<(String, String), Entry> {
    entries(source)
        .into_iter()
        .filter(|entry| !entry.msgid.is_empty())
        .map(|entry| {
            let key = (
                entry.msgctxt.clone().unwrap_or_default(),
                entry.msgid.clone(),
            );
            (key, entry)
        })
        .collect()
}

#[test]
fn the_parser_reads_every_part_of_a_message() {
    let source = r#"# A comment
msgid ""
msgstr ""
"Language: es\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\n"

#. Translators: a note
#: src/ui/app.rs
msgctxt "place"
msgid "Install"
msgstr "Instalación"

msgid "Line one "
"and two"
msgstr ""
"Línea uno "
"y dos"

msgid "{n} mod"
msgid_plural "{n} mods"
msgstr[0] "{n} mod"
msgstr[1] "{n} mods"

msgid "Escapes \"quoted\"\tand\\"
msgstr "Escapes \"citados\"\ty\\\n"
"#;
    let entries = entries(source);

    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0].msgid, "");
    assert_eq!(
        entries[0].msgstr,
        ["Language: es\nPlural-Forms: nplurals=2; plural=(n != 1);\n"]
    );
    assert_eq!(
        entries[1],
        Entry {
            msgctxt: Some("place".into()),
            msgid: "Install".into(),
            msgstr: vec!["Instalación".into()],
            ..Entry::default()
        }
    );
    assert_eq!(entries[2].msgid, "Line one and two");
    assert_eq!(entries[2].msgstr, ["Línea uno y dos"]);
    assert_eq!(entries[3].msgid_plural.as_deref(), Some("{n} mods"));
    assert_eq!(entries[3].msgstr, ["{n} mod", "{n} mods"]);
    assert_eq!(entries[4].msgid, "Escapes \"quoted\"\tand\\");
    assert_eq!(entries[4].msgstr, ["Escapes \"citados\"\ty\\\n"]);
}

#[test]
fn the_parser_marks_fuzzy_messages_and_skips_obsolete_ones() {
    let source = r#"#, fuzzy, rust-format
msgid "Old"
msgstr "Viejo"

#, rust-format
msgid "Kept"
msgstr "Guardado"

#~ msgid "Gone"
#~ msgstr "Ido"
"#;
    let entries = entries(source);

    assert_eq!(entries.len(), 2);
    assert!(entries[0].fuzzy);
    assert!(!entries[1].fuzzy);
    assert_eq!(entries[1].msgid, "Kept");
}

#[test]
fn a_comment_starts_the_next_message_without_a_blank_line() {
    let source = "msgid \"A\"\nmsgstr \"a\"\n#, fuzzy\nmsgid \"B\"\nmsgstr \"b\"\n";
    let entries = entries(source);

    assert_eq!(entries.len(), 2);
    assert!(!entries[0].fuzzy);
    assert!(entries[1].fuzzy);
}

#[test]
fn the_parser_ignores_text_it_does_not_know() {
    let source =
        "msgid \"A\"\nmsgstr[x] \"bad index\"\nmsgstr[2] \"third\"\nstray\nmsgstr \"first\"\n";
    let entries = entries(source);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].msgstr, ["first", "", "third"]);
    assert!(super::entries("").is_empty());
    assert!(super::entries("# only a comment\n\n").is_empty());
}

#[test]
fn a_dangling_backslash_stays() {
    assert_eq!(unquote(r#""end\"#), "end\\");
    assert_eq!(unquote("bare"), "bare");
    assert_eq!(unquote(r#""\q""#), "q");
}

#[test]
fn the_catalog_leaves_out_what_is_not_translated() {
    let catalog = Catalog::parse(
        r#"msgid ""
msgstr "Language: es\n"

msgid "Done"
msgstr "Listo"

msgid "Empty"
msgstr ""

#, fuzzy
msgid "Fuzzy"
msgstr "Borroso"

msgid "{n} mod"
msgid_plural "{n} mods"
msgstr[0] "{n} mod"
msgstr[1] ""
"#,
    );

    assert_eq!(catalog.get("", "Done"), Some(&["Listo".to_owned()][..]));
    assert_eq!(catalog.get("", ""), None);
    assert_eq!(catalog.get("", "Empty"), None);
    assert_eq!(catalog.get("", "Fuzzy"), None);
    assert_eq!(catalog.get("", "{n} mod"), None);
    assert_eq!(catalog.get("other", "Done"), None);
}

#[test]
fn the_locale_picks_the_language() {
    assert_eq!(code_from(&[]), None);
    assert_eq!(code_from(&[("LANG", "es_AR.UTF-8")]), Some("es"));
    assert_eq!(code_from(&[("LANG", "ja_JP.UTF-8")]), Some("ja"));
    assert_eq!(code_from(&[("LANG", "de-DE")]), Some("de"));
    assert_eq!(code_from(&[("LANG", "fr_FR@euro")]), Some("fr"));
    assert_eq!(code_from(&[("LANG", "it")]), Some("it"));
    assert_eq!(code_from(&[("LANG", "en_US.UTF-8")]), None);
    assert_eq!(code_from(&[("LANG", "pt_BR.UTF-8")]), None);
}

#[test]
fn lc_all_wins_over_lc_messages_and_lang() {
    let vars = [
        ("LC_ALL", "fr_FR.UTF-8"),
        ("LC_MESSAGES", "de_DE.UTF-8"),
        ("LANG", "es_AR.UTF-8"),
    ];
    assert_eq!(code_from(&vars), Some("fr"));
    assert_eq!(
        code_from(&[("LC_MESSAGES", "de_DE.UTF-8"), ("LANG", "es_AR.UTF-8")]),
        Some("de")
    );
    // An empty variable counts as unset.
    assert_eq!(
        code_from(&[("LC_ALL", ""), ("LANG", "es_AR.UTF-8")]),
        Some("es")
    );
}

#[test]
fn the_c_locale_keeps_english() {
    for locale in ["C", "POSIX", "C.UTF-8"] {
        assert_eq!(
            code_from(&[("LANG", locale), ("LANGUAGE", "es")]),
            None,
            "{locale}"
        );
    }
}

#[test]
fn language_lists_the_preferences_in_order() {
    let lang = ("LANG", "de_DE.UTF-8");
    assert_eq!(code_from(&[lang, ("LANGUAGE", "pt:ja:es")]), Some("ja"));
    assert_eq!(code_from(&[lang, ("LANGUAGE", "pt_BR:")]), Some("de"));
    assert_eq!(code_from(&[lang, ("LANGUAGE", "en_GB:es")]), None);
    assert_eq!(code_from(&[lang, ("LANGUAGE", "")]), Some("de"));
}

#[test]
fn english_is_the_default_in_tests() {
    assert_eq!(tr!("Cancel"), "Cancel");
    assert_eq!(tr!("Installation"), "Installation");
}

#[test]
fn text_shows_in_the_chosen_language() {
    let _spanish = use_language("es");
    assert_eq!(tr!("Cancel"), "Cancelar");
    assert_eq!(tr!("Install"), "Instalar");
    assert_eq!(tr!("Installation"), "Instalación");
    // Text marked with a context the catalog doesn't have stays in English.
    assert_eq!(trc!("button", "Install"), "Install");
    assert_eq!(
        trf!("Restore the original {game}?", game = "Sonic Adventure 2"),
        "¿Restaurar Sonic Adventure 2 original?"
    );
}

#[test]
fn text_without_a_translation_stays_in_english() {
    let _spanish = use_language("es");
    assert_eq!(translate("Not in any catalog"), "Not in any catalog");
    assert_eq!(translate_in("nowhere", "Cancel"), "Cancel");
    assert_eq!(translate_plural("one thing", "many things", 1), "one thing");
    assert_eq!(
        translate_plural("one thing", "many things", 2),
        "many things"
    );
}

#[test]
fn the_guard_goes_back_to_english() {
    {
        let _japanese = use_language("ja");
        assert_eq!(tr!("Cancel"), "キャンセル");
    }
    assert_eq!(tr!("Cancel"), "Cancel");
}

#[test]
#[should_panic(expected = "no catalog for pt")]
fn a_language_without_a_catalog_cannot_be_used() {
    let _ = use_language("pt");
}

fn already_installed(n: u64) -> String {
    trn!(
        "{download}, {n} already installed",
        "{download}, {n} already installed",
        n,
        download = "5 MB"
    )
}

#[test]
fn plurals_follow_each_language() {
    assert_eq!(already_installed(1), "5 MB, 1 already installed");
    {
        let _spanish = use_language("es");
        assert_eq!(already_installed(0), "5 MB, 0 ya instalados");
        assert_eq!(already_installed(1), "5 MB, 1 ya instalado");
        assert_eq!(already_installed(2), "5 MB, 2 ya instalados");
    }
    {
        let _french = use_language("fr");
        assert_eq!(already_installed(0), "5 MB, 0 déjà installé");
        assert_eq!(already_installed(1), "5 MB, 1 déjà installé");
        assert_eq!(already_installed(2), "5 MB, 2 déjà installés");
    }
    {
        let _japanese = use_language("ja");
        assert_eq!(already_installed(1), "5 MB、1個はインストール済み");
        assert_eq!(already_installed(7), "5 MB、7個はインストール済み");
    }
}

#[test]
fn counts_too_big_or_negative_are_many() {
    assert_eq!(count(3_usize), 3);
    assert_eq!(count(-1_i32), u64::MAX);
    assert_eq!(count(u128::MAX), u64::MAX);
}

#[test]
fn fill_replaces_placeholders_and_escapes() {
    let game = "SA2";
    let args: [(&str, &dyn Display); 2] = [("game", &game), ("n", &3)];
    assert_eq!(fill("{game} has {n} mods", &args), "SA2 has 3 mods");
    assert_eq!(fill("{{game}} and }}{{", &args), "{game} and }{");
    assert_eq!(fill("{unknown} {game", &args), "{unknown} {game");
    assert_eq!(fill("a } b { c", &args), "a } b { c");
    assert_eq!(fill("{n}{n}", &args), "33");
    assert_eq!(fill("", &args), "");
}

#[test]
fn values_are_never_read_as_templates() {
    let value = "{game}";
    let args: [(&str, &dyn Display); 2] = [("a", &value), ("game", &"no")];
    assert_eq!(fill("{a}", &args), "{game}");
}

#[test]
fn linguas_lists_every_language() {
    let listed: Vec<&str> = LINGUAS.split_whitespace().collect();
    let built_in: Vec<&str> = LANGUAGES.iter().map(|language| language.code).collect();
    assert_eq!(listed, built_in);
}

#[test]
fn every_catalog_translates_exactly_the_template() {
    let template = messages(TEMPLATE);
    assert!(template.len() > 200, "the template looks empty");
    for language in &LANGUAGES {
        let catalog = messages(language.source);
        let missing: Vec<_> = template
            .keys()
            .filter(|key| !catalog.contains_key(*key))
            .collect();
        let extra: Vec<_> = catalog
            .keys()
            .filter(|key| !template.contains_key(*key))
            .collect();
        assert!(missing.is_empty(), "{} lacks {missing:?}", language.code);
        assert!(extra.is_empty(), "{} has stale {extra:?}", language.code);
        for entry in catalog.values() {
            assert!(
                !entry.fuzzy,
                "{} has fuzzy {:?}",
                language.code, entry.msgid
            );
            assert!(
                !entry.msgstr.is_empty() && entry.msgstr.iter().all(|form| !form.is_empty()),
                "{} leaves {:?} untranslated",
                language.code,
                entry.msgid
            );
        }
    }
}

/// The Rust source files that mark text, as `update-po.sh` picks them.
fn marking_sources(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            marking_sources(&path, files);
        } else if name.ends_with(".rs") && !name.ends_with("_tests.rs") && name != "i18n.rs" {
            files.push(path);
        }
    }
}

/// The Rust string literal at the start of `code`, and the code after it.
fn string_literal(code: &str) -> Option<(String, &str)> {
    let mut chars = code.strip_prefix('"')?.char_indices();
    let mut text = String::new();
    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return Some((text, &code[index + 2..])),
            '\\' => match chars.next()?.1 {
                'n' => text.push('\n'),
                't' => text.push('\t'),
                // A line continuation skips the next line's indentation.
                '\n' => {
                    let rest = chars.as_str();
                    let skipped = rest.len() - rest.trim_start().len();
                    for _ in rest[..skipped].chars() {
                        chars.next();
                    }
                }
                other => text.push(other),
            },
            c => text.push(c),
        }
    }
    None
}

/// Every text the code marks for translation, by context and English text,
/// with where it is.
fn marked_texts() -> Vec<((String, String), String)> {
    let mut files = Vec::new();
    marking_sources(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut texts = Vec::new();
    for file in files {
        let code = std::fs::read_to_string(&file).unwrap();
        for (start, _) in code.match_indices("!(") {
            let name = code[..start]
                .rsplit(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap();
            if !["tr", "trc", "trf", "trn", "tr_noop"].contains(&name) {
                continue;
            }
            let Some((first, rest)) = string_literal(code[start + 2..].trim_start()) else {
                continue;
            };
            let key = if name == "trc" {
                let rest = rest.trim_start().strip_prefix(',').unwrap().trim_start();
                (first, string_literal(rest).unwrap().0)
            } else {
                (String::new(), first)
            };
            let line = code[..start].lines().count();
            texts.push((key, format!("{}:{line}", file.display())));
        }
    }
    texts
}

#[test]
fn the_template_has_every_marked_text() {
    let texts = marked_texts();
    assert!(texts.len() > 200, "found only {} texts", texts.len());
    let template = messages(TEMPLATE);
    // xgettext skips text inside path-qualified macros such as
    // `anyhow::bail!(tr!(...))`; import the macro and call it plainly.
    let missing: Vec<_> = texts
        .iter()
        .filter(|(key, _)| !template.contains_key(key))
        .collect();
    assert!(
        missing.is_empty(),
        "run build-aux/update-po.sh: {missing:#?}"
    );
}

#[test]
fn string_literals_read_like_rust() {
    assert_eq!(
        string_literal(
            r#""a \"b\" \\ \n\t\
            c" rest"#
        ),
        Some(("a \"b\" \\ \n\tc".to_owned(), " rest"))
    );
    assert_eq!(string_literal("not a string"), None);
    assert_eq!(string_literal(r#""unterminated"#), None);
    assert_eq!(string_literal(r#""dangling \"#), None);
}

#[test]
fn translations_keep_the_placeholders() {
    for language in &LANGUAGES {
        for entry in messages(language.source).values() {
            let expected = placeholders(&entry.msgid);
            if let Some(plural) = &entry.msgid_plural {
                assert_eq!(placeholders(plural), expected, "{:?}", entry.msgid);
            }
            for form in &entry.msgstr {
                assert_eq!(
                    placeholders(form),
                    expected,
                    "{}: {:?} → {form:?}",
                    language.code,
                    entry.msgid
                );
            }
        }
    }
}

type Rule = fn(u64) -> usize;

/// The `Plural-Forms` header, and its rule, of each plural rule we know.
const RULES: [(&str, Rule); 3] = [
    ("nplurals=2; plural=(n != 1);", |n| usize::from(n != 1)),
    ("nplurals=2; plural=(n > 1);", |n| usize::from(n > 1)),
    ("nplurals=1; plural=0;", |_| 0),
];

#[test]
fn plural_rules_match_the_catalog_headers() {
    let counts: Vec<u64> = (0..1000).chain([u64::MAX]).collect();
    for language in &LANGUAGES {
        let header = header_field(language.source, "Plural-Forms").unwrap();
        let (_, rule) = RULES
            .iter()
            .find(|(forms, _)| *forms == header)
            .unwrap_or_else(|| panic!("{}: no rule for {header}", language.code));
        for &n in &counts {
            assert_eq!((language.plural)(n), rule(n), "{} for {n}", language.code);
        }
        let forms: usize = header
            .strip_prefix("nplurals=")
            .and_then(|rest| rest.split(';').next())
            .and_then(|n| n.parse().ok())
            .unwrap();
        for entry in messages(language.source).values() {
            if entry.msgid_plural.is_some() {
                assert_eq!(entry.msgstr.len(), forms, "{}", language.code);
            }
        }
        assert_eq!(
            header_field(language.source, "Language").as_deref(),
            Some(language.code)
        );
    }
}

/// `text` as `.po` lines, split at `at` characters so continuation lines are
/// read too.
fn po_string(keyword: &str, text: &str, at: usize) -> String {
    let escape = |part: &str| {
        part.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
    };
    let chars: Vec<char> = text.chars().collect();
    let mut lines = vec![format!("{keyword} \"\"")];
    for chunk in chars.chunks(at.max(1)) {
        lines.push(format!("\"{}\"", escape(&chunk.iter().collect::<String>())));
    }
    lines.join("\n")
}

fn po_text() -> impl Strategy<Value = String> {
    // Any text but the control characters a .po file escapes in other ways.
    "[^\\x00-\\x08\\x0b-\\x1f\\x7f]{0,40}"
}

proptest! {
    #[test]
    fn the_parser_reads_back_what_was_written(
        context in proptest::option::of(po_text()),
        msgid in po_text().prop_filter("not the header", |id| !id.is_empty()),
        forms in proptest::collection::vec(po_text(), 1..4),
        plural in proptest::option::of(po_text()),
        fuzzy in any::<bool>(),
        at in 1_usize..10,
    ) {
        let mut source = String::from("msgid \"\"\nmsgstr \"Language: es\\n\"\n\n");
        if fuzzy {
            source.push_str("#, fuzzy\n");
        }
        if let Some(context) = &context {
            source.push_str(&po_string("msgctxt", context, at));
            source.push('\n');
        }
        source.push_str(&po_string("msgid", &msgid, at));
        source.push('\n');
        if let Some(plural) = &plural {
            source.push_str(&po_string("msgid_plural", plural, at));
            source.push('\n');
            for (index, form) in forms.iter().enumerate() {
                source.push_str(&po_string(&format!("msgstr[{index}]"), form, at));
                source.push('\n');
            }
        } else {
            source.push_str(&po_string("msgstr", &forms[0], at));
            source.push('\n');
        }

        let parsed = entries(&source);
        let expected = Entry {
            msgctxt: context,
            msgid,
            msgstr: if plural.is_some() { forms } else { vec![forms[0].clone()] },
            msgid_plural: plural,
            fuzzy,
        };
        prop_assert_eq!(parsed.len(), 2);
        prop_assert_eq!(&parsed[1], &expected);
    }

    #[test]
    fn text_without_placeholders_fills_to_itself(text in "[^{}]*") {
        prop_assert_eq!(fill(&text, &[("a", &1 as &dyn Display)]), text);
    }

    #[test]
    fn escaped_braces_fill_to_single_ones(text in ".*") {
        let template = text.replace('{', "{{").replace('}', "}}");
        prop_assert_eq!(fill(&template, &[("a", &1 as &dyn Display)]), text);
    }

    #[test]
    fn a_placeholder_fills_to_its_value(value in ".*", before in "[^{}]*", after in "[^{}]*") {
        let template = format!("{before}{{value}}{after}");
        prop_assert_eq!(fill(&template, &[("value", &value as &dyn Display)]), format!("{before}{value}{after}"));
    }

    #[test]
    fn the_language_is_the_locale_prefix(
        code in "[a-z]{2,3}",
        rest in "([_-][A-Z]{2})?(\\.[A-Za-z0-9-]+)?(@[a-z]+)?",
    ) {
        let locale = format!("{code}{rest}");
        prop_assert_eq!(language_code(&locale), code.as_str());
    }
}
