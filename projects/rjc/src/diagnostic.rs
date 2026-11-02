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

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub span: Span,
}

impl Error {
    pub fn new(message: impl Into<String>, span: Span) -> Error {
        Error {
            message: message.into(),
            span,
        }
    }

    /// Formats the error as `path:line:col: error: message` followed by the
    /// offending source line with the span underlined.
    pub fn render(&self, path: &str, source: &str) -> String {
        let start = self.span.start.min(source.len());
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = source[start..]
            .find('\n')
            .map_or(source.len(), |i| start + i);
        let line_number = source[..start].matches('\n').count() + 1;
        let column = source[line_start..start].chars().count() + 1;
        let line = &source[line_start..line_end];

        let underline_len = source[start..self.span.end.clamp(start, line_end)]
            .chars()
            .count()
            .max(1);

        let mut out = String::new();
        writeln!(
            out,
            "{path}:{line_number}:{column}: error: {}",
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
        out
    }
}
