use std::fmt::{self, Display, Write as _};

use dot::DotLabel;

use crate::{expression::Expression, ASTRef};

#[derive(Debug, Hash, Dot)]
pub struct Cmp {
    pub id: usize,
    #[dot_display]
    pub op: CmpOp,
    #[dot_edge]
    pub left: ASTRef<Expression>,
    #[dot_edge]
    pub right: ASTRef<Expression>,
}

#[derive(Debug, Hash)]
pub enum CmpOp {
    Eq,
    Ne,
    Le,
    Ge,
    Lt,
    Gt,
}

impl Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        let op = match self {
            CmpOp::Eq => "eq",
            CmpOp::Ne => "ne",
            CmpOp::Le => "le",
            CmpOp::Ge => "ge",
            CmpOp::Lt => "lt",
            CmpOp::Gt => "gt",
        };

        write!(f, "{}", op)
    }
}

impl DotLabel for Cmp {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}
