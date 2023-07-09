use super::*;

#[derive(Debug, Dot)]
pub struct Function {
    #[Display]
    pub symbol: Symbol,
    #[Display]
    pub name: String,
    #[Graph]
    pub args: Vec<PoolRef<Variable>>,
    // #[Graph]
    pub block: PoolRef<Block>,
    // #[Graph]
    pub return_type: Vec<Type>,
}
