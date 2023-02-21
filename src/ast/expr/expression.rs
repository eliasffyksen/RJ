use std::collections::VecDeque;
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
pub struct Req {
    pub data_type: ast::Type,
    pub store_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Res {
    pub data_type: ast::Type,
    pub value: String,
}

impl fmt::Display for Res {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.data_type, self.value)
    }
}

impl Res {
    pub fn fulfill(
        self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        request: Req,
    ) -> Result<Option<Self>, ConversionError> {
        if request.data_type != self.data_type {
            self = self.ir_convert(output, context, request)?;
        }

        let store_to = match request.store_to {
            None => return Ok(Some(self)),
            Some(store_to) => store_to,
        };

        writeln!(output, "  store {}, {}", self, store_to);

        Ok(None)
    }

    fn ir_convert(
        self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        request: Req,
    ) -> Result<Self, ConversionError> {
        match request.data_type {
            ast::Type::Any => Ok(self),
            ast::Type::Bool => self.ir_convert_bool(output, context),

            _ => Err(ConversionError {
                from: self.data_type,
                to: request.data_type,
            }),
        }
    }

    pub fn ir_convert_bool(
        self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
    ) -> Result<Self, ConversionError> {
        match self.data_type {
            ast::Type::Bool => {
                let register = context.claim_register();
                writeln!(output, "  %{} = icmp ne {}, 0", register, self).unwrap();

                Ok(Self {
                    data_type: ast::Type::Bool,
                    value: format!("%{}", register)
                })
            },

            _ => Err(ConversionError {
                from: self.data_type,
                to: ast::Type::Bool,
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
        requests: &mut VecDeque<expr::Req>,
    ) -> Result<(), ast::Error> {
        if requests.len() != self.expressions.len() {
            panic!(
                "Incorrect expression list count, expected {} values got {}",
                requests.len(),
                self.expressions.len(),
            )
        }

        for expression in &self.expressions {
            expression.ir(output, context, scope, &mut requests)?;
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
            parser::Rule::func_call => return Expr::FunctionCall(expr::FuncCall::ast(pair)),
            parser::Rule::equal => return Expr::Eq(Box::new(expr::Equal::ast(pair))),

            _ => unexpected_pair!(pair),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        requests: &mut VecDeque<Req>,
    ) -> Result<Option<Res>, ast::Error> {
        match self {
            Expr::Ident(ident) => {
                let expression_input = requests.pop_front().expect("Too many values to unpack");

                Self::ir_ident(ident, output, context, scope, expression_input)
            }

            Expr::Const(const_data) => {
                let expression_input = requests.pop_front().expect("Too many values to unpack");

                const_data.ir(output, context, expression_input)
            }

            Expr::FunctionCall(function_call) => {
                function_call.ir(output, context, scope, requests);
                todo!()
            }

            Expr::Eq(equal) => {
                let expression_input = requests.pop_front().expect("Too many values to unpack");

                equal.ir(output, context, scope, expression_input)
            }
        }
    }

    fn ir_ident(
        ident: &ast::Ident,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        request: Req,
    ) -> Result<Option<Res>, ast::Error> {
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

                let result = Res {
                    data_type: var_type,
                    value: format!("%{}", dst_register),
                }.fulfill(output, context, request);

                match result {
                    Ok(result) => Ok(result),
                    Err(err) => todo!(),
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
