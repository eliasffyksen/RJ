use std::env::args;

use pest::error::Error;
use pest::iterators::Pair;

mod file;
mod function;
mod stmt;
mod ident;
mod block;

use crate::file::File;

extern crate pest;
#[macro_use]
extern crate pest_derive;

#[derive(Parser)]
#[grammar = "rj.pest"]
struct RJParser;

#[derive(Default)]
pub struct IRContext {
    next_register: usize
}

impl IRContext {
    fn clear_register(&mut self) {
        self.next_register = 0;
    }

    fn claim_register(&mut self) -> usize {
        let register = self.next_register;
        self.next_register += 1;
        register
    }
}

pub trait GenerateAST<T> {
    fn generate_ast(pair: Pair<Rule>) -> T;
}

pub trait GenerateIR {
    fn generate_ir(&self, out: &mut impl std::io::Write, context: &mut IRContext) -> Result<(), std::io::Error>;
}

fn main() -> Result<(), Error<Rule>> {
    let args: Vec<String> = args().collect();

    let filename = args[1].clone();

    let file = File::read_file(filename.as_str())?;

    println!("AST: {:#?}", file);
    println!("CODE:");

    let mut out = std::io::stdout();

    let mut context: IRContext = Default::default();

    file.generate_ir(&mut out, &mut context)
        .expect("Failed writing to stdout");

    Ok(())
}
