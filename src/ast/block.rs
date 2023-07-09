use std::fmt::Write;

use dot::DotLabel;

use super::*;

#[derive(Debug, Dot, Hash)]
pub struct Block {
    pub id: usize,
    #[display]
    pub symbol: Symbol,
    #[graph]
    pub statements: Vec<PoolRef<statement::Statement>>,
}

impl DotLabel for Block {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}