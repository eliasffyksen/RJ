use crate::ast;

#[derive(Debug)]
pub struct Return {
    pub symbol: ast::Symbol,
    pub expressions: ast::PoolRef<ast::expression::List>,
}
