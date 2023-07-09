
use super::*;
use crate::ast;

#[derive(Debug, Dot)]
pub struct ExpressionList {
    #[Display]
    pub symbol: ast::Symbol,
    #[Graph]
    pub list: Vec<ast::PoolRef<Expression>>,
}
