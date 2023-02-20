use crate::ast;
use crate::parser;

#[derive(Debug, Clone)]
pub struct Ident {
    pub symbol: ast::SymbolRef,
    value: String,
}

impl Ident {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Ident {
        assert!(pair.as_rule() == parser::Rule::ident);

        Ident {
            value: pair.as_str().to_string(),
            symbol: ast::SymbolRef::from_pair(&pair),
        }
    }

    pub fn get(&self) -> &str {
        self.value.as_str()
    }
}
