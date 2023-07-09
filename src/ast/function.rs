use std::fmt::Write as _;

use dot::DotLabel;

use super::*;

#[derive(Debug, Dot, Hash)]
pub struct Function {
    pub id: usize,
    #[display]
    pub symbol: Symbol,
    #[display]
    pub name: String,
    #[graph]
    pub args: Vec<PoolRef<Variable>>,
    #[graph]
    pub block: PoolRef<Block>,
    #[display]
    pub return_type: TypeList,
}

impl DotLabel for Function {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}