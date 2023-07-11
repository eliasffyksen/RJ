use pest::iterators::Pair;

use rjc_ast::{expression::ExpressionList, statement::Assignment, Ident, Pool, PoolRef, PoolType};

use crate::{ASTParser, Rule};

impl ASTParser for Assignment {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType,
    {
        assert!(pair.as_rule() == Rule::assign);

        let mut targets = vec![];
        let mut expressions = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => targets.push(Ident::parse(pool, pair)),
                Rule::expr_list => expressions = Some(ExpressionList::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let assignment = Assignment {
            id: pool.len(),
            targets,
            expressions: expressions.expect("no expression list in assign pair"),
        };

        pool.add(assignment)
    }
}
