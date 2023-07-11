use pest::iterators::Pair;

use rjc_ast::{Ident, AST, ASTRef, ASTType, Symbol};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Ident {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::ident);

        let ident = Ident {
            id: pool.len(),
            symbol: Symbol::from_pair(&pair),
            name: pair.as_str().to_string(),
        };

        pool.add(ident)
    }
}
