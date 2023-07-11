use pest::iterators::Pair;

use rjc_ast::{
    expression::{Cmp, CmpOp, Expression},
    AST, ASTRef, ASTType,
};

use crate::{ASTParser, Rule};

fn cmp_op_from_pair(pair: Pair<Rule>) -> CmpOp {
    match pair.as_rule() {
        Rule::cmp_eq => CmpOp::Eq,
        Rule::cmp_ne => CmpOp::Ne,
        Rule::cmp_le => CmpOp::Le,
        Rule::cmp_ge => CmpOp::Ge,
        Rule::cmp_lt => CmpOp::Lt,
        Rule::cmp_gt => CmpOp::Gt,

        _ => panic!("invalid pair as CmpOp {:?}", pair.as_rule()),
    }
}

impl ASTParser for Cmp {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::cmp);

        let mut expressions = vec![];
        let mut cmp_op = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::pred_1 => expressions.push(Expression::parse(pool, pair)),

                _ => cmp_op = Some(cmp_op_from_pair(pair)),
            }
        }

        let mut expressions = expressions.into_iter();
        let left = expressions
            .next()
            .expect("no expressions in compare expression");
        let right = expressions
            .next()
            .expect("only one expressions in compare expression");

        let cmp = Cmp {
            id: pool.len(),
            op: cmp_op.expect("no compare operation in compare expression"),
            left,
            right,
        };

        pool.add(cmp)
    }
}
