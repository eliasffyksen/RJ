use pest::iterators::Pair;

use rjc_ast::{expression::Literal, AST, ASTRef, ASTType, Symbol};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Literal {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::literal);

        let symbol = Symbol::from_pair(&pair);
        let literal = Literal {
            id: pool.len(),
            symbol,
            value: pair.as_str().to_string(),
        };

        pool.add(literal)
    }
}
