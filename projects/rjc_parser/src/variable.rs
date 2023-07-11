use pest::iterators::Pair;

use rjc_ast::{Ident, AST, ASTRef, ASTType, Symbol, Type, Variable};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for Variable {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::var_decl);

        let symbol = Symbol::from_pair(&pair);
        let mut name = None;
        let mut _type = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => name = Some(Ident::parse(pool, pair)),
                Rule::var_type => _type = Some(Type::from_str(pair.as_str())),

                _ => unexpected_pair!(pair),
            }
        }

        let variable = Variable {
            id: pool.len(),
            symbol,
            name: name.expect("no name for variable"),
            type_: _type.expect("no type for variable"),
        };

        pool.add(variable)
    }
}
