use crate::ast;

#[derive(Debug, Dot)]
pub struct Return {
    #[Display]
    pub symbol: ast::Symbol,
    #[Graph]
    pub expressions: ast::PoolRef<ast::expression::ExpressionList>,
}
