use pest::iterators::Pair;

use crate::ident::{Ident, IdentImpl};
use crate::scope::Scopable;
use crate::stmt::Type;
use crate::{check_rule, unexpected_pair, Rule};

pub struct ExpressionInput {
    pub data_type: Type,
    pub store_to: Option<usize>,
}

pub struct ExpressionOutput {
    pub data_type: Type,
    pub register: usize,
}

#[derive(Debug)]
pub enum Expression {
    Ident(Ident),
}

impl Expression {
    pub fn ast(pair: Pair<Rule>) -> Vec<Expression> {
        check_rule(&pair, Rule::expr);

        let mut expressions = vec![];

        for element in pair.into_inner() {
            match element.as_rule() {
                Rule::expr_elm => expressions.push(Self::ast_expression_element(element)),

                _ => unexpected_pair(&element),
            }
        }

        expressions
    }

    fn ast_expression_element(pair: Pair<Rule>) -> Expression {
        check_rule(&pair, Rule::expr_elm);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => return Expression::Ident(Ident::ast(pair)),

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
        expression_input: ExpressionInput,
    ) -> Result<Option<ExpressionOutput>, std::io::Error> {
        match self {
            Expression::Ident(ident) => {
                Self::ir_ident(ident, output, context, scope, expression_input)
            }
        }
    }

    pub fn ir_ident(
        ident: &Ident,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        expression_input: ExpressionInput,
    ) -> Result<Option<ExpressionOutput>, std::io::Error> {
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

                match expression_input.store_to {
                    Some(store_register) => {
                        writeln!(
                            output,
                            "  store {} %{}, {}* %{}",
                            var_type.get_ir_type(),
                            dst_register,
                            var_type.get_ir_type(),
                            store_register,
                        )?;
                        Ok(None)
                    }
                    None => Ok(Some(ExpressionOutput {
                        data_type: var_type,
                        register: dst_register,
                    })),
                }
            }
            None => panic!("Unknown identifier: {:?}", ident),
        }
    }
}
