use std::fmt::Write as _;

use dot::Dot;

use crate::ast;
use super::*;

#[derive(Debug, Hash)]
pub enum Expression {
    Literal((ast::PoolRef<Literal>, usize)),
}

impl Dot for Expression {
    fn dot(&self, output: &mut dyn std::io::Write) -> std::io::Result<String> {
        let (node, id) = match self {
            Expression::Literal(node) => node,
        };

        let mut label = String::new();
        write!(label, "ast_node_{}", id).unwrap();

        let to_label = node.dot(output)?;

        writeln!(output, "{} [ shape = point ];", label)?;
        writeln!(output, "{} -> {};", label, to_label)?;

        Ok(label)
    }
}
