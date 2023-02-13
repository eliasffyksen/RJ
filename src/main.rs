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

fn check_rule(pair: &Pair<Rule>, rule: Rule) {
    if pair.as_rule() != rule {
        panic!("Expected rule {:?}, got {:?}", rule, pair);
    }
}

fn unexpected_pair(pair: &Pair<Rule>) {
    panic!("Unexpected pair {:?}", pair);
}

fn main() -> Result<(), Error<Rule>> {
    let args: Vec<String> = args().collect();

    let filename = args[1].clone();

    let file = File::read_file(filename.as_str())?;

    println!("AST: {:#?}", file);
    println!("CODE:");

    let mut out = std::io::stdout();

    let mut context: IRContext = Default::default();

    file.ir(&mut out, &mut context)
        .expect("Failed writing to stdout");

    Ok(())
}
