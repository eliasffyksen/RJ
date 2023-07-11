use std::fmt::Write as _;

use dot::DotLabel;

use crate::expression::ExpressionList;

use super::{Ident, ASTRef};

#[derive(Debug, Hash, Dot)]
pub struct Call {
    pub id: usize,
    #[dot_edge]
    pub ident: ASTRef<Ident>,
    #[dot_edge]
    pub expressions: ASTRef<ExpressionList>,
}

impl DotLabel for Call {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
