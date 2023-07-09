use dot::DotLabel;
use std::fmt::Write;

use crate::ast;

#[derive(Debug, Dot, Hash)]
pub struct Return {
    pub id: usize,
    #[dot_display]
    pub symbol: ast::Symbol,
    #[dot_edge]
    pub expressions: ast::PoolRef<ast::expression::ExpressionList>,
}

impl DotLabel for Return {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
