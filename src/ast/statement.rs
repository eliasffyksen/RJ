use super::{Variable, PoolRef};


#[derive(Debug)]
pub enum Statement {
    Variable(PoolRef<Variable>),
}
