use std::fmt;

use crate::parser;

#[derive(Debug, Default, Clone)]
pub struct Symbol {
    pub line: usize,
    pub column: usize,
    pub start: usize,
    pub end: usize,
}

impl Symbol {
    pub fn from_pair(pair: &parser::Pair<parser::Rule>) -> Symbol {
        Symbol {
            line: pair.line_col().0,
            column: pair.line_col().1,
            start: pair.as_span().start(),
            end: pair.as_span().end(),
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "_{}:{}", self.line, self.column)
    }
}
