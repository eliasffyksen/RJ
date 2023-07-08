use crate::ast;

#[derive(Debug)]
pub struct Literal {
    pub symbol: ast::Symbol,
    pub value: String,
}