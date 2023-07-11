use std::fmt::Write;

use dot::DotLabel;

use crate::{Symbol, ASTRef, expression::ExpressionList};

#[derive(Debug, Dot, Hash)]
pub struct Return {
    pub id: usize,
    #[dot_display]
    pub symbol: Symbol,
    #[dot_edge]
    pub expressions: ASTRef<ExpressionList>,
}

impl DotLabel for Return {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
