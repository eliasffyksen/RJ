use crate::ast;
use super::*;

#[derive(Debug, Dot)]
pub enum Expression {
    Constant(ast::PoolRef<Literal>),
}
