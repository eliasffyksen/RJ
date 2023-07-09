use std::fmt::Write as _;

use dot::DotLabel;

use super::Symbol;

#[derive(Debug, Dot, Hash)]
pub struct Ident {
    pub id: usize,
    #[display]
    pub symbol: Symbol,
    #[display]
    pub name: String,
}

impl DotLabel for Ident {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
