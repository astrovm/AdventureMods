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
}
