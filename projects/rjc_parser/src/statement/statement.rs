use pest::iterators::Pair;

use rjc_ast::{
    statement::{Assignment, If, Return, Statement},
    Call, Pool, PoolRef, PoolType, Variable,
};

use crate::{ASTParser, Rule};

impl ASTParser for Statement {
    fn parse(pool: &mut Pool, pair: Pair<Rule>) -> PoolRef<Self>
    where
        Self: PoolType,
    {
        assert!(pair.as_rule() == Rule::stmt);

        let pair = pair.into_inner().next().expect("no value in statement");

        let statement = match pair.as_rule() {
            Rule::var_decl => {
                Statement::VariableDeclaration((Variable::parse(pool, pair), pool.len()))
            }
            Rule::func_ret => Statement::Return((Return::parse(pool, pair), pool.len())),
            Rule::assign => Statement::Assignment((Assignment::parse(pool, pair), pool.len())),
            Rule::if_stmt => Statement::If((If::parse(pool, pair), pool.len())),
            Rule::func_call => Statement::Call((Call::parse(pool, pair), pool.len())),

            _ => unexpected_pair!(pair),
        };

        pool.add(statement)
    }
}
