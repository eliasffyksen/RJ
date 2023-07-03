use std::collections;
use std::io;

use crate::ast::expr;
use crate::ast::{self, scope};
use crate::parser;

#[derive(Debug)]
enum SumOp {
    Add,
    Sub,
}

impl SumOp {
    fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        match pair.as_rule() {
            parser::Rule::add => SumOp::Add,
            parser::Rule::sub => SumOp::Sub,

            _ => panic!("Unrecognised sum operation"),
        }
    }

    fn get_ir_op(&self) -> &'static str {
        match self {
            SumOp::Add => "add",
            SumOp::Sub => "sub",
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            SumOp::Add => "+",
            SumOp::Sub => "-",
        }
    }
}

#[derive(Debug)]
pub struct Sum {
    symbol: ast::Symbol,
    first: expr::Expr,
    operations: Vec<(SumOp, expr::Expr)>,
}

impl Sum {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        assert!(pair.as_rule() == parser::Rule::sum);

        let symbol = ast::Symbol::from_pair(&pair);

        let mut pairs = pair.into_inner();

        let mut result = Self {
            symbol,
            first: expr::Expr::ast(pairs.next().unwrap()),
            operations: vec![],
        };

        while pairs.peek().is_some() {
            result.operations.push((
                SumOp::ast(pairs.next().unwrap()),
                expr::Expr::ast(pairs.next().unwrap()),
            ));
        }

        result
    }

    pub fn ir(
        &self,
        output: &mut impl io::Write,
        context: &mut ast::IRContext,
        scope: &mut impl scope::Scopable,
        request: expr::Req,
    ) -> Result<Option<expr::Res>, ast::Error> {
        let mut expression_requests = (0..self.operations.len() + 1)
            .map(|_| expr::Req {
                data_type: ast::Type::I32,
                store_to: None,
            })
            .collect::<collections::VecDeque<_>>();

        let mut expression_results =
            self.first
                .ir(output, context, scope, &mut expression_requests)?;

        for (_, expression) in &self.operations {
            expression_results.extend(expression.ir(
                output,
                context,
                scope,
                &mut expression_requests,
            )?);
        }

        let mut expression_results = expression_results.into_iter();

        let mut last = expression_results.next().unwrap().unwrap();

        for (operation, _) in &self.operations {
            let next = expression_results.next().unwrap().unwrap();

            if last.data_type != next.data_type {
                todo!()
            }

            let register = context.claim_register();

            writeln!(
                output,
                "  %_{} = {} i32 {}, {}",
                register,
                operation.get_ir_op(),
                last.value,
                next.value
            )
            .unwrap();

            last = expr::Res {
                data_type: ast::Type::I32,
                value: format!("%_{}", register),
            }
        }

        let result = last.fulfill(output, request);

        match result {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_symbol_err(&self.symbol)),
        }
    }
}
