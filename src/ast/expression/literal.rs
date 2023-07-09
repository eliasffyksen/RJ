use dot::DotLabel;
use std::fmt::Write;

use crate::ast;

#[derive(Debug, Hash, Dot)]
pub struct Literal {
    pub id: usize,
    #[display]
    pub symbol: ast::Symbol,
    #[display]
    pub value: String,
}

impl DotLabel for Literal {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}