
use dot::DotLabel;
use std::fmt::Write;

use super::*;
use crate::ast;

#[derive(Debug, Dot, Hash)]
pub struct ExpressionList {
    pub id: usize,
    #[display]
    pub symbol: ast::Symbol,
    #[graph]
    pub list: Vec<ast::PoolRef<Expression>>,
}

impl DotLabel for ExpressionList {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}