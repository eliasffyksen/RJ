use pest::iterators::Pair;

use rjc_ast::{expression::ExpressionList, statement::Return, Pool, PoolRef, PoolType, Symbol};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Return {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType,
    {
        assert!(pair.as_rule() == Rule::func_ret);

        let symbol = Symbol::from_pair(&pair);
        let mut expression_list = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_list => expression_list = Some(ExpressionList::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let return_ = Return {
            id: pool.len(),
            symbol,
            expressions: expression_list.expect("no expression list in return statement"),
        };

        pool.add(return_)
    }
}
