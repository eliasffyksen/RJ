use pest::iterators::Pair;

use crate::{ast::{statement::If, Pool, PoolRef, PoolType, expression::Expression, Block}, parser::{ASTParser, Rule}};

impl ASTParser for If {
    fn parse(
        pool: &mut Pool,
        pair: Pair<Rule>,
    ) -> PoolRef<Self>
    where
        Self: PoolType,
    {
        assert!(pair.as_rule() == Rule::if_stmt);

        let mut condition = None;
        let mut if_block = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_elm => condition = Some(Expression::parse(pool, pair)),
                Rule::block => if_block = Some(Block::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let if_ = If {
            id: pool.len(),
            condition: condition.expect("no condition in if statement"),
            if_block: if_block.expect("no if block in if statement"),
        };

        pool.add(if_)
    }
}