use std::fmt::Display;

use pest::iterators::Pair;

use crate::Rule;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I32,
    Bool,
}

impl Type {
    pub fn get_ir_type(&self) -> &'static str {
        match self {
            Type::I32 => "i32",
            Type::Bool => "i1",
        }
    }

    pub fn ast(pair: Pair<Rule>) -> Type {
        if pair.as_rule() != Rule::var_type {
            panic!("Attempted to generate Type from non Type pair: {:?}", pair)
        }

        match pair.as_str() {
            "i32" => Type::I32,

            _ => panic!("Unknown type {:?}", pair),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Type::I32 => "i32",
            Type::Bool => "bool",
        };

        write!(f, "{}", value)
    }
}
