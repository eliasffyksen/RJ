use std::fmt::Write as _;

use dot::DotLabel;

use crate::ast::{PoolRef, expression::Expression, Block};

#[derive(Debug,Dot,Hash)]
pub struct If {
    pub id: usize,
    #[graph]
    pub condition: PoolRef<Expression>,
    #[graph]
    pub if_block: PoolRef<Block>,
}

impl DotLabel for If {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
