use std::collections::VecDeque;
use std::fmt;
use std::io;

use crate::ast;
use crate::ast::expr;
use crate::ast::scope;
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
    left: expr::Expr,
    right: expr::Expr,
    symbol: ast::Symbol,
}

impl Equal {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Equal {
        assert!(pair.as_rule() == parser::Rule::equal);

        let symbol = ast::Symbol::from_pair(&pair);

        let mut pairs = pair.into_inner();

        Equal {
            left: expr::Expr::ast(pairs.next().unwrap()),
            right: expr::Expr::ast(pairs.next().unwrap()),
            symbol,
        }
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        request: expr::Req,
    ) -> Result<Option<expr::Res>, ast::Error> {
        let mut expression_requsts = VecDeque::new();
        expression_requsts.push_back(expr::Req {
            data_type: ast::Type::Any,
            store_to: None,
        });
        expression_requsts.push_back(expr::Req {
            data_type: ast::Type::Any,
            store_to: None,
        });

        let left = self
            .left
            .ir(output, context, scope, &mut expression_requsts)?
            .unwrap();

        let right = self
            .right
            .ir(output, context, scope, &mut expression_requsts)?
            .unwrap();

        let result_register = if left.data_type == right.data_type {
            self.ir_compare_same(output, context, left, right, request)
        } else {
            Err(())
        };

        let result_register = match result_register {
            Ok(result_register) => Ok(result_register),

            Err(_) => Err(ast::Error {
                symbol: self.symbol.clone(),
                error: Box::new(IncompatibleOperation {
                    operation: "==",
                    types: vec![left.data_type.clone(), right.data_type.clone()],
                }),
            }),
        }?;

        let result = expr::Res {
            data_type: ast::Type::Bool,
            value: format!("%{}", result_register),
        }
        .fulfill(output, context, request);

        match result {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_symbol_err(&self.symbol)),
        }
    }

    fn ir_compare_same(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        left: expr::Res,
        right: expr::Res,
        request: expr::Req,
    ) -> Result<usize, ()> {
        assert!(left.data_type == right.data_type);

        let result_register = match left.data_type {
            ast::Type::I32 => self.ir_compare_native(output, context, left, right, request),
            ast::Type::Bool => self.ir_compare_native(output, context, left, right, request),

            _ => return Err(()),
        };

        Ok(result_register)
    }

    fn ir_compare_native(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        left: expr::Res,
        right: expr::Res,
        request: expr::Req,
    ) -> usize {
        assert!(left.data_type == right.data_type);

        let result_register = context.claim_register();

        writeln!(
            output,
            "  %{} = icmp eq {} {}, {}",
            result_register, left.data_type, left.value, right.value,
        )
        .unwrap();

        result_register
    }
}
