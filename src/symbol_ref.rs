use std::{
    fmt::{self, Debug, Display},
    io, cmp::min,
};

use crate::Rule;
use pest::iterators::Pair;

pub struct SymbolError {
    pub error: Box<dyn fmt::Display>,
    pub symbol: SymbolRef,
}

impl SymbolError {
    pub fn display(&self, output: &mut dyn io::Write, file_name: &str, input: &str) {
        writeln!(
            output,
            "{}:{}:{}: Error: {}",
            file_name, self.symbol.line, self.symbol.column, self.error
        )
        .unwrap();

        let line = input.lines().nth(self.symbol.line - 1).unwrap();

        writeln!(output, "\n{}", line).unwrap();

        let len = self.symbol.end - self.symbol.start;
        let len = min(len, line.len() - self.symbol.column);

        writeln!(output, "{}{}\n",
            " ".repeat(self.symbol.column - 1),
            "~".repeat(len),
        ).unwrap();
    }
}

#[derive(Debug, Default, Clone)]
pub struct SymbolRef {
    line: usize,
    column: usize,
    start: usize,
    end: usize,
}

impl SymbolRef {
    pub fn from_pair(pair: &Pair<Rule>) -> SymbolRef {
        SymbolRef {
            line: pair.line_col().0,
            column: pair.line_col().1,
            start: pair.as_span().start(),
            end: pair.as_span().end(),
        }
    }
}

impl Display for SymbolRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}
