use std::fmt;

use crate::parser;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I32,
    Bool,
    Any,
}

impl Type {
    pub fn get_ir_type(&self) -> &'static str {
        match self {
            Type::I32 => "i32",
            Type::Bool => "i1",
            Type::Any => panic!("Attempted to get ir type of Any"),
        }
    }

    pub fn ast(pair: parser::Pair<parser::Rule>) -> Type {
        if pair.as_rule() != parser::Rule::var_type {
            panic!("Attempted to generate Type from non Type pair: {:?}", pair)
        }

        match pair.as_str() {
            "i32" => Type::I32,

            _ => panic!("Unknown type {:?}", pair),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Type::I32 => "i32",
            Type::Bool => "bool",
            Type::Any => panic!("Attempted to get display value of Any"),
        };

        write!(f, "{}", value)
    }
}
