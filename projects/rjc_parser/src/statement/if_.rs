use pest::iterators::Pair;

use rjc_ast::{expression::Expression, statement::If, ASTRef, ASTType, Block, AST};

use crate::{ASTParser, Rule};

impl ASTParser for If {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::if_stmt);

        let mut condition = None;
        let mut blocks = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_elm => condition = Some(Expression::parse(pool, pair)),
                Rule::block => blocks.push(Block::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let mut blocks = blocks.into_iter();

        let if_block = blocks.next().expect("no if block in if statement");
        let else_block = blocks.next();

        let if_ = If {
            id: pool.len(),
            condition: condition.expect("no condition in if statement"),
            if_block,
            else_block,
        };

        pool.add(if_)
    }
}
