use std::fmt::Write as _;

use dot::Dot;

use crate::ast::*;
use super::*;

#[derive(Debug, Hash)]
pub enum Statement {
    VariableDeclaration((PoolRef<Variable>, usize)),
    Assignment((PoolRef<Assignment>, usize)),
    If((PoolRef<If>, usize)),
    Return((PoolRef<Return>, usize)),
}

impl Dot for Statement {
    fn dot(&self, output: &mut dyn std::io::Write) -> std::io::Result<String> {
        let (id, to_label) = match self {
            Statement::VariableDeclaration((node, id)) => (id, node.dot(output)?),
            Statement::Assignment((node, id)) => (id, node.dot(output)?),
            Statement::If((node, id)) => (id, node.dot(output)?),
            Statement::Return((node, id)) => (id, node.dot(output)?),
        };

        let mut label = String::new();
        write!(label, "ast_node_{}", id).unwrap();

        writeln!(output, "{} [ shape = point ];", label)?;
        writeln!(output, "{} -> {};", label, to_label)?;

        Ok(label)
    }
}
