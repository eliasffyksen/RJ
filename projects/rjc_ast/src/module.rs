use std::fmt::Write as _;

use dot::DotLabel;

use crate::{Function, PoolRef};

#[derive(Debug, Dot, Hash)]
pub struct Module {
    pub id: usize,
    #[dot_edge]
    pub functions: Vec<PoolRef<Function>>,
}

impl DotLabel for Module {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
