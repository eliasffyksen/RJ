use super::*;
use crate::ast;

impl ASTParser for ast::Block {
    fn parse(pool: &mut ast::Pool, pair: Pair<Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == Rule::block);

        let mut statements = vec![];

        for pair in pair.into_inner() {
            statements.push(ast::statement::Statement::parse(pool, pair))
        }

        let block = ast::Block {
            statements,
        };

        pool.add(block)
    }
}
