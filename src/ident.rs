use crate::GenerateAST;
use crate::Rule;

pub type Ident = String;

impl GenerateAST<Ident> for Ident {
    fn generate_ast(pair: pest::iterators::Pair<crate::Rule>) -> Ident {
        if pair.as_rule() != Rule::ident {
            panic!("Attempted to generate ident from non ident pair: {:?}", pair)
        }

        pair.as_str().to_string()
    }
}
