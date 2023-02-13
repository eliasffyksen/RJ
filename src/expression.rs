use pest::iterators::Pair;

use crate::ident::{Ident, IdentImpl};
use crate::{Rule, check_rule, unexpected_pair};

#[derive(Debug)]
pub enum Expression {
    Ident(Ident),
}

impl Expression {
    pub fn ast(pair: Pair<Rule>) -> Vec<Expression> {
        check_rule(&pair, Rule::expr);

        let mut expressions = vec![];

        for element in pair.into_inner() {
            match element.as_rule() {
                Rule::expr_elm => expressions.push(Self::ast_expression_element(element)),

                _ => unexpected_pair(&element),
            }
        }

        expressions
    }

    fn ast_expression_element(pair: Pair<Rule>) -> Expression {
        check_rule(&pair, Rule::expr_elm);

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => return Expression::Ident(Ident::ast(pair)),

                _ => unexpected_pair(&pair),
            }
        }

        panic!("No pair in expression");
    }
}
