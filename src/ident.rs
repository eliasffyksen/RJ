use crate::Rule;

pub type Ident = String;

pub trait IdentImpl {
    fn ast(pair: pest::iterators::Pair<crate::Rule>) -> Ident {
        if pair.as_rule() != Rule::ident {
            panic!("Attempted to generate ident from non ident pair: {:?}", pair)
        }

        pair.as_str().to_string()
    }
}

impl IdentImpl for Ident {}
