pub use crate::pest::iterators::Pair;
pub use crate::pest::Parser as ParserTrait;

#[derive(Parser)]
#[grammar = "parser/rj.pest"]
struct Parser;

macro_rules! unexpected_pair {
    ($pair:expr) => {
        panic!("Unexpected pair {}", $pair)
    };
}

mod module;
mod function;
mod block;
mod variable;
mod statement;
mod expression;
mod ident;

pub use module::from_file;

use crate::ast;

trait ASTParser {
    fn parse(pool: &mut ast::Pool, pair: Pair<Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType;
}
