use super::*;

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub args: Vec<PoolRef<Variable>>,
    pub block: PoolRef<Block>,
    pub return_type: Vec<Type>,
}
