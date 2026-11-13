//! Just enough JSON to read the lines cargo prints under `--message-format json`.
//!
//! The crate takes no dependencies (its manifest says why), and cargo's messages are one object per
//! line, so a small reader is cheaper than a parser crate. It reads any valid JSON; what it does
//! not do is keep a number's exact text, which no field the gate reads is.

/// One JSON value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// Any number, as a float.
    Number(f64),
    /// A string, with its escapes undone.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object, in the order its keys were written.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member called `key`, when this is an object that has one.
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The text, when this is a string.
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    /// The flag, when this is a boolean.
    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    /// The items, when this is an array.
    pub(crate) fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The member at `path`, one key per object level.
    pub(crate) fn at(&self, path: &[&str]) -> Option<&Value> {
        path.iter().try_fold(self, |value, key| value.get(key))
    }
}

/// Parses one JSON document.
///
/// # Errors
///
/// Says where the text stops being JSON.
pub(crate) fn parse(text: &str) -> Result<Value, String> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = reader.value()?;
    reader.space();
    if reader.at != reader.bytes.len() {
        return Err(format!("trailing text at byte {}", reader.at));
    }
    Ok(value)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at byte {}", byte as char, self.at))
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(format!("unexpected text at byte {}", self.at))
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.space();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("expected a value at byte {}", self.at)),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut members = Vec::new();
        self.space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.space();
            let key = self.string()?;
            self.space();
            self.expect(b':')?;
            members.push((key, self.value()?));
            self.space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(format!("expected `,` or `}}` at byte {}", self.at)),
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(format!("expected `,` or `]` at byte {}", self.at)),
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?;
        text.parse()
            .map(Value::Number)
            .map_err(|_| format!("bad number at byte {start}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.at;
            while self.peek().is_some_and(|b| b != b'"' && b != b'\\') {
                self.at += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?,
            );
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    self.escape(&mut out)?;
                }
                _ => return Err("unterminated string".to_owned()),
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> Result<(), String> {
        let Some(byte) = self.peek() else {
            return Err("unterminated escape".to_owned());
        };
        self.at += 1;
        match byte {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{8}'),
            b'f' => out.push('\u{c}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                let high = self.hex4()?;
                let code = if (0xD800..0xDC00).contains(&high) {
                    self.expect(b'\\')?;
                    self.expect(b'u')?;
                    let low = self.hex4()?;
                    0x10000 + ((high - 0xD800) << 10) + (low.wrapping_sub(0xDC00) & 0x3FF)
                } else {
                    high
                };
                out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            }
            _ => return Err(format!("bad escape at byte {}", self.at - 1)),
        }
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .ok_or("short \\u escape")?;
        self.at += 4;
        u32::from_str_radix(digits, 16).map_err(|_| format!("bad \\u escape `{digits}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, parse};

    #[test]
    fn a_cargo_artifact_line_reads_back_its_fields() {
        let line = r#"{"reason":"compiler-artifact","target":{"kind":["lib"],"name":"x"},"profile":{"test":true},"executable":"/t/d/x-1","fresh":false,"n":-1.5e3}"#;
        let value = parse(line).unwrap();
        assert_eq!(
            value.get("reason").and_then(Value::as_str),
            Some("compiler-artifact")
        );
        assert_eq!(
            value.at(&["profile", "test"]).and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            value
                .at(&["target", "kind"])
                .and_then(Value::as_array)
                .map(<[Value]>::len),
            Some(1)
        );
        assert_eq!(value.get("n"), Some(&Value::Number(-1500.0)));
        assert_eq!(value.get("missing"), None);
    }

    #[test]
    fn escapes_are_undone_including_a_surrogate_pair() {
        let value = parse(r#"["a\"b\\c\/d\n", "é😀", null, [], {}]"#).unwrap();
        let items = value.as_array().unwrap();
        assert_eq!(items[0].as_str(), Some("a\"b\\c/d\n"));
        assert_eq!(items[1].as_str(), Some("é😀"));
        assert_eq!(items[2], Value::Null);
        assert_eq!(items[3], Value::Array(Vec::new()));
        assert_eq!(items[4], Value::Object(Vec::new()));
    }

    #[test]
    fn text_that_is_not_json_is_refused() {
        for text in ["", "{", "[1,]", "{\"a\" 1}", "tru", "\"open", "1 2"] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}
