use std::fmt::Write as _;

use dot::Dot;

use crate::{ASTRef, Variable, Call};

use super::{Assignment, If, Return};

#[derive(Debug, Hash)]
pub enum Statement {
    VariableDeclaration((ASTRef<Variable>, usize)),
    Call((ASTRef<Call>, usize)),
    Assignment((ASTRef<Assignment>, usize)),
    If((ASTRef<If>, usize)),
    Return((ASTRef<Return>, usize)),
}

impl Dot for Statement {
    fn dot(&self, output: &mut dyn std::io::Write) -> std::io::Result<String> {
        let (id, to_label) = match self {
            Statement::VariableDeclaration((node, id)) => (id, node.dot(output)?),
            Statement::Assignment((node, id)) => (id, node.dot(output)?),
            Statement::If((node, id)) => (id, node.dot(output)?),
            Statement::Return((node, id)) => (id, node.dot(output)?),
            Statement::Call((node, id)) => (id, node.dot(output)?),
        };

        let mut label = String::new();
        write!(label, "ast_node_{}", id).unwrap();

        writeln!(output, "{} [ shape = point ];", label)?;
        writeln!(output, "{} -> {};", label, to_label)?;

        Ok(label)
    }
}
