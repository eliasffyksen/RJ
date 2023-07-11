use pest::iterators::Pair;
use rjc_ast::Symbol;

use crate::Rule;

pub trait SymbolFromPair {
    fn from_pair(pair: &Pair<Rule>) -> Symbol;
}

impl SymbolFromPair for Symbol {
    fn from_pair(pair: &Pair<Rule>) -> Symbol {
        Symbol {
            line: pair.line_col().0,
            column: pair.line_col().1,
            start: pair.as_span().start(),
            end: pair.as_span().end(),
        }
    }
}