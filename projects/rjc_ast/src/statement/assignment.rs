use std::fmt::Write as _;

use dot::DotLabel;

use crate::{expression::ExpressionList, Ident, ASTRef};

#[derive(Debug, Dot, Hash)]
pub struct Assignment {
    pub id: usize,
    #[dot_edge]
    pub targets: Vec<ASTRef<Ident>>,
    #[dot_edge]
    pub expressions: ASTRef<ExpressionList>,
}

impl DotLabel for Assignment {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
