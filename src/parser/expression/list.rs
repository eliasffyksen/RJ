use crate::ast;
use crate::parser;

impl parser::ASTParser for ast::expression::List {
    fn parse(pool: &mut ast::Pool, pair: pest::iterators::Pair<parser::Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == parser::Rule::expr_list);

        let expressions = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                _ => unexpected_pair!(pair),
            }
        }

        let expression_list = ast::expression::List{
            list: expressions,
        };

        pool.add(expression_list)
    }
}
