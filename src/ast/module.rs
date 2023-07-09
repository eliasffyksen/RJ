use super::*;

#[derive(Debug, Dot)]
pub struct Module {
    #[Graph]
    pub functions: Vec<PoolRef<Function>>,
}
