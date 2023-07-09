use std::fmt::Write as _;

use dot::DotLabel;

use crate::ast::{Ident, PoolRef, expression::ExpressionList};

#[derive(Debug, Dot, Hash)]
pub struct Assignment {
    pub id: usize,
    #[graph]
    pub targets: Vec<PoolRef<Ident>>,
    #[graph]
    pub expressions: PoolRef<ExpressionList>,
}

impl DotLabel for Assignment {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}

