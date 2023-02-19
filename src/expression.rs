use std::fmt::{format, Display, Write as _};
use std::io;
use std::slice::IterMut;

use pest::iterators::Pair;

use crate::ast_type::Type;
use crate::const_data::Const;
use crate::equal::Equal;
use crate::function::FunctionCall;
use crate::ident::Ident;
use crate::scope::{Scopable, ScopeEntry};
use crate::symbol_ref::{SymbolError, SymbolRef};
use crate::{check_rule, unexpected_pair, IRContext, Rule};

pub struct ConversionError {
    from: Type,
    to: Type,
}

impl ConversionError {
    pub fn to_symbol_err(self, symbol: &SymbolRef) -> SymbolError {
        SymbolError {
            error: Box::new(self),
            symbol: symbol.clone(),
        }
    }
}

impl Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Can not convert {} to {}", self.from, self.to)
    }
}

#[derive(Debug)]
pub struct ExpressionInput {
    pub data_type: Type,
    pub store_to: Option<String>,
}

impl ExpressionInput {
    pub fn ir_convert(
        &mut self,
        output: &mut impl io::Write,
        context: &mut IRContext,
        from_type: Type,
        from: &str,
    ) -> Result<String, ConversionError> {
        if self.data_type == Type::Any {
            self.data_type = from_type;
            return Ok(from.to_string());
        }

        if self.data_type == from_type {
            return Ok(from.to_string());
        }

        match from_type {
            Type::I32 => self.ir_convert_i32(output, context, from),

            _ => Err(ConversionError {
                from: from_type,
                to: self.data_type.clone(),
            }),
        }
    }

    pub fn ir_convert_i32(
        &self,
        output: &mut impl io::Write,
        context: &mut IRContext,
        from: &str,
    ) -> Result<String, ConversionError> {
        match self.data_type {
            Type::Bool => {
                let register = context.claim_register();
                writeln!(output, "  %{} = icmp ne {}, 0", register, from).unwrap();
                Ok(format!("i1 %{}", register))
            }

            _ => Err(ConversionError {
                from: Type::I32,
                to: self.data_type.clone(),
            }),
        }
    }
}

#[derive(Debug, Default)]
pub struct ExpressionList {
    pub expressions: Vec<Expression>,
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
    ) -> Result<(), SymbolError> {
        if expression_inputs.len() != self.expressions.len() {
            panic!(
                "Incorrect expression list count, expected {} values got {}",
                expression_inputs.len(),
                self.expressions.len(),
            )
        }

        let mut expression_inputs = expression_inputs.iter_mut();

        for expression in &self.expressions {
            expression.ir(output, context, scope, &mut expression_inputs)?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum Expression {
    Ident(Ident),
    Const(Const),
    FunctionCall(FunctionCall),
    Eq(Box<Equal>),
}

impl Expression {
    pub fn ast(pair: Pair<Rule>) -> Self {
        let pair = depred(pair);

        match pair.as_rule() {
            Rule::ident => return Expression::Ident(Ident::ast(pair)),
            Rule::int => return Expression::Const(Const::ast(pair)),
            Rule::func_call => return Expression::FunctionCall(FunctionCall::ast(pair)),
            Rule::equal => return Expression::Eq(Box::new(Equal::ast(pair))),

            _ => unexpected_pair(&pair),
        }

        panic!("No pair in expression");
    }

    pub fn ir(
        &self,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        expression_inputs: &mut IterMut<ExpressionInput>,
    ) -> Result<(), SymbolError> {
        match self {
            Expression::Ident(ident) => {
                let expression_input = expression_inputs.next().expect("Too many values to unpack");

                Self::ir_ident(ident, output, context, scope, expression_input)?;
                Ok(())
            }

            Expression::Const(const_data) => {
                let expression_input = expression_inputs.next().expect("Too many values to unpack");

                const_data.ir(output, context, expression_input)
            }

            Expression::FunctionCall(function_call) => {
                function_call.ir(output, context, scope, expression_inputs)
            }

            Expression::Eq(equal) => {
                let expression_input = expression_inputs.next().expect("Too many values to unpack");

                equal.ir(output, context, scope, expression_input)
            }
        }
    }

    fn ir_ident(
        ident: &Ident,
        output: &mut impl std::io::Write,
        context: &mut crate::IRContext,
        scope: &mut impl Scopable,
        expression_input: &mut ExpressionInput,
    ) -> Result<(), SymbolError> {
        match scope.get_entry(ident) {
            Some(scope_entry) => {
                let scope_entry = match scope_entry {
                    ScopeEntry::Variable(variable) => variable,

                    _ => {
                        panic!(
                            "expected {} to be variable, instead it is {:?}",
                            ident.get(),
                            scope_entry
                        )
                    }
                };

                let var_type = scope_entry.var_decl.var_type.clone();

                let dst_register = context.claim_register();
                let src_register = scope_entry.register;

                writeln!(
                    output,
                    "  %{} = load {}, {}* %{}",
                    dst_register,
                    var_type.get_ir_type(),
                    var_type.get_ir_type(),
                    src_register
                )
                .unwrap();

                let from = format!("{} %{}", var_type.get_ir_type(), dst_register);
                let from = expression_input.ir_convert(output, context, var_type, &from.as_str());
                let from = match from {
                    Ok(x) => x,
                    Err(err) => return Err(err.to_symbol_err(&ident.symbol)),
                };

                if let Some(store_to) = &expression_input.store_to {
                    writeln!(output, "  store {}, {}", from, store_to,).unwrap();
                    Ok(())
                } else {
                    let mut store_to = String::new();
                    write!(&mut store_to, "{}", from).unwrap();
                    expression_input.store_to = Some(store_to);
                    Ok(())
                }
            }
            None => panic!("Unknown identifier: {:?}", ident),
        }
    }
}

fn depred(pair: Pair<Rule>) -> Pair<Rule> {
    let mut pair = pair;

    while match pair.as_rule() {
        Rule::pred_0 => true,
        Rule::pred_max => true,
        Rule::expr_elm => true,

        _ => false,
    } {
        pair = pair.into_inner().next().unwrap();
    }

    pair
}
