use crate::ast;
use crate::parser;

impl parser::ASTParser for ast::statement::Statement {
    fn parse(pool: &mut ast::Pool, pair: parser::Pair<parser::Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == parser::Rule::stmt);

        let pair = pair.into_inner().next().expect("no value in statement");

        let statement = match pair.as_rule() {
            parser::Rule::var_decl => ast::statement::Statement::VariableDeclaration(
                (ast::Variable::parse(pool, pair), pool.len())
            ),
            parser::Rule::func_ret => ast::statement::Statement::Return(
                (ast::statement::Return::parse(pool, pair), pool.len())
            ),
            parser::Rule::assign => ast::statement::Statement::Assignment(
                (ast::statement::Assignment::parse(pool, pair), pool.len()),
            ),

            _ => unexpected_pair!(pair),
        };

        pool.add(statement)
    }
}
