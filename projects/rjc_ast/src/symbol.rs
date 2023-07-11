use std::fmt::Display;

#[derive(Debug, Hash)]
pub struct Symbol {
    pub line: usize,
    pub column: usize,
    pub start: usize,
    pub end: usize,
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)?;
        Ok(())
    }
}
