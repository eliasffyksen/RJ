use std::fmt::Write as _;

use dot::DotLabel;

use super::{expression::ExpressionList, Ident, PoolRef};

#[derive(Debug, Hash, Dot)]
pub struct Call {
    pub id: usize,
    #[dot_edge]
    pub ident: PoolRef<Ident>,
    #[dot_edge]
    pub expressions: PoolRef<ExpressionList>,
}

impl DotLabel for Call {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
