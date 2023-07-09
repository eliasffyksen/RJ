use crate::{
    ast::expression::{Expression, Sum, SumOp},
    parser::{ASTParser, Rule},
};

impl ASTParser for Sum {
    fn parse(
        pool: &mut crate::ast::Pool,
        pair: pest::iterators::Pair<crate::parser::Rule>,
    ) -> crate::ast::PoolRef<Self>
    where
        Self: crate::ast::PoolType,
    {
        assert!(pair.as_rule() == Rule::sum);

        let mut pairs = pair.into_inner();

        let first = Expression::parse(
            pool,
            pairs.next().expect("no first expression in summation"),
        );

        let mut rest = vec![];

        while let Some(op_pair) = pairs.next() {
            let expression = Expression::parse(
                pool,
                pairs.next().expect("not enough expressions in summation"),
            );

            let sum_op = match op_pair.as_rule() {
                Rule::add => SumOp::Add(expression),
                Rule::sub => SumOp::Sub(expression),

                _ => unexpected_pair!(op_pair),
            };

            rest.push(sum_op);
        }

        let sum = Sum {
            id: pool.len(),
            first,
            rest,
        };

        pool.add(sum)
    }
}
