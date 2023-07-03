use crate::ast;

use super::*;

impl ASTParser for ast::Variable {
    fn parse(pool: &mut ast::Pool, pair: Pair<Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == Rule::var_decl);

        let mut name = None;
        let mut _type = None;

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => name = Some(pair.as_str().to_string()),
                Rule::var_type => _type = Some(ast::Type::from_str(pair.as_str())),

                _ => unexpected_pair!(pair),
            }
        }

        let variable = ast::Variable {
            name: name.expect("no name for variable"),
            _type: _type.expect("no type for variable"),
        };

        pool.add(variable)
    }
}
