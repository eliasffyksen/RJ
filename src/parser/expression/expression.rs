use crate::ast;
use crate::parser;

impl parser::ASTParser for ast::expression::Expression {
    fn parse(pool: &mut ast::Pool, mut pair: pest::iterators::Pair<parser::Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        if pair.as_rule() == parser::Rule::expr_elm {
            pair = pair.into_inner().next().expect("no pair in expression");
        }

        let pair = unpred(pair);

        let expression = match pair.as_rule() {
            parser::Rule::literal => ast::expression::Expression::Literal(
                (ast::expression::Literal::parse(pool, pair), pool.len())
            ),
            parser::Rule::ident => ast::expression::Expression::Ident(
                (ast::Ident::parse(pool, pair), pool.len())
            ),
            parser::Rule::cmp => ast::expression::Expression::Cmp(
                (ast::expression::Cmp::parse(pool, pair), pool.len())
            ),

            _ => unexpected_pair!(pair),
        };

        pool.add(expression)
    }
}

fn unpred(pair: parser::Pair<parser::Rule>) -> parser::Pair<parser::Rule> {
    match pair.as_rule() {
        parser::Rule::pred_0 => (),
        parser::Rule::pred_1 => (),
        parser::Rule::pred_2 => (),
        parser::Rule::pred_max => (),

        _ => return pair,
    }

    let pair = pair.into_inner().next().expect("no child in pred");

    return unpred(pair);
}
