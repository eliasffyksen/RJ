use crate::ast;
use crate::parser;

impl parser::ASTParser for ast::expression::Literal {
    fn parse(pool: &mut ast::Pool, pair: pest::iterators::Pair<parser::Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == parser::Rule::literal);

        let symbol = ast::Symbol::from_pair(&pair);
        let literal = ast::expression::Literal{
            symbol,
            value: pair.as_str().to_string(),
        };

        pool.add(literal)
    }
}
