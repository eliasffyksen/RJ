#![feature(iterator_try_collect)]
#![allow(dead_code)]

use std::io;

mod config;

fn main() -> io::Result<()> {
    let config = config::Config::new();

    let pool = rjc_parser::from_file(config.file_name)?;

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
