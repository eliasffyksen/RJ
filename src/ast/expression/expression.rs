use crate::ast;

#[derive(Debug)]
pub struct Constant {
    pub symbol: ast::Symbol,
    pub value: String,
}

#[derive(Debug)]
pub enum Expression {
    Constant(ast::PoolRef<Constant>),
}
