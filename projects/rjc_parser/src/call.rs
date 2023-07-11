use pest::iterators::Pair;
use rjc_ast::{Call, AST, ASTRef, ASTType, Ident, expression::ExpressionList};

use crate::{ASTParser, Rule};

impl ASTParser for Call {
    fn parse(
        pool: &mut AST,
        pair: Pair<Rule>,
    ) -> ASTRef<Self>
    where
        Self: ASTType,
    {
        assert!(pair.as_rule() == Rule::func_call);

        let mut ident = None;
        let mut expressions = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => ident = Some(Ident::parse(pool, pair)),
                Rule::expr_list => expressions = Some(ExpressionList::parse(pool, pair)),

                _ => unexpected_pair!(pair),
            }
        }

        let call = Call {
            id: pool.len(),
            ident: ident.expect("no ident in function call"),
            expressions: expressions.expect("no expression list in function call"),
        };

        pool.add(call)
    }
}
