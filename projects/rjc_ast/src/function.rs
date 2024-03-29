use std::fmt::Write as _;

use dot::DotLabel;

use crate::{Block, Ident, ASTRef, Symbol, TypeList, Variable};

#[derive(Debug, Dot, Hash)]
pub struct Function {
    pub id: usize,
    #[dot_display]
    pub symbol: Symbol,
    #[dot_edge]
    pub ident: ASTRef<Ident>,
    #[dot_edge]
    pub args: Vec<ASTRef<Variable>>,
    #[dot_edge]
    pub block: ASTRef<Block>,
    #[dot_display]
    pub return_type: TypeList,
}

impl DotLabel for Function {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
