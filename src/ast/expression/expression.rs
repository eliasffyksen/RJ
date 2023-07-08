use crate::ast;
use super::*;

#[derive(Debug)]
pub enum Expression {
    Constant(ast::PoolRef<Literal>),
}
