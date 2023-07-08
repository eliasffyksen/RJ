use crate::parser;

#[derive(Debug)]
pub struct Symbol {
    line: usize,
    column: usize,
    start: usize,
    end: usize,
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
