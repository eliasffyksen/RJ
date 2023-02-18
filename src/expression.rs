use std::iter;

use pest::iterators::Pair;

use crate::const_data::{Const, ConstImpl};
use crate::ident::{Ident, IdentImpl};
use crate::scope::Scopable;
use crate::stmt::Type;
use crate::{check_rule, unexpected_pair, Rule};

pub struct ExpressionInput {
    pub data_type: Type,
    pub store_to: Option<usize>,
}

#[derive(Debug)]
pub enum Expression {
    Ident(Ident),
    Const(Const),
}

#[derive(Debug)]
pub struct ExpressionList {
    expressions: Vec<Expression>,
}

impl ExpressionList {
    pub fn ast(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::expr_list);

        let mut expressions = vec![];

        for element in pair.into_inner() {
            match element.as_rule() {
                Rule::expr_elm => expressions.push(Expression::ast(element)),

                _ => unexpected_pair(&element),
            }
        }

        Self { expressions }
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        expression_inputs: &mut Vec<ExpressionInput>,
    ) -> Result<(), std::io::Error> {
        if expression_inputs.len() != self.expressions.len() {
            panic!(
                "Incorrect expression list count, expected {} values got {}",
                expression_inputs.len(),
                self.expressions.len(),
            )
        }

        for (expression_input, expression) in iter::zip(expression_inputs, &self.expressions) {
            expression.ir(output, context, scope, expression_input)?;
        }

        Ok(())
    }
}

impl Expression {
    fn ast(pair: Pair<Rule>) -> Self {
        check_rule(&pair, Rule::expr_elm);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => return Expression::Ident(Ident::ast(pair)),
                Rule::int => return Expression::Const(Const::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }

        panic!("No pair in expression");
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        expression_input: &mut ExpressionInput,
    ) -> Result<(), std::io::Error> {
        match self {
            Expression::Ident(ident) => {
                Self::ir_ident(ident, output, context, scope, expression_input)
            }
            Expression::Const(const_data) => {
                Self::ir_const(const_data, output, expression_input)
            }
        }
    }

    fn ir_const(
        const_data: &Const,
        output: &mut impl std::io::Write,
        expression_input: &mut ExpressionInput,
    ) -> Result<(), std::io::Error> {
        if Type::I32 != expression_input.data_type {
            panic!(
                "Incompatible data, expected {:?} got i32",
                expression_input.data_type
            );
        }

        match expression_input.store_to {
            Some(store_register) => {
                writeln!(
                    output,
                    "  store i32 {}, i32* %{}",
                    const_data, store_register,
                )?;
                Ok(())
            }
            None => todo!(),
        }
    }

    fn ir_ident(
        ident: &Ident,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        expression_input: &mut ExpressionInput,
    ) -> Result<(), std::io::Error> {
        match scope.get_entry(ident) {
            Some(scope_entry) => {
                let var_type = scope_entry.var_decl.var_type.clone();

                if var_type != expression_input.data_type {
                    panic!(
                        "Incompatible data, expected {:?} got {:?}",
                        expression_input.data_type, var_type
                    );
                }

                let dst_register = context.claim_register();
                let src_register = scope_entry.register;

                writeln!(
                    output,
                    "  %{} = load {}, {}* %{}",
                    dst_register,
                    var_type.get_ir_type(),
                    var_type.get_ir_type(),
                    src_register
                )?;

                if let Some(store_to) = expression_input.store_to {
                    writeln!(
                        output,
                        "  store {} %{}, {}* %{}",
                        var_type.get_ir_type(),
                        dst_register,
                        var_type.get_ir_type(),
                        store_to,
                    )
                } else {
                    expression_input.store_to = Some(dst_register);
                    Ok(())
                }
            }
            None => panic!("Unknown identifier: {:?}", ident),
        }
    }
}
