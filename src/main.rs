#![feature(iterator_try_collect)]
#![allow(dead_code)]

use std::io;

extern crate pest;
#[macro_use]
extern crate pest_derive;

extern crate dot;
#[macro_use]
extern crate dot_derive;

mod ast;
mod config;
mod parser;

fn main() -> io::Result<()> {
    let config = config::Config::new();

    let pool = parser::from_file(config.file_name)?;

    if config.emit_ast {
        println!("{:#?}", pool);
    }

    let mut out = std::io::stdout();

    if config.emit_ast_graph {
        pool.graph(&mut out)?;
    }

    // let mut context: ast::IRContext = Default::default();

    // if config.emit_llvm {
    //     return file.ir(&mut out, &mut context);
    // }

    Ok(())
}
