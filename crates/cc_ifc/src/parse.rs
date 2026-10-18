//! Reader for STEP physical files (ISO 10303-21): the header schema and the
//! entity instances of the data section. Malformed input yields an error,
//! never a panic; nesting depth is bounded.

use std::collections::BTreeMap;

/// Deepest list nesting accepted in an attribute value.
const MAX_DEPTH: usize = 32;

/// A parsed attribute value.
#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    Null,
    Derived,
    Int(i64),
    Real(f64),
    Str(String),
    /// Enumeration literal without the dots (`T`, `F`, `ELEMENT`…).
    Enum(String),
    Ref(u32),
    List(Vec<Param>),
    /// Typed value of a SELECT, such as `IFCLABEL('x')`.
    Typed(String, Box<Param>),
}

impl Param {
    pub fn as_ref(&self) -> Option<u32> {
        match self {
            Param::Ref(id) => Some(*id),
            _ => None,
        }
    }

    /// Numeric value (integers widen; typed measures unwrap).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Param::Real(x) => Some(*x),
            Param::Int(i) => Some(*i as f64),
            Param::Typed(_, inner) => inner.as_f64(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Param::Str(s) => Some(s),
            Param::Typed(_, inner) => inner.as_str(),
            _ => None,
        }
    }

    pub fn as_enum(&self) -> Option<&str> {
        match self {
            Param::Enum(e) => Some(e),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Param::Enum(e) if e == "T" => Some(true),
            Param::Enum(e) if e == "F" => Some(false),
            Param::Typed(_, inner) => inner.as_bool(),
            _ => None,
        }
    }

    pub fn as_list(&self) -> &[Param] {
        match self {
            Param::List(items) => items,
            _ => &[],
        }
    }
}

/// An entity instance: upper-case type name and attributes. Complex
/// (multi-type) instances are kept with an empty name.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub name: String,
    pub args: Vec<Param>,
}

impl Entity {
    /// Attribute `i`, or `Null` when missing.
    pub fn arg(&self, i: usize) -> &Param {
        self.args.get(i).unwrap_or(&Param::Null)
    }
}

/// A parsed exchange structure.
#[derive(Debug, Clone, Default)]
pub struct StepFile {
    /// First schema of `FILE_SCHEMA` (e.g. `IFC4`, `IFC2X3`).
    pub schema: String,
    pub entities: BTreeMap<u32, Entity>,
}

impl StepFile {
    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// The instance `param` refers to.
    pub fn deref(&self, param: &Param) -> Option<&Entity> {
        param.as_ref().and_then(|id| self.get(id))
    }

    /// Instances of entity `name` (upper case), in file order.
    pub fn of_type<'a>(&'a self, name: &'a str) -> impl Iterator<Item = (u32, &'a Entity)> + 'a {
        self.entities.iter().filter(move |(_, e)| e.name == name).map(|(id, e)| (*id, e))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset of the problem.
    pub offset: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (byte {})", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

/// Parse a STEP physical file.
pub fn parse(text: &str) -> Result<StepFile, ParseError> {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    p.skip_ws();
    if !p.eat_keyword("ISO-10303-21") {
        return Err(p.error("not a STEP file (missing ISO-10303-21)"));
    }
    p.skip_ws();
    p.expect(b';')?;
    let mut file = StepFile::default();
    // Header and data sections, in order; anything else is skipped.
    loop {
        p.skip_ws();
        if p.at_end() {
            return Err(p.error("missing DATA section"));
        }
        if p.eat_keyword("DATA") {
            p.skip_ws();
            // IFC files may name the data section: DATA('name', ...);
            if p.peek() == Some(b'(') {
                p.value(0)?;
                p.skip_ws();
            }
            p.expect(b';')?;
            break;
        }
        // A header record `NAME(args);` or a section keyword `NAME;`.
        let name = p.keyword();
        if name.is_empty() {
            return Err(p.error("unexpected character in header"));
        }
        p.skip_ws();
        if p.peek() == Some(b'(') {
            let args = p.args()?;
            if name == "FILE_SCHEMA" {
                let first = args.first().map(|a| a.as_list()).and_then(|l| l.first()).and_then(Param::as_str);
                file.schema = first.unwrap_or_default().to_ascii_uppercase();
            }
        }
        p.skip_ws();
        p.expect(b';')?;
    }
    loop {
        p.skip_ws();
        if p.eat_keyword("ENDSEC") {
            break;
        }
        if p.at_end() {
            return Err(p.error("unterminated DATA section"));
        }
        p.expect(b'#')?;
        let id = p.integer()?;
        let id = u32::try_from(id).map_err(|_| p.error("instance name out of range"))?;
        p.skip_ws();
        p.expect(b'=')?;
        p.skip_ws();
        let entity = if p.peek() == Some(b'(') {
            // Complex instance: (A(...) B(...)); kept unnamed.
            p.value(0)?;
            Entity { name: String::new(), args: Vec::new() }
        } else {
            let name = p.keyword();
            if name.is_empty() {
                return Err(p.error("expected an entity name"));
            }
            p.skip_ws();
            Entity { name, args: p.args()? }
        };
        p.skip_ws();
        p.expect(b';')?;
        file.entities.insert(id, entity);
    }
    Ok(file)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> ParseError {
        ParseError { offset: self.i, message: message.to_string() }
    }

    fn at_end(&self) -> bool {
        self.i >= self.s.len()
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    /// Skip white space and `/* … */` comments.
    fn skip_ws(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => self.i += 1,
                Some(b'/') if self.s.get(self.i + 1) == Some(&b'*') => {
                    self.i += 2;
                    while !self.at_end() && !(self.s[self.i] == b'*' && self.s.get(self.i + 1) == Some(&b'/')) {
                        self.i += 1;
                    }
                    self.i = (self.i + 2).min(self.s.len());
                }
                _ => return,
            }
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), ParseError> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", c as char)))
        }
    }

    /// An identifier (letters, digits, `_`, `-`), upper-cased.
    fn keyword(&mut self) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' {
                self.i += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.s[start..self.i]).to_ascii_uppercase()
    }

    /// Consume `word` if it comes next (the trailing `;` is left).
    fn eat_keyword(&mut self, word: &str) -> bool {
        let start = self.i;
        if self.keyword() == word {
            true
        } else {
            self.i = start;
            false
        }
    }

    fn integer(&mut self) -> Result<i64, ParseError> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| self.error("expected an integer"))
    }

    /// A parenthesized attribute list.
    fn args(&mut self) -> Result<Vec<Param>, ParseError> {
        match self.value(0)? {
            Param::List(items) => Ok(items),
            _ => Err(self.error("expected '('")),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Param, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.error("attribute nesting too deep"));
        }
        self.skip_ws();
        let Some(c) = self.peek() else { return Err(self.error("unexpected end of file")) };
        match c {
            b'$' => {
                self.i += 1;
                Ok(Param::Null)
            }
            b'*' => {
                self.i += 1;
                Ok(Param::Derived)
            }
            b'#' => {
                self.i += 1;
                let id = self.integer()?;
                u32::try_from(id).map(Param::Ref).map_err(|_| self.error("instance name out of range"))
            }
            b'\'' => self.string().map(Param::Str),
            b'"' => {
                // Binary: kept as its hex text.
                self.i += 1;
                let start = self.i;
                while self.peek().is_some_and(|c| c != b'"') {
                    self.i += 1;
                }
                let text = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                self.expect(b'"')?;
                Ok(Param::Str(text))
            }
            b'.' => {
                self.i += 1;
                let name = self.keyword();
                self.expect(b'.')?;
                Ok(Param::Enum(name))
            }
            b'(' => {
                self.i += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b')') {
                    self.i += 1;
                    return Ok(Param::List(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b')') => {
                            self.i += 1;
                            return Ok(Param::List(items));
                        }
                        // Complex instances list records without commas.
                        Some(c) if c.is_ascii_alphabetic() => {}
                        _ => return Err(self.error("expected ',' or ')'")),
                    }
                }
            }
            b'+' | b'-' | b'0'..=b'9' => self.number(),
            c if c.is_ascii_alphabetic() => {
                let name = self.keyword();
                self.skip_ws();
                let inner = self.value(depth + 1)?;
                // NAME(x) wraps one value; NAME(a, b) (complex records) keeps the list.
                let inner = match inner {
                    Param::List(mut items) if items.len() == 1 => items.pop().unwrap_or(Param::Null),
                    other => other,
                };
                Ok(Param::Typed(name, Box::new(inner)))
            }
            _ => Err(self.error("unexpected character")),
        }
    }

    fn number(&mut self) -> Result<Param, ParseError> {
        let start = self.i;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.i += 1;
        }
        let mut real = false;
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => self.i += 1,
                b'.' => {
                    real = true;
                    self.i += 1;
                }
                b'E' | b'e' => {
                    real = true;
                    self.i += 1;
                    if matches!(self.peek(), Some(b'+' | b'-')) {
                        self.i += 1;
                    }
                }
                _ => break,
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
        if real {
            // "1." and "1.E-5" are valid STEP reals; Rust wants a digit after the point.
            let fixed = text.replace(".E", ".0E").replace(".e", ".0e");
            let fixed = if fixed.ends_with('.') { format!("{fixed}0") } else { fixed };
            fixed.parse().map(Param::Real).map_err(|_| self.error("invalid real"))
        } else {
            text.parse().map(Param::Int).map_err(|_| self.error("invalid integer"))
        }
    }

    /// A quoted string with its control directives decoded.
    fn string(&mut self) -> Result<String, ParseError> {
        self.expect(b'\'')?;
        let mut raw = Vec::new();
        loop {
            let Some(c) = self.peek() else { return Err(self.error("unterminated string")) };
            self.i += 1;
            if c == b'\'' {
                if self.peek() == Some(b'\'') {
                    self.i += 1;
                    raw.push(b'\'');
                } else {
                    break;
                }
            } else {
                raw.push(c);
            }
        }
        Ok(decode_string(&raw))
    }
}

fn hex_value(bytes: &[u8]) -> Option<u32> {
    u32::from_str_radix(std::str::from_utf8(bytes).ok()?, 16).ok()
}

/// Decode `\X2\…\X0\`, `\X4\…\X0\`, `\X\hh`, `\S\c` and `\\`; other
/// directives (`\P…\`) are dropped. Non-ASCII bytes pass through as UTF-8.
fn decode_string(raw: &[u8]) -> String {
    let mut out = String::new();
    let mut plain: Vec<u8> = Vec::new();
    let flush = |out: &mut String, plain: &mut Vec<u8>| {
        out.push_str(&String::from_utf8_lossy(plain));
        plain.clear();
    };
    let mut i = 0;
    while i < raw.len() {
        let rest = &raw[i..];
        if rest.starts_with(b"\\\\") {
            plain.push(b'\\');
            i += 2;
        } else if rest.starts_with(b"\\X2\\") || rest.starts_with(b"\\X4\\") {
            flush(&mut out, &mut plain);
            let width = if rest[2] == b'2' { 4 } else { 8 };
            let body_start = i + 4;
            let end = raw[body_start..].windows(4).position(|w| w == b"\\X0\\").map(|p| body_start + p);
            let body = &raw[body_start..end.unwrap_or(raw.len())];
            if width == 4 {
                let units: Vec<u16> = body.chunks(4).filter_map(hex_value).map(|v| v as u16).collect();
                out.push_str(&String::from_utf16_lossy(&units));
            } else {
                out.extend(body.chunks(8).filter_map(hex_value).filter_map(char::from_u32));
            }
            i = end.map_or(raw.len(), |e| e + 4);
        } else if rest.starts_with(b"\\X\\") && rest.len() >= 5 {
            flush(&mut out, &mut plain);
            if let Some(c) = hex_value(&rest[3..5]).and_then(char::from_u32) {
                out.push(c);
            }
            i += 5;
        } else if rest.starts_with(b"\\S\\") && rest.len() >= 4 {
            flush(&mut out, &mut plain);
            if let Some(c) = char::from_u32(rest[3] as u32 + 128) {
                out.push(c);
            }
            i += 4;
        } else if rest.starts_with(b"\\P") && rest.len() >= 4 && rest[3] == b'\\' {
            i += 4;
        } else {
            plain.push(raw[i]);
            i += 1;
        }
    }
    flush(&mut out, &mut plain);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('ViewDefinition [X]'),'2;1');
FILE_NAME('a.ifc','2026-01-01T00:00:00',(''),(''),'pp','os','');
FILE_SCHEMA(('IFC2X3'));
ENDSEC;
DATA;
/* a comment */
#1=IFCCARTESIANPOINT((0.,1.5,-2.E-3));
#2 = IFCWALL('2O2Fr$t4X7Zf8NOew3FLOH',$,'It''s T\\X2\\00E9\\X0\\rreo',*,.T.,(#1,#1),IFCLABEL('x'),12);
#3=(IFCA(1)IFCB(2));
#4=IFCX('\\X\\E9t\\S\\i','caf\u{e9}');
ENDSEC;
END-ISO-10303-21;
";

    #[test]
    fn parses_instances_and_values() {
        let f = parse(SAMPLE).unwrap();
        assert_eq!(f.schema, "IFC2X3");
        assert_eq!(f.entities.len(), 4);
        let p = f.get(1).unwrap();
        assert_eq!(p.name, "IFCCARTESIANPOINT");
        let coords: Vec<f64> = p.arg(0).as_list().iter().filter_map(Param::as_f64).collect();
        assert_eq!(coords, vec![0.0, 1.5, -0.002]);
        let w = f.get(2).unwrap();
        assert_eq!(w.arg(1), &Param::Null);
        assert_eq!(w.arg(2).as_str(), Some("It's Térreo"));
        assert_eq!(w.arg(3), &Param::Derived);
        assert_eq!(w.arg(4).as_bool(), Some(true));
        assert_eq!(w.arg(5).as_list().len(), 2);
        assert_eq!(w.arg(6), &Param::Typed("IFCLABEL".into(), Box::new(Param::Str("x".into()))));
        assert_eq!(w.arg(7).as_f64(), Some(12.0));
        assert_eq!(w.arg(99), &Param::Null);
        assert_eq!(f.get(3).unwrap().name, "");
        assert_eq!(f.get(4).unwrap().arg(0).as_str(), Some("\u{e9}t\u{e9}"));
        assert_eq!(f.get(4).unwrap().arg(1).as_str(), Some("caf\u{e9}"));
        assert_eq!(f.of_type("IFCWALL").count(), 1);
    }

    #[test]
    fn round_trips_the_writer() {
        use crate::step::{StepWriter, Value, reals};
        let mut w = StepWriter::new();
        let p = w.add("IFCCARTESIANPOINT", crate::args![reals(&[0.1, 2.0, 1e-7])]);
        w.add("IFCTHING", crate::args!["Elevação 🏠", Value::Enum("BOTH"), p, Value::Null]);
        let header = crate::step::Header {
            description: vec![],
            name: String::new(),
            time_stamp: String::new(),
            author: vec![],
            organization: vec![],
            preprocessor_version: String::new(),
            originating_system: String::new(),
            authorization: String::new(),
            schema: "IFC4".into(),
        };
        let f = parse(&w.finish(&header)).unwrap();
        assert_eq!(f.schema, "IFC4");
        let t = f.get(2).unwrap();
        assert_eq!(t.arg(0).as_str(), Some("Elevação 🏠"));
        assert_eq!(t.arg(1).as_enum(), Some("BOTH"));
        assert_eq!(t.arg(2).as_ref(), Some(1));
        let x: Vec<f64> = f.get(1).unwrap().arg(0).as_list().iter().filter_map(Param::as_f64).collect();
        assert_eq!(x, vec![0.1, 2.0, 1e-7]);
    }

    #[test]
    fn malformed_input_is_an_error() {
        for bad in [
            "",
            "hello",
            "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=IFCX(1,2;\nENDSEC;",
            "ISO-10303-21;\nDATA;\n#1=IFCX('open);\n",
            "ISO-10303-21;\nDATA;\n#1=IFCX(1);\n",
            "ISO-10303-21;\nDATA;\n#99999999999=IFCX(1);\nENDSEC;",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        let deep = format!("ISO-10303-21;\nDATA;\n#1=IFCX({}{});\nENDSEC;", "(".repeat(100), ")".repeat(100));
        assert!(parse(&deep).is_err());
    }
}
