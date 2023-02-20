use std::cmp::min;
use std::fmt;
use std::io;

use crate::ast;

pub struct Error {
    pub error: Box<dyn fmt::Display>,
    pub symbol: ast::Symbol,
}

impl Error {
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
        let len = min(len, line.len() - self.symbol.column + 1);

        writeln!(
            output,
            "{}{}\n",
            " ".repeat(self.symbol.column - 1),
            "~".repeat(len),
        )
        .unwrap();
    }
}
