use crate::{ast, parser::Rule};

use super::ASTParser;

impl ASTParser for ast::Ident {
    fn parse(pool: &mut ast::Pool, pair: pest::iterators::Pair<super::Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == Rule::ident);

        let ident = ast::Ident{
            id: pool.len(),
            symbol: ast::Symbol::from_pair(&pair),
            name: pair.as_str().to_string(),
        };

        pool.add(ident)
    }
}
