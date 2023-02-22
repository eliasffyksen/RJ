use std::collections::VecDeque;
use std::fmt;
use std::io;

use crate::ast;
use crate::ast::expr;
use crate::ast::scope;
use crate::parser;

pub struct IncompatibleOperation {
    pub operation: &'static str,
    pub types: Vec<ast::Type>,
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
        write!(f, "{} {}", self.data_type.get_ir_type(), self.value)
    }
}

impl Res {
    pub fn fulfill(
        self,
        output: &mut impl io::Write,
        request: Req,
    ) -> Result<Option<Self>, ConversionError> {
        if request.data_type != self.data_type && request.data_type != ast::Type::Any {
            return Err(ConversionError {
                from: self.data_type,
                to: request.data_type,
            });
        }

        let store_to = match request.store_to {
            Some(store_to) => store_to,

            None => return Ok(Some(self)),
        };

        writeln!(output, "  store {}, {}", self, store_to).unwrap();

        Ok(None)
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
    ) -> Result<Vec<Option<Res>>, ast::Error> {
        if requests.len() != self.expressions.len() {
            panic!(
                "Incorrect expression list count, expected {} values got {}",
                requests.len(),
                self.expressions.len(),
            )
        }

        let mut result = vec![];

        for expression in &self.expressions {
            result.extend(expression.ir(output, context, scope, requests)?);
        }

        Ok(result)
    }
}

#[derive(Debug)]
pub enum Expr {
    Ident(ast::Ident),
    Const(expr::Const),
    FunctionCall(expr::FuncCall),
    Cmp(Box<expr::Cmp>),
    Sum(Box<expr::Sum>),
}

impl Expr {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        let pair = depred(pair);

        match pair.as_rule() {
            parser::Rule::ident => return Expr::Ident(ast::Ident::ast(pair)),
            parser::Rule::int => return Expr::Const(expr::Const::ast(pair)),
            parser::Rule::func_call => return Expr::FunctionCall(expr::FuncCall::ast(pair)),
            parser::Rule::cmp => return Expr::Cmp(Box::new(expr::Cmp::ast(pair))),
            parser::Rule::sum => return Expr::Sum(Box::new(expr::Sum::ast(pair))),

            _ => unexpected_pair!(pair),
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        requests: &mut VecDeque<Req>,
    ) -> Result<Vec<Option<Res>>, ast::Error> {
        match self {
            Expr::Ident(ident) => {
                let expression_input = requests.pop_front().expect("Too many values to unpack");

                Ok(vec![Self::ir_ident(ident, output, context, scope, expression_input)?])
            }

            Expr::Const(const_data) => {
                let expression_input = requests.pop_front().expect("Too many values to unpack");

                Ok(vec![const_data.ir(output, expression_input)?])
            }

            Expr::FunctionCall(function_call) => {
                function_call.ir(output, context, scope, requests)
            }

            Expr::Cmp(cmp) => {
                let expression_input = requests.pop_front().expect("Too many values to unpack");

                Ok(vec![cmp.ir(output, context, scope, expression_input)?])
            }

            Expr::Sum(sum) => {
                let expression_input = requests.pop_front().expect("Too many values to unpack");

                Ok(vec![sum.ir(output, context, scope, expression_input)?])
            },
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
                }
                .fulfill(output, request);

                match result {
                    Ok(result) => Ok(result),
                    Err(_) => todo!(),
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
        parser::Rule::pred_1 => true,
        parser::Rule::pred_2 => true,
        parser::Rule::pred_max => true,
        parser::Rule::expr_elm => true,

        _ => false,
    } {
        pair = pair.into_inner().next().unwrap();
    }

    pair
}
