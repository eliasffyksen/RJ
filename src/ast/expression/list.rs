
use super::*;
use crate::ast;

#[derive(Debug)]
pub struct List {
    pub list: Vec<ast::PoolRef<Expression>>,
}
