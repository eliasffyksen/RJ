use std::fmt::Write as _;

use dot::DotLabel;

use crate::{expression::Expression, Block, ASTRef};

#[derive(Debug, Dot, Hash)]
pub struct If {
    pub id: usize,
    #[dot_edge]
    pub condition: ASTRef<Expression>,
    #[dot_edge]
    pub if_block: ASTRef<Block>,
}

impl DotLabel for If {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
