use super::*;

#[derive(Debug, Dot)]
pub struct Variable {
    #[Display]
    pub symbol: Symbol,
    #[Display]
    pub name: String,
    pub type_: Type,
}
