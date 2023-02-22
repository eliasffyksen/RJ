#![feature(iterator_try_collect)]
#![allow(dead_code)]

extern crate pest;
#[macro_use]
extern crate pest_derive;

mod ast;
mod config;
mod parser;

fn main() -> Result<(), ()> {
    let config = config::Config::new();

    let file = ast::File::read_file(config.file_name.as_str());
    let file = match file {
        Ok(file) => file,
        Err(err) => {
            println!("Syntax error: {}", err);
            panic!()
        }
    };

    if config.emit_ast {
        println!("{:#?}", file);
    }

    let mut out = std::io::stdout();

    let mut context: ast::IRContext = Default::default();

    if config.emit_llvm {
        return file.ir(&mut out, &mut context);
    }

    Ok(())
}
