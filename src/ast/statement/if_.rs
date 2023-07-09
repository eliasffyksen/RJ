use std::fmt::Write as _;

use dot::DotLabel;

use crate::ast::{PoolRef, expression::Expression, Block};

#[derive(Debug,Dot,Hash)]
pub struct If {
    pub id: usize,
    #[dot_edge]
    pub condition: PoolRef<Expression>,
    #[dot_edge]
    pub if_block: PoolRef<Block>,
}

impl DotLabel for If {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
