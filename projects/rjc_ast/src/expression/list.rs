use std::fmt::Write;

use dot::DotLabel;

use crate::{PoolRef, Symbol};

use super::Expression;

#[derive(Debug, Dot, Hash)]
pub struct ExpressionList {
    pub id: usize,
    #[dot_display]
    pub symbol: Symbol,
    #[dot_edge]
    pub list: Vec<PoolRef<Expression>>,
}

impl DotLabel for ExpressionList {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
