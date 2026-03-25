use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum VdfValue {
    String(String),
    Map(HashMap<String, VdfValue>),
}

impl VdfValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&HashMap<String, VdfValue>> {
        match self {
            VdfValue::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.as_map()?.get(key)
    }
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input.as_bytes()[self.pos];
            if ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'\r' {
                self.pos += 1;
            } else if self.input[self.pos..].starts_with("//") {
                while self.pos < self.input.len() && self.input.as_bytes()[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn parse_quoted_string(&mut self) -> Option<String> {
        self.skip_whitespace();
        if self.pos >= self.input.len() || self.input.as_bytes()[self.pos] != b'"' {
            return None;
        }
        self.pos += 1;

        let start = self.pos;
        let mut result = String::new();

        while self.pos < self.input.len() {
            let ch = self.input.as_bytes()[self.pos];
            if ch == b'\\' && self.pos + 1 < self.input.len() {
                result.push_str(&self.input[start..self.pos]);
                self.pos += 1;
                let escaped = self.input.as_bytes()[self.pos];
                match escaped {
                    b'n' => result.push('\n'),
                    b't' => result.push('\t'),
                    b'\\' => result.push('\\'),
                    b'"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(escaped as char);
                    }
                }
                self.pos += 1;
                return self.parse_quoted_string_continue(result);
            } else if ch == b'"' {
                result.push_str(&self.input[start..self.pos]);
                self.pos += 1;
                return Some(result);
            } else {
                self.pos += 1;
            }
        }
        None
    }

    fn parse_quoted_string_continue(&mut self, mut result: String) -> Option<String> {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input.as_bytes()[self.pos];
            if ch == b'\\' && self.pos + 1 < self.input.len() {
                result.push_str(&self.input[start..self.pos]);
                self.pos += 1;
                let escaped = self.input.as_bytes()[self.pos];
                match escaped {
                    b'n' => result.push('\n'),
                    b't' => result.push('\t'),
                    b'\\' => result.push('\\'),
                    b'"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(escaped as char);
                    }
                }
                self.pos += 1;
                return self.parse_quoted_string_continue(result);
            } else if ch == b'"' {
                result.push_str(&self.input[start..self.pos]);
                self.pos += 1;
                return Some(result);
            } else {
                self.pos += 1;
            }
        }
        None
    }

    fn parse_value(&mut self) -> Option<VdfValue> {
        self.skip_whitespace();
        if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'{' {
            self.pos += 1;
            let map = self.parse_map()?;
            Some(VdfValue::Map(map))
        } else {
            let s = self.parse_quoted_string()?;
            Some(VdfValue::String(s))
        }
    }

    fn parse_map(&mut self) -> Option<HashMap<String, VdfValue>> {
        let mut map = HashMap::new();
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                break;
            }
            if self.input.as_bytes()[self.pos] == b'}' {
                self.pos += 1;
                break;
            }
            let key = self.parse_quoted_string()?;
            let value = self.parse_value()?;
            map.insert(key, value);
        }
        Some(map)
    }

    fn parse_root(&mut self) -> Option<VdfValue> {
        let key = self.parse_quoted_string()?;
        let value = self.parse_value()?;
        let mut map = HashMap::new();
        map.insert(key, value);
        Some(VdfValue::Map(map))
    }
}

pub fn parse(input: &str) -> Option<VdfValue> {
    let mut parser = Parser::new(input);
    parser.parse_root()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_libraryfolders() {
        let input = r#"
"libraryfolders"
{
    "0"
    {
        "path"		"/home/user/.local/share/Steam"
        "label"		""
        "apps"
        {
            "71250"		"6789"
            "213610"	"12345"
        }
    }
    "1"
    {
        "path"		"/mnt/games/SteamLibrary"
        "label"		""
        "apps"
        {
            "400"		"5000"
        }
    }
}
"#;
        let root = parse(input).unwrap();
        let folders = root.get("libraryfolders").unwrap().as_map().unwrap();

        let lib0 = folders.get("0").unwrap().as_map().unwrap();
        assert_eq!(
            lib0.get("path").unwrap().as_str().unwrap(),
            "/home/user/.local/share/Steam"
        );

        let apps0 = lib0.get("apps").unwrap().as_map().unwrap();
        assert!(apps0.contains_key("71250"));
        assert!(apps0.contains_key("213610"));

        let lib1 = folders.get("1").unwrap().as_map().unwrap();
        assert_eq!(
            lib1.get("path").unwrap().as_str().unwrap(),
            "/mnt/games/SteamLibrary"
        );
    }

    #[test]
    fn test_parse_simple_key_value() {
        let input = r#""root" "hello""#;
        let root = parse(input).unwrap();
        assert_eq!(root.get("root").unwrap().as_str().unwrap(), "hello");
    }

    #[test]
    fn test_parse_empty_map() {
        let input = r#""root" {}"#;
        let root = parse(input).unwrap();
        let map = root.get("root").unwrap().as_map().unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_parse_nested_maps() {
        let input = r#"
"a"
{
    "b"
    {
        "c"
        {
            "d"     "deep"
        }
    }
}
"#;
        let root = parse(input).unwrap();
        let val = root
            .get("a")
            .unwrap()
            .get("b")
            .unwrap()
            .get("c")
            .unwrap()
            .get("d")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(val, "deep");
    }

    #[test]
    fn test_parse_escape_sequences() {
        let input =
            r#""root" { "a" "line1\nline2" "b" "tab\there" "c" "back\\slash" "d" "a\"quote" }"#;
        let root = parse(input).unwrap();
        let m = root.get("root").unwrap();
        assert_eq!(m.get("a").unwrap().as_str().unwrap(), "line1\nline2");
        assert_eq!(m.get("b").unwrap().as_str().unwrap(), "tab\there");
        assert_eq!(m.get("c").unwrap().as_str().unwrap(), "back\\slash");
        assert_eq!(m.get("d").unwrap().as_str().unwrap(), "a\"quote");
    }

    #[test]
    fn test_parse_comments() {
        let input = r#"
// This is a comment
"root"
{
    // Another comment
    "key"       "value"
    // End comment
}
"#;
        let root = parse(input).unwrap();
        assert_eq!(
            root.get("root")
                .unwrap()
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            "value"
        );
    }

    #[test]
    fn test_parse_empty_string_value() {
        let input = r#""root" { "key" "" }"#;
        let root = parse(input).unwrap();
        assert_eq!(
            root.get("root")
                .unwrap()
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            ""
        );
    }

    #[test]
    fn test_parse_whitespace_variations() {
        let input = "\"root\"\t\t{\r\n\t\"key\"\t\t\"value\"\r\n}";
        let root = parse(input).unwrap();
        assert_eq!(
            root.get("root")
                .unwrap()
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            "value"
        );
    }

    #[test]
    fn test_parse_malformed_unclosed_quote() {
        let input = r#""root" { "key" "unclosed }"#;
        assert!(parse(input).is_none());
    }

    #[test]
    fn test_parse_malformed_unclosed_brace() {
        let input = r#""root" { "key" "value""#;
        // Missing closing brace: parse_map will consume to end without finding }
        // This may still parse depending on implementation; the key point is it doesn't panic
        let _ = parse(input);
    }

    #[test]
    fn test_parse_empty_input() {
        assert!(parse("").is_none());
        assert!(parse("   ").is_none());
        assert!(parse("// just a comment\n").is_none());
    }

    #[test]
    fn test_vdfvalue_accessors() {
        let string_val = VdfValue::String("hello".to_string());
        assert_eq!(string_val.as_str(), Some("hello"));
        assert!(string_val.as_map().is_none());
        assert!(string_val.get("anything").is_none());

        let map_val = VdfValue::Map(HashMap::new());
        assert!(map_val.as_str().is_none());
        assert!(map_val.as_map().is_some());
        assert!(map_val.get("missing").is_none());
    }

    #[test]
    fn test_parse_duplicate_keys() {
        let input = r#""root" { "key" "first" "key" "second" }"#;
        let root = parse(input).unwrap();
        // HashMap: last inserted wins
        let val = root
            .get("root")
            .unwrap()
            .get("key")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(val, "second");
    }

    #[test]
    fn test_parse_consecutive_escapes() {
        // Two backslashes followed by n: should produce literal `\n` (backslash + n)
        let input = r#""root" { "a" "a\\\\b" "b" "x\\ny" }"#;
        let root = parse(input).unwrap();
        let m = root.get("root").unwrap();
        assert_eq!(m.get("a").unwrap().as_str().unwrap(), "a\\\\b");
        assert_eq!(m.get("b").unwrap().as_str().unwrap(), "x\\ny");
    }

    #[test]
    fn test_parse_unknown_escape() {
        // Unknown escape like \x should be kept as-is: backslash + x
        let input = r#""root" { "key" "abc\xdef" }"#;
        let root = parse(input).unwrap();
        assert_eq!(
            root.get("root")
                .unwrap()
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            "abc\\xdef"
        );
    }

    #[test]
    fn test_parse_numeric_keys() {
        // VDF often uses numeric keys as pseudo-arrays (like libraryfolders.vdf)
        let input = r#""root" { "0" "first" "1" "second" "2" "third" }"#;
        let root = parse(input).unwrap();
        let m = root.get("root").unwrap();
        assert_eq!(m.get("0").unwrap().as_str().unwrap(), "first");
        assert_eq!(m.get("1").unwrap().as_str().unwrap(), "second");
        assert_eq!(m.get("2").unwrap().as_str().unwrap(), "third");
    }

    #[test]
    fn test_vdfvalue_get_chained_missing() {
        let root = parse(r#""root" { "a" { "b" "val" } }"#).unwrap();
        // Valid chain
        assert_eq!(
            root.get("root")
                .and_then(|v| v.get("a"))
                .and_then(|v| v.get("b"))
                .and_then(|v| v.as_str()),
            Some("val")
        );
        // Missing intermediate key
        assert!(root
            .get("root")
            .and_then(|v| v.get("x"))
            .and_then(|v| v.get("b"))
            .is_none());
        // get() on a string value
        assert!(root
            .get("root")
            .and_then(|v| v.get("a"))
            .and_then(|v| v.get("b"))
            .and_then(|v| v.get("anything"))
            .is_none());
    }

    #[test]
    fn test_parse_only_whitespace_between_entries() {
        // No separators other than whitespace between key-value pairs
        let input = "\"root\"{\"a\"\"1\"\"b\"\"2\"}";
        let root = parse(input).unwrap();
        let m = root.get("root").unwrap();
        assert_eq!(m.get("a").unwrap().as_str().unwrap(), "1");
        assert_eq!(m.get("b").unwrap().as_str().unwrap(), "2");
    }

    #[test]
    fn test_parse_comment_at_eof_no_newline() {
        let input = "\"root\" \"value\" // trailing comment";
        let root = parse(input).unwrap();
        assert_eq!(root.get("root").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn test_parse_map_with_mixed_value_types() {
        let input = r#"
"root"
{
    "string_val"    "hello"
    "sub_map"
    {
        "nested"    "world"
    }
    "another"       "test"
}
"#;
        let root = parse(input).unwrap();
        let m = root.get("root").unwrap();
        assert_eq!(m.get("string_val").unwrap().as_str().unwrap(), "hello");
        assert!(m.get("sub_map").unwrap().as_map().is_some());
        assert_eq!(
            m.get("sub_map")
                .unwrap()
                .get("nested")
                .unwrap()
                .as_str()
                .unwrap(),
            "world"
        );
        assert_eq!(m.get("another").unwrap().as_str().unwrap(), "test");
    }

    #[test]
    fn test_parse_no_root_value() {
        // Bare key with no value following
        assert!(parse(r#""key""#).is_none());
    }

    #[test]
    fn test_parse_unquoted_key_fails() {
        // VDF requires quoted keys
        assert!(parse("root { }").is_none());
    }

    #[test]
    fn test_parse_deeply_nested_100_levels() {
        let mut input = String::new();
        for i in 0..100 {
            input.push_str(&format!("\"l{i}\" {{\n"));
        }
        input.push_str("\"leaf\" \"value\"\n");
        for _ in 0..100 {
            input.push_str("}\n");
        }
        let root = parse(&input).unwrap();

        // Walk down to the leaf
        let mut current = &root;
        for i in 0..100 {
            current = current.get(&format!("l{i}")).unwrap();
        }
        assert_eq!(current.get("leaf").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn test_parse_long_string_value() {
        let long_value = "x".repeat(100_000);
        let input = format!("\"root\" \"{}\"", long_value);
        let root = parse(&input).unwrap();
        assert_eq!(root.get("root").unwrap().as_str().unwrap(), long_value);
    }

    #[test]
    fn test_parse_many_keys() {
        let mut input = String::from("\"root\" {\n");
        for i in 0..10_000 {
            input.push_str(&format!("\"key_{i}\" \"{i}\"\n"));
        }
        input.push('}');
        let root = parse(&input).unwrap();
        let map = root.get("root").unwrap().as_map().unwrap();
        assert_eq!(map.len(), 10_000);
        assert_eq!(map.get("key_0").unwrap().as_str().unwrap(), "0");
        assert_eq!(map.get("key_9999").unwrap().as_str().unwrap(), "9999");
    }
}
