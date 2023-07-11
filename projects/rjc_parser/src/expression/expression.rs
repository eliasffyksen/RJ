use pest::iterators::Pair;

use rjc_ast::{
    expression::{Cmp, Expression, Literal, Sum},
    Call, Ident, AST, ASTRef, ASTType,
};

use crate::{ASTParser, Rule};

impl ASTParser for Expression {
    fn parse(pool: &mut AST, mut pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        if pair.as_rule() == Rule::expr_elm {
            pair = pair.into_inner().next().expect("no pair in expression");
        }

        let pair = unpred(pair);

        let expression = match pair.as_rule() {
            Rule::literal => Expression::Literal((Literal::parse(pool, pair), pool.len())),
            Rule::ident => Expression::Ident((Ident::parse(pool, pair), pool.len())),
            Rule::func_call => Expression::Call((Call::parse(pool, pair), pool.len())),
            Rule::cmp => Expression::Cmp((Cmp::parse(pool, pair), pool.len())),
            Rule::sum => Expression::Sum((Sum::parse(pool, pair), pool.len())),

            _ => unexpected_pair!(pair),
        };

        pool.add(expression)
    }
}

fn unpred(pair: Pair<Rule>) -> Pair<Rule> {
    match pair.as_rule() {
        Rule::pred_0 => (),
        Rule::pred_1 => (),
        Rule::pred_2 => (),
        Rule::pred_max => (),

        _ => return pair,
    }

    let pair = pair.into_inner().next().expect("no child in pred");

    return unpred(pair);
}
