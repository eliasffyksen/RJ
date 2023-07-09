use std::fmt::Write as _;

use dot::Dot;

use super::*;
use crate::ast;

#[derive(Debug, Hash)]
pub enum Expression {
    Literal((ast::PoolRef<Literal>, usize)),
    Ident((ast::PoolRef<ast::Ident>, usize)),
    Cmp((ast::PoolRef<ast::expression::Cmp>, usize)),
}

impl Dot for Expression {
    fn dot(&self, output: &mut dyn std::io::Write) -> std::io::Result<String> {
        let (to_label, id) = match self {
            Expression::Literal((node, id)) => (node.dot(output)?, *id),
            Expression::Ident((node, id)) => (node.dot(output)?, *id),
            Expression::Cmp((node, id)) => (node.dot(output)?, *id),
        };

        let mut label = String::new();
        write!(label, "ast_node_{}", id).unwrap();

        writeln!(output, "{} [ shape = point ];", label)?;
        writeln!(output, "{} -> {};", label, to_label)?;

        Ok(label)
    }
}
