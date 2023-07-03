use super::*;
use crate::ast;

impl ASTParser for ast::Function {
    fn parse(pool: &mut ast::Pool, pair: Pair<Rule>) -> ast::PoolRef<Self>
    where
        Self: ast::PoolType,
    {
        assert!(pair.as_rule() == Rule::func);

        let mut name = None;
        let mut args = vec![];
        let mut block = None;
        let mut return_type = vec![];

        for pair in pair.into_inner() {
            match pair.as_rule() {
                Rule::ident => name = Some(pair.as_str().to_string()),
                Rule::var_decl => args.push(ast::Variable::parse(pool, pair)),
                Rule::block => block = Some(ast::Block::parse(pool, pair)),
                Rule::ret_type => return_type.push(ast::Type::from_str(pair.as_str())),

                _ => unexpected_pair!(pair),
            }
        }

        let function = ast::Function {
            name: name.expect("no name defined for function"),
            args,
            block: block.expect("no block defined"),
            return_type,
        };

        pool.add(function)
    }
}
