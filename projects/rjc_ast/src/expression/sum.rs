use std::{
    collections::hash_map::DefaultHasher,
    fmt::Write as _,
    hash::{Hash, Hasher},
    io::{self, Write},
};

use dot::{Dot, DotLabel};

use crate::PoolRef;

use super::Expression;

#[derive(Debug, Hash, Dot)]
pub struct Sum {
    pub id: usize,
    #[dot_edge]
    pub first: PoolRef<Expression>,
    #[dot_edge]
    pub rest: Vec<SumOp>,
}

#[derive(Debug, Hash)]
pub enum SumOp {
    Add(PoolRef<Expression>),
    Sub(PoolRef<Expression>),
}

impl Dot for SumOp {
    fn dot(&self, output: &mut dyn Write) -> io::Result<String> {
        let mut label = String::new();
        write!(label, "ast_sum_{}", calculate_hash(self)).unwrap();

        match self {
            SumOp::Add(expression) => {
                let to_label = expression.dot(output)?;

                writeln!(output, "{} [ label = \"add\", shape = circle ];", label)?;
                writeln!(output, "{} -> {};", label, to_label)?;
            }
            SumOp::Sub(expression) => {
                let to_label = expression.dot(output)?;

                writeln!(output, "{} [ label =\"sub\", shape = circle ];", label)?;
                writeln!(output, "{} -> {};", label, to_label)?;
            }
        }

        Ok(label)
    }
}

impl DotLabel for Sum {
    fn dot_label(&self) -> String {
        let mut label = String::new();
        write!(label, "ast_node_{}", self.id).unwrap();
        label
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
