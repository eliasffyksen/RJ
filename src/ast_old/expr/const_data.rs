use std::io;

use crate::ast;
use crate::ast::expr;
use crate::parser;

#[derive(Debug)]
pub struct Const {
    value: expr::Res,
    symbol: ast::Symbol,
}

impl Const {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Const {
        match pair.as_rule() {
            parser::Rule::int => Const {
                value: expr::Res {
                    data_type: ast::Type::I32,
                    value: pair
                        .as_str()
                        .parse::<i32>()
                        .expect("Failed to parse int")
                        .to_string(),
                },
                symbol: ast::Symbol::from_pair(&pair),
            },

            parser::Rule::bool => Const {
                value: expr::Res {
                    data_type: ast::Type::Bool,
                    value: match pair.as_str() {
                        "true" => "1".to_string(),
                        "false" => "0".to_string(),

                        _ => unexpected_pair!(pair),
                    }
                },
                symbol: ast::Symbol::from_pair(&pair),
            },

            _ => unexpected_pair!(pair),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        request: expr::Req,
    ) -> Result<Option<expr::Res>, ast::Error> {
        match self.value.clone().fulfill(output, request) {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_symbol_err(&self.symbol)),
        }
    }
}
