use pest::iterators::Pair;

use rjc_ast::{statement::Statement, Block, Pool, PoolRef, PoolType, Symbol};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Block {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType,
    {
        assert!(pair.as_rule() == Rule::block);

        let symbol = Symbol::from_pair(&pair);
        let mut statements = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::stmt => statements.push(Statement::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let block = Block {
            id: pool.len(),
            symbol: symbol,
            statements,
        };

        pool.add(block)
    }
}
