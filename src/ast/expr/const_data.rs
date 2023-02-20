use std::io;

use crate::ast;
use crate::ast::expr;
use crate::parser;

#[derive(Debug)]
pub struct Const {
    data_type: ast::Type,
    value: String,
    symbol: ast::Symbol,
}

impl Const {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Const {
        assert!(pair.as_rule() == parser::Rule::int);

        Const {
            data_type: ast::Type::I32,
            value: format!(
                "i32 {}",
                pair.as_str()
                    .to_string()
                    .parse::<i32>()
                    .expect("Failed to parse int"),
            ),
            symbol: ast::Symbol::from_pair(&pair),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        expression_input: &mut expr::Input,
    ) -> Result<(), ast::Error> {
        let from =
            expression_input.ir_convert(output, context, ast::Type::I32, self.value.as_str());
        let from = match from {
            Ok(x) => x,
            Err(err) => return Err(err.to_symbol_err(&self.symbol)),
        };

        match &expression_input.store_to {
            Some(store_register) => {
                writeln!(output, "  store {}, {}", from, store_register).unwrap();
            }
            None => {
                expression_input.store_to = Some(from);
            }
        }

        Ok(())
    }
}
