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
        assert!(pair.as_rule() == parser::Rule::int);

        Const {
            value: expr::Res {
                data_type: ast::Type::I32,
                value: pair.as_str().parse::<i32>()
                    .expect("Failed to parse int").to_string(),
            },
            symbol: ast::Symbol::from_pair(&pair),
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
