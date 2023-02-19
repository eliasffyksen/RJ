use crate::{
    ast_type::Type,
    expression::ExpressionInput,
    symbol_ref::{SymbolError, SymbolRef},
    Rule,
};

#[derive(Debug)]
pub struct Const {
    data_type: Type,
    value: String,
    symbol: SymbolRef,
}

impl Const {
    pub fn ast(pair: pest::iterators::Pair<crate::Rule>) -> Const {
        if pair.as_rule() != Rule::int {
            panic!("Attempted to generate int from non ident int: {:?}", pair)
        }

        Const {
            data_type: Type::I32,
            value: format!(
                "i32 {}",
                pair.as_str()
                    .to_string()
                    .parse::<i32>()
                    .expect("Failed to parse int"),
            ),
            symbol: SymbolRef::from_pair(&pair),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        expression_input: &mut ExpressionInput,
    ) -> Result<(), SymbolError> {
        let store_to = expression_input.ir_convert(context, Type::I32, self.value.as_str());
        let from = match store_to {
            Ok(x) => x,
            Err(err) => return Err(err.to_symbol_err(&self.symbol)),
        };

        if Type::I32 != expression_input.data_type {
            panic!(
                "Incompatible data, expected {:?} got i32",
                expression_input.data_type
            );
        }

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
