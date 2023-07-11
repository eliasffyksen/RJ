use pest::iterators::Pair;

use rjc_ast::{expression::Expression, statement::If, Block, AST, ASTRef, ASTType};

use crate::{ASTParser, Rule};

impl ASTParser for If {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
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
