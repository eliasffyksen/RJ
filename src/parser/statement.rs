use super::*;
use crate::ast;

impl ASTParser for ast::Statement {
    fn parse(pool: &mut ast::Pool, pair: Pair<Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == Rule::stmt);

        let pair = pair.into_inner().next().expect("no value in statement");

        let statement = match pair.as_rule() {
            Rule::var_decl => ast::Statement::Variable(ast::Variable::parse(pool, pair)),

            _ => unexpected_pair!(pair),
        };

        pool.add(statement)
    }
}
