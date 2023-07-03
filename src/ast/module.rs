use super::*;

#[derive(Debug)]
pub struct Module {
    pub functions: Vec<PoolRef<Function>>,
}
