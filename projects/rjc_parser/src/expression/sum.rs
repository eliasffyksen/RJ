use pest::iterators::Pair;

use rjc_ast::{
    expression::{Expression, Sum, SumOp},
    Pool, PoolRef, PoolType,
};

use crate::{ASTParser, Rule};

impl ASTParser for Sum {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType,
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
