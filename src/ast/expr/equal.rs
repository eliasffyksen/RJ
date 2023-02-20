use std::{fmt, io};

use crate::ast;
use crate::ast::{expr, scope};
use crate::parser;

struct IncompatibleOperation {
    operation: &'static str,
    types: Vec<ast::Type>,
}

impl fmt::Display for IncompatibleOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Can not preform operation '{}' on incompatible types: {}",
            self.operation,
            self.types
                .iter()
                .map(|t| format!("{}", t))
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

#[derive(Debug)]
pub struct Equal {
    left: expr::Expression,
    right: expr::Expression,
    symbol: ast::SymbolRef,
}

impl Equal {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Equal {
        assert!(pair.as_rule() == parser::Rule::equal);

        let symbol = ast::SymbolRef::from_pair(&pair);

        let mut pairs = pair.into_inner();

        Equal {
            left: expr::Expression::ast(pairs.next().unwrap()),
            right: expr::Expression::ast(pairs.next().unwrap()),
            symbol,
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        expression_input: &mut expr::ExpressionInput,
    ) -> Result<(), ast::SymbolError> {
        let mut left_expression_input = vec![expr::ExpressionInput {
            data_type: ast::Type::Any,
            store_to: None,
        }];

        self.left.ir(
            output,
            context,
            scope,
            &mut left_expression_input.iter_mut(),
        )?;
        let left = left_expression_input.pop().unwrap();

        let mut right_expression_input = vec![expr::ExpressionInput {
            data_type: ast::Type::Any,
            store_to: None,
        }];

        self.right.ir(
            output,
            context,
            scope,
            &mut right_expression_input.iter_mut(),
        )?;
        let right = right_expression_input.pop().unwrap();

        if left.data_type == right.data_type {
            let success = match left.data_type {
                ast::Type::I32 => {
                    self.ir_compare_int(output, context, scope, &left, &right, expression_input)?;
                    true
                }
                ast::Type::Bool => {
                    self.ir_compare_int(output, context, scope, &left, &right, expression_input)?;
                    true
                }
                _ => false,
            };
            if success {
                return Ok(());
            }
        }

        Err(ast::SymbolError {
            symbol: self.symbol.clone(),
            error: Box::new(IncompatibleOperation {
                operation: "==",
                types: vec![left.data_type.clone(), right.data_type.clone()],
            }),
        })
    }

    fn ir_compare_int(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        left: &expr::ExpressionInput,
        right: &expr::ExpressionInput,
        expression_output: &mut expr::ExpressionInput,
    ) -> Result<(), ast::SymbolError> {
        // TODO: CHANGE THIS!!!!! HACK TO GET IT WORKING!
        let result_register = context.claim_register();
        let data_type = left.data_type.clone();
        let left_value = left
            .store_to
            .clone()
            .unwrap()
            .split(" ")
            .nth(1)
            .unwrap()
            .to_string();
        let right_value = right
            .store_to
            .clone()
            .unwrap()
            .split(" ")
            .nth(1)
            .unwrap()
            .to_string();

        writeln!(
            output,
            "  %{} = icmp eq {} {}, {}",
            result_register, data_type, left_value, right_value
        )
        .unwrap();

        let value = format!("i1 %{}", result_register);
        let value = expression_output.ir_convert(output, context, ast::Type::Bool, value.as_str());
        let value = match value {
            Ok(value) => value,
            Err(err) => return Err(err.to_symbol_err(&self.symbol)),
        };

        match &expression_output.store_to {
            Some(store_register) => {
                writeln!(output, "  store {}, {}", value, store_register).unwrap();
            }
            None => {
                expression_output.store_to = Some(value);
            }
        }

        Ok(())
    }
}
