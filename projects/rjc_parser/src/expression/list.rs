use pest::iterators::Pair;

use rjc_ast::{
    expression::{Expression, ExpressionList},
    AST, ASTRef, ASTType, Symbol,
};

use crate::{ASTParser, Rule, SymbolFromPair};

impl ASTParser for ExpressionList {
    fn parse(pool: &mut AST, pair: Pair<Rule>) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::expr_list);

        let symbol = Symbol::from_pair(&pair);
        let mut expressions = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::expr_elm => expressions.push(Expression::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let expression_list = ExpressionList {
            id: pool.len(),
            symbol,
            list: expressions,
        };

        pool.add(expression_list)
    }
}
