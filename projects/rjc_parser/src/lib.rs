#[macro_use]
extern crate pest_derive;

pub use pest::iterators::Pair;
pub use pest::Parser as ParserTrait;

#[derive(Parser)]
#[grammar = "rj.pest"]
struct Parser;

macro_rules! unexpected_pair {
    ($pair:expr) => {
        panic!("Unexpected pair {}", $pair)
    };
}

mod block;
mod call;
mod expression;
mod function;
mod ident;
mod module;
mod statement;
mod symbol;
mod variable;

pub use module::from_file;
use rjc_ast::{Pool, PoolRef, PoolType};

pub use symbol::SymbolFromPair;

trait ASTParser {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType;
}
