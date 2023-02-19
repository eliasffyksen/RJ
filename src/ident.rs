use pest::iterators::Pair;

use crate::{Rule, symbol_ref::SymbolRef};

#[derive(Debug, Clone)]
pub struct Ident {
    pub symbol: SymbolRef,
    value: String
}

impl Ident {
    pub fn ast(pair: Pair<Rule>) -> Ident {
        if pair.as_rule() != Rule::ident {
            panic!("Attempted to generate ident from non ident pair: {:?}", pair)
        }

        Ident {
            value: pair.as_str().to_string(),
            symbol: SymbolRef::from_pair(&pair)
        }
    }

    pub fn get(&self) -> &str {
        self.value.as_str()
    }
}
