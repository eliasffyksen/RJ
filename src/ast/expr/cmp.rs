use std::collections::VecDeque;
use std::io;

use crate::ast;
use crate::ast::expr;
use crate::ast::scope;
use crate::parser;

#[derive(Debug)]
enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    fn ast(pair: parser::Pair<parser::Rule>) -> Self {
        match pair.as_rule() {
            parser::Rule::cmp_eq => CmpOp::Eq,
            parser::Rule::cmp_ne => CmpOp::Ne,
            parser::Rule::cmp_lt => CmpOp::Lt,
            parser::Rule::cmp_le => CmpOp::Le,
            parser::Rule::cmp_gt => CmpOp::Gt,
            parser::Rule::cmp_ge => CmpOp::Ge,

            _ => unexpected_pair!(pair),
        }
    }

    fn get_ir_opp(&self) -> &'static str {
        match self {
            CmpOp::Eq => "eq",
            CmpOp::Ne => "ne",
            CmpOp::Lt => "slt",
            CmpOp::Le => "sle",
            CmpOp::Gt => "sgt",
            CmpOp::Ge => "sge",
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
}

#[derive(Debug)]
pub struct Cmp {
    operation: CmpOp,
    left: expr::Expr,
    right: expr::Expr,
    symbol: ast::Symbol,
}

impl Cmp {
    pub fn ast(pair: parser::Pair<parser::Rule>) -> Cmp {
        assert!(pair.as_rule() == parser::Rule::cmp);

        let symbol = ast::Symbol::from_pair(&pair);

        let mut pairs = pair.into_inner();

        Cmp {
            left: expr::Expr::ast(pairs.next().unwrap()),
            operation: CmpOp::ast(pairs.next().unwrap()),
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
            .ir(output, context, scope, &mut expression_requsts)?;
        let left = left.into_iter().next().unwrap().unwrap();

        let right = self
            .right
            .ir(output, context, scope, &mut expression_requsts)?;
        let right = right.into_iter().next().unwrap().unwrap();

        if left.data_type != right.data_type {
            return Err(ast::Error {
                symbol: self.symbol.clone(),
                error: Box::new(expr::IncompatibleOperation {
                    operation: self.operation.as_str(),
                    types: vec![left.data_type.clone(), right.data_type.clone()],
                }),
            });
        }

        let result_register = context.claim_register();

        writeln!(
            output,
            "  %{} = icmp {} {} {}, {}",
            result_register,
            self.operation.get_ir_opp(),
            left.data_type,
            left.value,
            right.value,
        )
        .unwrap();

        let result = expr::Res {
            data_type: ast::Type::Bool,
            value: format!("%{}", result_register),
        }
        .fulfill(output, request);

        match result {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_symbol_err(&self.symbol)),
        }
    }
}
