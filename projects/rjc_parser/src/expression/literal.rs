use pest::iterators::Pair;

use rjc_ast::{expression::Literal, Pool, PoolRef, PoolType, Symbol};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Literal {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType,
    {
        assert!(pair.as_rule() == Rule::literal);

        let symbol = Symbol::from_pair(&pair);
        let literal = Literal {
            id: pool.len(),
            symbol,
            value: pair.as_str().to_string(),
        };

        pool.add(literal)
    }
}
