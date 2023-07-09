use dot::DotLabel;
use std::fmt::Write;

use crate::ast;

#[derive(Debug, Dot, Hash)]
pub struct Return {
    pub id: usize,
    #[display]
    pub symbol: ast::Symbol,
    #[graph]
    pub expressions: ast::PoolRef<ast::expression::ExpressionList>,
}

impl DotLabel for Return {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
