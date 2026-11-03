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

    let mut structs = vec![];
    let mut functions = vec![];

    for pair in module.into_inner() {
        match pair.as_rule() {
            Rule::struct_def => structs.push(struct_def(pair)?),
            Rule::func => functions.push(function(pair)?),
            Rule::EOI => {}
            rule => unreachable!("unexpected {rule:?} in module"),
        }
    }

    Ok(Module { structs, functions })
}

fn struct_def(pair: Pair<Rule>) -> Result<StructDef> {
    let span = span(&pair);
    let mut inner = pair.into_inner();

    Ok(StructDef {
        name: ident(inner.next().unwrap()),
        fields: inner.map(var_decl).collect::<Result<_>>()?,
        span,
    })
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
            Rule::while_stmt => "while loop",
            Rule::var_decl => "variable declaration",
            Rule::var_type
            | Rule::ret_type
            | Rule::scalar_type
            | Rule::array_type
            | Rule::named_type => "type",
            Rule::postfix => "expression",
            Rule::index_suffix => "`[`",
            Rule::field_suffix => "`.`",
            Rule::struct_lit => "struct",
            Rule::field_init => "field",
            Rule::struct_def => "struct definition",
            Rule::array_list | Rule::array_repeat => "array",
            Rule::array_len => "array length",
            Rule::place => "variable",
            Rule::stmt => "statement",
            Rule::func => "function",
            Rule::expr_list => "expression list",
            Rule::EOI => "end of file",
            Rule::cmp_eq => "`==`",
            Rule::cmp_ne => "`!=`",
            Rule::cmp_lt => "`<`",
            Rule::cmp_le => "`<=`",
            Rule::cmp_gt => "`>`",
            Rule::cmp_ge => "`>=`",
            Rule::add => "`+`",
            Rule::sub => "`-`",
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
            Rule::var_decl => params.push(var_decl(pair)?),
            Rule::ret_type => returns.push(ty(pair.into_inner().next().unwrap())?),
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

fn ty(pair: Pair<Rule>) -> Result<TypeExpr> {
    debug_assert_eq!(pair.as_rule(), Rule::var_type);
    let span = span(&pair);
    let pair = pair.into_inner().next().unwrap();

    let kind = match pair.as_rule() {
        Rule::scalar_type => match pair.as_str() {
            "i32" => TypeExprKind::I32,
            "bool" => TypeExprKind::Bool,
            other => unreachable!("unknown type {other:?}"),
        },
        Rule::array_type => {
            let mut inner = pair.into_inner();
            let elem = ty(inner.next().unwrap())?;
            let len = array_len(inner.next().unwrap())?;

            TypeExprKind::Array(Box::new(elem), len)
        }
        Rule::named_type => TypeExprKind::Named(ident(pair.into_inner().next().unwrap())),
        rule => unreachable!("unexpected {rule:?} in type"),
    };

    Ok(TypeExpr { kind, span })
}

fn array_len(pair: Pair<Rule>) -> Result<u32> {
    pair.as_str()
        .parse()
        .map_err(|_| Error::new("array length is too large", span(&pair)))
}

fn var_decl(pair: Pair<Rule>) -> Result<VarDecl> {
    let span = span(&pair);
    let mut inner = pair.into_inner();

    Ok(VarDecl {
        name: ident(inner.next().unwrap()),
        ty: ty(inner.next().unwrap())?,
        span,
    })
}

fn place(pair: Pair<Rule>) -> Result<Place> {
    let span = span(&pair);
    let mut inner = pair.into_inner();

    let name = ident(inner.next().unwrap());
    let mut path = vec![];

    for elem in inner {
        match elem.as_rule() {
            Rule::index_suffix => {
                path.push(PlaceElem::Index(expr(elem.into_inner().next().unwrap())?))
            }
            Rule::field_suffix => {
                path.push(PlaceElem::Field(ident(elem.into_inner().next().unwrap())))
            }
            rule => unreachable!("unexpected {rule:?} in place"),
        }
    }

    Ok(Place { name, path, span })
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
        Rule::var_decl => StmtKind::Decl(var_decl(pair)?),

        Rule::func_ret => StmtKind::Return(expr_list(pair.into_inner().next().unwrap())?),

        Rule::assign => {
            let mut targets = vec![];
            let mut values = vec![];

            for pair in pair.into_inner() {
                match pair.as_rule() {
                    Rule::place => targets.push(place(pair)?),
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

        Rule::while_stmt => {
            let mut inner = pair.into_inner();

            StmtKind::While {
                cond: expr(inner.next().unwrap())?,
                body: block(inner.next().unwrap())?,
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

        Rule::array_list => ExprKind::ArrayList(pair.into_inner().map(expr).collect::<Result<_>>()?),

        Rule::array_repeat => {
            let mut inner = pair.into_inner();
            let value = expr(inner.next().unwrap())?;
            let len = array_len(inner.next().unwrap())?;

            ExprKind::ArrayRepeat(Box::new(value), len)
        }

        Rule::struct_lit => {
            let mut inner = pair.into_inner();
            let name = ident(inner.next().unwrap());
            let mut fields = vec![];

            for field in inner {
                let mut field = field.into_inner();
                let name = ident(field.next().unwrap());
                fields.push((name, expr(field.next().unwrap())?));
            }

            ExprKind::StructLit { name, fields }
        }

        // `a.b[i].c` parses as one flat sequence; fold it left to right.
        Rule::postfix => {
            let mut inner = pair.into_inner();
            let mut base = expr(inner.next().unwrap())?;

            for suffix in inner {
                let suffix_span = self::span(&suffix);
                let span = Span::new(base.span.start, suffix_span.end);
                let rule = suffix.as_rule();
                let inner = suffix.into_inner().next().unwrap();

                let kind = match rule {
                    Rule::index_suffix => ExprKind::Index {
                        base: Box::new(base),
                        index: Box::new(expr(inner)?),
                    },
                    Rule::field_suffix => ExprKind::Field {
                        base: Box::new(base),
                        field: ident(inner),
                    },
                    rule => unreachable!("unexpected {rule:?} in postfix"),
                };

                base = Expr { kind, span };
            }

            return Ok(base);
        }

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

        assert!(matches!(module.functions[0].params[0].ty.kind, TypeExprKind::Bool));
        assert!(matches!(module.functions[0].returns[..], [TypeExpr { kind: TypeExprKind::Bool, .. }]));
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
    fn skips_comments() {
        let module = parse(
            "// leading
             fn f(): i32 { // trailing
               return 1 // after
             }",
        )
        .unwrap();

        assert_eq!(module.functions[0].body.stmts.len(), 1);
    }

    #[test]
    fn parses_array_types_and_indexing() {
        let module = parse("fn f(a: [bool; 4]): [i32; 2] { a[1] = a[0] return [0; 2] }").unwrap();

        let TypeExprKind::Array(elem, 4) = &module.functions[0].params[0].ty.kind else {
            panic!("expected [bool; 4]");
        };
        assert!(matches!(elem.kind, TypeExprKind::Bool));
    }

    #[test]
    fn parses_structs() {
        let module = parse(
            "struct Inner { v: [i32; 2] }
             struct Outer { a: Inner, b: bool, }
             fn f(o: Outer): i32 {
               o.a.v[1] = Outer(a: Inner(v: [1, 2]), b: true).a.v[0]
               return o.a.v[1]
             }",
        )
        .unwrap();

        assert_eq!(module.structs.len(), 2);
        assert_eq!(module.structs[1].fields.len(), 2);

        let StmtKind::Assign { targets, values } = &module.functions[0].body.stmts[0].kind else {
            panic!("expected assignment");
        };
        assert_eq!(targets[0].path.len(), 3);
        assert!(matches!(values[0].kind, ExprKind::Index { .. }));
    }

    #[test]
    fn struct_literal_is_not_confused_with_a_block() {
        let module = parse("fn f(done: bool): i32 { if done { return 1 } return 0 }").unwrap();
        assert!(matches!(module.functions[0].body.stmts[0].kind, StmtKind::If { .. }));
    }

    #[test]
    fn rejects_out_of_range_int() {
        let source = "fn f(): i32 { return 99999999999 }";
        let err = parse(source).unwrap_err();

        assert_eq!(&source[err.span.start..err.span.end], "99999999999");
    }
}
