use super::*;

#[derive(Debug, Dot)]
pub struct Block {
    #[Display]
    pub symbol: Symbol,
    #[Graph]
    pub statements: Vec<PoolRef<statement::Statement>>,
}
