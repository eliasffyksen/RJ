use std::fmt::Write as _;

use dot::DotLabel;

use super::*;

#[derive(Debug, Dot, Hash)]
pub struct Module {
    pub id: usize,
    #[graph]
    pub functions: Vec<PoolRef<Function>>,
}

impl DotLabel for Module {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}