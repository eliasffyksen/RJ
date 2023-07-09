use super::*;
use crate::ast;

impl ASTParser for ast::Block {
    fn parse(pool: &mut ast::Pool, pair: Pair<Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == Rule::block);

        let symbol = ast::Symbol::from_pair(&pair);
        let mut statements = vec![];

        for pair in pair.into_inner() {
            statements.push(ast::statement::Statement::parse(pool, pair))
        }

        let block = ast::Block {
            id: pool.len(),
            symbol: symbol,
            statements,
        };

        pool.add(block)
    }
}
