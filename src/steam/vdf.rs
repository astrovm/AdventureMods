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

        let mut result = String::new();
        let mut start = self.pos;

        while self.pos < self.input.len() {
            let ch = self.input.as_bytes()[self.pos];
            if ch == b'\\' && self.pos + 1 < self.input.len() {
                result.push_str(&self.input[start..self.pos]);
                self.pos += 1;
                match self.input.as_bytes()[self.pos] {
                    b'n' => result.push('\n'),
                    b't' => result.push('\t'),
                    b'\\' => result.push('\\'),
                    b'"' => result.push('"'),
                    b => {
                        result.push('\\');
                        result.push(b as char);
                    }
                }
                self.pos += 1;
                start = self.pos;
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
                return None;
            }
            if self.input.as_bytes()[self.pos] == b'}' {
                self.pos += 1;
                return Some(map);
            }
            let key = self.parse_quoted_string()?;
            let value = self.parse_value()?;
            map.insert(key, value);
        }
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
    let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let mut parser = Parser::new(input);
    let value = parser.parse_root()?;
    parser.skip_whitespace();

    if parser.pos == input.len() {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "vdf_tests.rs"]
mod tests;
