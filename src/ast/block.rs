use super::*;

#[derive(Debug)]
pub struct Block {
    pub statements: Vec<PoolRef<statement::Statement>>,
}
