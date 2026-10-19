//! Minimal writer for STEP physical files (ISO 10303-21).

use std::fmt::Write;

/// Instance name of an entity in the data section (`#n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub u32);

/// An attribute value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Unset optional attribute (`$`).
    Null,
    /// Attribute redeclared as derived in a subtype (`*`).
    Derived,
    Bool(bool),
    Int(i64),
    Real(f64),
    Str(String),
    /// Enumeration literal, written as `.NAME.`.
    Enum(&'static str),
    /// Enumeration literal read from data; anything but `A-Z`, `0-9` and `_`
    /// is written as `NOTDEFINED`.
    EnumName(String),
    Ref(Id),
    List(Vec<Value>),
    /// Typed value for SELECT attributes, such as `IFCLABEL('x')`.
    Typed(&'static str, Box<Value>),
}

impl From<f64> for Value {
    fn from(x: f64) -> Self {
        Value::Real(x)
    }
}

impl From<i64> for Value {
    fn from(x: i64) -> Self {
        Value::Int(x)
    }
}

impl From<bool> for Value {
    fn from(x: bool) -> Self {
        Value::Bool(x)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<Id> for Value {
    fn from(id: Id) -> Self {
        Value::Ref(id)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(x: Option<T>) -> Self {
        x.map_or(Value::Null, Into::into)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(items: Vec<T>) -> Self {
        Value::List(items.into_iter().map(Into::into).collect())
    }
}

/// A string attribute, or `$` when empty.
pub fn opt_str(s: &str) -> Value {
    if s.is_empty() { Value::Null } else { Value::Str(s.to_string()) }
}

/// A list of reals, for coordinates and direction ratios.
pub fn reals(xs: &[f64]) -> Value {
    Value::List(xs.iter().map(|&x| Value::Real(x)).collect())
}

/// Builds the attribute list of an entity instance.
#[macro_export]
macro_rules! args {
    ($($v:expr),* $(,)?) => { vec![$($crate::step::Value::from($v)),*] };
}

/// File header fields (`FILE_DESCRIPTION`, `FILE_NAME`, `FILE_SCHEMA`).
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub description: Vec<String>,
    pub name: String,
    pub time_stamp: String,
    pub author: Vec<String>,
    pub organization: Vec<String>,
    pub preprocessor_version: String,
    pub originating_system: String,
    pub authorization: String,
    pub schema: String,
}

/// Accumulates entity instances and serializes the exchange structure.
#[derive(Debug, Default)]
pub struct StepWriter {
    data: String,
    count: u32,
}

impl StepWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an instance of `entity` (upper case, e.g. `IFCWALL`) and return its name.
    pub fn add(&mut self, entity: &str, args: Vec<Value>) -> Id {
        self.count += 1;
        let id = Id(self.count);
        let _ = write!(self.data, "#{}={}(", id.0, entity);
        write_list(&mut self.data, &args);
        self.data.push_str(");\n");
        id
    }

    pub fn len(&self) -> usize {
        self.count as usize
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn finish(self, header: &Header) -> String {
        let strings = |items: &[String]| Value::List(items.iter().map(|s| Value::Str(s.clone())).collect());
        let mut out = String::with_capacity(self.data.len() + 512);
        out.push_str("ISO-10303-21;\nHEADER;\n");
        let mut line = |name: &str, args: Vec<Value>| {
            out.push_str(name);
            out.push('(');
            write_list(&mut out, &args);
            out.push_str(");\n");
        };
        line("FILE_DESCRIPTION", vec![strings(&header.description), "2;1".into()]);
        line(
            "FILE_NAME",
            vec![
                header.name.clone().into(),
                header.time_stamp.clone().into(),
                strings(&header.author),
                strings(&header.organization),
                header.preprocessor_version.clone().into(),
                header.originating_system.clone().into(),
                header.authorization.clone().into(),
            ],
        );
        line("FILE_SCHEMA", vec![strings(std::slice::from_ref(&header.schema))]);
        out.push_str("ENDSEC;\nDATA;\n");
        out.push_str(&self.data);
        out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        out
    }
}

fn write_list(out: &mut String, items: &[Value]) {
    for (i, v) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_value(out, v);
    }
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push('$'),
        Value::Derived => out.push('*'),
        Value::Bool(b) => out.push_str(if *b { ".T." } else { ".F." }),
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::Real(x) => out.push_str(&real(*x)),
        Value::Str(s) => encode_string(out, s),
        Value::Enum(e) => {
            let _ = write!(out, ".{e}.");
        }
        Value::EnumName(e) => {
            let valid = !e.is_empty() && e.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            let _ = write!(out, ".{}.", if valid { e.as_str() } else { "NOTDEFINED" });
        }
        Value::Ref(id) => {
            let _ = write!(out, "#{}", id.0);
        }
        Value::List(items) => {
            out.push('(');
            write_list(out, items);
            out.push(')');
        }
        Value::Typed(name, inner) => {
            out.push_str(name);
            out.push('(');
            write_value(out, inner);
            out.push(')');
        }
    }
}

/// A real literal: always with a decimal point, never in exponent form.
fn real(x: f64) -> String {
    // Round away floating point noise below the export precision.
    let x = (x * 1e9).round() / 1e9;
    let x = if x == 0.0 || !x.is_finite() { 0.0 } else { x };
    let mut s = format!("{x}");
    if !s.contains('.') {
        s.push('.');
    }
    s
}

/// A quoted string: `'` and `\` are escaped, characters outside printable
/// ASCII are written as UTF-16 code units in `\X2\…\X0\` blocks.
fn encode_string(out: &mut String, s: &str) {
    out.push('\'');
    let mut wide = String::new();
    let flush = |out: &mut String, wide: &mut String| {
        if !wide.is_empty() {
            out.push_str("\\X2\\");
            out.push_str(wide);
            out.push_str("\\X0\\");
            wide.clear();
        }
    };
    for c in s.chars() {
        if (' '..='~').contains(&c) {
            flush(out, &mut wide);
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(c),
            }
        } else {
            let mut units = [0u16; 2];
            for u in c.encode_utf16(&mut units) {
                let _ = write!(wide, "{u:04X}");
            }
        }
    }
    flush(out, &mut wide);
    out.push('\'');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(v: Value) -> String {
        let mut s = String::new();
        write_value(&mut s, &v);
        s
    }

    #[test]
    fn reals_have_a_decimal_point() {
        assert_eq!(real(1.0), "1.");
        assert_eq!(real(-0.0), "0.");
        assert_eq!(real(0.15), "0.15");
        assert_eq!(real(0.1 + 0.2), "0.3");
        assert_eq!(real(0.00001), "0.00001");
        assert_eq!(real(-2.5), "-2.5");
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(value("It's a \\ test".into()), "'It''s a \\\\ test'");
        assert_eq!(value("Térreo".into()), "'T\\X2\\00E9\\X0\\rreo'");
        assert_eq!(value("Elevação".into()), "'Eleva\\X2\\00E700E3\\X0\\o'");
        assert_eq!(value("🏠".into()), "'\\X2\\D83CDFE0\\X0\\'");
    }

    #[test]
    fn writes_instances_and_header() {
        let mut w = StepWriter::new();
        let p = w.add("IFCCARTESIANPOINT", args![reals(&[0.0, 1.0, 2.5])]);
        let ty = Value::Typed("IFCLABEL", Box::new("x".into()));
        w.add("IFCTEST", args![p, Value::Null, Value::Derived, Value::Enum("BOTH"), true, 3i64, ty, vec![p, p]]);
        let header = Header {
            description: vec!["ViewDefinition [X]".into()],
            name: "a.ifc".into(),
            time_stamp: "2026-01-01T00:00:00".into(),
            author: vec![String::new()],
            organization: vec![String::new()],
            preprocessor_version: "pp".into(),
            originating_system: "os".into(),
            authorization: String::new(),
            schema: "IFC4".into(),
        };
        let text = w.finish(&header);
        assert!(text.starts_with("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [X]'),'2;1');\n"));
        assert!(text.contains("FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n"));
        assert!(text.contains("#1=IFCCARTESIANPOINT((0.,1.,2.5));\n"));
        assert!(text.contains("#2=IFCTEST(#1,$,*,.BOTH.,.T.,3,IFCLABEL('x'),(#1,#1));\n"));
        assert!(text.ends_with("ENDSEC;\nEND-ISO-10303-21;\n"));
    }
}
