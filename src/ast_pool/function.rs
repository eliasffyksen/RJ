use super::*;

pub struct Function {
    name: String,
    args: Vec<PoolRef<Function>>,
    block: PoolRef<Block>,
    return_types: Vec<Type>,
}
