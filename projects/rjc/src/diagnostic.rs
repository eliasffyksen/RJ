//! Compiler diagnostics: errors with stable codes, rendered for people or as
//! JSON for tools.

use std::fmt::Write as _;

/// Byte range into the source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }
}

/// Stable error codes. Codes are never reused or renumbered; `rjc --explain
/// CODE` prints the explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    // Syntax
    Syntax,
    IntTooLarge,
    ArrayLenTooLarge,
    UnusedValue,
    // Names
    UnknownVariable,
    UnknownFunction,
    UnknownType,
    UnknownField,
    UnknownMethod,
    Duplicate,
    WrongKindOfName,
    ThisOutsideMethod,
    MethodOnBuiltin,
    // Types
    Mismatch,
    Count,
    BadOperands,
    NotBool,
    BadIndexType,
    NotIndexable,
    BadConversion,
    MultipleValues,
    EmptyArray,
    StructFields,
    // Flow
    UseBeforeAssign,
    MissingReturn,
    Unreachable,
    AssignedTwice,
    IndexOutOfBounds,
    // Program
    Main,
    InfiniteType,
}

impl Code {
    pub const ALL: &'static [Code] = &[
        Code::Syntax,
        Code::IntTooLarge,
        Code::ArrayLenTooLarge,
        Code::UnusedValue,
        Code::UnknownVariable,
        Code::UnknownFunction,
        Code::UnknownType,
        Code::UnknownField,
        Code::UnknownMethod,
        Code::Duplicate,
        Code::WrongKindOfName,
        Code::ThisOutsideMethod,
        Code::MethodOnBuiltin,
        Code::Mismatch,
        Code::Count,
        Code::BadOperands,
        Code::NotBool,
        Code::BadIndexType,
        Code::NotIndexable,
        Code::BadConversion,
        Code::MultipleValues,
        Code::EmptyArray,
        Code::StructFields,
        Code::UseBeforeAssign,
        Code::MissingReturn,
        Code::Unreachable,
        Code::AssignedTwice,
        Code::IndexOutOfBounds,
        Code::Main,
        Code::InfiniteType,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Code::Syntax => "E0001",
            Code::IntTooLarge => "E0002",
            Code::ArrayLenTooLarge => "E0003",
            Code::UnusedValue => "E0004",
            Code::UnknownVariable => "E0100",
            Code::UnknownFunction => "E0101",
            Code::UnknownType => "E0102",
            Code::UnknownField => "E0103",
            Code::UnknownMethod => "E0104",
            Code::Duplicate => "E0105",
            Code::WrongKindOfName => "E0106",
            Code::ThisOutsideMethod => "E0107",
            Code::MethodOnBuiltin => "E0108",
            Code::Mismatch => "E0200",
            Code::Count => "E0201",
            Code::BadOperands => "E0202",
            Code::NotBool => "E0203",
            Code::BadIndexType => "E0204",
            Code::NotIndexable => "E0205",
            Code::BadConversion => "E0206",
            Code::MultipleValues => "E0207",
            Code::EmptyArray => "E0208",
            Code::StructFields => "E0209",
            Code::UseBeforeAssign => "E0300",
            Code::MissingReturn => "E0301",
            Code::Unreachable => "E0302",
            Code::AssignedTwice => "E0303",
            Code::IndexOutOfBounds => "E0304",
            Code::Main => "E0400",
            Code::InfiniteType => "E0401",
        }
    }

    pub fn from_id(id: &str) -> Option<Code> {
        Code::ALL.iter().copied().find(|code| code.id().eq_ignore_ascii_case(id))
    }

    pub fn title(self) -> &'static str {
        match self {
            Code::Syntax => "syntax error",
            Code::IntTooLarge => "integer literal too large",
            Code::ArrayLenTooLarge => "array length too large",
            Code::UnusedValue => "unused value",
            Code::UnknownVariable => "unknown variable",
            Code::UnknownFunction => "unknown function",
            Code::UnknownType => "unknown type",
            Code::UnknownField => "unknown field",
            Code::UnknownMethod => "unknown method",
            Code::Duplicate => "defined more than once",
            Code::WrongKindOfName => "wrong kind of name",
            Code::ThisOutsideMethod => "`this` outside a method",
            Code::MethodOnBuiltin => "method on a built-in type",
            Code::Mismatch => "type mismatch",
            Code::Count => "wrong number of values",
            Code::BadOperands => "invalid operands",
            Code::NotBool => "condition is not bool",
            Code::BadIndexType => "index is not i32",
            Code::NotIndexable => "value cannot be indexed",
            Code::BadConversion => "invalid conversion",
            Code::MultipleValues => "several values where one is expected",
            Code::EmptyArray => "empty array literal",
            Code::StructFields => "wrong struct fields",
            Code::UseBeforeAssign => "used before assigned",
            Code::MissingReturn => "missing return",
            Code::Unreachable => "unreachable statement",
            Code::AssignedTwice => "assigned more than once",
            Code::IndexOutOfBounds => "index out of bounds",
            Code::Main => "invalid `main`",
            Code::InfiniteType => "type contains itself",
        }
    }

    pub fn explanation(self) -> &'static str {
        match self {
            Code::Syntax => {
                "The source doesn't follow RJ's grammar. The message says what was \
                 expected at the marked position."
            }
            Code::IntTooLarge => {
                "Integer literals must fit in an i32 (-2147483648 to 2147483647)."
            }
            Code::ArrayLenTooLarge => "Array lengths must fit in 32 bits.",
            Code::UnusedValue => {
                "Only calls can be used as statements. A plain value like `x.y` on \
                 its own line does nothing; assign it or remove it."
            }
            Code::UnknownVariable => {
                "No variable with this name is declared in scope. Declare it first \
                 with `name: type`."
            }
            Code::UnknownFunction => "No function with this name is defined.",
            Code::UnknownType => {
                "No type with this name is defined. Built-in types are `i32`, \
                 `bool` and arrays `[T; N]`; others are declared with `type`."
            }
            Code::UnknownField => "The struct has no field with this name.",
            Code::UnknownMethod => {
                "The type has no method with this name. Methods are declared as \
                 `fn Type.name(...)`."
            }
            Code::Duplicate => {
                "Each function, type, method, field and variable (within one scope) \
                 must have a unique name."
            }
            Code::WrongKindOfName => {
                "The name refers to something else than it is used as, for example \
                 calling a variable or reading a function as a value."
            }
            Code::ThisOutsideMethod => {
                "`this` is the receiver of a method and only exists inside \
                 `fn Type.name(...)`."
            }
            Code::MethodOnBuiltin => {
                "Methods can only be declared on types declared with `type`. To add \
                 methods to an i32, declare `type MyInt i32`."
            }
            Code::Mismatch => {
                "The value has a different type than expected. Named types are \
                 distinct from what they name: convert with `Name(value)` or \
                 `i32(value)`. Literals adapt to named types automatically."
            }
            Code::Count => {
                "A call, assignment or return has a different number of values than \
                 expected. Calls returning several values provide all of them."
            }
            Code::BadOperands => {
                "`+` and `-` take two i32 values; `<`, `<=`, `>` and `>=` compare \
                 i32 values; `==` and `!=` compare two values of the same i32 or \
                 bool type. Arrays and structs can't be compared."
            }
            Code::NotBool => "Conditions of `if` and `while` must be bool.",
            Code::BadIndexType => "Array indexes must be i32 (or a named type of i32).",
            Code::NotIndexable => "Only arrays can be indexed with `[...]`.",
            Code::BadConversion => {
                "`Name(value)` converts between types with the same underlying type, \
                 or between structs with the same fields. It takes exactly one value."
            }
            Code::MultipleValues => {
                "A call returning several values can only be used where a list of \
                 values is expected: as arguments, on the right of an assignment, or \
                 in `return`. Assign the values to variables first."
            }
            Code::EmptyArray => {
                "The element type of `[]` can't be inferred. Use a repeat such as \
                 `[0; 0]`."
            }
            Code::StructFields => {
                "Struct values are built with every field named exactly once: \
                 `Point(x: 1, y: 2)`."
            }
            Code::UseBeforeAssign => {
                "Variables must be assigned on every path before they are read. A \
                 loop body may run zero times, so assignments inside it don't count \
                 afterwards. Assigning a part (`a[i] = v`) also reads the rest."
            }
            Code::MissingReturn => {
                "A function with return values must `return` on every path. Loops \
                 never count as returning."
            }
            Code::Unreachable => "Statements after a `return` in the same block never run.",
            Code::AssignedTwice => {
                "One assignment can't write the same variable twice, including a \
                 variable and a part of it (`a, a[0] = ...`)."
            }
            Code::IndexOutOfBounds => {
                "A constant index is outside the array. Valid indexes are 0 to \
                 length - 1."
            }
            Code::Main => "Programs need a `fn main(): i32`; its result is the exit code.",
            Code::InfiniteType => {
                "A type can't contain itself by value, directly or through arrays \
                 and other types, because it would have infinite size. Refer to \
                 other values by index instead (`type NodeId i32`)."
            }
        }
    }
}

#[derive(Debug)]
pub struct Error {
    pub code: Code,
    pub message: String,
    pub span: Span,
    /// A suggestion for fixing the error.
    pub help: Option<String>,
    /// For type mismatches.
    pub expected: Option<String>,
    pub found: Option<String>,
}

impl Error {
    pub fn new(code: Code, message: impl Into<String>, span: Span) -> Error {
        Error {
            code,
            message: message.into(),
            span,
            help: None,
            expected: None,
            found: None,
        }
    }

    pub fn help(&mut self, help: impl Into<String>) -> &mut Self {
        self.help = Some(help.into());
        self
    }

    pub fn types(&mut self, expected: impl ToString, found: impl ToString) -> &mut Self {
        self.expected = Some(expected.to_string());
        self.found = Some(found.to_string());
        self
    }

    /// Formats the error as `path:line:col: error[CODE]: message` followed by
    /// the offending source line with the span underlined and any help.
    pub fn render(&self, path: &str, source: &str) -> String {
        let start = self.span.start.min(source.len());
        let (line_number, column) = position(source, start);
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = source[start..]
            .find('\n')
            .map_or(source.len(), |i| start + i);
        let line = &source[line_start..line_end];

        let underline_len = source[start..self.span.end.clamp(start, line_end)]
            .chars()
            .count()
            .max(1);

        let mut out = String::new();
        writeln!(
            out,
            "{path}:{line_number}:{column}: error[{}]: {}",
            self.code.id(),
            self.message
        )
        .unwrap();
        writeln!(out, "  {line}").unwrap();
        writeln!(
            out,
            "  {}{}",
            " ".repeat(column - 1),
            "~".repeat(underline_len)
        )
        .unwrap();
        if let Some(help) = &self.help {
            writeln!(out, "  help: {help}").unwrap();
        }
        out
    }

    /// One JSON object describing the error.
    pub fn to_json(&self, path: &str, source: &str) -> String {
        let start = self.span.start.min(source.len());
        let end = self.span.end.clamp(start, source.len());
        let (line, column) = position(source, start);
        let (end_line, end_column) = position(source, end);

        let mut fields = vec![
            format!("\"code\":{}", json_string(self.code.id())),
            format!("\"title\":{}", json_string(self.code.title())),
            "\"severity\":\"error\"".to_string(),
            format!("\"message\":{}", json_string(&self.message)),
            format!("\"file\":{}", json_string(path)),
            format!(
                "\"span\":{{\"start\":{start},\"end\":{end},\"line\":{line},\"column\":{column},\
                 \"end_line\":{end_line},\"end_column\":{end_column}}}"
            ),
        ];

        for (key, value) in [
            ("help", &self.help),
            ("expected", &self.expected),
            ("found", &self.found),
        ] {
            if let Some(value) = value {
                fields.push(format!("\"{key}\":{}", json_string(value)));
            }
        }

        format!("{{{}}}", fields.join(","))
    }
}

/// `{"diagnostics":[...]}` for a list of errors.
pub fn to_json(errors: &[Error], path: &str, source: &str) -> String {
    let errors = errors.iter().map(|e| e.to_json(path, source)).collect::<Vec<_>>();
    format!("{{\"diagnostics\":[{}]}}", errors.join(","))
}

/// 1-based line and column (in characters) of a byte offset.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let line = source[..offset].matches('\n').count() + 1;
    let column = source[line_start..offset].chars().count() + 1;
    (line, column)
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The candidate closest to `name`, if it's close enough to be a likely typo.
pub fn suggest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).clamp(1, 3);

    candidates
        .into_iter()
        .filter(|candidate| *candidate != name)
        .map(|candidate| (edit_distance(name, candidate), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, candidate)| (*distance, *candidate))
        .map(|(_, candidate)| candidate)
}

/// Edit distance where swapping two neighbouring characters counts as one
/// edit, the most common typo.
fn edit_distance(a: &str, b: &str) -> usize {
    let a = a.chars().collect::<Vec<_>>();
    let b = b.chars().collect::<Vec<_>>();
    let mut d = vec![vec![0; b.len() + 1]; a.len() + 1];

    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);

            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }

    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_explained() {
        let mut ids = Code::ALL.iter().map(|code| code.id()).collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Code::ALL.len());

        for code in Code::ALL {
            assert_eq!(Code::from_id(code.id()), Some(*code));
            assert!(!code.explanation().is_empty());
        }
    }

    #[test]
    fn suggests_close_names() {
        assert_eq!(suggest("lenght", ["length", "width"]), Some("length"));
        assert_eq!(suggest("bmup", ["bump", "add"]), Some("bump"));
        assert_eq!(suggest("x", ["y", "total"]), Some("y"));
        assert_eq!(suggest("count", ["width", "total"]), None);
    }

    #[test]
    fn json_escapes_strings() {
        let mut error = Error::new(Code::Mismatch, "expected \"a\"\n", Span::new(4, 5));
        error.types("NodeId", "i32");

        let json = error.to_json("f.rj", "abc\nde");
        assert!(json.contains(r#""message":"expected \"a\"\n""#), "{json}");
        assert!(json.contains(r#""line":2,"column":1"#), "{json}");
        assert!(json.contains(r#""expected":"NodeId","found":"i32""#), "{json}");
    }
}
