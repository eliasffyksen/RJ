use crate::ast::*;
use super::*;

#[derive(Debug, Dot)]
pub enum Statement {
    VariableDeclaration(PoolRef<Variable>),
    Return(PoolRef<Return>),
}
