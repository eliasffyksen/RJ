
#![feature(iterator_try_collect)]

use std::io::Write;

use argparse::{ArgumentParser, Store, StoreTrue};
use pest::error::Error;
use pest::iterators::Pair;
use symbol_ref::SymbolRef;

mod block;
mod expression;
mod file;
mod function;
mod ident;
mod scope;
mod stmt;
mod const_data;
mod symbol_ref;
mod if_stmt;
mod ast_type;

use crate::file::File;

extern crate pest;
#[macro_use]
extern crate pest_derive;

#[derive(Parser)]
#[grammar = "rj.pest"]
struct RJParser;

#[derive(Default)]
pub struct IRContext {
    next_register: usize,
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
    let mut emit_ast = false;
    let mut emit_llvm = false;
    let mut file_name = String::new();

    {
        let mut ap = ArgumentParser::new();

        ap.refer(&mut emit_ast)
            .add_option(&["--emit-ast"], StoreTrue, "Emit LLVM IR");

        ap.refer(&mut emit_llvm)
            .add_option(&["--emit-llvm"], StoreTrue, "Name for the greeting");

        ap.refer(&mut file_name)
            .add_argument("file", Store, "File to parse")
            .required();

        ap.parse_args_or_exit();
    }

    let file = File::read_file(file_name.as_str());
    let file = match file {
        Ok(file) => file,
        Err(err) => {
            println!("Syntax error: {}", err);
            panic!()
        },
    };

    if emit_ast {
        println!("{:#?}", file);
    }

    let mut out = std::io::stdout();

    let mut context: IRContext = Default::default();

    if emit_llvm {
        file.ir(&mut out, &mut context);
    }

    Ok(())
}
