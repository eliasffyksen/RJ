//! Converts pest's parse tree into the [`ast`](super::ast).
//!
//! The grammar guarantees the shape of every pair, so structural mismatches
//! are bugs in this file (`unreachable!`). Problems in the user's program are
//! returned as [`Error`]s.

use pest::{error::InputLocation, iterators::Pair, Parser as _};
use pest_derive::Parser;

use super::ast::*;
use crate::diagnostic::{Error, Span};

#[derive(Parser)]
#[grammar = "syntax/rj.pest"]
struct RjParser;

type Result<T> = std::result::Result<T, Error>;

pub fn parse(source: &str) -> Result<Module> {
    let module = RjParser::parse(Rule::module, source)
        .map_err(from_pest_error)?
        .next()
        .unwrap();

    let mut functions = vec![];

    for pair in module.into_inner() {
        match pair.as_rule() {
            Rule::func => functions.push(function(pair)?),
            Rule::EOI => {}
            rule => unreachable!("unexpected {rule:?} in module"),
        }
    }

    Ok(Module { functions })
}

fn from_pest_error(error: pest::error::Error<Rule>) -> Error {
    let span = match error.location {
        InputLocation::Pos(pos) => Span::new(pos, pos),
        InputLocation::Span((start, end)) => Span::new(start, end),
    };

    let error = error.renamed_rules(|rule| {
        match rule {
            Rule::expr_elm | Rule::pred_0 | Rule::pred_1 | Rule::pred_2 | Rule::pred_max => {
                "expression"
            }
            Rule::func_call => "function call",
            Rule::func_ret => "return statement",
            Rule::assign => "assignment",
            Rule::if_stmt => "if statement",
            Rule::var_decl => "variable declaration",
            Rule::var_type | Rule::ret_type => "type",
            Rule::stmt => "statement",
            Rule::func => "function",
            Rule::expr_list => "expression list",
            Rule::EOI => "end of file",
            other => return format!("{other:?}"),
        }
        .to_string()
    });

    Error::new(error.variant.message(), span)
}

fn span(pair: &Pair<Rule>) -> Span {
    let span = pair.as_span();
    Span::new(span.start(), span.end())
}

fn function(pair: Pair<Rule>) -> Result<Function> {
    let span = span(&pair);
    let mut inner = pair.into_inner();
    let name = ident(inner.next().unwrap());

    let mut params = vec![];
    let mut returns = vec![];
    let mut body = None;

    for pair in inner {
        match pair.as_rule() {
            Rule::var_decl => params.push(var_decl(pair)),
            Rule::ret_type => returns.push(ty(pair.into_inner().next().unwrap())),
            Rule::block => body = Some(block(pair)?),
            rule => unreachable!("unexpected {rule:?} in function"),
        }
    }

    Ok(Function {
        name,
        params,
        returns,
        body: body.unwrap(),
        span,
    })
}

fn ident(pair: Pair<Rule>) -> Ident {
    debug_assert_eq!(pair.as_rule(), Rule::ident);

    Ident {
        name: pair.as_str().to_string(),
        span: span(&pair),
    }
}

fn ty(pair: Pair<Rule>) -> Type {
    match pair.as_str() {
        "i32" => Type::I32,
        "bool" => Type::Bool,
        other => unreachable!("unknown type {other:?}"),
    }
}

fn var_decl(pair: Pair<Rule>) -> VarDecl {
    let span = span(&pair);
    let mut inner = pair.into_inner();

    VarDecl {
        name: ident(inner.next().unwrap()),
        ty: ty(inner.next().unwrap()),
        span,
    }
}

fn block(pair: Pair<Rule>) -> Result<Block> {
    let span = span(&pair);
    let stmts = pair.into_inner().map(stmt).collect::<Result<_>>()?;

    Ok(Block { stmts, span })
}

fn stmt(pair: Pair<Rule>) -> Result<Stmt> {
    let span = span(&pair);
    let pair = pair.into_inner().next().unwrap();

    let kind = match pair.as_rule() {
        Rule::var_decl => StmtKind::Decl(var_decl(pair)),

        Rule::func_ret => StmtKind::Return(expr_list(pair.into_inner().next().unwrap())?),

        Rule::assign => {
            let mut targets = vec![];
            let mut values = vec![];

            for pair in pair.into_inner() {
                match pair.as_rule() {
                    Rule::ident => targets.push(ident(pair)),
                    Rule::expr_list => values = expr_list(pair)?,
                    rule => unreachable!("unexpected {rule:?} in assignment"),
                }
            }

            StmtKind::Assign { targets, values }
        }

        Rule::func_call => StmtKind::Call(call(pair)?),

        Rule::if_stmt => {
            let mut inner = pair.into_inner();

            StmtKind::If {
                cond: expr(inner.next().unwrap())?,
                then_block: block(inner.next().unwrap())?,
                else_block: inner.next().map(block).transpose()?,
            }
        }

        rule => unreachable!("unexpected {rule:?} in statement"),
    };

    Ok(Stmt { kind, span })
}

fn expr_list(pair: Pair<Rule>) -> Result<Vec<Expr>> {
    debug_assert_eq!(pair.as_rule(), Rule::expr_list);

    pair.into_inner().map(expr).collect()
}

fn call(pair: Pair<Rule>) -> Result<Call> {
    let mut inner = pair.into_inner();

    Ok(Call {
        callee: ident(inner.next().unwrap()),
        args: expr_list(inner.next().unwrap())?,
    })
}

fn expr(mut pair: Pair<Rule>) -> Result<Expr> {
    // Skip the precedence-level wrapper rules.
    while matches!(
        pair.as_rule(),
        Rule::expr_elm | Rule::pred_0 | Rule::pred_1 | Rule::pred_2 | Rule::pred_max | Rule::literal
    ) {
        pair = pair.into_inner().next().unwrap();
    }

    let span = span(&pair);

    let kind = match pair.as_rule() {
        Rule::int => {
            let value = pair
                .as_str()
                .parse()
                .map_err(|_| Error::new("integer literal does not fit in i32", span))?;

            ExprKind::Int(value)
        }

        Rule::bool => ExprKind::Bool(pair.as_str() == "true"),

        Rule::ident => ExprKind::Var(ident(pair)),

        Rule::func_call => ExprKind::Call(call(pair)?),

        Rule::cmp => {
            let mut inner = pair.into_inner();
            let lhs = expr(inner.next().unwrap())?;
            let op = match inner.next().unwrap().as_rule() {
                Rule::cmp_eq => BinOp::Eq,
                Rule::cmp_ne => BinOp::Ne,
                Rule::cmp_lt => BinOp::Lt,
                Rule::cmp_le => BinOp::Le,
                Rule::cmp_gt => BinOp::Gt,
                Rule::cmp_ge => BinOp::Ge,
                rule => unreachable!("unexpected {rule:?} as comparison operator"),
            };
            let rhs = expr(inner.next().unwrap())?;

            ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }
        }

        // `a + b - c` parses as one flat sequence; fold it left-associatively.
        Rule::sum => {
            let mut inner = pair.into_inner();
            let mut lhs = expr(inner.next().unwrap())?;

            while let Some(op) = inner.next() {
                let op = match op.as_rule() {
                    Rule::add => BinOp::Add,
                    Rule::sub => BinOp::Sub,
                    rule => unreachable!("unexpected {rule:?} as sum operator"),
                };
                let rhs = expr(inner.next().unwrap())?;
                let span = Span::new(lhs.span.start, rhs.span.end);

                lhs = Expr {
                    kind: ExprKind::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    span,
                };
            }

            return Ok(lhs);
        }

        rule => unreachable!("unexpected {rule:?} in expression"),
    };

    Ok(Expr { kind, span })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_test_programs() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test");

        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();

            if let Err(err) = parse(&source) {
                panic!("{}", err.render(&path.display().to_string(), &source));
            }
        }
    }

    #[test]
    fn sums_are_left_associative() {
        let module = parse("fn f(): i32 { return 1 - 2 + 3 }").unwrap();
        let StmtKind::Return(values) = &module.functions[0].body.stmts[0].kind else {
            panic!("expected return");
        };
        let ExprKind::Binary { op, lhs, .. } = &values[0].kind else {
            panic!("expected binary expression");
        };

        assert_eq!(*op, BinOp::Add);
        assert!(matches!(lhs.kind, ExprKind::Binary { op: BinOp::Sub, .. }));
    }

    #[test]
    fn parses_bool_type() {
        let module = parse("fn f(b: bool): bool { return b }").unwrap();

        assert_eq!(module.functions[0].params[0].ty, Type::Bool);
        assert_eq!(module.functions[0].returns, vec![Type::Bool]);
    }

    #[test]
    fn keywords_are_not_identifiers() {
        let module = parse("fn f(): bool { return true }").unwrap();
        let StmtKind::Return(values) = &module.functions[0].body.stmts[0].kind else {
            panic!("expected return");
        };
        assert!(matches!(values[0].kind, ExprKind::Bool(true)));

        let module = parse("fn f(trueish: bool): bool { return trueish }").unwrap();
        assert_eq!(module.functions[0].params[0].name.name, "trueish");

        assert!(parse("fn f(return: i32) {}").is_err());
    }

    #[test]
    fn rejects_out_of_range_int() {
        let source = "fn f(): i32 { return 99999999999 }";
        let err = parse(source).unwrap_err();

        assert_eq!(&source[err.span.start..err.span.end], "99999999999");
    }
}
