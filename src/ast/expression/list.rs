
use super::*;
use crate::ast;

#[derive(Debug)]
pub struct List {
    pub symbol: ast::Symbol,
    pub list: Vec<ast::PoolRef<Expression>>,
}
