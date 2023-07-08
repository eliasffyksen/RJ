#![feature(iterator_try_collect)]
#![allow(dead_code)]

use std::io;

extern crate pest;
#[macro_use]
extern crate pest_derive;

mod ast;
mod config;
mod parser;

fn main() -> io::Result<()> {
    let config = config::Config::new();

    let pool = parser::from_file(config.file_name)?;

    if config.emit_ast {
        println!("{:#?}", pool);
    }

    // let mut out = std::io::stdout();

    // let mut context: ast::IRContext = Default::default();

    // if config.emit_llvm {
    //     return file.ir(&mut out, &mut context);
    // }

    Ok(())
}
