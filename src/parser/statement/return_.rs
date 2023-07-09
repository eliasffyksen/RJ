use crate::ast;
use crate::parser;

impl parser::ASTParser for ast::statement::Return {
    fn parse(pool: &mut ast::Pool, pair: pest::iterators::Pair<parser::Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == parser::Rule::func_ret);

        let symbol = ast::Symbol::from_pair(&pair);
        let mut expression_list = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                parser::Rule::expr_list => expression_list = Some(ast::expression::ExpressionList::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let return_ = ast::statement::Return{
            symbol,
            expressions: expression_list.expect("no expression list in return statement"),
        };

        pool.add(return_)
    }
}
