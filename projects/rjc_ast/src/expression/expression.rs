use std::fmt::Write as _;

use dot::Dot;

use crate::{Call, Ident, PoolRef};

use super::{Cmp, Literal, Sum};

#[derive(Debug, Hash)]
pub enum Expression {
    Literal((PoolRef<Literal>, usize)),
    Ident((PoolRef<Ident>, usize)),
    Call((PoolRef<Call>, usize)),
    Cmp((PoolRef<Cmp>, usize)),
    Sum((PoolRef<Sum>, usize)),
}

impl Dot for Expression {
    fn dot(&self, output: &mut dyn std::io::Write) -> std::io::Result<String> {
        let (to_label, id) = match self {
            Expression::Literal((node, id)) => (node.dot(output)?, *id),
            Expression::Ident((node, id)) => (node.dot(output)?, *id),
            Expression::Call((node, id)) => (node.dot(output)?, *id),
            Expression::Cmp((node, id)) => (node.dot(output)?, *id),
            Expression::Sum((node, id)) => (node.dot(output)?, *id),
        };

        let mut label = String::new();
        write!(label, "ast_node_{}", id).unwrap();

        writeln!(output, "{} [ shape = point ];", label)?;
        writeln!(output, "{} -> {};", label, to_label)?;

        Ok(label)
    }
}
