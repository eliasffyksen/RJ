use crate::Rule;

pub type Const = i32;

pub trait ConstImpl {
    fn ast(pair: pest::iterators::Pair<crate::Rule>) -> Const {
        if pair.as_rule() != Rule::int {
            panic!("Attempted to generate int from non ident int: {:?}", pair)
        }

        pair.as_str().to_string().parse().expect("Failed to parse int")
    }
}

impl ConstImpl for Const {}
