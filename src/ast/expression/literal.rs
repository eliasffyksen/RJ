use crate::ast;

#[derive(Debug, Dot)]
pub struct Literal {
    #[Display]
    pub symbol: ast::Symbol,
    #[Display]
    pub value: String,
}