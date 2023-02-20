pub use crate::pest::iterators::Pair;
pub use crate::pest::Parser as ParserTrait;

#[derive(Parser)]
#[grammar = "parser/rj.pest"]
pub struct Parser;
