use pest::iterators::Pair;

use rjc_ast::{
    expression::{Expression, ExpressionList},
    Pool, PoolRef, PoolType, Symbol,
};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for ExpressionList {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType,
    {
        assert!(pair.as_rule() == Rule::expr_list);

        let symbol = Symbol::from_pair(&pair);
        let mut expressions = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_elm => expressions.push(Expression::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let expression_list = ExpressionList {
            id: pool.len(),
            symbol,
            list: expressions,
        };

        pool.add(expression_list)
    }
}
