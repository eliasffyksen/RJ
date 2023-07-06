use crate::ast::*;
use super::*;

#[derive(Debug)]
pub enum Statement {
    VariableDeclaration(PoolRef<Variable>),
    Return(PoolRef<Return>),
}
