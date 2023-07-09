use std::fmt::Write as _;

use dot::DotLabel;

use super::*;

#[derive(Debug, Dot, Hash)]
pub struct Variable {
    pub id: usize,
    #[display]
    pub symbol: Symbol,
    #[display]
    pub name: String,
    pub type_: Type,
}

impl DotLabel for Variable {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
