use std::fmt::Write;

use dot::DotLabel;

use crate::{statement::Statement, ASTRef, Symbol};

#[derive(Debug, Dot, Hash)]
pub struct Block {
    pub id: usize,
    #[dot_display]
    pub symbol: Symbol,
    #[dot_edge]
    pub statements: Vec<ASTRef<Statement>>,
}

impl DotLabel for Block {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
