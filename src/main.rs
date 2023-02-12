use std::env::args;
use std::fs::read_to_string;

use pest::error::Error;
use pest::iterators::Pair;

mod file;
mod function;

use crate::file::File;

extern crate pest;
#[macro_use]
extern crate pest_derive;

#[derive(Parser)]
#[grammar = "rj.pest"]
struct RJParser;
pub trait GenerateAST<T> {
    fn generate_ast(pair: Pair<Rule>) -> T;
}

pub trait GenerateIR {
    fn generate_ir(&self, out: &mut impl std::io::Write) -> Result<(), std::io::Error>;
}

fn main() -> Result<(), Error<Rule>> {
    let args: Vec<String> = args().collect();

    let filename = args[1].clone();

    let file = File::read_file(filename.as_str())?;

    let mut out = std::io::stdout();

    file.generate_ir(&mut out)
        .expect("Failed writing to stdout");

    Ok(())
}
