macro_rules! unexpected_pair {
    ($pair:expr) => {
        panic!("Unexpected pair {}", $pair)
    };
}

pub mod expr;
pub mod scope;
pub mod stmt;

mod ast_type;
mod file;
mod function;
mod ident;
mod ircontext;
mod symbol;
mod error;

pub use ast_type::Type;
pub use file::File;
pub use function::Func;
pub use ident::Ident;
pub use ircontext::IRContext;
pub use symbol::Symbol;
pub use error::Error;
