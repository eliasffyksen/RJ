use crate::ast;

#[derive(Debug)]
pub struct Return {
    pub expressions: ast::PoolRef<ast::expression::List>
}
