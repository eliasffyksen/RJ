use super::*;

#[derive(Debug)]
pub struct Block {
    pub symbol: Symbol,
    pub statements: Vec<PoolRef<statement::Statement>>,
}
