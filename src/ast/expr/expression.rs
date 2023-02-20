use std::fmt;
use std::fmt::Write;
use std::io;
use std::slice;

use crate::ast;
use crate::ast::expr;
use crate::ast::scope;
use crate::parser;

pub struct ConversionError {
    from: ast::Type,
    to: ast::Type,
}

impl ConversionError {
    pub fn to_symbol_err(self, symbol: &ast::Symbol) -> ast::Error {
        ast::Error {
            error: Box::new(self),
            symbol: symbol.clone(),
        }
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Can not convert {} to {}", self.from, self.to)
    }
}

#[derive(Debug)]
pub struct Input {
    pub data_type: ast::Type,
    pub store_to: Option<String>,
}

impl Input {
    pub fn ir_convert(
        &mut self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        from_type: ast::Type,
        from: &str,
    ) -> Result<String, ConversionError> {
        if self.data_type == ast::Type::Any {
            self.data_type = from_type;
            return Ok(from.to_string());
        }

        if self.data_type == from_type {
            return Ok(from.to_string());
        }

        match from_type {
            ast::Type::I32 => self.ir_convert_i32(output, context, from),

            _ => Err(ConversionError {
                from: from_type,
                to: self.data_type.clone(),
            }),
        }
    }

    pub fn ir_convert_i32(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        from: &str,
    ) -> Result<String, ConversionError> {
        match self.data_type {
            ast::Type::Bool => {
                let register = context.claim_register();
                writeln!(output, "  %{} = icmp ne {}, 0", register, from).unwrap();
                Ok(format!("i1 %{}", register))
            }

            _ => Err(ConversionError {
                from: ast::Type::I32,
                to: self.data_type.clone(),
            }),
        }
    }
}

#[derive(Debug, Default)]
pub struct List {
    pub expressions: Vec<Expr>,
}

impl List {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::expr_list);

        let mut expressions = vec![];

        for element in pair.into_inner() {
            match element.as_rule() {
                parser::Rule::expr_elm => expressions.push(Expr::ast(element)),

                _ => unexpected_pair!(element),
            }
        }

        Self { expressions }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        expression_inputs: &mut Vec<expr::Input>,
    ) -> Result<(), ast::Error> {
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
pub enum Expr {
    Ident(ast::Ident),
    Const(expr::Const),
    FunctionCall(expr::FuncCall),
    Eq(Box<expr::Equal>),
}

impl Expr {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        let pair = depred(pair);

        match pair.as_rule() {
            parser::Rule::ident => return Expr::Ident(ast::Ident::ast(pair)),
            parser::Rule::int => return Expr::Const(expr::Const::ast(pair)),
            parser::Rule::func_call => {
                return Expr::FunctionCall(expr::FuncCall::ast(pair))
            }
            parser::Rule::equal => return Expr::Eq(Box::new(expr::Equal::ast(pair))),

            _ => unexpected_pair!(pair),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        expression_inputs: &mut slice::IterMut<Input>,
    ) -> Result<(), ast::Error> {
        match self {
            Expr::Ident(ident) => {
                let expression_input = expression_inputs.next().expect("Too many values to unpack");

                Self::ir_ident(ident, output, context, scope, expression_input)?;
                Ok(())
            }

            Expr::Const(const_data) => {
                let expression_input = expression_inputs.next().expect("Too many values to unpack");

                const_data.ir(output, context, expression_input)
            }

            Expr::FunctionCall(function_call) => {
                function_call.ir(output, context, scope, expression_inputs)
            }

            Expr::Eq(equal) => {
                let expression_input = expression_inputs.next().expect("Too many values to unpack");

                equal.ir(output, context, scope, expression_input)
            }
        }
    }

    fn ir_ident(
        ident: &ast::Ident,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        expression_input: &mut Input,
    ) -> Result<(), ast::Error> {
        match scope.get_entry(ident) {
            Some(scope_entry) => {
                let scope_entry = match scope_entry {
                    scope::Entry::Variable(variable) => variable,

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

fn depred(pair: parser::Pair<parser::Rule>) -> parser::Pair<parser::Rule> {
    let mut pair = pair;

    while match pair.as_rule() {
        parser::Rule::pred_0 => true,
        parser::Rule::pred_max => true,
        parser::Rule::expr_elm => true,

        _ => false,
    } {
        pair = pair.into_inner().next().unwrap();
    }

    pair
}
