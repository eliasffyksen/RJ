use crate::ast;
use crate::parser;

impl parser::ASTParser for ast::expression::ExpressionList {
    fn parse(pool: &mut ast::Pool, pair: pest::iterators::Pair<parser::Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == parser::Rule::expr_list);

        let symbol = ast::Symbol::from_pair(&pair);
        let mut expressions = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::expr_elm => expressions.push(ast::expression::Expression::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let expression_list = ast::expression::ExpressionList {
            id: pool.len(),
            symbol,
            list: expressions,
        };

        pool.add(expression_list)
    }
}
